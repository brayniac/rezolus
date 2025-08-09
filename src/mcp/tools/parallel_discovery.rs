use crate::viewer::tsdb::{Tsdb, UntypedSeries};
use crate::mcp::tools::cgroup_discovery::CgroupCorrelationResult;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct ParallelCorrelationResult {
    pub metric1: String,
    pub metric2: String,
    pub correlation: f64,
    pub sample_count: usize,
}

/// Parallel correlation discovery using rayon
pub fn parallel_discover_correlations(
    tsdb: &Arc<Tsdb>,
    min_correlation: Option<f64>,
) -> Result<Vec<ParallelCorrelationResult>, Box<dyn std::error::Error>> {
    let min_correlation = min_correlation.unwrap_or(0.5);
    
    // Collect all aggregated series upfront
    let mut all_series = Vec::new();
    
    // Aggregate counters
    for name in tsdb.counter_names() {
        if let Some(collection) = tsdb.counters(name, ()) {
            let rate = collection.rate();
            let aggregated = rate.sum();
            if !aggregated.inner.is_empty() {
                all_series.push((name.to_string(), Arc::new(aggregated)));
            }
        }
    }
    
    // Aggregate gauges
    for name in tsdb.gauge_names() {
        if let Some(collection) = tsdb.gauges(name, ()) {
            let untyped = collection.untyped();
            let aggregated = untyped.sum();
            if !aggregated.inner.is_empty() {
                all_series.push((name.to_string(), Arc::new(aggregated)));
            }
        }
    }
    
    eprintln!("Parallel discovery: {} aggregated metrics", all_series.len());
    
    // Generate all unique pairs
    let mut pairs = Vec::new();
    for i in 0..all_series.len() {
        for j in i+1..all_series.len() {
            if !should_skip_pair(&all_series[i].0, &all_series[j].0) {
                pairs.push((i, j));
            }
        }
    }
    
    eprintln!("Analyzing {} metric pairs in parallel", pairs.len());
    
    // Process pairs in parallel
    let results: Vec<_> = pairs
        .par_iter()
        .filter_map(|(i, j)| {
            let (name1, series1) = &all_series[*i];
            let (name2, series2) = &all_series[*j];
            
            match compute_correlation(series1, series2) {
                Ok((corr, count)) if corr.abs() >= min_correlation && count >= 10 => {
                    Some(ParallelCorrelationResult {
                        metric1: name1.clone(),
                        metric2: name2.clone(),
                        correlation: corr,
                        sample_count: count,
                    })
                }
                _ => None,
            }
        })
        .collect();
    
    // Sort by absolute correlation
    let mut sorted_results = results;
    sorted_results.sort_by(|a, b| {
        b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap()
    });
    
    Ok(sorted_results)
}

/// Parallel per-cgroup correlation discovery
pub fn parallel_cgroup_correlations(
    tsdb: &Arc<Tsdb>,
    min_correlation: Option<f64>,
    max_cgroups: Option<usize>,
) -> Result<Vec<CgroupCorrelationResult>, Box<dyn std::error::Error>> {
    let min_correlation = min_correlation.unwrap_or(0.5);
    
    // Get cgroup names
    let mut cgroup_names = std::collections::HashSet::new();
    if let Some(collection) = tsdb.counters("cgroup_cpu_usage", ()) {
        for labels in collection.labels() {
            if let Some(name) = labels.inner.get("name") {
                cgroup_names.insert(name.clone());
            }
        }
    }
    
    let cgroup_names: Vec<_> = if let Some(max) = max_cgroups {
        cgroup_names.into_iter().take(max).collect()
    } else {
        cgroup_names.into_iter().collect()
    };
    eprintln!("Analyzing {} cgroups in parallel", cgroup_names.len());
    
    // Get metric names once
    let cgroup_metrics: Vec<String> = tsdb.counter_names()
        .iter()
        .filter(|name| name.starts_with("cgroup_"))
        .map(|s| s.to_string())
        .collect();
    
    // Process each cgroup in parallel
    let all_results: Vec<Vec<CgroupCorrelationResult>> = cgroup_names
        .par_iter()
        .map(|cgroup_name| {
            analyze_single_cgroup(tsdb, cgroup_name, &cgroup_metrics, min_correlation)
        })
        .collect();
    
    // Flatten and sort results
    let mut results: Vec<_> = all_results.into_iter().flatten().collect();
    results.sort_by(|a, b| {
        b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap()
    });
    
    Ok(results)
}

fn analyze_single_cgroup(
    tsdb: &Arc<Tsdb>,
    cgroup_name: &str,
    metric_names: &[String],
    min_correlation: f64,
) -> Vec<CgroupCorrelationResult> {
    let mut series_map = HashMap::new();
    let mut results = Vec::new();
    
    // Collect series for this cgroup
    for metric_name in metric_names {
        if let Some(collection) = tsdb.counters(metric_name, [("name", cgroup_name)]) {
            let rate = collection.rate();
            let series = rate.sum();
            if !series.inner.is_empty() {
                series_map.insert(metric_name.clone(), series);
            }
        }
    }
    
    // Compute correlations within this cgroup
    let metrics_list: Vec<_> = series_map.keys().cloned().collect();
    for i in 0..metrics_list.len() {
        for j in i+1..metrics_list.len() {
            let metric1 = &metrics_list[i];
            let metric2 = &metrics_list[j];
            
            if let (Some(series1), Some(series2)) = 
                (series_map.get(metric1), series_map.get(metric2)) {
                
                if let Ok((corr, count)) = compute_correlation_owned(series1, series2) {
                    if corr.abs() >= min_correlation && count >= 10 {
                        results.push(CgroupCorrelationResult {
                            cgroup_name: cgroup_name.to_string(),
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
    
    results
}

// Use the CgroupCorrelationResult from cgroup_discovery module

fn should_skip_pair(metric1: &str, metric2: &str) -> bool {
    if metric1 == metric2 {
        return true;
    }
    
    let obvious_pairs = [
        ("memory_total", "memory_free"),
        ("memory_total", "memory_available"),
        ("memory_total", "memory_cached"),
    ];
    
    for (a, b) in &obvious_pairs {
        if (metric1 == *a && metric2 == *b) || (metric1 == *b && metric2 == *a) {
            return true;
        }
    }
    
    false
}

fn compute_correlation(
    series1: &Arc<UntypedSeries>,
    series2: &Arc<UntypedSeries>,
) -> Result<(f64, usize), Box<dyn std::error::Error>> {
    compute_correlation_owned(series1.as_ref(), series2.as_ref())
}

fn compute_correlation_owned(
    series1: &UntypedSeries,
    series2: &UntypedSeries,
) -> Result<(f64, usize), Box<dyn std::error::Error>> {
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