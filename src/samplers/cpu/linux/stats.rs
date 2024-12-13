use crate::common::RwLockCounterGroup;

use metriken::*;

pub const MAX_CGROUPS: usize = 4096;

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "busy", unit = "nanoseconds" }
)]
pub static CPU_USAGE_BUSY: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "user", unit = "nanoseconds" }
)]
pub static CPU_USAGE_USER: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "nice", unit = "nanoseconds" }
)]
pub static CPU_USAGE_NICE: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "system", unit = "nanoseconds" }
)]
pub static CPU_USAGE_SYSTEM: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "softirq", unit = "nanoseconds" }
)]
pub static CPU_USAGE_SOFTIRQ: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "irq", unit = "nanoseconds" }
)]
pub static CPU_USAGE_IRQ: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "steal", unit = "nanoseconds" }
)]
pub static CPU_USAGE_STEAL: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "guest", unit = "nanoseconds" }
)]
pub static CPU_USAGE_GUEST: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/usage",
    description = "The amount of CPU time spent executing in a given state",
    metadata = { state = "guest_nice", unit = "nanoseconds" }
)]
pub static CPU_USAGE_GUEST_NICE: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/cycles",
    description = "The number of elapsed CPU cycles on a per-cgroup basis",
    metadata = { unit = "cycles" }
)]
pub static CPU_CYCLES: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/instructions",
    description = "The number of elapsed CPU cycles on a per-cgroup basis",
    metadata = { unit = "cycles" }
)]
pub static CPU_INSTRUCTIONS: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/aperf",
    description = "The number of elapsed CPU cycles on a per-cgroup basis",
    metadata = { unit = "cycles" }
)]
pub static CPU_APERF: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/mperf",
    description = "The number of elapsed CPU cycles on a per-cgroup basis",
    metadata = { unit = "cycles" }
)]
pub static CPU_MPERF: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cpu/tsc",
    description = "The number of elapsed CPU cycles on a per-cgroup basis",
    metadata = { unit = "cycles" }
)]
pub static CPU_TSC: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cgroup/cpu/cycles",
    description = "The number of elapsed CPU cycles on a per-cgroup basis",
    metadata = { unit = "cycles" }
)]
pub static CGROUP_CPU_CYCLES: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cgroup/cpu/instructions",
    description = "The number of elapsed CPU cycles on a per-cgroup basis",
    metadata = { unit = "cycles" }
)]
pub static CGROUP_CPU_INSTRUCTIONS: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(name = "cgroup/cpu/aperf")]
pub static CGROUP_CPU_APERF: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(name = "cgroup/cpu/mperf")]
pub static CGROUP_CPU_MPERF: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(name = "cgroup/cpu/tsc")]
pub static CGROUP_CPU_TSC: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

pub fn cpu_metric_percore_formatter(metric: &MetricEntry, format: Format) -> String {
    match format {
        Format::Simple => {
            let id = metric
                .metadata()
                .get("id")
                .expect("no `id` for metric formatter");
            format!("{}/cpu{id}", metric.name())
        }
        _ => metric.name().to_string(),
    }
}

pub fn cpu_usage_percore_formatter(metric: &MetricEntry, format: Format) -> String {
    match format {
        Format::Simple => {
            let id = metric
                .metadata()
                .get("id")
                .expect("no `id` for metric formatter");
            let state = metric
                .metadata()
                .get("state")
                .expect("no `state` for metric formatter");
            format!("{}/{state}/cpu{id}", metric.name())
        }
        _ => metric.name().to_string(),
    }
}
