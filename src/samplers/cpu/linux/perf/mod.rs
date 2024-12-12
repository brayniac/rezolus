//! Collects CPU perf counters using BPF and traces:
//! * `sched_switch`
//!
//! Initializes perf events to collect cycles and instructions.
//!
//! And produces these stats:
//! * `cpu/cycles`
//! * `cpu/instructions`
//!
//! These stats can be used to calculate the IPC and IPNS in post-processing or
//! in an observability stack.

const NAME: &str = "cpu_perf";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_perf.bpf.rs"));
}

use bpf::*;

use crate::common::*;
use crate::samplers::cpu::linux::stats::*;
use crate::*;

use std::sync::Arc;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let cpus = crate::common::linux::cpus()?;

    let totals = vec![&CPU_CYCLES, &CPU_INSTRUCTIONS];

    let metrics = ["cpu/cycles", "cpu/instructions"];

    let mut cpu_counters = ScopedCounters::new();

    for cpu in cpus {
        for metric in metrics {
            cpu_counters.push(
                cpu,
                DynamicCounterBuilder::new(metric)
                    .metadata("id", format!("{}", cpu))
                    .formatter(cpu_metric_percore_formatter)
                    .build(),
            );
        }
    }

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .perf_event("cycles", perf_event::events::Hardware::CPU_CYCLES)
        .perf_event("instructions", perf_event::events::Hardware::INSTRUCTIONS)
        .cpu_counters("counters", totals, cpu_counters)
        .packed_counters("cgroup_cycles", &CGROUP_CPU_CYCLES)
        .packed_counters("cgroup_instructions", &CGROUP_CPU_INSTRUCTIONS)
        .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_cycles" => &self.maps.cgroup_cycles,
            "cgroup_instructions" => &self.maps.cgroup_instructions,
            "counters" => &self.maps.counters,
            "cycles" => &self.maps.cycles,
            "instructions" => &self.maps.instructions,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} handle__sched_switch() BPF instruction count: {}",
            self.progs.handle__sched_switch.insn_cnt()
        );
    }
}
