use crate::viewer::tsdb::{Tsdb, UntypedSeries};
use std::collections::{HashMap, BTreeMap};
use std::sync::Arc;

#[derive(Debug)]
pub struct CorrelationResult {
    pub coefficient: f64,
    pub p_value: f64,
    pub series1_name: String,
    pub series2_name: String,
    pub sample_count: usize,
}

#[derive(Debug)]
pub enum CorrelationAnalysis {
    /// Single correlation between two individual series
    Single(CorrelationResult),
    /// Multiple correlations when one or both metrics have multiple series
    Multiple {
        /// Best (highest absolute value) correlation found
        best: CorrelationResult,
        /// All individual correlations computed
        all: Vec<CorrelationResult>,
        /// Aggregated correlation (if applicable)
        aggregated: Option<CorrelationResult>,
    },
}

impl CorrelationAnalysis {
    /// Get the most meaningful correlation for dashboard creation
    pub fn get_primary_correlation(&self) -> &CorrelationResult {
        match self {
            CorrelationAnalysis::Single(result) => result,
            CorrelationAnalysis::Multiple { best, aggregated, .. } => {
                // Prefer aggregated if it's strong and meaningful
                if let Some(agg) = aggregated {
                    if agg.coefficient.abs() > best.coefficient.abs() * 0.95 {
                        return agg;
                    }
                }
                best
            }
        }
    }
    
    /// Get dashboard recommendation
    pub fn get_dashboard_recommendation(&self) -> String {
        match self {
            CorrelationAnalysis::Single(_) => {
                "Create a scatter plot showing correlation over time.".to_string()
            }
            CorrelationAnalysis::Multiple { best, all, aggregated } => {
                let use_aggregated = if let Some(agg) = aggregated {
                    agg.coefficient.abs() > best.coefficient.abs() * 0.95
                } else {
                    false
                };
                
                if use_aggregated {
                    "Create dashboard with:\n\
                    1. Aggregated time series overlay\n\
                    2. Correlation scatter plot\n\
                    3. Heatmap showing individual series contributions".to_string()
                } else {
                    format!(
                        "Create dashboard focused on {} with:\n\
                        1. Time series comparison\n\
                        2. Scatter plot for strongest correlation\n\
                        3. Table of all correlation coefficients",
                        best.series1_name
                    )
                }
            }
        }
    }
    
    pub fn to_summary(&self) -> String {
        match self {
            CorrelationAnalysis::Single(result) => {
                format!(
                    "Correlation between {} and {}: r={:.3} (p={:.4}, n={})",
                    result.series1_name, result.series2_name, 
                    result.coefficient, result.p_value, result.sample_count
                )
            }
            CorrelationAnalysis::Multiple { best, all, aggregated } => {
                // Determine if aggregated is stronger than individual correlations
                let use_aggregated = if let Some(agg) = aggregated {
                    agg.coefficient.abs() > best.coefficient.abs() * 0.95
                } else {
                    false
                };
                
                let mut summary = if use_aggregated && aggregated.is_some() {
                    let agg = aggregated.as_ref().unwrap();
                    format!(
                        "Strongest signal is AGGREGATED: {} vs {} (r={:.3}, p={:.4})\n\
                        This suggests a system-wide relationship rather than individual component correlation.\n",
                        agg.series1_name, agg.series2_name, agg.coefficient, agg.p_value
                    )
                } else {
                    format!(
                        "Strongest signal is INDIVIDUAL: {} vs {} (r={:.3})\n\
                        This suggests specific components are driving the correlation.\n",
                        best.series1_name, best.series2_name, best.coefficient
                    )
                };
                
                summary.push_str(&format!("\nAnalyzed {} series combinations:\n", all.len()));
                
                // Show top individual correlations
                let mut sorted = all.clone();
                sorted.sort_by(|a, b| {
                    b.coefficient.abs().partial_cmp(&a.coefficient.abs()).unwrap()
                });
                
                summary.push_str("Top individual correlations:\n");
                for (i, result) in sorted.iter().take(5).enumerate() {
                    summary.push_str(&format!(
                        "  {}. {} vs {}: r={:.3}\n",
                        i + 1, result.series1_name, result.series2_name, result.coefficient
                    ));
                }
                
                if let Some(agg) = aggregated {
                    if !use_aggregated {
                        summary.push_str(&format!(
                            "\nAggregated correlation: r={:.3} (p={:.4})\n",
                            agg.coefficient, agg.p_value
                        ));
                    }
                    
                    // Add interpretation
                    let variance = calculate_correlation_variance(&sorted);
                    if variance > 0.2 {
                        summary.push_str("High variance in individual correlations suggests uneven load or component-specific issues.\n");
                    } else {
                        summary.push_str("Low variance in individual correlations suggests uniform system behavior.\n");
                    }
                }
                
                summary
            }
        }
    }
}

/// Analyze correlation between two metrics
pub fn analyze_correlation(
    tsdb: &Arc<Tsdb>,
    metric1: &str,
    metric2: &str,
) -> Result<CorrelationAnalysis, Box<dyn std::error::Error>> {
    // Get all series for each metric
    let series1 = get_all_series(tsdb, metric1)?;
    let series2 = get_all_series(tsdb, metric2)?;
    
    if series1.is_empty() || series2.is_empty() {
        return Err(format!("No data found for metrics: {} or {}", metric1, metric2).into());
    }
    
    // Case 1: Both metrics have single series
    if series1.len() == 1 && series2.len() == 1 {
        let result = compute_correlation(
            &series1[0].1,
            &series2[0].1,
            &series1[0].0,
            &series2[0].0,
        )?;
        return Ok(CorrelationAnalysis::Single(result));
    }
    
    // Case 2: One or both metrics have multiple series
    let mut all_correlations = Vec::new();
    
    // Compute all pairwise correlations
    for (name1, data1) in &series1 {
        for (name2, data2) in &series2 {
            if let Ok(result) = compute_correlation(data1, data2, name1, name2) {
                all_correlations.push(result);
            }
        }
    }
    
    if all_correlations.is_empty() {
        return Err("Could not compute any correlations".into());
    }
    
    // Find best correlation
    let best = all_correlations
        .iter()
        .max_by(|a, b| a.coefficient.abs().partial_cmp(&b.coefficient.abs()).unwrap())
        .unwrap()
        .clone();
    
    // Compute aggregated correlation if it makes sense
    let aggregated = if series1.len() > 1 || series2.len() > 1 {
        // Aggregate series and compute correlation
        let agg1 = aggregate_series(&series1);
        let agg2 = aggregate_series(&series2);
        
        compute_correlation(
            &agg1,
            &agg2,
            &format!("{} (aggregated)", metric1),
            &format!("{} (aggregated)", metric2),
        ).ok()
    } else {
        None
    };
    
    Ok(CorrelationAnalysis::Multiple {
        best,
        all: all_correlations,
        aggregated,
    })
}

/// Get all series for a metric
fn get_all_series(
    tsdb: &Tsdb,
    metric: &str,
) -> Result<Vec<(String, UntypedSeries)>, Box<dyn std::error::Error>> {
    let mut series = Vec::new();
    
    // Try to get as counter first
    if let Some(collection) = tsdb.counters(metric, ()) {
        let rate_collection = collection.rate();
        for (labels, data) in rate_collection.iter() {
            let name = format_series_name(metric, &labels.inner);
            series.push((name, data.clone()));
        }
    }
    // Try as gauge
    else if let Some(collection) = tsdb.gauges(metric, ()) {
        // Gauge collections work differently - need to convert to untyped
        let untyped_collection = collection.untyped();
        for (labels, data) in untyped_collection.iter() {
            let name = format_series_name(metric, &labels.inner);
            series.push((name, data.clone()));
        }
    }
    
    Ok(series)
}

/// Format a series name from metric and labels
fn format_series_name(metric: &str, labels: &BTreeMap<String, String>) -> String {
    if labels.is_empty() {
        metric.to_string()
    } else {
        // Find the most important label (id, name, direction, op, etc.)
        let key_label = labels.get("id")
            .or_else(|| labels.get("name"))
            .or_else(|| labels.get("direction"))
            .or_else(|| labels.get("op"))
            .or_else(|| labels.values().next());
            
        if let Some(value) = key_label {
            format!("{}[{}]", metric, value)
        } else {
            metric.to_string()
        }
    }
}

/// Aggregate multiple series into one
fn aggregate_series(series_list: &[(String, UntypedSeries)]) -> UntypedSeries {
    if series_list.len() == 1 {
        return series_list[0].1.clone();
    }
    
    // Create a map of timestamp -> sum of values
    let mut aggregated: HashMap<u64, f64> = HashMap::new();
    let mut counts: HashMap<u64, usize> = HashMap::new();
    
    for (_, series) in series_list {
        for (timestamp, value) in series.inner.iter() {
            *aggregated.entry(*timestamp).or_insert(0.0) += value;
            *counts.entry(*timestamp).or_insert(0) += 1;
        }
    }
    
    // Convert to average values and BTreeMap
    let mut result = BTreeMap::new();
    for (timestamp, sum) in aggregated {
        let count = counts[&timestamp] as f64;
        result.insert(timestamp, sum / count);
    }
    
    UntypedSeries { inner: result }
}

/// Compute Pearson correlation coefficient
fn compute_correlation(
    series1: &UntypedSeries,
    series2: &UntypedSeries,
    name1: &str,
    name2: &str,
) -> Result<CorrelationResult, Box<dyn std::error::Error>> {
    // Align series by timestamp
    let (values1, values2) = align_series(series1, series2);
    
    if values1.len() < 3 {
        return Err("Not enough data points for correlation".into());
    }
    
    let n = values1.len() as f64;
    
    // Calculate means
    let mean1: f64 = values1.iter().sum::<f64>() / n;
    let mean2: f64 = values2.iter().sum::<f64>() / n;
    
    // Calculate correlation coefficient
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
    
    let coefficient = if denominator1 > 0.0 && denominator2 > 0.0 {
        numerator / (denominator1.sqrt() * denominator2.sqrt())
    } else {
        0.0
    };
    
    // Calculate p-value using t-distribution approximation
    let t_statistic = coefficient * ((n - 2.0) / (1.0 - coefficient * coefficient)).sqrt();
    let p_value = calculate_p_value(t_statistic, n as usize - 2);
    
    Ok(CorrelationResult {
        coefficient,
        p_value,
        series1_name: name1.to_string(),
        series2_name: name2.to_string(),
        sample_count: values1.len(),
    })
}

/// Align two series by matching timestamps
fn align_series(series1: &UntypedSeries, series2: &UntypedSeries) -> (Vec<f64>, Vec<f64>) {
    let mut values1 = Vec::new();
    let mut values2 = Vec::new();
    
    // Create maps for fast lookup
    let map1: HashMap<u64, f64> = series1.inner.iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    let map2: HashMap<u64, f64> = series2.inner.iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    
    // Find common timestamps
    for (timestamp, value1) in &map1 {
        if let Some(&value2) = map2.get(timestamp) {
            values1.push(*value1);
            values2.push(value2);
        }
    }
    
    (values1, values2)
}

/// Calculate variance in correlation coefficients
fn calculate_correlation_variance(correlations: &[CorrelationResult]) -> f64 {
    if correlations.is_empty() {
        return 0.0;
    }
    
    let values: Vec<f64> = correlations.iter().map(|c| c.coefficient.abs()).collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter()
        .map(|v| (v - mean).powi(2))
        .sum::<f64>() / values.len() as f64;
    
    variance.sqrt() // Return standard deviation
}

/// Calculate approximate p-value for correlation coefficient
fn calculate_p_value(t_statistic: f64, degrees_of_freedom: usize) -> f64 {
    // Simplified p-value calculation
    // For a two-tailed test with t-distribution
    let t_abs = t_statistic.abs();
    
    // Use approximation for p-value
    if degrees_of_freedom < 3 {
        return 1.0;
    }
    
    // Rough approximation using normal distribution for large df
    if t_abs < 1.96 {
        0.05 + (1.96 - t_abs) * 0.475 / 1.96
    } else if t_abs < 2.58 {
        0.01 + (2.58 - t_abs) * 0.04 / (2.58 - 1.96)
    } else {
        0.01 * (2.58 / t_abs).powi(2)
    }
}

// Re-export for use in server
impl Clone for CorrelationResult {
    fn clone(&self) -> Self {
        CorrelationResult {
            coefficient: self.coefficient,
            p_value: self.p_value,
            series1_name: self.series1_name.clone(),
            series2_name: self.series2_name.clone(),
            sample_count: self.sample_count,
        }
    }
}