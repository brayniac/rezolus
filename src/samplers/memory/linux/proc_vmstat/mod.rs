use crate::*;

use crate::samplers::memory::stats::*;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(s) = ProcVmstat::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

const NAME: &str = "memory_vmstat";

pub struct ProcVmstat {
    interval: Interval,
    counters: HashMap<&'static str, &'static LazyCounter>,
    file: File,
}

impl ProcVmstat {
    #[allow(dead_code)]
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let counters = HashMap::from([
            ("numa_hit", &MEMORY_NUMA_HIT),
            ("numa_miss", &MEMORY_NUMA_MISS),
            ("numa_foreign", &MEMORY_NUMA_FOREIGN),
            ("numa_interleave", &MEMORY_NUMA_INTERLEAVE),
            ("numa_local", &MEMORY_NUMA_LOCAL),
            ("numa_other", &MEMORY_NUMA_OTHER),
        ]);

        let file = std::fs::File::open("/proc/vmstat")
            .map(|f| File::from_std(f))
            .map_err(|e| {
                error!("Failed to open /proc/vmstat: {e}");
            })?;

        Ok(Self {
            file,
            counters,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for ProcVmstat {
    async fn sample(&mut self) {
        self.interval.tick().await;

        let _ = self.sample_proc_vmstat().await;
    }
}

impl ProcVmstat {
    async fn sample_proc_vmstat(&mut self) -> Result<(), std::io::Error> {
        self.file.rewind().await?;

        let mut data = String::new();
        self.file.read_to_string(&mut data).await?;

        let lines = data.lines();

        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.is_empty() {
                continue;
            }

            if let Some(counter) = self.counters.get_mut(*parts.first().unwrap()) {
                if let Some(Ok(v)) = parts.get(1).map(|v| v.parse::<u64>()) {
                    counter.set(v);
                }
            }
        }

        Ok(())
    }
}
