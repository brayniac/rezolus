use metriken::*;

use crate::agent::*;

#[metric(
    name = "cgroup_cpu_throttled_time",
    description = "The total time a cgroup has been throttled by the CPU controller in nanoseconds",
    metadata = { unit = "nanoseconds" }
)]
pub static CGROUP_CPU_THROTTLED_TIME: CounterGroup = CounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cgroup_cpu_throttled_count",
    description = "The number of times a cgroup has been throttled by the CPU controller",
    metadata = { unit = "events" }
)]
pub static CGROUP_CPU_THROTTLED_COUNT: CounterGroup = CounterGroup::new(MAX_CGROUPS);