use super::stats::*;
use super::*;
use crate::common::{Interval, Nop};

#[distributed_slice(REZOLUS_SAMPLERS)]
fn init(config: &Config) -> Box<dyn Sampler> {
    if let Ok(s) = Uptime::new(config) {
        Box::new(s)
    } else {
        Box::new(Nop {})
    }
}

const NAME: &str = "rezolus_uptime";

pub struct Uptime {
    interval: Interval,
    start: Instant,
}

impl Uptime {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        Ok(Self {
            interval: Interval::new(Instant::now(), config.interval(NAME)),
            start: Instant::now(),
        })
    }
}

impl Sampler for Uptime {
    fn sample(&mut self) {
        if let Ok(_elapsed) = self.interval.try_wait(Instant::now()) {
            // adds the elapsed time since last sample to the counter
            REZOLUS_UPTIME.set(self.start.elapsed().as_nanos());
        }
    }
}
