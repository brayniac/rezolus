//! Collects NUMA memory statistics using BPF by hooking:
//! * `vmstat_update` - periodic vmstat aggregation
//!
//! And produces these per-node stats:
//! * `memory_numa_hit`
//! * `memory_numa_miss`
//! * `memory_numa_foreign`
//! * `memory_numa_interleave`
//! * `memory_numa_local`
//! * `memory_numa_other`

const NAME: &str = "memory_numa";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/memory_numa.bpf.rs"));
}

mod stats;

use bpf::*;
use stats::*;

use crate::agent::*;

use std::sync::Arc;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    // Check if this should replace or complement vmstat sampler
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // For now, we're using global counters (index 0)
    // TODO: Add per-node support once we can reliably read node data
    let _ = MEMORY_NUMA_HIT.set(0, 0);
    let _ = MEMORY_NUMA_MISS.set(0, 0);
    let _ = MEMORY_NUMA_FOREIGN.set(0, 0);
    let _ = MEMORY_NUMA_INTERLEAVE.set(0, 0);
    let _ = MEMORY_NUMA_LOCAL.set(0, 0);
    let _ = MEMORY_NUMA_OTHER.set(0, 0);

    let bpf = BpfBuilder::new(
        NAME,
        BpfProgStats {
            run_time: &BPF_RUN_TIME,
            run_count: &BPF_RUN_COUNT,
        },
        ModSkelBuilder::default,
    )
    .packed_counters("numa_hit", &MEMORY_NUMA_HIT)
    .packed_counters("numa_miss", &MEMORY_NUMA_MISS)
    .packed_counters("numa_foreign", &MEMORY_NUMA_FOREIGN)
    .packed_counters("numa_interleave", &MEMORY_NUMA_INTERLEAVE)
    .packed_counters("numa_local", &MEMORY_NUMA_LOCAL)
    .packed_counters("numa_other", &MEMORY_NUMA_OTHER)
    .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "numa_hit" => &self.maps.numa_hit,
            "numa_miss" => &self.maps.numa_miss,
            "numa_foreign" => &self.maps.numa_foreign,
            "numa_interleave" => &self.maps.numa_interleave,
            "numa_local" => &self.maps.numa_local,
            "numa_other" => &self.maps.numa_other,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} __zone_statistics() BPF instruction count: {}",
            self.progs.__zone_statistics.insn_cnt()
        );
        debug!(
            "{NAME} refresh_cpu_vm_stats() BPF instruction count: {}",
            self.progs.refresh_cpu_vm_stats.insn_cnt()
        );
    }
}