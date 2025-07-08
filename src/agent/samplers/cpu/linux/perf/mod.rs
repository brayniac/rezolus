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

fn handle_cgroup_event(data: &[u8]) -> i32 {
    let mut cgroup_info = bpf::types::cgroup_info::default();
    
    if plain::copy_from_bytes(&mut cgroup_info, data).is_ok() {
        let name = cgroup::format_cgroup_name(&cgroup_info);
        let id = cgroup::CgroupInfo::id(&cgroup_info) as usize;
        
        // Set metadata for all metrics
        cgroup::set_name(id, &name, &CGROUP_CPU_CYCLES);
        cgroup::set_name(id, &name, &CGROUP_CPU_INSTRUCTIONS);
    }
    
    0
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // Set root cgroup name for all metrics
    cgroup::set_name(1, "/", &CGROUP_CPU_CYCLES);
    cgroup::set_name(1, "/", &CGROUP_CPU_INSTRUCTIONS);

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
    .ringbuf_handler("cgroup_info", handle_cgroup_event)
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
