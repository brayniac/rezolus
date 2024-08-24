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
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    // try to initialize the bpf based sampler
    if let Ok(s) = CpuUsage::new(config) {
        Some(Box::new(s))
    // try to fallback to the /proc/stat based sampler if there was an error
    } else if let Ok(s) = ProcStat::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    // try to use the /proc/stat based sampler since BPF was not enabled for
    // this build
    if let Ok(s) = ProcStat::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}
