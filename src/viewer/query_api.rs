use super::*;
use axum::extract::{Query as QueryParams, State};
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Query API endpoints for PromQL-style queries
pub fn query_router() -> Router<Arc<QueryState>> {
    Router::new()
        .route("/query", get(instant_query))
        .route("/query_range", get(range_query))
        .route("/dashboards", get(list_dashboards))
        .route("/dashboard/{name}", get(get_dashboard))
        .route("/metrics", get(list_metrics))
        .route("/labels/{metric}", get(list_labels))
}

/// State for query API
pub struct QueryState {
    pub tsdb: Tsdb,
}

/// Instant query parameters
#[derive(Debug, Deserialize)]
pub struct InstantQueryParams {
    /// PromQL expression
    pub query: String,
    /// Evaluation timestamp (Unix seconds)
    pub time: Option<i64>,
}

/// Range query parameters
#[derive(Debug, Deserialize)]
pub struct RangeQueryParams {
    /// PromQL expression
    pub query: String,
    /// Start timestamp (Unix seconds)
    pub start: i64,
    /// End timestamp (Unix seconds)
    pub end: i64,
    /// Query resolution step in seconds
    pub step: u64,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status: "success".to_string(),
            data: Some(data),
            error: None,
        }
    }
    
    pub fn error(error: String) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            error: Some(error),
        }
    }
}

/// Execute an instant query
async fn instant_query(
    State(state): State<Arc<QueryState>>,
    QueryParams(params): QueryParams<InstantQueryParams>,
) -> Json<ApiResponse<QueryResult>> {
    // For now, execute simplified queries against the TSDB
    match execute_simple_query(&state.tsdb, &params.query, params.time) {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

/// Execute a range query
async fn range_query(
    State(_state): State<Arc<QueryState>>,
    QueryParams(_params): QueryParams<RangeQueryParams>,
) -> Json<ApiResponse<QueryResult>> {
    // For now, return error as range queries aren't implemented yet
    Json(ApiResponse::error("Range queries not yet implemented".to_string()))
}

/// List available dashboards
async fn list_dashboards() -> Json<ApiResponse<Vec<DashboardInfo>>> {
    let dashboards = vec![
        DashboardInfo { name: "overview".to_string(), title: "Overview".to_string() },
        DashboardInfo { name: "cpu".to_string(), title: "CPU".to_string() },
        DashboardInfo { name: "network".to_string(), title: "Network".to_string() },
        DashboardInfo { name: "blockio".to_string(), title: "BlockIO".to_string() },
        DashboardInfo { name: "scheduler".to_string(), title: "Scheduler".to_string() },
        DashboardInfo { name: "syscall".to_string(), title: "Syscall".to_string() },
        DashboardInfo { name: "softirq".to_string(), title: "SoftIRQ".to_string() },
        DashboardInfo { name: "rezolus".to_string(), title: "Rezolus".to_string() },
        DashboardInfo { name: "cgroups".to_string(), title: "Cgroups".to_string() },
    ];
    
    Json(ApiResponse::success(dashboards))
}

/// Get a specific dashboard definition
async fn get_dashboard(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Json<ApiResponse<super::dashboard::common::PromQLDashboard>> {
    match super::dashboard::promql_dashboards::get_dashboard(&name) {
        Some(dashboard) => Json(ApiResponse::success(dashboard)),
        None => Json(ApiResponse::error(format!("Dashboard '{}' not found", name))),
    }
}

/// List available metrics
async fn list_metrics(
    State(state): State<Arc<QueryState>>,
) -> Json<ApiResponse<Vec<String>>> {
    // Extract metric names from TSDB
    let metrics = extract_metric_names(&state.tsdb);
    Json(ApiResponse::success(metrics))
}

/// List labels for a metric
async fn list_labels(
    State(state): State<Arc<QueryState>>,
    axum::extract::Path(metric): axum::extract::Path<String>,
) -> Json<ApiResponse<Vec<String>>> {
    // Extract label names for the metric
    let labels = extract_label_names(&state.tsdb, &metric);
    Json(ApiResponse::success(labels))
}

#[derive(Debug, Serialize)]
pub struct DashboardInfo {
    pub name: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "resultType")]
pub enum QueryResult {
    #[serde(rename = "vector")]
    Vector { result: Vec<VectorResult> },
    #[serde(rename = "matrix")]
    Matrix { result: Vec<MatrixResult> },
    #[serde(rename = "scalar")]
    Scalar { result: (f64, String) },
}

#[derive(Debug, Serialize)]
pub struct VectorResult {
    pub metric: std::collections::HashMap<String, String>,
    pub value: (i64, String),
}

#[derive(Debug, Serialize)]
pub struct MatrixResult {
    pub metric: std::collections::HashMap<String, String>,
    pub values: Vec<(i64, String)>,
}

/// Execute a simplified query against the TSDB
/// This is a temporary implementation until full PromQL support is added
fn execute_simple_query(
    tsdb: &Tsdb,
    query: &str,
    time: Option<i64>,
) -> Result<QueryResult, Box<dyn std::error::Error>> {
    let time = time.unwrap_or_else(|| chrono::Utc::now().timestamp());
    
    // Check for arithmetic operations FIRST
    if query.contains(" * ") {
        let parts: Vec<&str> = query.split(" * ").collect();
        if parts.len() == 2 {
            let left_str = parts[0].trim();
            let right_str = parts[1].trim();
            
            // Try to parse right side as a number first (common case for unit conversions)
            if let Some(multiplier) = parse_number(right_str) {
                // Execute left side and multiply by scalar
                if let Ok(left_result) = execute_simple_query(tsdb, left_str, Some(time)) {
                    // Handle scalar multiplication
                    match left_result {
                        QueryResult::Matrix { result } => {
                            // Transform all series in the result
                            let transformed_results: Vec<MatrixResult> = result.into_iter()
                                .map(|matrix_result| {
                                    let transformed_values: Vec<(i64, String)> = matrix_result.values.iter()
                                        .map(|(timestamp, value_str)| {
                                            let value: f64 = value_str.parse().unwrap_or(0.0);
                                            let new_value = value * multiplier;
                                            (*timestamp, new_value.to_string())
                                        })
                                        .collect();
                                    
                                    MatrixResult {
                                        metric: matrix_result.metric.clone(),
                                        values: transformed_values,
                                    }
                                })
                                .collect();
                            
                            return Ok(QueryResult::Matrix {
                                result: transformed_results,
                            });
                        }
                        QueryResult::Vector { result } => {
                            if !result.is_empty() {
                                let left_value: f64 = result[0].value.1.parse().unwrap_or(0.0);
                                let final_value = left_value * multiplier;
                                
                                return Ok(QueryResult::Vector {
                                    result: vec![VectorResult {
                                        metric: result[0].metric.clone(),
                                        value: (time, final_value.to_string()),
                                    }],
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        return Err(format!("Failed to execute multiplication query: {}", query).into());
    }
    
    if query.contains(" / ") {
        let parts: Vec<&str> = query.split(" / ").collect();
        if parts.len() == 2 {
            let left_str = parts[0].trim();
            let right_str = parts[1].trim();
            
            // Try to parse right side as a number first (common case for unit conversions)
            if let Some(divisor) = parse_number(right_str) {
                // Execute left side and divide by scalar
                if let Ok(left_result) = execute_simple_query(tsdb, left_str, Some(time)) {
                    // Handle scalar division
                    match left_result {
                        QueryResult::Matrix { result } => {
                            // Transform all series in the result
                            let transformed_results: Vec<MatrixResult> = result.into_iter()
                                .map(|matrix_result| {
                                    let transformed_values: Vec<(i64, String)> = matrix_result.values.iter()
                                        .map(|(timestamp, value_str)| {
                                            let value: f64 = value_str.parse().unwrap_or(0.0);
                                            let new_value = value / divisor;
                                            (*timestamp, new_value.to_string())
                                        })
                                        .collect();
                                    
                                    MatrixResult {
                                        metric: matrix_result.metric.clone(),
                                        values: transformed_values,
                                    }
                                })
                                .collect();
                            
                            return Ok(QueryResult::Matrix {
                                result: transformed_results,
                            });
                        }
                        QueryResult::Vector { result } => {
                            if !result.is_empty() {
                                let left_value: f64 = result[0].value.1.parse().unwrap_or(0.0);
                                let final_value = left_value / divisor;
                                
                                return Ok(QueryResult::Vector {
                                    result: vec![VectorResult {
                                        metric: result[0].metric.clone(),
                                        value: (time, final_value.to_string()),
                                    }],
                                });
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                // Right side is another query - time series division
                if let Ok(left_result) = execute_simple_query(tsdb, left_str, Some(time)) {
                    if let Ok(right_result) = execute_simple_query(tsdb, right_str, Some(time)) {
                        match (left_result, right_result) {
                            (QueryResult::Matrix { result: left_matrix }, QueryResult::Matrix { result: right_matrix }) => {
                                if !left_matrix.is_empty() && !right_matrix.is_empty() {
                                    let left_series = &left_matrix[0];
                                    let right_series = &right_matrix[0];
                                    
                                    // Create a map for efficient lookup of right side values by timestamp
                                    let right_values: std::collections::HashMap<i64, f64> = right_series.values.iter()
                                        .map(|(ts, val_str)| (*ts, val_str.parse().unwrap_or(0.0)))
                                        .collect();
                                    
                                    // Divide left series by right series at matching timestamps
                                    let transformed_values: Vec<(i64, String)> = left_series.values.iter()
                                        .filter_map(|(timestamp, value_str)| {
                                            let left_value: f64 = value_str.parse().unwrap_or(0.0);
                                            if let Some(&right_value) = right_values.get(timestamp) {
                                                if right_value != 0.0 {
                                                    let result = left_value / right_value;
                                                    Some((*timestamp, result.to_string()))
                                                } else {
                                                    None // Skip division by zero
                                                }
                                            } else {
                                                None // Skip if no matching timestamp
                                            }
                                        })
                                        .collect();
                                        
                                    return Ok(QueryResult::Matrix {
                                        result: vec![MatrixResult {
                                            metric: left_series.metric.clone(),
                                            values: transformed_values,
                                        }],
                                    });
                                }
                            }
                            (QueryResult::Vector { result: left_vector }, QueryResult::Vector { result: right_vector }) => {
                                if !left_vector.is_empty() && !right_vector.is_empty() {
                                    let left_value: f64 = left_vector[0].value.1.parse().unwrap_or(0.0);
                                    let right_value: f64 = right_vector[0].value.1.parse().unwrap_or(0.0);
                                    if right_value != 0.0 {
                                        let final_value = left_value / right_value;
                                        
                                        return Ok(QueryResult::Vector {
                                            result: vec![VectorResult {
                                                metric: left_vector[0].metric.clone(),
                                                value: (time, final_value.to_string()),
                                            }],
                                        });
                                    }
                                }
                            }
                            _ => {
                            }
                        }
                    }
                }
            }
        }
        
        return Err(format!("Failed to execute division query: {}", query).into());
    }
    
    // Check for histogram_quantile function
    if query.starts_with("histogram_quantile(") {
        if let Some((quantile, metric, labels)) = parse_histogram_quantile_query(query) {
            let label_refs: Vec<(&str, &str)> = labels.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            
            // Convert quantile (0.99) to percentage (99.0)
            let percentile = quantile * 100.0;
            
            if let Some(percentile_series) = tsdb.percentiles(&metric, label_refs.as_slice(), &[percentile]) {
                if !percentile_series.is_empty() {
                    let series = &percentile_series[0];
                    
                    // Return the full time series as matrix data
                    let mut values = Vec::new();
                    for (timestamp, value) in series.inner.iter() {
                        // Convert timestamp from nanoseconds since epoch to seconds
                        let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                        values.push((timestamp_secs, value.to_string()));
                    }
                    
                    let mut metric_labels = std::collections::HashMap::new();
                    metric_labels.insert("__name__".to_string(), format!("{}_{}", metric, (quantile * 100.0) as u32));
                    
                    return Ok(QueryResult::Matrix {
                        result: vec![MatrixResult {
                            metric: metric_labels,
                            values,
                        }],
                    });
                }
            }
        }
        
        return Err(format!("Failed to execute histogram_quantile query: {}", query).into());
    }

    // Check for avg() function
    if query.starts_with("avg(") && query.ends_with(")") {
        let inner_query = &query[4..query.len()-1];
        if let Ok(result) = execute_simple_query(tsdb, inner_query, Some(time)) {
            match result {
                QueryResult::Matrix { result: matrix } => {
                    if !matrix.is_empty() {
                        // Average the values across all series
                        let mut avg_values: std::collections::HashMap<i64, (f64, usize)> = std::collections::HashMap::new();
                        
                        for series in &matrix {
                            for (timestamp, value_str) in &series.values {
                                let value: f64 = value_str.parse().unwrap_or(0.0);
                                let entry = avg_values.entry(*timestamp).or_insert((0.0, 0));
                                entry.0 += value;
                                entry.1 += 1;
                            }
                        }
                        
                        let mut values: Vec<(i64, String)> = avg_values.into_iter()
                            .map(|(timestamp, (sum, count))| {
                                let avg = sum / count as f64;
                                (timestamp, avg.to_string())
                            })
                            .collect();
                        values.sort_by_key(|&(ts, _)| ts);
                        
                        let mut metric_labels = std::collections::HashMap::new();
                        if let Some(name) = matrix[0].metric.get("__name__") {
                            metric_labels.insert("__name__".to_string(), name.clone());
                        }
                        
                        return Ok(QueryResult::Matrix {
                            result: vec![MatrixResult {
                                metric: metric_labels,
                                values,
                            }],
                        });
                    }
                }
                _ => {}
            }
        }
        return Err(format!("Failed to execute avg query: {}", query).into());
    }
    
    // Check for sum() function with optional by clause
    if query.starts_with("sum") {
        // Parse sum by (label) or just sum()
        let (inner_query, group_by_labels) = if query.starts_with("sum by (") {
            // Extract the grouping labels
            if let Some(close_paren) = query[8..].find(')') {
                let labels_str = &query[8..8+close_paren];
                let labels: Vec<String> = labels_str.split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                
                // Find the inner query after the closing paren and opening paren
                let rest = &query[8+close_paren+1..];
                if rest.starts_with(" (") && rest.ends_with(")") {
                    let inner = &rest[2..rest.len()-1];
                    (inner, Some(labels))
                } else {
                    return Err(format!("Invalid sum by syntax: {}", query).into());
                }
            } else {
                return Err(format!("Invalid sum by syntax: {}", query).into());
            }
        } else if query.starts_with("sum(") && query.ends_with(")") {
            (&query[4..query.len()-1], None)
        } else {
            return Err(format!("Invalid sum syntax: {}", query).into());
        };
        
        if let Ok(result) = execute_simple_query(tsdb, inner_query, Some(time)) {
            match result {
                QueryResult::Matrix { result: matrix } => {
                    if !matrix.is_empty() {
                        if let Some(group_labels) = group_by_labels {
                            // Group by specified labels
                            let mut grouped_series: std::collections::HashMap<
                                std::collections::BTreeMap<String, String>,
                                std::collections::HashMap<i64, f64>
                            > = std::collections::HashMap::new();
                            
                            for series in &matrix {
                                // Extract the grouping key
                                let mut group_key = std::collections::BTreeMap::new();
                                for label in &group_labels {
                                    if let Some(value) = series.metric.get(label) {
                                        group_key.insert(label.clone(), value.clone());
                                    }
                                }
                                
                                // Add values to the group
                                let group_values = grouped_series.entry(group_key).or_insert_with(std::collections::HashMap::new);
                                for (timestamp, value_str) in &series.values {
                                    let value: f64 = value_str.parse().unwrap_or(0.0);
                                    *group_values.entry(*timestamp).or_insert(0.0) += value;
                                }
                            }
                            
                            // Convert grouped series to results
                            let mut results = Vec::new();
                            for (group_labels, timestamp_values) in grouped_series {
                                let mut values: Vec<(i64, String)> = timestamp_values.into_iter()
                                    .map(|(ts, val)| (ts, val.to_string()))
                                    .collect();
                                values.sort_by_key(|&(ts, _)| ts);
                                
                                let mut metric_labels = std::collections::HashMap::new();
                                if let Some(name) = matrix[0].metric.get("__name__") {
                                    metric_labels.insert("__name__".to_string(), name.clone());
                                }
                                // Add the group labels
                                for (k, v) in group_labels {
                                    metric_labels.insert(k, v);
                                }
                                
                                results.push(MatrixResult {
                                    metric: metric_labels,
                                    values,
                                });
                            }
                            
                            // Sort results for consistent ordering
                            results.sort_by(|a, b| {
                                // Create a stable sort key from all labels
                                // Sort labels alphabetically and format as {key1="value1", key2="value2"}
                                let format_labels = |labels: &std::collections::HashMap<String, String>| -> String {
                                    let mut sorted_labels: Vec<_> = labels.iter()
                                        .filter(|(k, _)| *k != "__name__") // Exclude __name__ from sort key
                                        .collect();
                                    sorted_labels.sort_by_key(|(k, _)| k.as_str());
                                    
                                    let label_strings: Vec<String> = sorted_labels.iter()
                                        .map(|(k, v)| format!("{}=\"{}\"", k, v))
                                        .collect();
                                    format!("{{{}}}", label_strings.join(", "))
                                };
                                
                                // First try to compare by ID if it exists
                                let a_id = a.metric.get("id").map(|s| s.as_str()).unwrap_or("");
                                let b_id = b.metric.get("id").map(|s| s.as_str()).unwrap_or("");
                                
                                let id_cmp = match (a_id.parse::<i32>(), b_id.parse::<i32>()) {
                                    (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
                                    _ => a_id.cmp(b_id)
                                };
                                
                                if id_cmp != std::cmp::Ordering::Equal {
                                    return id_cmp;
                                }
                                
                                // If IDs are equal or don't exist, use full label comparison
                                let a_labels = format_labels(&a.metric);
                                let b_labels = format_labels(&b.metric);
                                a_labels.cmp(&b_labels)
                            });
                            
                            return Ok(QueryResult::Matrix { result: results });
                        } else {
                            // No grouping - sum all series together
                            let mut sum_values: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
                            
                            for series in &matrix {
                                for (timestamp, value_str) in &series.values {
                                    let value: f64 = value_str.parse().unwrap_or(0.0);
                                    *sum_values.entry(*timestamp).or_insert(0.0) += value;
                                }
                            }
                            
                            let mut values: Vec<(i64, String)> = sum_values.into_iter()
                                .map(|(timestamp, sum)| (timestamp, sum.to_string()))
                                .collect();
                            values.sort_by_key(|&(ts, _)| ts);
                            
                            let mut metric_labels = std::collections::HashMap::new();
                            if let Some(name) = matrix[0].metric.get("__name__") {
                                metric_labels.insert("__name__".to_string(), name.clone());
                            }
                            
                            return Ok(QueryResult::Matrix {
                                result: vec![MatrixResult {
                                    metric: metric_labels,
                                    values,
                                }],
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        return Err(format!("Failed to execute sum query: {}", query).into());
    }

    // Parse simple metric queries like "cpu_usage" or "network_bytes{direction=\"transmit\"}"
    if let Some((metric, labels)) = parse_simple_metric_query(query) {
        // Check for irate() function
        if query.starts_with("irate(") {
            // Convert to the format Labels expects
            let label_refs: Vec<(&str, &str)> = labels.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            
            // Return all individual series (don't aggregate unless explicitly requested)
            if let Some(collection) = tsdb.counters(&metric, label_refs.as_slice()) {
                let rate_collection = collection.rate();
                
                // Always return individual series
                let mut results = Vec::new();
                
                for (labels, series) in rate_collection.iter() {
                    let mut values = Vec::new();
                    for (timestamp, value) in series.inner.iter() {
                        let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                        values.push((timestamp_secs, value.to_string()));
                    }
                    
                    if !values.is_empty() {
                        let mut metric_labels = std::collections::HashMap::new();
                        metric_labels.insert("__name__".to_string(), metric.clone());
                        // Add the actual labels
                        for (k, v) in &labels.inner {
                            metric_labels.insert(k.clone(), v.clone());
                        }
                        
                        results.push(MatrixResult {
                            metric: metric_labels,
                            values,
                        });
                    }
                }
                
                // Sort results if we have any
                if !results.is_empty() {
                    results.sort_by(|a, b| {
                        let a_id = a.metric.get("id").map(|s| s.as_str()).unwrap_or("");
                        let b_id = b.metric.get("id").map(|s| s.as_str()).unwrap_or("");
                        
                        // First compare by ID (numeric if possible)
                        let id_cmp = match (a_id.parse::<i32>(), b_id.parse::<i32>()) {
                            (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
                            _ => a_id.cmp(b_id)
                        };
                        
                        if id_cmp != std::cmp::Ordering::Equal {
                            return id_cmp;
                        }
                        
                        // If IDs are equal, create a stable sort key from all labels
                        // Sort labels alphabetically and format as {key1="value1", key2="value2"}
                        let format_labels = |labels: &std::collections::HashMap<String, String>| -> String {
                            let mut sorted_labels: Vec<_> = labels.iter()
                                .filter(|(k, _)| *k != "__name__") // Exclude __name__ from sort key
                                .collect();
                            sorted_labels.sort_by_key(|(k, _)| k.as_str());
                            
                            let label_strings: Vec<String> = sorted_labels.iter()
                                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                                .collect();
                            format!("{{{}}}", label_strings.join(", "))
                        };
                        
                        let a_labels = format_labels(&a.metric);
                        let b_labels = format_labels(&b.metric);
                        a_labels.cmp(&b_labels)
                    });
                }
                
                // Return results (even if empty)
                return Ok(QueryResult::Matrix { result: results });
            }
        } else {
            // Direct metric query - try counters first, then gauges
            let label_refs: Vec<(&str, &str)> = labels.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            
            // Try counters first (most metrics are counters)
            if let Some(collection) = tsdb.counters(&metric, label_refs.as_slice()) {
                let series = collection.rate().sum(); // For counters, use rate
                
                let mut values = Vec::new();
                for (timestamp, value) in series.inner.iter() {
                    // Convert timestamp from nanoseconds since epoch to seconds
                    let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                    values.push((timestamp_secs, value.to_string()));
                }
                
                let mut metric_labels = std::collections::HashMap::new();
                metric_labels.insert("__name__".to_string(), metric);
                
                return Ok(QueryResult::Matrix {
                    result: vec![MatrixResult {
                        metric: metric_labels,
                        values,
                    }],
                });
            }
            
            // Try gauges for instantaneous values
            if let Some(collection) = tsdb.gauges(&metric, label_refs.as_slice()) {
                let series = collection.sum();
                
                let mut values = Vec::new();
                for (timestamp, value) in series.inner.iter() {
                    // Convert timestamp from nanoseconds since epoch to seconds
                    let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                    values.push((timestamp_secs, value.to_string()));
                }
                
                let mut metric_labels = std::collections::HashMap::new();
                metric_labels.insert("__name__".to_string(), metric);
                
                return Ok(QueryResult::Matrix {
                    result: vec![MatrixResult {
                        metric: metric_labels,
                        values,
                    }],
                });
            }
        }
    }
    
    Err(format!("Unsupported query: {}", query).into())
}

/// Parse a simple metric query
fn parse_simple_metric_query(query: &str) -> Option<(String, Vec<(String, String)>)> {
    // Remove function wrappers like irate() if present
    let query = if query.starts_with("irate(") && query.ends_with(")") {
        &query[6..query.len()-1]
    } else {
        query
    };
    
    // Remove time range like [1m] if present
    let query = if let Some(bracket_pos) = query.find('[') {
        &query[..bracket_pos]
    } else {
        query
    };
    
    // Split metric name and labels
    if let Some(brace_pos) = query.find('{') {
        let metric = query[..brace_pos].to_string();
        let labels_str = &query[brace_pos+1..query.len()-1];
        
        // Parse labels
        let labels: Vec<(String, String)> = labels_str
            .split(',')
            .filter_map(|pair| {
                let parts: Vec<&str> = pair.split('=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim().to_string();
                    let value = parts[1].trim().trim_matches('"').to_string();
                    Some((key, value))
                } else {
                    None
                }
            })
            .collect();
        
        Some((metric, labels))
    } else {
        Some((query.to_string(), vec![]))
    }
}

/// Parse a histogram_quantile query
/// Format: histogram_quantile(0.99, metric_name{labels})
fn parse_histogram_quantile_query(query: &str) -> Option<(f64, String, Vec<(String, String)>)> {
    if !query.starts_with("histogram_quantile(") || !query.ends_with(")") {
        return None;
    }
    
    // Remove "histogram_quantile(" and ")"
    let content = &query[19..query.len()-1];
    
    // Split on first comma to separate quantile from metric
    let mut parts = content.splitn(2, ',');
    let quantile_str = parts.next()?.trim();
    let metric_part = parts.next()?.trim();
    
    // Parse quantile value
    let quantile: f64 = quantile_str.parse().ok()?;
    
    // Parse metric name and labels using existing function
    if let Some((metric, labels)) = parse_simple_metric_query(metric_part) {
        Some((quantile, metric, labels))
    } else {
        None
    }
}

/// Parse a number from a string
fn parse_number(s: &str) -> Option<f64> {
    // Rust's built-in parser handles scientific notation like 1e9, 1e6, etc.
    s.parse().ok()
}

/// Extract metric names from TSDB
fn extract_metric_names(tsdb: &Tsdb) -> Vec<String> {
    // This would need to be implemented based on TSDB structure
    // For now, return common metrics
    vec![
        "cpu_usage".to_string(),
        "cpu_instructions".to_string(),
        "cpu_cycles".to_string(),
        "network_bytes".to_string(),
        "network_packets".to_string(),
        "blockio_bytes".to_string(),
        "blockio_operations".to_string(),
        "blockio_latency".to_string(),
        "syscall".to_string(),
        "syscall_latency".to_string(),
        "scheduler_runqueue_latency".to_string(),
        "tcp_packet_latency".to_string(),
    ]
}

/// Extract label names for a metric
fn extract_label_names(tsdb: &Tsdb, metric: &str) -> Vec<String> {
    // This would need to be implemented based on TSDB structure
    // For now, return common labels based on metric
    match metric {
        "cpu_usage" => vec!["state".to_string(), "cpu".to_string()],
        "network_bytes" | "network_packets" => vec!["direction".to_string()],
        "blockio_bytes" | "blockio_operations" | "blockio_latency" => vec!["op".to_string()],
        "syscall" | "syscall_latency" => vec!["op".to_string()],
        _ => vec![],
    }
}