use crate::viewer::promql::{QueryEngine, QueryResult};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AnomalyResult {
    pub metric_name: String,
    pub metric_query: String,
    pub method: AnomalyMethod,
    pub threshold: f64,
    pub anomalies: Vec<Anomaly>,
    pub total_samples: usize,
    pub statistics: Statistics,
}

#[derive(Debug, Clone)]
pub struct Anomaly {
    pub timestamp: f64,
    pub value: f64,
    pub score: f64, // Z-score, IQR multiplier, or MAD score
    pub labels: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Statistics {
    pub mean: f64,
    pub std_dev: f64,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum AnomalyMethod {
    ZScore,
    InterquartileRange,
    MedianAbsoluteDeviation,
}

impl std::fmt::Display for AnomalyMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnomalyMethod::ZScore => write!(f, "Z-Score"),
            AnomalyMethod::InterquartileRange => write!(f, "IQR"),
            AnomalyMethod::MedianAbsoluteDeviation => write!(f, "MAD"),
        }
    }
}

/// Detect anomalies in a metric using the specified method
pub fn detect_anomalies(
    engine: &Arc<QueryEngine>,
    metric_query: &str,
    metric_name: Option<&str>,
    method: AnomalyMethod,
    threshold: f64,
    start: f64,
    end: f64,
    step: f64,
) -> Result<AnomalyResult, Box<dyn std::error::Error>> {
    // Query the metric
    let result = engine.query_range(metric_query, start, end, step)?;
    
    // Extract time series data
    let series_data = extract_series_data(&result)?;
    
    if series_data.is_empty() {
        return Err("No data returned from query".into());
    }
    
    let mut all_anomalies = Vec::new();
    let mut all_values = Vec::new();
    
    // Process each series
    for (labels, values) in series_data {
        all_values.extend(values.iter().map(|(_, v)| *v));
        
        // Detect anomalies for this series
        let anomalies = match method {
            AnomalyMethod::ZScore => detect_zscore(&values, threshold),
            AnomalyMethod::InterquartileRange => detect_iqr(&values, threshold),
            AnomalyMethod::MedianAbsoluteDeviation => detect_mad(&values, threshold),
        };
        
        // Add labels to each anomaly
        for idx in anomalies {
            if let Some((timestamp, value)) = values.get(idx) {
                let score = calculate_anomaly_score(&values, *value, method);
                all_anomalies.push(Anomaly {
                    timestamp: *timestamp,
                    value: *value,
                    score,
                    labels: labels.clone(),
                });
            }
        }
    }
    
    // Calculate overall statistics
    let statistics = calculate_statistics(&all_values);
    
    // Sort anomalies by score magnitude
    all_anomalies.sort_by(|a, b| {
        b.score.abs().partial_cmp(&a.score.abs()).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    Ok(AnomalyResult {
        metric_name: metric_name.unwrap_or(metric_query).to_string(),
        metric_query: metric_query.to_string(),
        method,
        threshold,
        anomalies: all_anomalies,
        total_samples: all_values.len(),
        statistics,
    })
}

/// Extract series data from query result
fn extract_series_data(
    result: &QueryResult,
) -> Result<Vec<(std::collections::HashMap<String, String>, Vec<(f64, f64)>)>, Box<dyn std::error::Error>> {
    match result {
        QueryResult::Matrix { result } => {
            Ok(result.iter().map(|sample| {
                (sample.metric.clone(), sample.values.clone())
            }).collect())
        }
        QueryResult::Vector { result } => {
            Ok(result.iter().map(|sample| {
                (sample.metric.clone(), vec![sample.value])
            }).collect())
        }
        QueryResult::Scalar { result } => {
            Ok(vec![(std::collections::HashMap::new(), vec![*result])])
        }
    }
}

/// Z-Score anomaly detection
fn detect_zscore(values: &[(f64, f64)], threshold: f64) -> Vec<usize> {
    let data: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
    
    if data.len() < 2 {
        return Vec::new();
    }
    
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let variance = data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / data.len() as f64;
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return Vec::new();
    }
    
    data.iter()
        .enumerate()
        .filter(|(_, &value)| ((value - mean) / std_dev).abs() > threshold)
        .map(|(i, _)| i)
        .collect()
}

/// Interquartile Range (IQR) anomaly detection
fn detect_iqr(values: &[(f64, f64)], threshold: f64) -> Vec<usize> {
    let mut data: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
    
    if data.len() < 4 {
        return Vec::new();
    }
    
    data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let q1_idx = data.len() / 4;
    let q3_idx = 3 * data.len() / 4;
    let q1 = data[q1_idx];
    let q3 = data[q3_idx];
    let iqr = q3 - q1;
    
    if iqr == 0.0 {
        return Vec::new();
    }
    
    let lower_bound = q1 - threshold * iqr;
    let upper_bound = q3 + threshold * iqr;
    
    values.iter()
        .enumerate()
        .filter(|(_, (_, v))| *v < lower_bound || *v > upper_bound)
        .map(|(i, _)| i)
        .collect()
}

/// Median Absolute Deviation (MAD) anomaly detection
fn detect_mad(values: &[(f64, f64)], threshold: f64) -> Vec<usize> {
    let data: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
    
    if data.len() < 3 {
        return Vec::new();
    }
    
    // Calculate median
    let mut sorted_data = data.clone();
    sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let median = if sorted_data.len() % 2 == 0 {
        (sorted_data[sorted_data.len() / 2 - 1] + sorted_data[sorted_data.len() / 2]) / 2.0
    } else {
        sorted_data[sorted_data.len() / 2]
    };
    
    // Calculate MAD
    let mut deviations: Vec<f64> = data.iter().map(|v| (v - median).abs()).collect();
    deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mad = if deviations.len() % 2 == 0 {
        (deviations[deviations.len() / 2 - 1] + deviations[deviations.len() / 2]) / 2.0
    } else {
        deviations[deviations.len() / 2]
    };
    
    if mad == 0.0 {
        return Vec::new();
    }
    
    // Modified Z-score using MAD
    data.iter()
        .enumerate()
        .filter(|(_, &value)| {
            let modified_z_score = 0.6745 * (value - median) / mad;
            modified_z_score.abs() > threshold
        })
        .map(|(i, _)| i)
        .collect()
}

/// Calculate the anomaly score for a value
fn calculate_anomaly_score(values: &[(f64, f64)], value: f64, method: AnomalyMethod) -> f64 {
    let data: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
    
    match method {
        AnomalyMethod::ZScore => {
            let mean = data.iter().sum::<f64>() / data.len() as f64;
            let variance = data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / data.len() as f64;
            let std_dev = variance.sqrt();
            if std_dev > 0.0 {
                (value - mean) / std_dev
            } else {
                0.0
            }
        }
        AnomalyMethod::InterquartileRange => {
            let mut sorted = data.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let q1 = sorted[sorted.len() / 4];
            let q3 = sorted[3 * sorted.len() / 4];
            let iqr = q3 - q1;
            if iqr > 0.0 {
                if value < q1 {
                    (q1 - value) / iqr
                } else if value > q3 {
                    (value - q3) / iqr
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
        AnomalyMethod::MedianAbsoluteDeviation => {
            let mut sorted = data.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = if sorted.len() % 2 == 0 {
                (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
            } else {
                sorted[sorted.len() / 2]
            };
            
            let mut deviations: Vec<f64> = data.iter().map(|v| (v - median).abs()).collect();
            deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            
            let mad = if deviations.len() % 2 == 0 {
                (deviations[deviations.len() / 2 - 1] + deviations[deviations.len() / 2]) / 2.0
            } else {
                deviations[deviations.len() / 2]
            };
            
            if mad > 0.0 {
                0.6745 * (value - median) / mad
            } else {
                0.0
            }
        }
    }
}

/// Calculate statistics for a set of values
fn calculate_statistics(values: &[f64]) -> Statistics {
    if values.is_empty() {
        return Statistics {
            mean: 0.0,
            std_dev: 0.0,
            median: 0.0,
            q1: 0.0,
            q3: 0.0,
            min: 0.0,
            max: 0.0,
        };
    }
    
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let std_dev = variance.sqrt();
    
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let median = if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    
    let q1 = sorted[sorted.len() / 4];
    let q3 = sorted[3 * sorted.len() / 4];
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    
    Statistics {
        mean,
        std_dev,
        median,
        q1,
        q3,
        min,
        max,
    }
}

/// Format anomaly result for display
pub fn format_anomaly_result(result: &AnomalyResult) -> String {
    let mut output = String::new();
    
    output.push_str(&format!(
        "Anomaly Detection Results\n\
         ========================\n\
         Metric: {}\n",
        result.metric_name
    ));
    
    if result.metric_name != result.metric_query {
        output.push_str(&format!("Query: {}\n", result.metric_query));
    }
    
    output.push_str(&format!(
        "\nMethod: {} (threshold: {})\n\
         Total samples: {}\n\
         Anomalies found: {}\n\
         Anomaly rate: {:.2}%\n\
         \n\
         Statistics:\n\
         -----------\n\
         Mean: {:.4}\n\
         Std Dev: {:.4}\n\
         Median: {:.4}\n\
         Q1: {:.4}\n\
         Q3: {:.4}\n\
         Min: {:.4}\n\
         Max: {:.4}\n",
        result.method,
        result.threshold,
        result.total_samples,
        result.anomalies.len(),
        100.0 * result.anomalies.len() as f64 / result.total_samples as f64,
        result.statistics.mean,
        result.statistics.std_dev,
        result.statistics.median,
        result.statistics.q1,
        result.statistics.q3,
        result.statistics.min,
        result.statistics.max,
    ));
    
    if !result.anomalies.is_empty() {
        output.push_str("\nTop Anomalies (by score magnitude):\n");
        output.push_str("------------------------------------\n");
        
        for (i, anomaly) in result.anomalies.iter().take(10).enumerate() {
            let timestamp_str = format_timestamp(anomaly.timestamp);
            output.push_str(&format!(
                "{}. {} | Value: {:.4} | Score: {:.2}\n",
                i + 1,
                timestamp_str,
                anomaly.value,
                anomaly.score,
            ));
            
            // Show labels if present
            if !anomaly.labels.is_empty() {
                let labels: Vec<String> = anomaly.labels
                    .iter()
                    .filter(|(k, _)| *k != "__name__")
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                if !labels.is_empty() {
                    output.push_str(&format!("   Labels: {}\n", labels.join(", ")));
                }
            }
        }
        
        if result.anomalies.len() > 10 {
            output.push_str(&format!("\n... and {} more anomalies\n", result.anomalies.len() - 10));
        }
    } else {
        output.push_str("\nNo anomalies detected.\n");
    }
    
    output
}

/// Format timestamp for display
fn format_timestamp(timestamp: f64) -> String {
    use chrono::{DateTime, Utc};
    let datetime = DateTime::<Utc>::from_timestamp(timestamp as i64, 0)
        .unwrap_or_else(|| Utc::now());
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}