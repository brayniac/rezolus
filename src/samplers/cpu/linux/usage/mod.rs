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
fn init(config: Arc<Config>, runtime: &Runtime) {
    // check if sampler should be enabled
    if !config.enabled(NAME) {
        return;
    }

    // spawn the sampler
    runtime.spawn(async {
        if let Ok(mut s) = CpuUsage::init(config.clone()).or_else(|_| ProcStat::init(config)) {
            loop {
                s.sample().await;
            }
        }
    });
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>, runtime: &Runtime) {
    // check if sampler should be enabled
    if !config.enabled(NAME) {
        return;
    }

    // spawn the sampler
    runtime.spawn(async {
        if let Ok(mut s) = ProcStat::init(config) {
            loop {
                s.sample().await;
            }
        }
    });
}
