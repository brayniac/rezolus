use metriken::*;

#[metric(
    name = "cpu/cores",
    description = "The total number of logical cores that are currently online"
)]
pub static CPU_CORES: LazyGauge = LazyGauge::new(Gauge::default);

#[metric(
    name = "cpu/usage/total",
    description = "The amount of CPU time spent busy",
    formatter = cpu_usage_total_formatter,
    metadata = { state = "busy", unit = "nanoseconds" }
)]
pub static CPU_USAGE_BUSY: LazyCounter = LazyCounter::new(Counter::default);

#[metric(
    name = "cpu/usage/total",
    description = "The amount of CPU time spent executing normal tasks is user mode",
    formatter = cpu_usage_total_formatter,
    metadata = { state = "user", unit = "nanoseconds" }
)]
pub static CPU_USAGE_USER: LazyCounter = LazyCounter::new(Counter::default);

#[metric(
    name = "cpu/usage/total",
    description = "The amount of CPU time spent executing low priority tasks in user mode",
    formatter = cpu_usage_total_formatter,
    metadata = { state = "nice", unit = "nanoseconds" }
)]
pub static CPU_USAGE_NICE: LazyCounter = LazyCounter::new(Counter::default);

#[metric(
    name = "cpu/usage/total",
    description = "The amount of CPU time spent executing tasks in kernel mode",
    formatter = cpu_usage_total_formatter,
    metadata = { state = "system", unit = "nanoseconds" }
)]
pub static CPU_USAGE_SYSTEM: LazyCounter = LazyCounter::new(Counter::default);

pub fn cpu_usage_total_formatter(metric: &MetricEntry, format: Format) -> String {
    match format {
        Format::Simple => {
            let state = metric
                .metadata()
                .get("state")
                .expect("no `state` for metric formatter");
            format!("cpu/usage/{state}/total")
        }
        _ => metric.name().to_string(),
    }
}

/// A function to format per-cgroup metrics.
///
/// For the `Simple` format, the metrics will be formatted according to the
/// a pattern which depends on the metric metadata:
/// `{name}/cgroup{id}` eg: `cpu/cycles/cgroup0`
///
/// For the `Prometheus` format, if the metric has an `cgroup` set in the
/// metadata, the metric name is left as-is. Note: we rely on the exposition
/// logic to convert the `/`s to `_`s in the metric name.
#[allow(dead_code)]
pub fn cgroup_metric_formatter(metric: &MetricEntry, format: Format) -> String {
    match format {
        Format::Simple => {
            let name = metric.name().to_string();

            if metric.metadata().contains_key("cgroup") {
                format!(
                    "{name}/cgroup{}",
                    metric.metadata().get("cgroup").unwrap_or("unknown"),
                )
            } else {
                panic!("cgroup wasn't set")
            }
        }
        Format::Prometheus => {
            let metadata: Vec<String> = metric
                .metadata()
                .iter()
                .map(|(key, value)| format!("{key}=\"{value}\""))
                .collect();
            let metadata = metadata.join(", ");

            let name = if metric.metadata().contains_key("cgroup") {
                metric.name().to_string()
            } else {
                panic!("cgroup wasn't set")
            };

            format!("{}{{{metadata}}}", name)
        }
        _ => metriken::default_formatter(metric, format),
    }
}
