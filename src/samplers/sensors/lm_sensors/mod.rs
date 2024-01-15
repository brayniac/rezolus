use ::lm_sensors::Value;

use super::stats::*;
use super::*;
use crate::common::{Nop};

#[distributed_slice(SENSOR_SAMPLERS)]
fn init(config: &Config) -> Box<dyn Sampler> {
    if let Ok(s) = LmSensors::new(config) {
        Box::new(s)
    } else {
        Box::new(Nop {})
    }
}

const NAME: &str = "sensors_lm_sensors";

pub struct LmSensors {
    prev: Instant,
    next: Instant,
    interval: Duration,
    sensors: ::lm_sensors::LMSensors,
    // file: File,
}

impl LmSensors {
    #[allow(dead_code)]
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let now = Instant::now();

        Ok(Self {
            sensors: ::lm_sensors::Initializer::default().initialize().expect("file not found"),
            prev: now,
            next: now,
            interval: config.interval(NAME),
        })
    }
}

impl Sampler for LmSensors {
    fn sample(&mut self) {
        let now = Instant::now();

        if now < self.next {
            return;
        }

        let mut data = HashMap::<String, HashMap<String, i64>>::new();

        for chip in self.sensors.chip_iter(None) {
            if let Ok(name) = chip.name() {
                for feature in chip.feature_iter() {
                    if let Ok(label) = feature.label() {
                        for sub_feature in feature.sub_feature_iter() {
                            if let Ok(value) = sub_feature.value() {
                                let value = match value {
                                    Value::TemperatureInput(v) => v as i64,
                                    _ => { continue; }
                                };
                                if let Some(c) = data.get_mut(&name) {
                                    c.insert(label.clone(), value);
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(chip) = data.get("coretemp-isa-0000") {
            if let Some(temp) = chip.get("Package id 0") {
                CPU_TEMPERATURE.set(*temp);
            }
        }

        if let Some(chip) = data.get("cpu_thermal-virtual-0") {
            if let Some(temp) = chip.get("temp1") {
                CPU_TEMPERATURE.set(*temp);
            }
        }

        if let Some(chip) = data.get("k10temp-pci-00c3") {
            if let Some(temp) = chip.get("Tdie") {
                CPU_TEMPERATURE.set(*temp);
            } else if let Some(temp) = chip.get("Tctl") {
                CPU_TEMPERATURE.set(*temp);
            }
        }

        // determine when to sample next
        let next = self.next + self.interval;

        // it's possible we fell behind
        if next > now {
            // if we didn't, sample at the next planned time
            self.next = next;
        } else {
            // if we did, sample after the interval has elapsed
            self.next = now + self.interval;
        }

        // mark when we last sampled
        self.prev = now;
    }
}
