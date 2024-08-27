use crate::*;

const NAME: &str = "tcp_traffic";

#[cfg(feature = "bpf")]
mod bpf;

mod proc;

#[cfg(feature = "bpf")]
use bpf::*;

use proc::*;

#[cfg(feature = "bpf")]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    TcpTraffic::init(config).or_else(|_| ProcNetSnmp::init(config))
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    ProcNetSnmp::init(config)
}
