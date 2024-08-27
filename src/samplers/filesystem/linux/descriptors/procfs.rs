use crate::samplers::filesystem::*;
use crate::*;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::NAME;

pub struct Procfs {
    interval: Interval,
    file: File,
}

impl Procfs {
    pub fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let file = std::fs::File::open("/proc/sys/fs/file-nr")
            .map(|f| File::from_std(f))
            .map_err(|e| {
                error!("failed to open: {e}");
            })?;

        Ok(Box::new(Self {
            file,
            interval: config.interval(NAME),
        }))
    }
}

#[async_trait]
impl Sampler for Procfs {
    async fn sample(&mut self) {
        self.interval.tick().await;

        let now = Instant::now();
        METADATA_FILESYSTEM_DESCRIPTORS_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

        let _ = self.sample_procfs().await;

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_FILESYSTEM_DESCRIPTORS_RUNTIME.add(elapsed);
        let _ = METADATA_FILESYSTEM_DESCRIPTORS_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}

impl Procfs {
    async fn sample_procfs(&mut self) -> Result<(), std::io::Error> {
        self.file.rewind().await?;

        let mut data = String::new();
        self.file.read_to_string(&mut data).await?;

        let mut lines = data.lines();

        if let Some(line) = lines.next() {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() == 3 {
                if let Ok(open) = parts[0].parse::<i64>() {
                    FILESYSTEM_DESCRIPTORS_OPEN.set(open);
                }
            }
        }

        Ok(())
    }
}
