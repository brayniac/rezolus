use super::*;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub struct ProcCpuinfo {
    file: File,
    interval: Interval,
}

impl ProcCpuinfo {
    pub fn new(config: &Config) -> Result<Self, ()> {
        let file = std::fs::File::open("/proc/cpuinfo")
            .map(|f| File::from_std(f))
            .map_err(|e| {
                error!("failed to open /proc/cpuinfo: {e}");
            })?;

        Ok(Self {
            file,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for ProcCpuinfo {
    async fn sample(&mut self) {
        self.interval.tick().await;

        let now = Instant::now();
        METADATA_CPU_PERF_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());


        let _ = self.sample_proc_cpuinfo().await;

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_CPU_PERF_RUNTIME.add(elapsed);
        let _ = METADATA_CPU_PERF_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}

impl ProcCpuinfo {
    async fn sample_proc_cpuinfo(&mut self) -> Result<(), std::io::Error> {
        self.file.rewind().await?;

        let mut data = String::new();
        self.file.read_to_string(&mut data).await?;

        let mut online_cores = 0;

        let lines = data.lines();

        let mut frequency = 0;

        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if let Some(&"processor") = parts.first() {
                online_cores += 1;
            }

            if let (Some(&"cpu"), Some(&"MHz")) = (parts.first(), parts.get(1)) {
                if let Some(Ok(freq)) = parts
                    .get(3)
                    .map(|v| v.parse::<f64>().map(|v| v.floor() as u64))
                {
                    let _ = CPU_FREQUENCY_HISTOGRAM.increment(freq);
                    frequency += freq;
                }
            }
        }

        CPU_CORES.set(online_cores);

        if frequency != 0 && online_cores != 0 {
            CPU_FREQUENCY_AVERAGE.set(frequency as i64 / online_cores);
        }

        Ok(())
    }
}
