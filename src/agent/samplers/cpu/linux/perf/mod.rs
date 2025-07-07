//! Collects CPU perf counters using BPF and traces:
//! * `sched_switch`
//!
//! Initializes perf events to collect cycles and instructions.
//!
//! And produces these stats:
//! * `cpu_cycles`
//! * `cpu_instructions`
//! * `cgroup_cpu_cycles`
//! * `cgroup_cpu_instructions`
//!
//! These stats can be used to calculate the IPC and IPNS in post-processing or
//! in an observability stack.

const NAME: &str = "cpu_perf";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_perf.bpf.rs"));
}

use bpf::*;

use crate::agent::bpf::cgroup;
use crate::agent::*;

use std::sync::Arc;

mod stats;

use stats::*;

crate::impl_cgroup_info!(bpf::types::cgroup_info);

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let metric_names = [&CGROUP_CPU_CYCLES, &CGROUP_CPU_INSTRUCTIONS];
    cgroup::set_cgroup_metadata(1, "/", &metric_names);

    let bpf = BpfBuilder::new(
        NAME,
        BpfProgStats {
            run_time: &BPF_RUN_TIME,
            run_count: &BPF_RUN_COUNT,
        },
        ModSkelBuilder::default,
    )
    .perf_event("cycles", PerfEvent::cpu_cycles(), &CPU_CYCLES)
    .perf_event("instructions", PerfEvent::instructions(), &CPU_INSTRUCTIONS)
    .packed_counters("cgroup_cycles", &CGROUP_CPU_CYCLES)
    .packed_counters("cgroup_instructions", &CGROUP_CPU_INSTRUCTIONS)
    .ringbuf_handler("cgroup_info", cgroup::create_cgroup_handler(&metric_names))
    .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_cycles" => &self.maps.cgroup_cycles,
            "cgroup_info" => &self.maps.cgroup_info,
            "cgroup_instructions" => &self.maps.cgroup_instructions,
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
