//! Collects Syscall stats using BPF and traces:
//! * `raw_syscalls/sys_enter`
//!
//! And produces these stats:
//! * `syscall`
//! * `cgroup_syscall`

const NAME: &str = "irq_counts";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/irq_counts.bpf.rs"));
}

mod stats;

use bpf::*;
use stats::*;

use crate::common::*;
use crate::samplers::irq::linux::irq_lut;
use crate::*;

use std::sync::Arc;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let counters = vec![
        &IRQ_INTERRUPTS_OTHER,
    ];

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .cpu_counters("counters", counters)
        .map("irq_lut", irq_lut())
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
            self.progs.irq_enter.insn_cnt()
        );
    }
}
