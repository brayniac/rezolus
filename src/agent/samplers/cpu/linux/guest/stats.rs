use metriken::*;

use crate::agent::timing::AcquisitionGroup;
use crate::agent::MAX_CPUS;
use linkme::distributed_slice;

/// Brackets the per-CPU read of the guest-only counters (`GuestInner::refresh`).
/// Single writer: only `refresh` calls `acquire()`/`finish()`, and it reads
/// every CPU's group in one pass.
pub static CPU_GUEST_ACQ: AcquisitionGroup = AcquisitionGroup::new(
    crate::agent::samplers::bpf_sampler_name("cpu_guest"),
    "cpu_guest_sweep",
);

#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static CPU_GUEST_ACQ_REG: &'static AcquisitionGroup = &CPU_GUEST_ACQ;

#[metric(
    name = "cpu_guest_cycles",
    description = "Cycles spent executing guest code on this CPU (perf exclude_host)",
    metadata = { unit = "cycles", acq_group = "cpu_guest_sweep" }
)]
pub static CPU_GUEST_CYCLES: CounterGroup = CounterGroup::new(MAX_CPUS);

#[metric(
    name = "cpu_guest_instructions",
    description = "Instructions retired by guest code on this CPU (perf exclude_host)",
    metadata = { unit = "instructions", acq_group = "cpu_guest_sweep" }
)]
pub static CPU_GUEST_INSTRUCTIONS: CounterGroup = CounterGroup::new(MAX_CPUS);
