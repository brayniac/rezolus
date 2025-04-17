//! Collects CPU Throttling stats using BPF and cgroup filesystem:
//! * Uses scheduler tracepoints to identify cgroups
//! * Reads throttling stats from cgroup filesystem
//!
//! And produces these stats:
//! * `cgroup_cpu_throttled_time`
//! * `cgroup_cpu_throttled_count`
//!
//! These stats show when and for how long cgroups are being throttled by the CPU controller.

const NAME: &str = "cpu_throttled";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_throttled.bpf.rs"));
}

use bpf::*;

use crate::agent::*;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use walkdir::WalkDir;

mod stats;

use stats::*;

unsafe impl plain::Plain for bpf::types::cgroup_info {}

// Structure to track last seen throttling stats for a cgroup
#[derive(Debug, Default, Clone)]
struct ThrottleStats {
    nr_periods: u64,
    nr_throttled: u64,
    throttled_time: u64,
}

// Cgroup monitor that periodically scans cgroup stats
struct CgroupThrottleMonitor {
    // Last seen stats for each cgroup
    last_stats: HashMap<u32, ThrottleStats>,
    // Cgroup paths by ID
    cgroup_paths: HashMap<u32, String>,
}

impl CgroupThrottleMonitor {
    fn new() -> Self {
        Self {
            last_stats: HashMap::new(),
            cgroup_paths: HashMap::new(),
        }
    }

    // Find all cgroup directories
    fn scan_cgroups(&mut self) {
        // Scan for CPU controller cgroups
        for entry in WalkDir::new("/sys/fs/cgroup/cpu,cpuacct")
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let path = entry.path();
            
            // Check if this is a valid cgroup
            if !path.join("cpu.stat").exists() {
                continue;
            }

            // Get cgroup ID
            if let Some(id) = self.get_cgroup_id(&path) {
                self.cgroup_paths.insert(id, path.to_string_lossy().to_string());
            }
        }
    }

    // Get cgroup ID from kernel
    fn get_cgroup_id(&self, path: &Path) -> Option<u32> {
        // In a real implementation, we would get this from cgroup controller
        // For now we use a hash of the path as a placeholder
        let path_str = path.to_string_lossy();
        let hash = path_str.as_bytes().iter().fold(0, |acc, &x| acc.wrapping_add(x as u32));
        Some(hash % MAX_CGROUPS as u32)
    }

    // Read and update throttling stats
    fn update_throttling_stats(&mut self) {
        for (id, path) in &self.cgroup_paths {
            let stat_path = Path::new(path).join("cpu.stat");
            if !stat_path.exists() {
                continue;
            }

            if let Ok(stats) = self.read_throttling_stats(&stat_path) {
                let last_stats = self.last_stats.entry(*id).or_default();
                
                // Check for changes in throttling
                if stats.nr_throttled > last_stats.nr_throttled {
                    // Update throttled count
                    let throttled_count = stats.nr_throttled - last_stats.nr_throttled;
                    let _ = CGROUP_CPU_THROTTLED_COUNT.set(*id as usize, throttled_count);
                }
                
                // Check for changes in throttled time
                if stats.throttled_time > last_stats.throttled_time {
                    // Update throttled time
                    let throttled_time = stats.throttled_time - last_stats.throttled_time;
                    let _ = CGROUP_CPU_THROTTLED_TIME.set(*id as usize, throttled_time);
                }
                
                // Save current stats for next comparison
                *last_stats = stats;
            }
        }
    }

    // Read throttling stats from cpu.stat file
    fn read_throttling_stats(&self, path: &Path) -> Result<ThrottleStats, std::io::Error> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut stats = ThrottleStats::default();

        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            match parts[0] {
                "nr_periods" => {
                    stats.nr_periods = parts[1].parse().unwrap_or(0);
                }
                "nr_throttled" => {
                    stats.nr_throttled = parts[1].parse().unwrap_or(0);
                }
                "throttled_time" => {
                    stats.throttled_time = parts[1].parse().unwrap_or(0);
                }
                _ => {}
            }
        }

        Ok(stats)
    }
}

fn handle_event(data: &[u8]) -> i32 {
    let mut cgroup_info = bpf::types::cgroup_info::default();

    if plain::copy_from_bytes(&mut cgroup_info, data).is_ok() {
        let name = std::str::from_utf8(&cgroup_info.name)
            .unwrap()
            .trim_end_matches(char::from(0))
            .replace("\\x2d", "-");

        let pname = std::str::from_utf8(&cgroup_info.pname)
            .unwrap()
            .trim_end_matches(char::from(0))
            .replace("\\x2d", "-");

        let gpname = std::str::from_utf8(&cgroup_info.gpname)
            .unwrap()
            .trim_end_matches(char::from(0))
            .replace("\\x2d", "-");

        let name = if !gpname.is_empty() {
            if cgroup_info.level > 3 {
                format!(".../{gpname}/{pname}/{name}")
            } else {
                format!("/{gpname}/{pname}/{name}")
            }
        } else if !pname.is_empty() {
            format!("/{pname}/{name}")
        } else if !name.is_empty() {
            format!("/{name}")
        } else {
            "".to_string()
        };

        let id = cgroup_info.id;

        set_name(id as usize, name)
    }

    0
}

fn set_name(id: usize, name: String) {
    if !name.is_empty() {
        CGROUP_CPU_THROTTLED_TIME.insert_metadata(id, "name".to_string(), name.clone());
        CGROUP_CPU_THROTTLED_COUNT.insert_metadata(id, "name".to_string(), name);
    }
}

struct Throttled {
    bpf: AsyncBpf,
    monitor: Arc<Mutex<CgroupThrottleMonitor>>,
}

#[async_trait]
impl Sampler for Throttled {
    async fn refresh(&self) {
        // Refresh BPF data
        self.bpf.refresh().await;
        
        // Update cgroup throttling stats
        let mut monitor = self.monitor.lock().await;
        monitor.update_throttling_stats();
    }
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // Set the root cgroup name
    set_name(1, "/".to_string());

    // Create and initialize the BPF program
    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .packed_counters("cgroup_throttled_time", &CGROUP_CPU_THROTTLED_TIME)
        .packed_counters("cgroup_throttled_count", &CGROUP_CPU_THROTTLED_COUNT)
        .ringbuf_handler("cgroup_info", handle_event)
        .build()?;

    // Create and initialize the cgroup monitor
    let mut monitor = CgroupThrottleMonitor::new();
    monitor.scan_cgroups();
    
    let monitor = Arc::new(Mutex::new(monitor));
    
    // Spawn a background task to periodically scan for new cgroups
    let monitor_clone = monitor.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut monitor = monitor_clone.lock().await;
            monitor.scan_cgroups();
        }
    });

    Ok(Some(Box::new(Throttled {
        bpf,
        monitor,
    })))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_info" => &self.maps.cgroup_info,
            "cgroup_throttled_time" => &self.maps.cgroup_throttled_time,
            "cgroup_throttled_count" => &self.maps.cgroup_throttled_count,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} handle_sched_switch() BPF instruction count: {}",
            self.progs.handle_sched_switch.insn_cnt()
        );
        debug!(
            "{NAME} update_throttle_stats() BPF instruction count: {}",
            self.progs.update_throttle_stats.insn_cnt()
        );
    }
}
_empty() {
            if cgroup_info.level > 3 {
                format!(".../{gpname}/{pname}/{name}")
            } else {
                format!("/{gpname}/{pname}/{name}")
            }
        } else if !pname.is_empty() {
            format!("/{pname}/{name}")
        } else if !name.is_empty() {
            format!("/{name}")
        } else {
            "".to_string()
        };

        let id = cgroup_info.id;

        set_name(id as usize, name)
    }

    0
}

fn set_name(id: usize, name: String) {
    if !name.is_empty() {
        CGROUP_CPU_THROTTLED_TIME.insert_metadata(id, "name".to_string(), name.clone());
        CGROUP_CPU_THROTTLED_COUNT.insert_metadata(id, "name".to_string(), name);
    }
}

struct Throttled {
    bpf: AsyncBpf,
    monitor: Arc<Mutex<CgroupThrottleMonitor>>,
}

#[async_trait]
impl Sampler for Throttled {
    async fn refresh(&self) {
        // Refresh BPF data
        self.bpf.refresh().await;
        
        // Update cgroup throttling stats
        let mut monitor = self.monitor.lock().await;
        monitor.update_throttling_stats();
    }
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // Set the root cgroup name
    set_name(1, "/".to_string());

    // Create and initialize the BPF program
    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .packed_counters("cgroup_throttled_time", &CGROUP_CPU_THROTTLED_TIME)
        .packed_counters("cgroup_throttled_count", &CGROUP_CPU_THROTTLED_COUNT)
        .ringbuf_handler("cgroup_info", handle_event)
        .build()?;

    // Create and initialize the cgroup monitor
    let mut monitor = CgroupThrottleMonitor::new();
    monitor.scan_cgroups();
    
    let monitor = Arc::new(Mutex::new(monitor));
    
    // Spawn a background task to periodically scan for new cgroups
    let monitor_clone = monitor.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut monitor = monitor_clone.lock().await;
            monitor.scan_cgroups();
        }
    });

    Ok(Some(Box::new(Throttled {
        bpf,
        monitor,
    })))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_info" => &self.maps.cgroup_info,
            "cgroup_throttled_time" => &self.maps.cgroup_throttled_time,
            "cgroup_throttled_count" => &self.maps.cgroup_throttled_count,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} handle_sched_switch() BPF instruction count: {}",
            self.progs.handle_sched_switch.insn_cnt()
        );
        debug!(
            "{NAME} update_throttle_stats() BPF instruction count: {}",
            self.progs.update_throttle_stats.insn_cnt()
        );
    }
}
_empty() {
            if cgroup_info.level > 3 {
                format!(".../{gpname}/{pname}/{name}")
            } else {
                format!("/{gpname}/{pname}/{name}")
            }
        } else if !pname.is_empty() {
            format!("/{pname}/{name}")
        } else if !name.is_empty() {
            format!("/{name}")
        } else {
            "".to_string()
        };

        let id = cgroup_info.id;

        set_name(id as usize, name)
    }

    0
}

fn set_name(id: usize, name: String) {
    if !name.is_empty() {
        CGROUP_CPU_THROTTLED_TIME.insert_metadata(id, "name".to_string(), name.clone());
        CGROUP_CPU_THROTTLED_COUNT.insert_metadata(id, "name".to_string(), name);
    }
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // Set the root cgroup name
    set_name(1, "/".to_string());

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .packed_counters("cgroup_throttled_time", &CGROUP_CPU_THROTTLED_TIME)
        .packed_counters("cgroup_throttled_count", &CGROUP_CPU_THROTTLED_COUNT)
        .ringbuf_handler("cgroup_info", handle_event)
        .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_info" => &self.maps.cgroup_info,
            "cgroup_throttled_time" => &self.maps.cgroup_throttled_time,
            "cgroup_throttled_count" => &self.maps.cgroup_throttled_count,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} cpu_cfs_throttle_enter() BPF instruction count: {}",
            self.progs.cpu_cfs_throttle_enter.insn_cnt()
        );
        debug!(
            "{NAME} cpu_cfs_unthrottle_enter() BPF instruction count: {}",
            self.progs.cpu_cfs_unthrottle_enter.insn_cnt()
        );
        debug!(
            "{NAME} tg_throttle_up_enter() BPF instruction count: {}",
            self.progs.tg_throttle_up_enter.insn_cnt()
        );
        debug!(
            "{NAME} tg_throttle_down_enter() BPF instruction count: {}",
            self.progs.tg_throttle_down_enter.insn_cnt()
        );
    }
}