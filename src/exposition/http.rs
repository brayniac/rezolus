use crate::PERCENTILES;
use metriken::{Counter, Gauge, Histogram};

use warp::Filter;

/// HTTP exposition
pub async fn http() {
    let http = filters::http();

    warp::serve(http).run(([0, 0, 0, 0], 4242)).await;
}

mod filters {
    use super::*;

    /// The combined set of http endpoint filters
    pub fn http() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        prometheus_stats().or(human_stats()).or(hardware_info())
    }

    /// GET /metrics
    pub fn prometheus_stats(
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("metrics")
            .and(warp::get())
            .and_then(handlers::prometheus_stats)
    }

    /// GET /vars
    pub fn human_stats(
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("vars")
            .and(warp::get())
            .and_then(handlers::human_stats)
    }

    /// GET /hardware_info
    pub fn hardware_info(
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        warp::path!("hardware_info")
            .and(warp::get())
            .and_then(handlers::hwinfo)
    }
}

mod handlers {
    use metriken::{Duration, UnixInstant};
    use super::*;
    use core::convert::Infallible;

    pub async fn prometheus_stats() -> Result<impl warp::Reply, Infallible> {
        let end = UnixInstant::now();
        let start = end - Duration::from_secs(1);

        let mut data = Vec::new();

        for metric in &metriken::metrics() {
            let any = match metric.as_any() {
                Some(any) => any,
                None => {
                    continue;
                }
            };

            if metric.name().starts_with("log_") {
                continue;
            }

            if let Some(counter) = any.downcast_ref::<Counter>() {
                if metric.metadata().is_empty() {
                    data.push(format!(
                        "# TYPE {}_total counter\n{}_total {}",
                        metric.name(),
                        metric.name(),
                        counter.value()
                    ));
                } else {
                    data.push(format!(
                        "# TYPE {} counter\n{} {}",
                        metric.name(),
                        metric.formatted(metriken::Format::Prometheus),
                        counter.value()
                    ));
                }
            } else if let Some(gauge) = any.downcast_ref::<Gauge>() {
                data.push(format!(
                    "# TYPE {} gauge\n{} {}",
                    metric.name(),
                    metric.formatted(metriken::Format::Prometheus),
                    gauge.value()
                ));
            } else if let Some(histogram) = any.downcast_ref::<Histogram>() {
                let percentiles: Vec<f64> = PERCENTILES.iter().map(|v| v.1).collect();

                if let Some(Ok(snapshot)) = histogram.snapshot_between(start..end) {
                    if let Ok(result) = snapshot.percentiles(&percentiles) {
                        for (percentile, value) in result.iter().map(|(p, b)| (p, b.end())) {
                            data.push(format!(
                                "# TYPE {} gauge\n{}{{percentile=\"{:02}\"}} {}",
                                metric.name(),
                                metric.name(),
                                percentile,
                                value,
                            ));
                        }
                    }
                }
            }
        }

        data.sort();
        data.dedup();
        let mut content = data.join("\n");
        content += "\n";
        let parts: Vec<&str> = content.split('/').collect();
        Ok(parts.join("_"))
    }

    pub async fn human_stats() -> Result<impl warp::Reply, Infallible> {
        let end = UnixInstant::now();
        let start = end - Duration::from_secs(1);

        let mut data = Vec::new();

        for metric in &metriken::metrics() {
            let any = match metric.as_any() {
                Some(any) => any,
                None => {
                    continue;
                }
            };

            if metric.name().starts_with("log_") {
                continue;
            }

            if let Some(counter) = any.downcast_ref::<Counter>() {
                data.push(format!(
                    "{}: {}",
                    metric.formatted(metriken::Format::Simple),
                    counter.value()
                ));
            } else if let Some(gauge) = any.downcast_ref::<Gauge>() {
                data.push(format!(
                    "{}: {}",
                    metric.formatted(metriken::Format::Simple),
                    gauge.value()
                ));
            } else if let Some(histogram) = any.downcast_ref::<Histogram>() {
                let percentiles: Vec<f64> = PERCENTILES.iter().map(|v| v.1).collect();

                if let Some(Ok(snapshot)) = histogram.snapshot_between(start..end) {
                    if let Ok(result) = snapshot.percentiles(&percentiles) {
                        for (label, value) in PERCENTILES.iter().map(|v| v.0).zip(result.iter().map(|(_, b)| b.end())) {
                            data.push(format!(
                                "{}/{}: {}",
                                metric.formatted(metriken::Format::Simple),
                                label,
                                value,
                            ));
                        }
                    }
                }
            }
        }

        data.sort();
        let mut content = data.join("\n");
        content += "\n";
        Ok(content)
    }

    pub async fn hwinfo() -> Result<impl warp::Reply, Infallible> {
        if let Ok(hwinfo) = crate::samplers::hwinfo::hardware_info() {
            Ok(warp::reply::json(hwinfo))
        } else {
            Ok(warp::reply::json(&false))
        }
    }
}
