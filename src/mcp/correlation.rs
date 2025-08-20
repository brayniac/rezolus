use crate::viewer::promql::{MatrixSample, QueryEngine, QueryResult};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CorrelationResult {
    pub metric1: String,
    pub metric2: String,
    pub metric1_name: Option<String>, // Human-readable name
    pub metric2_name: Option<String>, // Human-readable name
    pub correlation: f64,
    pub sample_count: usize,
    pub series_pairs: Vec<SeriesCorrelation>,
}

#[derive(Debug, Clone)]
pub struct SeriesCorrelation {
    pub labels1: HashMap<String, String>,
    pub labels2: HashMap<String, String>,
    pub correlation: f64,
    pub sample_count: usize,
}

/// Calculate correlation between two PromQL expressions
/// 
/// This handles various cases:
/// - Simple metrics: `cpu_usage` vs `memory_used`
/// - Rate queries: `irate(cpu_cycles[5m])` vs `irate(instructions[5m])`
/// - Aggregations: `sum by (name) (irate(cgroup_cpu_usage[5m]))` vs `sum by (id) (irate(cpu_usage[5m]))`
pub fn calculate_correlation(
    engine: &Arc<QueryEngine>,
    expr1: &str,
    expr2: &str,
    start: f64,
    end: f64,
    step: f64,
) -> Result<CorrelationResult, Box<dyn std::error::Error>> {
    calculate_correlation_with_names(engine, expr1, expr2, None, None, start, end, step)
}

/// Calculate correlation with optional human-readable names
pub fn calculate_correlation_with_names(
    engine: &Arc<QueryEngine>,
    expr1: &str,
    expr2: &str,
    name1: Option<&str>,
    name2: Option<&str>,
    start: f64,
    end: f64,
    step: f64,
) -> Result<CorrelationResult, Box<dyn std::error::Error>> {
    // Query both expressions
    let result1 = engine.query_range(expr1, start, end, step)?;
    let result2 = engine.query_range(expr2, start, end, step)?;

    // Extract matrix samples
    let samples1 = extract_matrix_samples(&result1)?;
    let samples2 = extract_matrix_samples(&result2)?;

    if samples1.is_empty() || samples2.is_empty() {
        return Err("No data returned from queries".into());
    }

    // Calculate correlations between all series pairs
    let mut series_pairs = Vec::new();
    let mut all_correlations = Vec::new();

    for s1 in &samples1 {
        for s2 in &samples2 {
            if let Some(corr) = calculate_series_correlation(s1, s2) {
                series_pairs.push(SeriesCorrelation {
                    labels1: s1.metric.clone(),
                    labels2: s2.metric.clone(),
                    correlation: corr.0,
                    sample_count: corr.1,
                });
                all_correlations.push((corr.0, corr.1));
            }
        }
    }

    // Calculate overall correlation
    // Weight by sample count if we have multiple series
    let (overall_correlation, total_samples) = if all_correlations.len() == 1 {
        all_correlations[0]
    } else {
        // Weighted average of correlations
        let total_weight: usize = all_correlations.iter().map(|(_, n)| n).sum();
        let weighted_sum: f64 = all_correlations
            .iter()
            .map(|(r, n)| r * (*n as f64))
            .sum();
        
        if total_weight > 0 {
            (weighted_sum / (total_weight as f64), total_weight)
        } else {
            (0.0, 0)
        }
    };

    // Sort series pairs by absolute correlation
    series_pairs.sort_by(|a, b| {
        b.correlation.abs()
            .partial_cmp(&a.correlation.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(CorrelationResult {
        metric1: expr1.to_string(),
        metric2: expr2.to_string(),
        metric1_name: name1.map(|s| s.to_string()),
        metric2_name: name2.map(|s| s.to_string()),
        correlation: overall_correlation,
        sample_count: total_samples,
        series_pairs,
    })
}

/// Calculate correlation between two specific time series
fn calculate_series_correlation(
    series1: &MatrixSample,
    series2: &MatrixSample,
) -> Option<(f64, usize)> {
    // Create maps for efficient timestamp lookup
    let mut values1_map: HashMap<u64, f64> = HashMap::new();
    for (ts, val) in &series1.values {
        // Convert to nanoseconds for consistent comparison
        let ts_ns = (*ts * 1e9) as u64;
        values1_map.insert(ts_ns, *val);
    }

    // Find matching timestamps and collect value pairs
    let mut paired_values = Vec::new();
    for (ts, val2) in &series2.values {
        let ts_ns = (*ts * 1e9) as u64;
        if let Some(val1) = values1_map.get(&ts_ns) {
            paired_values.push((*val1, *val2));
        }
    }

    // Need at least 3 points for meaningful correlation
    if paired_values.len() < 3 {
        return None;
    }

    // Calculate Pearson correlation coefficient
    let n = paired_values.len() as f64;
    let sum_x: f64 = paired_values.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = paired_values.iter().map(|(_, y)| y).sum();
    let sum_xx: f64 = paired_values.iter().map(|(x, _)| x * x).sum();
    let sum_yy: f64 = paired_values.iter().map(|(_, y)| y * y).sum();
    let sum_xy: f64 = paired_values.iter().map(|(x, y)| x * y).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_xx - sum_x * sum_x) * (n * sum_yy - sum_y * sum_y)).sqrt();

    if denominator == 0.0 {
        // Perfect correlation (all values are the same)
        Some((0.0, paired_values.len()))
    } else {
        Some((numerator / denominator, paired_values.len()))
    }
}

/// Extract matrix samples from a query result
fn extract_matrix_samples(
    result: &QueryResult,
) -> Result<Vec<MatrixSample>, Box<dyn std::error::Error>> {
    match result {
        QueryResult::Matrix { result } => Ok(result.clone()),
        QueryResult::Vector { result } => {
            // Convert vector to single-sample matrix
            Ok(result
                .iter()
                .map(|s| MatrixSample {
                    metric: s.metric.clone(),
                    values: vec![s.value],
                })
                .collect())
        }
        QueryResult::Scalar { result } => {
            // Convert scalar to single-sample matrix
            Ok(vec![MatrixSample {
                metric: HashMap::new(),
                values: vec![*result],
            }])
        }
    }
}

/// Format correlation result for display
pub fn format_correlation_result(result: &CorrelationResult) -> String {
    let mut output = String::new();
    
    // Use human-readable names if available, otherwise fall back to queries
    let display1 = result.metric1_name.as_ref().unwrap_or(&result.metric1);
    let display2 = result.metric2_name.as_ref().unwrap_or(&result.metric2);
    
    output.push_str(&format!(
        "Correlation Analysis\n\
         ====================\n\
         Metric 1: {}\n",
        display1
    ));
    
    // If we have a name, also show the query
    if result.metric1_name.is_some() {
        output.push_str(&format!("  Query: {}\n", result.metric1));
    }
    
    output.push_str(&format!("Metric 2: {}\n", display2));
    
    if result.metric2_name.is_some() {
        output.push_str(&format!("  Query: {}\n", result.metric2));
    }
    
    output.push_str(&format!(
        "\nOverall correlation: {:.4}\n\
         Total sample pairs: {}\n\
         Interpretation: {}\n",
        result.correlation,
        result.sample_count,
        interpret_correlation(result.correlation)
    ));

    if result.series_pairs.len() > 1 {
        output.push_str(&format!(
            "\nSeries-level correlations ({} pairs):\n",
            result.series_pairs.len()
        ));

        // Show top correlations
        let show_count = 10.min(result.series_pairs.len());
        for (i, pair) in result.series_pairs.iter().take(show_count).enumerate() {
            output.push_str(&format!(
                "{}. r={:.4} (n={}) ",
                i + 1,
                pair.correlation,
                pair.sample_count
            ));

            // Format labels compactly
            if !pair.labels1.is_empty() {
                let labels1: Vec<String> = pair.labels1
                    .iter()
                    .filter(|(k, _)| *k != "__name__")
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                if !labels1.is_empty() {
                    output.push_str(&format!("[{}]", labels1.join(",")));
                }
            }
            
            output.push_str(" vs ");
            
            if !pair.labels2.is_empty() {
                let labels2: Vec<String> = pair.labels2
                    .iter()
                    .filter(|(k, _)| *k != "__name__")
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                if !labels2.is_empty() {
                    output.push_str(&format!("[{}]", labels2.join(",")));
                }
            }
            
            output.push_str("\n");
        }

        if result.series_pairs.len() > show_count {
            output.push_str(&format!(
                "... and {} more pairs\n",
                result.series_pairs.len() - show_count
            ));
        }
    }

    output
}

fn interpret_correlation(r: f64) -> &'static str {
    let abs_r = r.abs();
    if abs_r >= 0.9 {
        if r > 0.0 {
            "Very strong positive correlation"
        } else {
            "Very strong negative correlation"
        }
    } else if abs_r >= 0.7 {
        if r > 0.0 {
            "Strong positive correlation"
        } else {
            "Strong negative correlation"
        }
    } else if abs_r >= 0.5 {
        if r > 0.0 {
            "Moderate positive correlation"
        } else {
            "Moderate negative correlation"
        }
    } else if abs_r >= 0.3 {
        if r > 0.0 {
            "Weak positive correlation"
        } else {
            "Weak negative correlation"
        }
    } else {
        "Very weak or no correlation"
    }
}