use crate::Duration;
use ringlog::Level;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::Path;

mod general;
mod log;
mod prometheus;
mod sampler;

use general::*;
use log::*;
use prometheus::*;
use sampler::*;

#[derive(Deserialize)]
pub struct Config {
    general: General,
    #[serde(default)]
    log: Log,
    #[serde(default)]
    prometheus: Prometheus,
    #[serde(default)]
    defaults: SamplerConfig,
    #[serde(default)]
    samplers: HashMap<String, SamplerConfig>,
}

impl Config {
    pub fn load(path: &dyn AsRef<Path>) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| {
                eprintln!("unable to open config file: {e}");
                std::process::exit(1);
            })
            .unwrap();

        let config: Config = toml::from_str(&content)
            .map_err(|e| {
                eprintln!("failed to parse config file: {e}");
                std::process::exit(1);
            })
            .unwrap();

        config.prometheus().check();

        config.defaults.check("default");

        for (name, config) in config.samplers.iter() {
            config.check(name);
        }

        Ok(config)
    }

    pub fn log(&self) -> &Log {
        &self.log
    }

    pub fn defaults(&self) -> &SamplerConfig {
        &self.defaults
    }

    pub fn sampler_config(&self, name: &str) -> Option<&SamplerConfig> {
        self.samplers.get(name)
    }

    pub fn general(&self) -> &General {
        &self.general
    }

    pub fn prometheus(&self) -> &Prometheus {
        &self.prometheus
    }

    pub fn enabled(&self, name: &str) -> bool {
        self.samplers
            .get(name)
            .and_then(|v| v.enabled)
            .unwrap_or(self.defaults.enabled.unwrap_or(enabled()))
    }

    pub fn bpf(&self, name: &str) -> bool {
        self.samplers
            .get(name)
            .and_then(|v| v.bpf)
            .unwrap_or(self.defaults.bpf.unwrap_or(enabled()))
    }

    pub fn interval(&self, name: &str) -> Duration {
        let interval = self
            .samplers
            .get(name)
            .and_then(|v| v.interval.as_ref())
            .unwrap_or(self.defaults.interval.as_ref().unwrap_or(&interval()))
            .parse::<humantime::Duration>()
            .unwrap();

        Duration::from_nanos(interval.as_nanos() as u64)
    }

    pub fn distribution_interval(&self, name: &str) -> Duration {
        let interval = self
            .samplers
            .get(name)
            .and_then(|v| v.distribution_interval.as_ref())
            .unwrap_or(
                self.defaults
                    .distribution_interval
                    .as_ref()
                    .unwrap_or(&distribution_interval()),
            )
            .parse::<humantime::Duration>()
            .unwrap();

        Duration::from_nanos(interval.as_nanos() as u64)
    }
}

pub fn enabled() -> bool {
    true
}

pub fn disabled() -> bool {
    false
}

pub fn four() -> u8 {
    4
}

pub fn interval() -> String {
    "10ms".into()
}

pub fn distribution_interval() -> String {
    "50ms".into()
}
