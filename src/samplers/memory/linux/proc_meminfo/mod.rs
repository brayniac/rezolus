use crate::*;

use crate::common::units::KIBIBYTES;
use crate::common::Interval;
use crate::samplers::memory::stats::*;
use metriken::Gauge;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(s) = ProcMeminfo::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

const NAME: &str = "memory_meminfo";

pub struct ProcMeminfo {
    interval: Interval,
    file: File,
    gauges: HashMap<&'static str, &'static Gauge>,
}

impl ProcMeminfo {
    #![allow(dead_code)]
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let gauges: HashMap<&str, &Gauge> = HashMap::from([
            ("MemTotal:", &*MEMORY_TOTAL),
            ("MemFree:", &*MEMORY_FREE),
            ("MemAvailable:", &*MEMORY_AVAILABLE),
            ("Buffers:", &*MEMORY_BUFFERS),
            ("Cached:", &*MEMORY_CACHED),
        ]);

        let file = std::fs::File::open("/proc/meminfo")
            .map(|f| File::from_std(f))
            .map_err(|e| {
                error!("Failed to open /proc/meminfo: {e}");
            })?;

        Ok(Self {
            file,
            gauges,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for ProcMeminfo {
    async fn sample(&mut self) {
        self.interval.tick().await;

        let now = Instant::now();
        METADATA_MEMORY_MEMINFO_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

        let _ = self.sample_proc_meminfo().await;

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_MEMORY_MEMINFO_RUNTIME.add(elapsed);
        let _ = METADATA_MEMORY_MEMINFO_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}

impl ProcMeminfo {
    async fn sample_proc_meminfo(&mut self) -> Result<(), std::io::Error> {
        self.file.rewind().await?;

        let mut data = String::new();
        self.file.read_to_string(&mut data).await?;

        let lines = data.lines();

        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            if let Some(gauge) = self.gauges.get_mut(*parts.first().unwrap()) {
                if let Some(Ok(v)) = parts.get(1).map(|v| v.parse::<i64>()) {
                    gauge.set(v * KIBIBYTES as i64);
                }
            }
        }

        Ok(())
    }
}
