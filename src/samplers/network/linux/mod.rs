use crate::*;

mod stats;

use crate::samplers::hwinfo::hardware_info;
use metriken::Counter;

use std::io::Read;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

mod interfaces;
mod traffic;


pub struct SysfsNetSampler {
    stats: Vec<(&'static Lazy<Counter>, &'static str, HashMap<String, File>)>,
}

impl SysfsNetSampler {
    pub fn new(
        config: &Config,
        name: &str,
        mut metrics: Vec<(&'static Lazy<Counter>, &'static str)>,
    ) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(name) {
            return Err(());
        }

        let hwinfo = hardware_info().map_err(|e| {
            error!("failed to load hardware info: {e}");
        })?;

        let mut stats = Vec::new();
        let mut data = String::new();

        for (counter, stat) in metrics.drain(..) {
            let mut if_stats = HashMap::new();

            for interface in &hwinfo.network {
                if interface.driver.is_none() {
                    continue;
                }

                if let Ok(mut f) = std::fs::File::open(&format!(
                    "/sys/class/net/{}/statistics/{stat}",
                    interface.name
                )) {
                    data.clear();

                    if f.read_to_string(&mut data).is_ok() && data.trim_end().parse::<u64>().is_ok()
                    {
                        if_stats.insert(interface.name.to_string(), File::from_std(f));
                    }
                }
            }

            stats.push((counter, stat, if_stats));
        }

        Ok(Self {
            stats,
        })
    }

    async fn sample_now(&mut self) {
        let mut data = String::new();

        'outer: for (counter, _stat, ref mut if_stats) in &mut self.stats {
            let mut sum = 0;

            for file in if_stats.values_mut() {
                if file.rewind().await.is_ok() {
                    data.clear();

                    if let Err(e) = file.read_to_string(&mut data).await {
                        error!("error reading: {e}");
                        continue 'outer;
                    }

                    if let Ok(v) = data.trim_end().parse::<u64>() {
                        sum += v;
                    } else {
                        continue 'outer;
                    }
                }
            }

            counter.set(sum);
        }
    }
}
