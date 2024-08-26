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
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    // try to initialize the bpf based sampler
    if let Ok(s) = TcpTraffic::new(config) {
        Some(Box::new(s))
    // try to fallback to the /proc/net/snmp based sampler if there was an error
    } else if let Ok(s) = ProcNetSnmp::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    // try to use the /proc/net/snmp based sampler since BPF was not enabled for
    // this build
    if let Ok(s) = ProcNetSnmp::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}
