use metriken::MetricEntry;
use metriken::Format;
use crate::common::HISTOGRAM_GROUPING_POWER;
use metriken::{metric, Counter, LazyCounter, RwLockHistogram};

#[metric(
    name = "scheduler/runqueue/latency",
    description = "Distribution of the amount of time tasks were waiting in the runqueue",
    metadata = { unit = "nanoseconds" }
)]
pub static SCHEDULER_RUNQUEUE_LATENCY: RwLockHistogram =
    RwLockHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "scheduler/running",
    description = "Distribution of the amount of time tasks were on-CPU",
    metadata = { unit = "nanoseconds" }
)]
pub static SCHEDULER_RUNNING: RwLockHistogram = RwLockHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "scheduler/offcpu",
    description = "Distribution of the amount of time tasks were off-CPU",
    metadata = { unit = "nanoseconds" }
)]
pub static SCHEDULER_OFFCPU: RwLockHistogram = RwLockHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "scheduler/context_switch/involuntary",
    description = "The number of involuntary context switches"
)]
pub static SCHEDULER_IVCSW: LazyCounter = LazyCounter::new(Counter::default);

/// A function to format the cpu metrics that allows for export of both total
/// and per-CPU metrics.
///
/// For the `Simple` format, the metrics will be formatted according to the
/// a pattern which depends on the metric metadata:
/// `{name}/cpu{id}` eg: `cpu/frequency/cpu0`
/// `{name}/total` eg: `cpu/cycles/total`
/// `{name}/{state}/cpu{id}` eg: `cpu/usage/user/cpu0`
/// `{name}/{state}/total` eg: `cpu/usage/user/total`
///
/// For the `Prometheus` format, if the metric has an `id` set in the metadata,
/// the metric name is left as-is. Otherwise, `/total` is appended. Note: we
/// rely on the exposition logic to convert the `/`s to `_`s in the metric name.
pub fn scheduler_metric_formatter(metric: &MetricEntry, format: Format) -> String {
    let name = metric.name().to_string();

    match format {
        Format::Simple => {
            let name = metric.name().to_string();

            if let Some(group) = metric.metadata().get("group") {
                format!("{group}/{name}")
            } else {
                name
            }
        }
        Format::Prometheus => {
            if metric.metadata().is_empty() {
                return name;
            }

            let metadata: Vec<String> = metric
                .metadata()
                .into_iter()
                .map(|(key, value)| format!("{key}=\"{value}\""))
                .collect();
            let metadata = metadata.join(", ");

            let name = if let Some(group) = metric.metadata().get("group") {
                format!("{group}/{name}")
            } else {
                name
            };

            format!("{name}{{{metadata}}}")
        }
        _ => metriken::default_formatter(metric, format),
    }
}