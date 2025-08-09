use crate::viewer::tsdb::Tsdb;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct CgroupCorrelationResult {
    pub cgroup_name: String,
    pub metric1: String,
    pub metric2: String,
    pub correlation: f64,
    pub sample_count: usize,
}

/// Discover correlations for cgroup metrics, analyzing each cgroup separately
pub fn discover_cgroup_correlations(
    tsdb: &Arc<Tsdb>,
    min_correlation: Option<f64>,
) -> Result<Vec<CgroupCorrelationResult>, Box<dyn std::error::Error>> {
    let min_correlation = min_correlation.unwrap_or(0.5);
    let mut results = Vec::new();
    
    // First, identify all unique cgroup names
    let mut cgroup_names = HashSet::new();
    
    // Check one cgroup metric to get all cgroup names
    if let Some(collection) = tsdb.counters("cgroup_cpu_usage", ()) {
        for labels in collection.labels() {
            if let Some(name) = labels.inner.get("name") {
                cgroup_names.insert(name.clone());
            }
        }
    }
    
    eprintln!("Found {} cgroups to analyze", cgroup_names.len());
    
    // Get all cgroup metric names
    let cgroup_metrics: Vec<String> = tsdb.counter_names()
        .iter()
        .filter(|name| name.starts_with("cgroup_"))
        .map(|s| s.to_string())
        .collect();
    
    let gauge_metrics: Vec<String> = tsdb.gauge_names()
        .iter()
        .filter(|name| name.starts_with("cgroup_"))
        .map(|s| s.to_string())
        .collect();
    
    eprintln!("Found {} cgroup counter metrics and {} gauge metrics", 
              cgroup_metrics.len(), gauge_metrics.len());
    
    // For each cgroup, analyze correlations (limit to first 5 for performance)
    for (idx, cgroup_name) in cgroup_names.iter().take(5).enumerate() {
        eprintln!("Analyzing cgroup {} of {}: {}", idx + 1, cgroup_names.len().min(5), cgroup_name);
        
        // Collect time series for this specific cgroup
        let mut series_map = HashMap::new();
        
        // Get counter metrics for this cgroup
        for metric_name in &cgroup_metrics {
            if let Some(collection) = tsdb.counters(metric_name, [("name", cgroup_name.as_str())]) {
                let rate = collection.rate();
                // Get the sum for this specific cgroup
                let series = rate.sum();
                if !series.inner.is_empty() {
                    series_map.insert(metric_name.clone(), series);
                }
            }
        }
        
        // Get gauge metrics for this cgroup
        for metric_name in &gauge_metrics {
            if let Some(collection) = tsdb.gauges(metric_name, [("name", cgroup_name.as_str())]) {
                let untyped = collection.untyped();
                // Get the sum for this specific cgroup
                let series = untyped.sum();
                if !series.inner.is_empty() {
                    series_map.insert(metric_name.clone(), series);
                }
            }
        }
        
        // Also correlate with system-wide metrics for this cgroup
        let system_metrics = get_system_metrics(tsdb);
        
        // Analyze correlations within this cgroup
        let metrics_list: Vec<_> = series_map.keys().cloned().collect();
        for i in 0..metrics_list.len() {
            for j in i+1..metrics_list.len() {
                let metric1 = &metrics_list[i];
                let metric2 = &metrics_list[j];
                
                if let (Some(series1), Some(series2)) = 
                    (series_map.get(metric1), series_map.get(metric2)) {
                    
                    if let Ok((corr, count)) = compute_correlation(series1, series2) {
                        if corr.abs() >= min_correlation && count >= 10 {
                            results.push(CgroupCorrelationResult {
                                cgroup_name: cgroup_name.clone(),
                                metric1: metric1.clone(),
                                metric2: metric2.clone(),
                                correlation: corr,
                                sample_count: count,
                            });
                        }
                    }
                }
            }
        }
        
        // Skip system correlation for now (too expensive)
        // TODO: Add back with better performance
    }
    
    // Sort by absolute correlation
    results.sort_by(|a, b| {
        b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap()
    });
    
    Ok(results)
}

/// Get aggregated system-wide metrics
fn get_system_metrics(tsdb: &Tsdb) -> HashMap<String, crate::viewer::tsdb::UntypedSeries> {
    let mut metrics = HashMap::new();
    
    // Get non-cgroup metrics
    for name in tsdb.counter_names() {
        if !name.starts_with("cgroup_") {
            if let Some(collection) = tsdb.counters(name, ()) {
                let rate = collection.rate();
                let aggregated = rate.sum();
                if !aggregated.inner.is_empty() {
                    metrics.insert(name.to_string(), aggregated);
                }
            }
        }
    }
    
    for name in tsdb.gauge_names() {
        if !name.starts_with("cgroup_") {
            if let Some(collection) = tsdb.gauges(name, ()) {
                let untyped = collection.untyped();
                let aggregated = untyped.sum();
                if !aggregated.inner.is_empty() {
                    metrics.insert(name.to_string(), aggregated);
                }
            }
        }
    }
    
    metrics
}

fn compute_correlation(
    series1: &crate::viewer::tsdb::UntypedSeries,
    series2: &crate::viewer::tsdb::UntypedSeries,
) -> Result<(f64, usize), Box<dyn std::error::Error>> {
    use std::collections::HashMap;
    
    // Align series
    let mut values1 = Vec::new();
    let mut values2 = Vec::new();
    
    let map2: HashMap<u64, f64> = series2.inner.iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    
    for (timestamp, value1) in series1.inner.iter() {
        if let Some(&value2) = map2.get(timestamp) {
            values1.push(*value1);
            values2.push(value2);
        }
    }
    
    if values1.len() < 3 {
        return Err("Not enough data points".into());
    }
    
    let n = values1.len() as f64;
    
    // Calculate means
    let mean1 = values1.iter().sum::<f64>() / n;
    let mean2 = values2.iter().sum::<f64>() / n;
    
    // Calculate correlation
    let mut numerator = 0.0;
    let mut denominator1 = 0.0;
    let mut denominator2 = 0.0;
    
    for i in 0..values1.len() {
        let diff1 = values1[i] - mean1;
        let diff2 = values2[i] - mean2;
        numerator += diff1 * diff2;
        denominator1 += diff1 * diff1;
        denominator2 += diff2 * diff2;
    }
    
    let correlation = if denominator1 > 0.0 && denominator2 > 0.0 {
        numerator / (denominator1.sqrt() * denominator2.sqrt())
    } else {
        0.0
    };
    
    Ok((correlation, values1.len()))
}

pub fn format_cgroup_report(results: &[CgroupCorrelationResult]) -> String {
    let mut summary = String::new();
    
    // Group by cgroup
    let mut by_cgroup: HashMap<String, Vec<&CgroupCorrelationResult>> = HashMap::new();
    for result in results {
        by_cgroup.entry(result.cgroup_name.clone())
            .or_default()
            .push(result);
    }
    
    summary.push_str(&format!("Analyzed {} cgroups\n\n", by_cgroup.len()));
    
    for (cgroup_name, cgroup_results) in by_cgroup {
        if cgroup_results.is_empty() {
            continue;
        }
        
        summary.push_str(&format!("📦 CGROUP: {}\n", cgroup_name));
        
        // Top correlations for this cgroup
        let mut sorted = cgroup_results.clone();
        sorted.sort_by(|a, b| {
            b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap()
        });
        
        for (i, result) in sorted.iter().take(5).enumerate() {
            summary.push_str(&format!(
                "  {}. {} vs {} (r={:.3})\n",
                i + 1, result.metric1, result.metric2, result.correlation
            ));
        }
        summary.push_str("\n");
    }
    
    // Overall strongest correlations
    summary.push_str("🏆 STRONGEST CGROUP CORRELATIONS OVERALL:\n");
    let mut all_sorted = results.to_vec();
    all_sorted.sort_by(|a, b| {
        b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap()
    });
    
    for (i, result) in all_sorted.iter().take(10).enumerate() {
        summary.push_str(&format!(
            "{}. [{}] {} vs {} (r={:.3})\n",
            i + 1, result.cgroup_name, result.metric1, result.metric2, result.correlation
        ));
    }
    
    summary
}