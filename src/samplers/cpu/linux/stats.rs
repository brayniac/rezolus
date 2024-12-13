use crate::common::RwLockCounterGroup;
use crate::samplers::cpu::stats::*;

use metriken::*;

pub const MAX_CGROUPS: usize = 4096;

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

#[metric(
    name = "cgroup/cpu/aperf"
)]
pub static CGROUP_CPU_APERF: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cgroup/cpu/mperf"
)]
pub static CGROUP_CPU_MPERF: RwLockCounterGroup = RwLockCounterGroup::new(MAX_CGROUPS);

#[metric(
    name = "cgroup/cpu/tsc"
)]
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
