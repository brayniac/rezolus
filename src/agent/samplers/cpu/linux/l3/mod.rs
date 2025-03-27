const NAME: &str = "cpu_l3";

use crate::agent::*;

use metriken::LazyGauge;
use perf_event::ReadFormat;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::Mutex;

use std::collections::{HashSet, HashMap};
use std::path::Path;

mod stats;

use stats::*;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let inner = CpuL3Inner::new()?;

    Ok(Some(Box::new(CpuL3 {
        inner: inner.into(),
    })))
}

struct CpuL3 {
    inner: Mutex<CpuL3Inner>,
}

#[async_trait]
impl Sampler for CpuL3 {
    async fn refresh(&self) {
        let mut inner = self.inner.lock().await;

        let _ = inner.refresh().await;
    }
}

struct CpuL3Inner {
    l3_caches: Vec<L3Cache>,
}

impl CpuL3Inner {
    pub fn new() -> Result<Self, std::io::Error> {
        let l3_caches = get_l3_caches()?;

        Ok(Self { l3_caches })
    }

    pub async fn refresh(&mut self) -> Result<(), std::io::Error> {
        for l3_cache in self.l3_caches {
            if let Ok(group_data) = l3_cache.l3_access.read_group() {
                if let (Some(l3_access), Some(l3_miss)) = (group_data.get(l3_cache.l3_access), group_data.get(l3_cache.l3_miss)) {
                    let l3_access = l3_access.value();
                    let l3_miss = l3_miss.value();

                    for cpu in l3_cache.siblings {
                        CPU_L3_ACCESS.set(cpu, l3_access);
                        CPU_L3_MISS.set(cpu, l3_miss);
                    }
                }
            }
        }

        Ok(())
    }
}

struct L3Cache {
    l3_access: perf_event::Counter,
    l3_miss: perf_event::Counter,
    /// all cores which share this cache
    siblings: Vec<usize>,
}

pub fn get_l3_caches() -> Result<Vec<L3Cache>, std::io::Error> {
    let mut l3_domains = Vec::new();
    let sys_cpu_path = Path::new("/sys/devices/system/cpu");

    // Find all CPU directories
    let cpu_dirs: Vec<PathBuf> = std::fs::read_dir(sys_cpu_path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map_or(false, |name| {
                    name.starts_with("cpu") && name[3..].chars().all(char::is_numeric)
                })
        })
        .collect();

    // Track unique L3 cache domains to avoid duplicates
    let mut processed_l3_domains = HashSet::new();

    for cpu_dir in cpu_dirs {
        let cache_dir = cpu_dir.join("cache");

        // Find L3 cache index file
        let l3_index_path = cache_dir
            .read_dir()?
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry.path().file_name().expect("no filename").to_str().map(|name| {
                    name.starts_with("index")
                        && entry.path().join("level").exists()
                        && std::fs::read_to_string(entry.path().join("level"))
                            .unwrap_or_default()
                            .trim()
                            == "3"
                }).expect("no l3 index found")
            });

        if let Some(l3_index) = l3_index_path {
            let shared_cpu_list_path = l3_index.path().join("shared_cpu_list");

            // Read shared CPU list
            if let Ok(shared_cpu_content) = std::fs::read_to_string(&shared_cpu_list_path) {
                let shared_cores = parse_cpu_list(&shared_cpu_content);

                // Avoid processing duplicate L3 cache domains
                let shared_cores_key: Vec<usize> = shared_cores.clone();
                if !processed_l3_domains.contains(&shared_cores_key) {
                    processed_l3_domains.insert(shared_cores_key);

                    l3_domains.push(shared_cores);
                }
            }
        }
    }

    let mut l3_caches = Vec::new();

    for l3_domain in l3_domains {
        let cpu = *l3_domain.first().expect("empty l3 domain");

        if let Ok(mut l3_access) = perf_event::Builder::new(perf_event::events::Raw::new(0xFF04))
            .one_cpu(cpu)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .pinned(true)
            .read_format(
                ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING | ReadFormat::GROUP,
            )
            .build()
        {
            if let Ok(mut l3_miss) = perf_event::Builder::new(perf_event::events::Raw::new(0x104))
                .one_cpu(cpu)
                .any_pid()
                .exclude_hv(false)
                .exclude_kernel(false)
                .build_with_group(&mut l3_access)
            {
                match l3_access.enable_group() {
                    Ok(_) => {
                        l3_caches.push(L3Cache {
                            l3_access,
                            l3_miss,
                            siblings: l3_domain,
                        })
                    }
                    Err(e) => {
                        error!("failed to enable the perf group on CPU{cpu}: {e}");
                    }
                }                
            }
        }
    }

    Ok(l3_caches)
}

fn parse_cpu_list(list: &str) -> Vec<usize> {
    let mut cores = Vec::new();

    for range in list.trim().split(',') {
        if let Some((start, end)) = range.split_once('-') {
            // Range of cores
            if let (Ok(start_num), Ok(end_num)) = (start.parse::<usize>(), end.parse::<usize>()) {
                cores.extend(start_num..=end_num);
            }
        } else {
            // Single core
            if let Ok(core) = range.parse::<usize>() {
                cores.push(core);
            }
        }
    }

    cores.sort_unstable();
    cores.dedup();
    cores
}
