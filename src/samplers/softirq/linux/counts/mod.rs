//! Collects Syscall stats using BPF and traces:
//! * `raw_syscalls/sys_enter`
//!
//! And produces these stats:
//! * `syscall`
//! * `cgroup_syscall`

const NAME: &str = "softirq_counts";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/softirq_counts.bpf.rs"));
}

mod stats;

use bpf::*;
use stats::*;

use crate::common::*;
use crate::samplers::softirq::linux::softirq_lut;
use crate::*;

use std::sync::Arc;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let counters = vec![
        &SOFTIRQ_INTERRUPTS_OTHER,
    ];

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .cpu_counters("counters", counters)
        .map("irq_lut", softirq_lut())
        .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "counters" => &self.maps.counters,
            "irq_lut" => &self.maps.irq_lut,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} irq_enter() BPF instruction count: {}",
            self.progs.softirq_enter.insn_cnt()
        );
    }
}
