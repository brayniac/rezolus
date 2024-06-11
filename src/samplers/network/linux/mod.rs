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
                if interface.driver.is_none() {
                    continue;
                }

                info!("initializing {stat} for if: {}", interface.name);

                let path = format!(
                    "/sys/class/net/{}/statistics/{stat}",
                    interface.name
                );

                match std::fs::File::open(&path) {
                	Ok(mut f) => match f.read_to_string(&mut d) {
                		Ok(_) => {
                			if d.parse::<u64>().is_ok() {
	                			info!("tracking: {stat} for {}", interface.name);
	                        	if_stats.insert(interface.name.to_string(), f);
	                		} else {
	                			error!("failed to parse: {d}");
	                		}
	                	}
                		Err(e) => {
                			error!("failed to read: {e}");
                		}
                	}
                	Err(e) => {
                		error!("failed to open {path}: {e}");
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
