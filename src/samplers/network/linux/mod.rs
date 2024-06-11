use crate::common::{Interval, Nop};
use crate::samplers::hwinfo::hardware_info;
use crate::samplers::network::stats::*;
use crate::samplers::network::*;
use metriken::Counter;
use std::fs::File;
use std::io::Read;
use std::io::Seek;

mod interfaces;
mod traffic;

pub struct SysfsNetSampler {
    interval: Interval,
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
        let mut d = String::new();

        for (counter, stat) in metrics.drain(..) {
            let mut if_stats = HashMap::new();

            for interface in &hwinfo.network {
            	println!("initializing for if: {}", interface.name);
                // if interface.driver.is_none() {
                //     continue;
                // }

                if let Ok(mut f) = std::fs::File::open(&format!(
                    "/sys/class/net/{}/statistics/{stat}",
                    interface.name
                )) {
                    if f.read_to_string(&mut d).is_ok() && d.parse::<u64>().is_ok() {
                        println!("tracking: {stat} for {}", interface.name);
                        if_stats.insert(interface.name.to_string(), f);
                    }
                }
            }

            stats.push((counter, stat, if_stats));
        }

        Ok(Self {
            stats,
            interval: Interval::new(Instant::now(), config.interval(name)),
        })
    }
}

impl Sampler for SysfsNetSampler {
    fn sample(&mut self) {
        if self.interval.try_wait(Instant::now()).is_err() {
            return;
        }

        let mut data = String::new();

        'outer: for (counter, _stat, ref mut if_stats) in &mut self.stats {
            let mut sum = 0;

            for file in if_stats.values_mut() {
                if file.rewind().is_ok() {
                    if let Err(e) = file.read_to_string(&mut data) {
                        error!("error reading: {e}");
                        continue 'outer;
                    }

                    if let Ok(v) = data.parse::<u64>() {
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
