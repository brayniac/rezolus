use crate::*;

const NAME: &str = "cpu_usage";

#[cfg(feature = "bpf")]
mod bpf;

mod proc_stat;

#[cfg(feature = "bpf")]
use bpf::*;

use proc_stat::*;

#[cfg(feature = "bpf")]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    CpuUsage::init(config).or_else(|_| ProcStat::init(config))
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    ProcStat::init(config)
}
