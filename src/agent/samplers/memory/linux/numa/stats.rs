use crate::agent::*;
use metriken::*;

// BPF program stats
#[metric(
    name = "rezolus_bpf_run_time",
    description = "The amount of time Rezolus BPF programs have been executing",
    metadata = { unit = "nanoseconds", sampler = "memory_numa" }
)]
pub static BPF_RUN_TIME: LazyCounter = LazyCounter::new(Counter::default);

#[metric(
    name = "rezolus_bpf_run_count",
    description = "The number of times Rezolus BPF programs have been run",
    metadata = { sampler = "memory_numa" }
)]
pub static BPF_RUN_COUNT: LazyCounter = LazyCounter::new(Counter::default);

// Maximum number of NUMA nodes we support
pub const MAX_NUMA_NODES: usize = 1024;

// Per-node NUMA hit counter
#[metric(
    name = "memory_numa_hit",
    description = "The number of allocations that succeeded on the intended node",
    metadata = { node = "NUMA node ID" }
)]
pub static MEMORY_NUMA_HIT: CounterGroup = CounterGroup::new(MAX_NUMA_NODES);

// Per-node NUMA miss counter
#[metric(
    name = "memory_numa_miss", 
    description = "The number of allocations that did not succeed on the intended node",
    metadata = { node = "NUMA node ID" }
)]
pub static MEMORY_NUMA_MISS: CounterGroup = CounterGroup::new(MAX_NUMA_NODES);

// Per-node NUMA foreign counter
#[metric(
    name = "memory_numa_foreign",
    description = "The number of allocations that were not intended for a node that were serviced by this node",
    metadata = { node = "NUMA node ID" }
)]
pub static MEMORY_NUMA_FOREIGN: CounterGroup = CounterGroup::new(MAX_NUMA_NODES);

// Per-node NUMA interleave counter
#[metric(
    name = "memory_numa_interleave",
    description = "The number of interleave policy allocations that succeeded on the intended node", 
    metadata = { node = "NUMA node ID" }
)]
pub static MEMORY_NUMA_INTERLEAVE: CounterGroup = CounterGroup::new(MAX_NUMA_NODES);

// Per-node NUMA local counter
#[metric(
    name = "memory_numa_local",
    description = "The number of allocations that succeeded on the local node",
    metadata = { node = "NUMA node ID" }
)]
pub static MEMORY_NUMA_LOCAL: CounterGroup = CounterGroup::new(MAX_NUMA_NODES);

// Per-node NUMA other counter
#[metric(
    name = "memory_numa_other",
    description = "The number of allocations on this node that were allocated by a process on another node",
    metadata = { node = "NUMA node ID" }
)]
pub static MEMORY_NUMA_OTHER: CounterGroup = CounterGroup::new(MAX_NUMA_NODES);