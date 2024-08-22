use crate::common::Interval;
use super::super::*;
use crate::{error, Config, Instant, Sampler};
use std::fs::File;
use std::io::{Read, Seek};

use super::NAME;

pub struct Procfs {
    interval: Interval,
    file: File,
}

impl Procfs {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let file = std::fs::File::open("/proc/sys/fs/file-nr").map_err(|e| {
            error!("failed to open: {e}");
        })?;

        Ok(Self {
            file,
            interval: Interval::new(Instant::now(), config.interval(NAME)),
        })
    }
}

impl Sampler for Procfs {
    fn sample(&mut self) {
        let now = Instant::now();

        if self.interval.try_wait(now).is_err() {
            return;
        }

        METADATA_FILESYSTEM_DESCRIPTORS_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

        let _ = self.sample_procfs();

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_FILESYSTEM_DESCRIPTORS_RUNTIME.add(elapsed);
        let _ = METADATA_FILESYSTEM_DESCRIPTORS_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}

impl Procfs {
    fn sample_procfs(&mut self) -> Result<(), std::io::Error> {
        self.file.rewind()?;

        let mut data = String::new();
        self.file.read_to_string(&mut data)?;

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
