use crate::viewer::tsdb::Tsdb;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FastCorrelationResult {
    pub metric1: String,
    pub metric2: String,
    pub correlation: f64,
    pub sample_count: usize,
}

/// Fast correlation discovery that only checks aggregated metrics
pub fn fast_discover_correlations(
    tsdb: &Arc<Tsdb>,
    min_correlation: Option<f64>,
) -> Result<Vec<FastCorrelationResult>, Box<dyn std::error::Error>> {
    let min_correlation = min_correlation.unwrap_or(0.5);
    let mut results = Vec::new();
    
    // Get aggregated time series for each metric
    let mut aggregated_series = Vec::new();
    
    // Aggregate counters
    for name in tsdb.counter_names() {
        if let Some(collection) = tsdb.counters(name, ()) {
            let rate = collection.rate();
            let aggregated = rate.sum();
            if !aggregated.inner.is_empty() {
                aggregated_series.push((name.to_string(), aggregated));
            }
        }
    }
    
    // Aggregate gauges
    for name in tsdb.gauge_names() {
        if let Some(collection) = tsdb.gauges(name, ()) {
            let untyped = collection.untyped();
            let aggregated = untyped.sum();
            if !aggregated.inner.is_empty() {
                aggregated_series.push((name.to_string(), aggregated));
            }
        }
    }
    
    eprintln!("Fast discovery: {} aggregated metrics", aggregated_series.len());
    
    // Compute correlations between all pairs
    for i in 0..aggregated_series.len() {
        for j in i+1..aggregated_series.len() {
            let (name1, series1) = &aggregated_series[i];
            let (name2, series2) = &aggregated_series[j];
            
            // Skip obvious pairs
            if should_skip_pair(name1, name2) {
                continue;
            }
            
            // Compute correlation
            if let Ok((corr, count)) = compute_fast_correlation(&series1, &series2) {
                if corr.abs() >= min_correlation && count >= 10 {
                    results.push(FastCorrelationResult {
                        metric1: name1.clone(),
                        metric2: name2.clone(),
                        correlation: corr,
                        sample_count: count,
                    });
                }
            }
        }
    }
    
    // Sort by absolute correlation
    results.sort_by(|a, b| {
        b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap()
    });
    
    Ok(results)
}

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

fn compute_fast_correlation(
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