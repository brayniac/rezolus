//! Collects CPU Throttling stats using the cgroup filesystem:
//! * Reads throttling stats directly from cgroup filesystem
//! * Tracks changes in throttling metrics over time
//!
//! And produces these stats:
//! * `cgroup_cpu_throttled_time`
//! * `cgroup_cpu_throttled_count`
//!
//! These stats show when and for how long cgroups are being throttled by the CPU controller.

const NAME: &str = "cpu_throttled";

use crate::agent::*;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use walkdir::WalkDir;

mod stats;

use stats::*;

// Structure to track last seen throttling stats for a cgroup
#[derive(Debug, Clone)]
struct ThrottleStats {
    nr_periods: u64,
    nr_throttled: u64,
    throttled_time: u64,
    last_update: std::time::Instant,
}

impl Default for ThrottleStats {
    fn default() -> Self {
        Self {
            nr_periods: 0,
            nr_throttled: 0,
            throttled_time: 0,
            last_update: std::time::Instant::now(),
        }
    }
}

// Structure for a cgroup
#[derive(Debug, Clone)]
struct CgroupInfo {
    id: usize,
    path: PathBuf,
    name: String,
    stats: ThrottleStats,
}

// Cgroup monitor that periodically scans cgroup stats
struct CgroupThrottleMonitor {
    // Cgroups by ID
    cgroups: HashMap<usize, CgroupInfo>,
    // Next cgroup ID to assign
    next_id: usize,
    // Base path for cgroups
    base_path: PathBuf,
}

impl CgroupThrottleMonitor {
    fn new() -> Self {
        // Start IDs at 1 since we reserve 0 for the root cgroup
        let next_id = 1;

        // Default base path for the CPU controller
        let base_path = PathBuf::from("/sys/fs/cgroup/cpu,cpuacct");

        // Initialize with an empty cgroup map
        Self {
            cgroups: HashMap::new(),
            next_id,
            base_path,
        }
    }

    // Generate a unique ID for a cgroup
    fn generate_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // Find all cgroup directories and update the map
    fn scan_cgroups(&mut self) {
        debug!("Scanning for cgroups in: {}", self.base_path.display());

        // Track seen paths to detect removed cgroups
        let mut seen_paths = Vec::new();

        // Scan cgroup filesystem recursively
        let walker = WalkDir::new(&self.base_path)
            .min_depth(0) // Include base directory
            .follow_links(false);

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            // Track this path
            seen_paths.push(path.to_path_buf());

            // Skip if not a directory
            if !entry.file_type().is_dir() {
                continue;
            }

            // Check if this is a valid cgroup with CPU controller
            let stat_path = path.join("cpu.stat");
            if !stat_path.exists() {
                continue;
            }

            // Calculate a relative path from the base for naming
            let rel_path = path.strip_prefix(&self.base_path).unwrap_or(path);
            let name = format!("/{}", rel_path.display());

            // Check if we already have this cgroup by path
            let existing_id = self
                .cgroups
                .iter()
                .find(|(_, info)| info.path == path)
                .map(|(id, _)| *id);

            match existing_id {
                Some(id) => {
                    // Update existing cgroup name if needed
                    if let Some(info) = self.cgroups.get_mut(&id) {
                        if info.name != name {
                            info.name = name.clone();
                            // Update metadata
                            set_name(id, &name);
                        }
                    }
                }
                None => {
                    // New cgroup found
                    let id = self.generate_id();
                    debug!("Found new cgroup: {} (ID: {})", name, id);

                    // Initialize stats
                    let stats = match read_throttling_stats(&stat_path) {
                        Ok(stats) => stats,
                        Err(e) => {
                            debug!("Error reading stats for {}: {}", path.display(), e);
                            ThrottleStats::default()
                        }
                    };

                    // Create cgroup info
                    let cgroup = CgroupInfo {
                        id,
                        path: path.to_path_buf(),
                        name: name.clone(),
                        stats,
                    };

                    // Save cgroup and set metadata
                    self.cgroups.insert(id, cgroup);
                    set_name(id, &name);
                }
            }
        }

        // Remove cgroups that no longer exist
        self.cgroups
            .retain(|_, info| seen_paths.contains(&info.path));
    }

    // Update throttling stats for all known cgroups
    fn update_throttling_stats(&mut self) -> Result<(), std::io::Error> {
        for (id, info) in &mut self.cgroups {
            let stat_path = info.path.join("cpu.stat");
            if !stat_path.exists() {
                continue;
            }

            match read_throttling_stats(&stat_path) {
                Ok(new_stats) => {
                    // Calculate the delta in throttling metrics
                    let throttled_count_delta = new_stats.nr_throttled - info.stats.nr_throttled;
                    let throttled_time_delta = new_stats.throttled_time - info.stats.throttled_time;

                    // Only update metrics if there's been a change
                    if throttled_count_delta > 0 {
                        debug!(
                            "Cgroup {} throttled {} times",
                            info.name, throttled_count_delta
                        );
                        let _ = CGROUP_CPU_THROTTLED_COUNT.set(*id, throttled_count_delta);
                    }

                    if throttled_time_delta > 0 {
                        debug!(
                            "Cgroup {} throttled for {}ns",
                            info.name, throttled_time_delta
                        );
                        let _ = CGROUP_CPU_THROTTLED_TIME.set(*id, throttled_time_delta);
                    }

                    // Update stored stats
                    info.stats = new_stats;
                }
                Err(e) => {
                    debug!("Error reading stats for {}: {}", info.path.display(), e);
                }
            }
        }

        Ok(())
    }
}

// Read throttling stats from cpu.stat file
fn read_throttling_stats(path: &Path) -> Result<ThrottleStats, std::io::Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let mut stats = ThrottleStats {
        last_update: std::time::Instant::now(),
        ..Default::default()
    };

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

fn set_name(id: usize, name: &str) {
    if !name.is_empty() {
        CGROUP_CPU_THROTTLED_TIME.insert_metadata(id, "name".to_string(), name.to_string());
        CGROUP_CPU_THROTTLED_COUNT.insert_metadata(id, "name".to_string(), name.to_string());
    }
}

struct Throttled {
    monitor: Arc<Mutex<CgroupThrottleMonitor>>,
}

#[async_trait]
impl Sampler for Throttled {
    async fn refresh(&self) {
        let mut monitor = self.monitor.lock().await;
        if let Err(e) = monitor.update_throttling_stats() {
            error!("Error updating throttling stats: {}", e);
        }
    }
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // Set the root cgroup name
    set_name(0, "/");

    // Create and initialize the cgroup monitor
    let mut monitor = CgroupThrottleMonitor::new();

    // Initial scan for cgroups
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

    Ok(Some(Box::new(Throttled { monitor })))
}
