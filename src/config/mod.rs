use crate::common::HISTOGRAM_GROUPING_POWER;
use crate::debug;

use ringlog::Level;
use serde::Deserialize;

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;

mod agent;
mod general;
mod log;
mod prometheus;
mod sampler;

pub use agent::AgentConfig;

use general::General;
use log::Log;
use prometheus::Prometheus;
use sampler::Sampler as SamplerConfig;

fn enabled() -> bool {
    true
}

fn histogram_grouping_power() -> u8 {
    HISTOGRAM_GROUPING_POWER
}


