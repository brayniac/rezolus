use super::*;
use crate::viewer::dashboard::common::{PanelType, PromQLDashboard, PromQLGroup, PromQLPanel, PromQLQueryDef, Unit};
use axum::extract::{Path, Query as QueryParams, State};
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use regex;
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
        .route("/metadata", get(get_metadata))
        .route("/metrics/detailed", get(list_metrics_detailed))
        .route("/ai/generate", post(generate_ai_dashboard))
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
    /// Selected cgroups for filtering (comma-separated)
    pub selected_cgroups: Option<String>,
    /// Filter type for cgroup panels (selected/unselected)
    pub cgroup_filter: Option<String>,
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

/// Apply cgroup filter template to a query
fn apply_cgroup_filter_template(
    query: &str,
    selected_cgroups: Option<&str>,
    filter_type: Option<&str>,
) -> String {
    // If no template placeholder, return as-is
    if !query.contains("{{CGROUP_FILTER}}") {
        return query.to_string();
    }
    
    eprintln!("DEBUG template: query='{}', selected='{}', filter_type='{}'", 
             query, selected_cgroups.unwrap_or("none"), filter_type.unwrap_or("none"));
    
    // Determine what to replace the template with
    let filter_clause = match filter_type {
        Some("unselected") => {
            // For unselected, we want to exclude the selected cgroups
            match selected_cgroups {
                Some(cgroups) if !cgroups.is_empty() => {
                    // Parse comma-separated cgroups
                    let cgroup_list: Vec<&str> = cgroups.split(',').collect();
                    let pattern = cgroup_list.join("|");
                    format!("{{name!~\"{}\"}}", pattern)
                }
                _ => {
                    // No cgroups selected, include all
                    String::new()
                }
            }
        }
        Some("selected") => {
            // For selected, we want to include only the selected cgroups
            match selected_cgroups {
                Some(cgroups) if !cgroups.is_empty() => {
                    // Parse comma-separated cgroups
                    let cgroup_list: Vec<&str> = cgroups.split(',').collect();
                    let pattern = cgroup_list.join("|");
                    format!("{{name=~\"{}\"}}", pattern)
                }
                _ => {
                    // No cgroups selected, return no data
                    "{name=\"__none__\"}".to_string()
                }
            }
        }
        _ => {
            // No filter type specified, remove the placeholder
            String::new()
        }
    };
    
    eprintln!("  Replacing {{{{CGROUP_FILTER}}}} with: '{}'", filter_clause);
    let result = query.replace("{{CGROUP_FILTER}}", &filter_clause);
    eprintln!("  Result: '{}'", result);
    result
}

/// Execute an instant query
async fn instant_query(
    State(state): State<Arc<QueryState>>,
    QueryParams(params): QueryParams<InstantQueryParams>,
) -> Json<ApiResponse<QueryResult>> {
    // Apply cgroup filter template if present
    let query = apply_cgroup_filter_template(
        &params.query,
        params.selected_cgroups.as_deref(),
        params.cgroup_filter.as_deref(),
    );
    
    // Execute the templated query
    match execute_simple_query(&state.tsdb, &query, params.time) {
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
    Path(name): Path<String>,
) -> Json<ApiResponse<super::dashboard::common::PromQLDashboard>> {
    match super::dashboard::get_dashboard(&name) {
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
/// Helper function to extract content respecting parentheses balance
fn extract_balanced_content(s: &str) -> String {
    s.to_string()
}

/// Helper function to find an operator at the top level (not inside parentheses)
fn find_top_level_operator(query: &str, op: &str) -> Option<usize> {
    let mut paren_depth = 0;
    let bytes = query.as_bytes();
    let op_bytes = op.as_bytes();
    
    for i in 0..bytes.len() {
        if bytes[i] == b'(' {
            paren_depth += 1;
        } else if bytes[i] == b')' {
            paren_depth -= 1;
        } else if paren_depth == 0 {
            // Check if we found the operator at top level
            if i + op_bytes.len() <= bytes.len() {
                let slice = &bytes[i..i + op_bytes.len()];
                if slice == op_bytes {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// This is a temporary implementation until full PromQL support is added
fn execute_simple_query(
    tsdb: &Tsdb,
    query: &str,
    time: Option<i64>,
) -> Result<QueryResult, Box<dyn std::error::Error>> {
    let time = time.unwrap_or_else(|| chrono::Utc::now().timestamp());
    
    // Check for aggregation functions FIRST (before arithmetic operations)
    // This ensures we properly handle queries like avg(sum(...) / 1e9)
    
    // Check for avg() function
    if query.starts_with("avg(") && query.ends_with(")") {
        let inner_query = extract_balanced_content(&query[4..query.len()-1]);
        if let Ok(result) = execute_simple_query(tsdb, &inner_query, Some(time)) {
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
                        
                        // Convert to single series with averaged values
                        let mut values: Vec<(i64, String)> = avg_values.into_iter()
                            .map(|(timestamp, (sum, count))| {
                                let avg = if count > 0 { sum / count as f64 } else { 0.0 };
                                (timestamp, avg.to_string())
                            })
                            .collect();
                        values.sort_by_key(|v| v.0);
                        
                        return Ok(QueryResult::Matrix {
                            result: vec![MatrixResult {
                                metric: std::collections::HashMap::new(),
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
    
    // Handle parenthesized expressions first
    // If the entire query is wrapped in parentheses, evaluate the inner expression
    if query.starts_with('(') && query.ends_with(')') {
        // Check if these are just outer parentheses
        let mut paren_count = 0;
        let mut is_outer = true;
        for (i, ch) in query.chars().enumerate() {
            if ch == '(' {
                paren_count += 1;
            } else if ch == ')' {
                paren_count -= 1;
                // If we hit 0 before the end, these aren't just outer parens
                if paren_count == 0 && i < query.len() - 1 {
                    is_outer = false;
                    break;
                }
            }
        }
        if is_outer && paren_count == 0 {
            // Remove outer parentheses and re-evaluate
            let inner = &query[1..query.len()-1];
            return execute_simple_query(tsdb, inner, Some(time));
        }
    }
    
    // Now check for arithmetic operations at the top level only
    // Use a smarter approach that respects parentheses
    // Check operations in reverse order of precedence: +, -, *, /
    // This ensures that lower precedence operations are evaluated last
    
    // Check for addition operation first (lowest precedence)
    if let Some(op_pos) = find_top_level_operator(query, " + ") {
        let left_str = query[..op_pos].trim();
        let right_str = query[op_pos + 3..].trim();
        
        // Execute both sides
        let left_result = execute_simple_query(tsdb, left_str, Some(time));
        let right_result = execute_simple_query(tsdb, right_str, Some(time));
        
        if let (Ok(left_result), Ok(right_result)) = (left_result, right_result) {
            match (left_result, right_result) {
                (QueryResult::Matrix { result: left_matrix }, QueryResult::Matrix { result: right_matrix }) => {
                    if !left_matrix.is_empty() && !right_matrix.is_empty() {
                        // For simplicity, assume single series (common for gauge metrics)
                        if left_matrix.len() == 1 && right_matrix.len() == 1 {
                            let left_series = &left_matrix[0];
                            let right_series = &right_matrix[0];
                            
                            // Create a map for efficient lookup of right side values
                            let right_values: std::collections::HashMap<i64, f64> = right_series.values.iter()
                                .map(|(ts, val_str)| (*ts, val_str.parse().unwrap_or(0.0)))
                                .collect();
                            
                            // Add left and right at matching timestamps
                            let result_values: Vec<(i64, String)> = left_series.values.iter()
                                .filter_map(|(timestamp, value_str)| {
                                    let left_value: f64 = value_str.parse().unwrap_or(0.0);
                                    if let Some(&right_value) = right_values.get(timestamp) {
                                        let result = left_value + right_value;
                                        Some((*timestamp, result.to_string()))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            
                            if !result_values.is_empty() {
                                return Ok(QueryResult::Matrix {
                                    result: vec![MatrixResult {
                                        metric: std::collections::HashMap::new(),
                                        values: result_values,
                                    }],
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        return Err(format!("Failed to execute addition query: {}", query).into());
    }
    
    // Check for subtraction operation
    if let Some(op_pos) = find_top_level_operator(query, " - ") {
        let left_str = query[..op_pos].trim();
        let right_str = query[op_pos + 3..].trim();
        
        // Execute both sides
        let left_result = execute_simple_query(tsdb, left_str, Some(time));
        let right_result = execute_simple_query(tsdb, right_str, Some(time));
        
        if let (Ok(left_result), Ok(right_result)) = (left_result, right_result) {
            match (left_result, right_result) {
                (QueryResult::Matrix { result: left_matrix }, QueryResult::Matrix { result: right_matrix }) => {
                    if !left_matrix.is_empty() && !right_matrix.is_empty() {
                        // For simplicity, assume single series (common for gauge metrics)
                        if left_matrix.len() == 1 && right_matrix.len() == 1 {
                            let left_series = &left_matrix[0];
                            let right_series = &right_matrix[0];
                            
                            // Create a map for efficient lookup of right side values
                            let right_values: std::collections::HashMap<i64, f64> = right_series.values.iter()
                                .map(|(ts, val_str)| (*ts, val_str.parse().unwrap_or(0.0)))
                                .collect();
                            
                            // Subtract right from left at matching timestamps
                            let result_values: Vec<(i64, String)> = left_series.values.iter()
                                .filter_map(|(timestamp, value_str)| {
                                    let left_value: f64 = value_str.parse().unwrap_or(0.0);
                                    if let Some(&right_value) = right_values.get(timestamp) {
                                        let result = left_value - right_value;
                                        Some((*timestamp, result.to_string()))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            
                            if !result_values.is_empty() {
                                return Ok(QueryResult::Matrix {
                                    result: vec![MatrixResult {
                                        metric: std::collections::HashMap::new(),
                                        values: result_values,
                                    }],
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        return Err(format!("Failed to execute subtraction query: {}", query).into());
    }
    
    // Check for multiplication (higher precedence)
    if let Some(op_pos) = find_top_level_operator(query, " * ") {
        let left_str = query[..op_pos].trim();
        let right_str = query[op_pos + 3..].trim();
        
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
        
        return Err(format!("Failed to execute multiplication query: {}", query).into());
    }
    
    if let Some(op_pos) = find_top_level_operator(query, " / ") {
        let mut left_str = query[..op_pos].trim();
        let mut right_str = query[op_pos + 3..].trim();
            
            // Remove outer parentheses if the entire expression is wrapped
            if left_str.starts_with('(') && left_str.ends_with(')') {
                // Check if parentheses are balanced (simple check)
                let mut paren_count = 0;
                let mut is_outer = true;
                for (i, ch) in left_str.chars().enumerate() {
                    if ch == '(' {
                        paren_count += 1;
                    } else if ch == ')' {
                        paren_count -= 1;
                        // If we hit 0 before the end, these aren't just outer parens
                        if paren_count == 0 && i < left_str.len() - 1 {
                            is_outer = false;
                            break;
                        }
                    }
                }
                if is_outer && paren_count == 0 {
                    left_str = &left_str[1..left_str.len()-1];
                }
            }
            
            if right_str.starts_with('(') && right_str.ends_with(')') {
                let mut paren_count = 0;
                let mut is_outer = true;
                for (i, ch) in right_str.chars().enumerate() {
                    if ch == '(' {
                        paren_count += 1;
                    } else if ch == ')' {
                        paren_count -= 1;
                        if paren_count == 0 && i < right_str.len() - 1 {
                            is_outer = false;
                            break;
                        }
                    }
                }
                if is_outer && paren_count == 0 {
                    right_str = &right_str[1..right_str.len()-1];
                }
            }
            
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
                let left_result = execute_simple_query(tsdb, left_str, Some(time));
                let right_result = execute_simple_query(tsdb, right_str, Some(time));
                
                if let (Ok(left_result), Ok(right_result)) = (left_result, right_result) {
                        match (left_result, right_result) {
                            (QueryResult::Matrix { result: left_matrix }, QueryResult::Matrix { result: right_matrix }) => {
                                if !left_matrix.is_empty() && !right_matrix.is_empty() {
                                    // Handle multiple series by matching on labels
                                    let mut result_series = Vec::new();
                                    
                                    // Create a map of right series by their labels for efficient lookup
                                    let mut right_series_map: std::collections::HashMap<String, &MatrixResult> = std::collections::HashMap::new();
                                    for series in &right_matrix {
                                        // Create a key from the metric labels (usually the 'name' label for cgroups)
                                        let key = if let Some(name) = series.metric.get("name") {
                                            name.clone()
                                        } else {
                                            // For aggregated sums without labels, use empty key to match any
                                            // This handles cases like sum(...) / sum(...)
                                            String::new()
                                        };
                                        right_series_map.insert(key, series);
                                    }
                                    
                                    // Process each left series
                                    for left_series in &left_matrix {
                                        // Find matching right series
                                        let key = if let Some(name) = left_series.metric.get("name") {
                                            name.clone()
                                        } else {
                                            // For aggregated sums without labels, use empty key to match any
                                            String::new()
                                        };
                                        
                                        if let Some(right_series) = right_series_map.get(&key) {
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
                                                
                                            if !transformed_values.is_empty() {
                                                result_series.push(MatrixResult {
                                                    metric: left_series.metric.clone(),
                                                    values: transformed_values,
                                                });
                                            }
                                        }
                                    }
                                    
                                    if !result_series.is_empty() {
                                        return Ok(QueryResult::Matrix {
                                            result: result_series,
                                        });
                                    }
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
        
        return Err(format!("Failed to execute division query: {}", query).into());
    }
    
    // Check for subtraction operation
    if let Some(op_pos) = find_top_level_operator(query, " - ") {
        let left_str = query[..op_pos].trim();
        let right_str = query[op_pos + 3..].trim();
        
        // Execute both sides
        let left_result = execute_simple_query(tsdb, left_str, Some(time));
        let right_result = execute_simple_query(tsdb, right_str, Some(time));
        
        if let (Ok(left_result), Ok(right_result)) = (left_result, right_result) {
            match (left_result, right_result) {
                (QueryResult::Matrix { result: left_matrix }, QueryResult::Matrix { result: right_matrix }) => {
                    if !left_matrix.is_empty() && !right_matrix.is_empty() {
                        // For simplicity, assume single series (common for gauge metrics)
                        if left_matrix.len() == 1 && right_matrix.len() == 1 {
                            let left_series = &left_matrix[0];
                            let right_series = &right_matrix[0];
                            
                            // Create a map for efficient lookup of right side values
                            let right_values: std::collections::HashMap<i64, f64> = right_series.values.iter()
                                .map(|(ts, val_str)| (*ts, val_str.parse().unwrap_or(0.0)))
                                .collect();
                            
                            // Subtract right from left at matching timestamps
                            let result_values: Vec<(i64, String)> = left_series.values.iter()
                                .filter_map(|(timestamp, value_str)| {
                                    let left_value: f64 = value_str.parse().unwrap_or(0.0);
                                    if let Some(&right_value) = right_values.get(timestamp) {
                                        let result = left_value - right_value;
                                        Some((*timestamp, result.to_string()))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            
                            if !result_values.is_empty() {
                                return Ok(QueryResult::Matrix {
                                    result: vec![MatrixResult {
                                        metric: std::collections::HashMap::new(),
                                        values: result_values,
                                    }],
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        return Err(format!("Failed to execute subtraction query: {}", query).into());
    }
    
    // Check for addition operation
    if let Some(op_pos) = find_top_level_operator(query, " + ") {
        let left_str = query[..op_pos].trim();
        let right_str = query[op_pos + 3..].trim();
        
        // Execute both sides
        let left_result = execute_simple_query(tsdb, left_str, Some(time));
        let right_result = execute_simple_query(tsdb, right_str, Some(time));
        
        if let (Ok(left_result), Ok(right_result)) = (left_result, right_result) {
            match (left_result, right_result) {
                (QueryResult::Matrix { result: left_matrix }, QueryResult::Matrix { result: right_matrix }) => {
                    if !left_matrix.is_empty() && !right_matrix.is_empty() {
                        // For simplicity, assume single series (common for gauge metrics)
                        if left_matrix.len() == 1 && right_matrix.len() == 1 {
                            let left_series = &left_matrix[0];
                            let right_series = &right_matrix[0];
                            
                            // Create a map for efficient lookup of right side values
                            let right_values: std::collections::HashMap<i64, f64> = right_series.values.iter()
                                .map(|(ts, val_str)| (*ts, val_str.parse().unwrap_or(0.0)))
                                .collect();
                            
                            // Add left and right at matching timestamps
                            let result_values: Vec<(i64, String)> = left_series.values.iter()
                                .filter_map(|(timestamp, value_str)| {
                                    let left_value: f64 = value_str.parse().unwrap_or(0.0);
                                    if let Some(&right_value) = right_values.get(timestamp) {
                                        let result = left_value + right_value;
                                        Some((*timestamp, result.to_string()))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            
                            if !result_values.is_empty() {
                                return Ok(QueryResult::Matrix {
                                    result: vec![MatrixResult {
                                        metric: std::collections::HashMap::new(),
                                        values: result_values,
                                    }],
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        return Err(format!("Failed to execute addition query: {}", query).into());
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
        // Helper function to check if a label set matches the filters
        let matches_filters = |series_labels: &super::tsdb::Labels| -> bool {
            for (filter_key, filter_value) in &labels {
                // Handle special operators
                if filter_key.starts_with("__regex__") {
                    // Regex match operator =~
                    let actual_key = &filter_key[9..];
                    if let Some(series_value) = series_labels.inner.get(actual_key) {
                        if let Ok(re) = regex::Regex::new(filter_value) {
                            if !re.is_match(series_value) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                } else if filter_key.starts_with("__nregex__") {
                    // Not-regex operator !~
                    let actual_key = &filter_key[10..];
                    if let Some(series_value) = series_labels.inner.get(actual_key) {
                        if let Ok(re) = regex::Regex::new(filter_value) {
                            if re.is_match(series_value) {
                                return false;
                            }
                        }
                    }
                } else if filter_key.starts_with("__ne__") {
                    // Not-equal operator !=
                    let actual_key = &filter_key[6..];
                    if let Some(series_value) = series_labels.inner.get(actual_key) {
                        if series_value == filter_value {
                            return false;
                        }
                    }
                } else {
                    // Normal equality operator =
                    if let Some(series_value) = series_labels.inner.get(filter_key) {
                        if series_value != filter_value {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
            true
        };
        
        // Check for irate() function
        if query.starts_with("irate(") {
            // Get the metric data - for regex operators, get all series then filter
            let has_special_operators = labels.iter().any(|(k, _)| k.starts_with("__"));
            
            if has_special_operators {
                // Get all series, convert to rate, then filter
                // This is not ideal but works with current API
                if let Some(collection) = tsdb.counters(&metric, ()) {
                    let rate_collection = collection.rate();
                    
                    // Filter the rate collection
                    let mut filtered_collection = super::tsdb::UntypedCollection::default();
                    for (series_labels, series) in rate_collection.iter() {
                        if matches_filters(&series_labels) {
                            filtered_collection.insert(series_labels.clone(), series.clone());
                        }
                    }
                    
                    let rate_collection = filtered_collection;
                    
                    // Convert to results - filtering already applied
                    let mut results = Vec::new();
                    for (series_labels, series) in rate_collection.iter() {
                        let mut values = Vec::new();
                        for (timestamp, value) in series.inner.iter() {
                            let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                            values.push((timestamp_secs, value.to_string()));
                        }
                        
                        if !values.is_empty() {
                            let mut metric_labels = std::collections::HashMap::new();
                            metric_labels.insert("__name__".to_string(), metric.clone());
                            // Add all labels from the series
                            for (k, v) in &series_labels.inner {
                                metric_labels.insert(k.clone(), v.clone());
                            }
                            
                            results.push(MatrixResult {
                                metric: metric_labels,
                                values,
                            });
                        }
                    }
                    
                    return Ok(QueryResult::Matrix { result: results });
                }
            }
            
            // Non-cgroup metrics or cgroups without filters - use original logic
            // Convert to the format Labels expects (but only for non-special operators)
            let label_refs: Vec<(&str, &str)> = labels.iter()
                .filter(|(k, _)| !k.starts_with("__"))  // Skip special operators
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
            // Direct metric query - check if we need to apply special filtering
            let has_special_operators = labels.iter().any(|(k, _)| k.starts_with("__"));
            
            if has_special_operators {
                // Get all series and filter them
                // Try counters first
                if let Some(collection) = tsdb.counters(&metric, ()) {
                    // Convert to untyped (raw values, no rate)
                    let untyped_collection = collection.untyped();
                    
                    // Filter and return individual series
                    let mut results = Vec::new();
                    for (series_labels, series) in untyped_collection.iter() {
                        if matches_filters(&series_labels) {
                            let mut values = Vec::new();
                            for (timestamp, value) in series.inner.iter() {
                                let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                                values.push((timestamp_secs, value.to_string()));
                            }
                            
                            if !values.is_empty() {
                                let mut metric_labels = std::collections::HashMap::new();
                                metric_labels.insert("__name__".to_string(), metric.clone());
                                // Add all labels from the series
                                for (k, v) in &series_labels.inner {
                                    metric_labels.insert(k.clone(), v.clone());
                                }
                                
                                results.push(MatrixResult {
                                    metric: metric_labels,
                                    values,
                                });
                            }
                        }
                    }
                    
                    if !results.is_empty() {
                        return Ok(QueryResult::Matrix { result: results });
                    }
                }
                
                // Try gauges
                if let Some(collection) = tsdb.gauges(&metric, ()) {
                    // Convert gauge collection to untyped
                    let untyped_collection = collection.untyped();
                    
                    // Filter and return individual series
                    let mut results = Vec::new();
                    for (series_labels, series) in untyped_collection.iter() {
                        if matches_filters(&series_labels) {
                            let mut values = Vec::new();
                            for (timestamp, value) in series.inner.iter() {
                                let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                                values.push((timestamp_secs, value.to_string()));
                            }
                            
                            if !values.is_empty() {
                                let mut metric_labels = std::collections::HashMap::new();
                                metric_labels.insert("__name__".to_string(), metric.clone());
                                // Add all labels from the series
                                for (k, v) in &series_labels.inner {
                                    metric_labels.insert(k.clone(), v.clone());
                                }
                                
                                results.push(MatrixResult {
                                    metric: metric_labels,
                                    values,
                                });
                            }
                        }
                    }
                    
                    if !results.is_empty() {
                        return Ok(QueryResult::Matrix { result: results });
                    }
                }
            } else {
                // No special operators - use normal filtering
                let label_refs: Vec<(&str, &str)> = labels.iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                
                // Special handling for cgroup metrics - return individual series
                if metric.starts_with("cgroup_") && labels.is_empty() {
                    // Get all cgroup series without aggregation
                    if let Some(collection) = tsdb.counters(&metric, ()) {
                        let rate_collection = collection.rate();
                        
                        let mut results = Vec::new();
                        for (series_labels, series) in rate_collection.iter() {
                            let mut values = Vec::new();
                            for (timestamp, value) in series.inner.iter() {
                                let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                                values.push((timestamp_secs, value.to_string()));
                            }
                            
                            if !values.is_empty() {
                                let mut metric_labels = std::collections::HashMap::new();
                                metric_labels.insert("__name__".to_string(), metric.clone());
                                // Add all labels from the series
                                for (k, v) in &series_labels.inner {
                                    metric_labels.insert(k.clone(), v.clone());
                                }
                                
                                results.push(MatrixResult {
                                    metric: metric_labels,
                                    values,
                                });
                            }
                        }
                        
                        if !results.is_empty() {
                            return Ok(QueryResult::Matrix { result: results });
                        }
                    }
                }
                
                // Try counters first (most metrics are counters)
                if let Some(collection) = tsdb.counters(&metric, label_refs.as_slice()) {
                    // Return raw counter values, not rate
                    let untyped_collection = collection.untyped();
                    
                    // If there are specific label filters, return the matching series
                    // Otherwise, return all individual series
                    let mut results = Vec::new();
                    for (series_labels, series) in untyped_collection.iter() {
                        let mut values = Vec::new();
                        for (timestamp, value) in series.inner.iter() {
                            let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                            values.push((timestamp_secs, value.to_string()));
                        }
                        
                        if !values.is_empty() {
                            let mut metric_labels = std::collections::HashMap::new();
                            metric_labels.insert("__name__".to_string(), metric.clone());
                            // Add all labels from the series
                            for (k, v) in &series_labels.inner {
                                metric_labels.insert(k.clone(), v.clone());
                            }
                            
                            results.push(MatrixResult {
                                metric: metric_labels,
                                values,
                            });
                        }
                    }
                    
                    if !results.is_empty() {
                        return Ok(QueryResult::Matrix { result: results });
                    }
                }
                
                // Try gauges for instantaneous values
                if let Some(collection) = tsdb.gauges(&metric, label_refs.as_slice()) {
                    // Return raw gauge values, not summed
                    let untyped_collection = collection.untyped();
                    
                    let mut results = Vec::new();
                    for (series_labels, series) in untyped_collection.iter() {
                        let mut values = Vec::new();
                        for (timestamp, value) in series.inner.iter() {
                            let timestamp_secs = (*timestamp as f64 / 1_000_000_000.0) as i64;
                            values.push((timestamp_secs, value.to_string()));
                        }
                        
                        if !values.is_empty() {
                            let mut metric_labels = std::collections::HashMap::new();
                            metric_labels.insert("__name__".to_string(), metric.clone());
                            // Add all labels from the series
                            for (k, v) in &series_labels.inner {
                                metric_labels.insert(k.clone(), v.clone());
                            }
                            
                            results.push(MatrixResult {
                                metric: metric_labels,
                                values,
                            });
                        }
                    }
                    
                    if !results.is_empty() {
                        return Ok(QueryResult::Matrix { result: results });
                    }
                }
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
    
    // Remove time range like [1m] if present, but preserve labels
    let query = if let Some(bracket_pos) = query.find('[') {
        if let Some(close_bracket) = query[bracket_pos..].find(']') {
            // Remove just the time range part [1m], keeping everything else
            let before_bracket = &query[..bracket_pos];
            let after_bracket = &query[bracket_pos + close_bracket + 1..];
            let mut result = String::from(before_bracket);
            result.push_str(after_bracket);
            result
        } else {
            query.to_string()
        }
    } else {
        query.to_string()
    };
    
    // Split metric name and labels
    if let Some(brace_pos) = query.find('{') {
        let metric = query[..brace_pos].to_string();
        let labels_str = &query[brace_pos+1..query.len()-1];
        
        // Parse labels - handle =, !=, =~, !~ operators
        let mut labels = Vec::new();
        
        for pair in labels_str.split(',') {
            let pair = pair.trim();
            
            // Check for regex match operator =~
            if let Some(pos) = pair.find("=~") {
                let key = pair[..pos].trim().to_string();
                let value = pair[pos+2..].trim().trim_matches('"').to_string();
                // Mark this as a regex match with a special prefix
                labels.push((format!("__regex__{}", key), value));
            }
            // Check for not-equal operator !=
            else if let Some(pos) = pair.find("!=") {
                let key = pair[..pos].trim().to_string();
                let value = pair[pos+2..].trim().trim_matches('"').to_string();
                // Mark this as a not-equal match
                labels.push((format!("__ne__{}", key), value));
            }
            // Check for not-regex operator !~
            else if let Some(pos) = pair.find("!~") {
                let key = pair[..pos].trim().to_string();
                let value = pair[pos+2..].trim().trim_matches('"').to_string();
                // Mark this as a not-regex match
                labels.push((format!("__nregex__{}", key), value));
            }
            // Normal equality operator =
            else if let Some(pos) = pair.find('=') {
                let key = pair[..pos].trim().to_string();
                let value = pair[pos+1..].trim().trim_matches('"').to_string();
                labels.push((key, value));
            }
        }
        
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
    // For cgroup metrics, extract actual cgroup names (values of the "name" label)
    if metric.starts_with("cgroup_") {
        let mut cgroup_names = std::collections::HashSet::new();
        
        // Try to get the collection and extract the "name" label values
        if let Some(collection) = tsdb.counters(metric, ()) {
            for labels in collection.labels() {
                if let Some(name) = labels.inner.get("name") {
                    cgroup_names.insert(name.clone());
                }
            }
        }
        
        // Convert to sorted vector
        let mut names: Vec<String> = cgroup_names.into_iter().collect();
        names.sort();
        return names;
    }
    
    // For other metrics, return common labels based on metric
    match metric {
        "cpu_usage" => vec!["state".to_string(), "cpu".to_string()],
        "network_bytes" | "network_packets" => vec!["direction".to_string()],
        "blockio_bytes" | "blockio_operations" | "blockio_latency" => vec!["op".to_string()],
        "syscall" | "syscall_latency" => vec!["op".to_string()],
        _ => vec![],
    }
}

/// Metadata response
#[derive(Debug, Serialize)]
struct MetadataResponse {
    source: String,
    version: String,
    filename: String,
}

/// Get metadata about the data source
async fn get_metadata(
    State(state): State<Arc<QueryState>>,
) -> Json<ApiResponse<MetadataResponse>> {
    // Access metadata from the TSDB
    let metadata = MetadataResponse {
        source: state.tsdb.source(),
        version: state.tsdb.version(),
        filename: state.tsdb.filename(),
    };
    
    Json(ApiResponse::success(metadata))
}

/// Detailed metric information for AI context
#[derive(Debug, Serialize)]
struct DetailedMetric {
    name: String,
    description: String,
    unit: String,
    labels: Vec<String>,
    example_query: String,
}

/// List all metrics with detailed descriptions
async fn list_metrics_detailed(
    State(_state): State<Arc<QueryState>>,
) -> Json<ApiResponse<Vec<DetailedMetric>>> {
    let metrics = vec![
        DetailedMetric {
            name: "cpu_usage".to_string(),
            description: "CPU usage in nanoseconds per core. Use with irate() to get utilization.".to_string(),
            unit: "nanoseconds".to_string(),
            labels: vec!["id".to_string(), "state".to_string()],
            example_query: "sum by (id) (irate(cpu_usage[1m])) / 1e9".to_string(),
        },
        DetailedMetric {
            name: "cpu_instructions".to_string(),
            description: "CPU instructions executed per core".to_string(),
            unit: "count".to_string(),
            labels: vec!["id".to_string()],
            example_query: "irate(cpu_instructions[1m])".to_string(),
        },
        DetailedMetric {
            name: "cpu_cycles".to_string(),
            description: "CPU cycles per core".to_string(),
            unit: "count".to_string(),
            labels: vec!["id".to_string()],
            example_query: "irate(cpu_cycles[1m])".to_string(),
        },
        DetailedMetric {
            name: "memory_cached".to_string(),
            description: "Memory used for cache".to_string(),
            unit: "bytes".to_string(),
            labels: vec![],
            example_query: "memory_cached".to_string(),
        },
        DetailedMetric {
            name: "memory_buffers".to_string(),
            description: "Memory used for buffers".to_string(),
            unit: "bytes".to_string(),
            labels: vec![],
            example_query: "memory_buffers".to_string(),
        },
        DetailedMetric {
            name: "network_bytes".to_string(),
            description: "Network bytes transmitted/received".to_string(),
            unit: "bytes".to_string(),
            labels: vec!["direction".to_string()],
            example_query: "irate(network_bytes{direction=\"receive\"}[1m]) * 8".to_string(),
        },
        DetailedMetric {
            name: "network_packets".to_string(),
            description: "Network packets transmitted/received".to_string(),
            unit: "packets".to_string(),
            labels: vec!["direction".to_string()],
            example_query: "irate(network_packets{direction=\"receive\"}[1m])".to_string(),
        },
        DetailedMetric {
            name: "tcp_bytes".to_string(),
            description: "TCP bytes transmitted/received".to_string(),
            unit: "bytes".to_string(),
            labels: vec!["direction".to_string()],
            example_query: "irate(tcp_bytes{direction=\"receive\"}[1m]) * 8".to_string(),
        },
        DetailedMetric {
            name: "tcp_packets".to_string(),
            description: "TCP packets transmitted/received".to_string(),
            unit: "packets".to_string(),
            labels: vec!["direction".to_string()],
            example_query: "irate(tcp_packets[1m])".to_string(),
        },
        DetailedMetric {
            name: "tcp_retransmit".to_string(),
            description: "TCP retransmissions".to_string(),
            unit: "count".to_string(),
            labels: vec![],
            example_query: "irate(tcp_retransmit[1m])".to_string(),
        },
        DetailedMetric {
            name: "tcp_packet_latency".to_string(),
            description: "TCP packet latency histogram".to_string(),
            unit: "nanoseconds".to_string(),
            labels: vec![],
            example_query: "histogram_quantile(0.99, tcp_packet_latency)".to_string(),
        },
        DetailedMetric {
            name: "blockio_operations".to_string(),
            description: "Block I/O operations".to_string(),
            unit: "operations".to_string(),
            labels: vec!["op".to_string()],
            example_query: "irate(blockio_operations{op=\"read\"}[1m])".to_string(),
        },
        DetailedMetric {
            name: "scheduler_context_switch".to_string(),
            description: "Context switches per CPU".to_string(),
            unit: "count".to_string(),
            labels: vec!["id".to_string()],
            example_query: "sum(irate(scheduler_context_switch[1m]))".to_string(),
        },
        DetailedMetric {
            name: "syscall".to_string(),
            description: "System calls".to_string(),
            unit: "count".to_string(),
            labels: vec![],
            example_query: "sum(irate(syscall[1m]))".to_string(),
        },
        DetailedMetric {
            name: "softirq".to_string(),
            description: "Software interrupts".to_string(),
            unit: "count".to_string(),
            labels: vec![],
            example_query: "sum(irate(softirq[1m]))".to_string(),
        },
        DetailedMetric {
            name: "cgroup_cpu_usage".to_string(),
            description: "CPU usage per cgroup in nanoseconds".to_string(),
            unit: "nanoseconds".to_string(),
            labels: vec!["name".to_string()],
            example_query: "sum by (name) (irate(cgroup_cpu_usage[1m])) / 1e9".to_string(),
        },
        DetailedMetric {
            name: "cgroup_cpu_instructions".to_string(),
            description: "Instructions executed per cgroup".to_string(),
            unit: "count".to_string(),
            labels: vec!["name".to_string()],
            example_query: "sum by (name) (irate(cgroup_cpu_instructions[1m]))".to_string(),
        },
        DetailedMetric {
            name: "cgroup_cpu_cycles".to_string(),
            description: "CPU cycles per cgroup".to_string(),
            unit: "count".to_string(),
            labels: vec!["name".to_string()],
            example_query: "sum by (name) (irate(cgroup_cpu_cycles[1m]))".to_string(),
        },
    ];
    
    Json(ApiResponse::success(metrics))
}

/// AI dashboard generation request
#[derive(Debug, Deserialize)]
struct AIDashboardRequest {
    prompt: String,
}

/// AI dashboard response
#[derive(Debug, Serialize, Deserialize)]
struct AIDashboardResponse {
    panels: Vec<PromQLPanel>,
}

/// Extract metrics information from TSDB
fn extract_metrics_from_tsdb(tsdb: &Tsdb) -> String {
    use crate::viewer::metric_descriptions::get_metric_description;
    
    let mut counters = Vec::new();
    let mut gauges = Vec::new();
    let mut histograms = Vec::new();
    
    // Get all counter metrics
    for name in tsdb.counter_names() {
        let labels = tsdb.get_metric_labels(name);
        // Try to get description from TSDB first, then fallback to hardcoded descriptions
        let description = tsdb.get_metric_description(name)
            .or_else(|| get_metric_description(name))
            .unwrap_or("");
        
        // Generate proper examples with actual label values
        let (label_desc, example) = if name == "cpu_usage" {
            let desc = if !labels.is_empty() {
                format!(" (labels: {})", labels.join(", "))
            } else {
                String::new()
            };
            (desc, format!("sum by (id) (irate({}[1m])) / 1e9 - USE type:\"heatmap\" for per-CPU view!", name))
        } else if name == "network_bytes" {
            (" (labels: direction=\"receive\"|\"transmit\")".to_string(),
             format!("irate({}{{direction=\"receive\"}}[1m]) * 8", name))
        } else if name == "tcp_bytes" {
            (" (labels: direction=\"receive\"|\"transmit\")".to_string(),
             format!("irate({}{{direction=\"receive\"}}[1m]) * 8", name))
        } else if name == "blockio_operations" {
            (" (labels: op=\"read\"|\"write\")".to_string(),
             format!("irate({}{{op=\"read\"}}[1m])", name))
        } else if name == "syscall" && !labels.is_empty() {
            (" (labels: op=\"read\"|\"write\"|...)".to_string(),
             format!("irate({}{{op=\"read\"}}[1m])", name))
        } else if !labels.is_empty() {
            (format!(" (labels: {})", labels.join(", ")),
             format!("irate({}[1m])", name))
        } else {
            (String::new(), format!("irate({}[1m])", name))
        };
        
        let desc_text = if !description.is_empty() {
            format!(". {}", description)
        } else {
            String::new()
        };
        counters.push(format!("- {}{}: Counter{}. Example: {}", name, label_desc, desc_text, example));
    }
    
    // Get all gauge metrics  
    for name in tsdb.gauge_names() {
        let labels = tsdb.get_metric_labels(name);
        // Try to get description from TSDB first, then fallback to hardcoded descriptions
        let description = tsdb.get_metric_description(name)
            .or_else(|| get_metric_description(name))
            .unwrap_or("");
        let label_desc = if !labels.is_empty() {
            format!(" (labels: {})", labels.join(", "))
        } else {
            String::new()
        };
        let desc_text = if !description.is_empty() {
            format!(". {}", description)
        } else {
            String::new()
        };
        gauges.push(format!("- {}{}: Gauge{}. Use directly: {}", name, label_desc, desc_text, name));
    }
    
    // Get all histogram metrics
    for name in tsdb.histogram_names() {
        let labels = tsdb.get_metric_labels(name);
        // Try to get description from TSDB first, then fallback to hardcoded descriptions
        let description = tsdb.get_metric_description(name)
            .or_else(|| get_metric_description(name))
            .unwrap_or("");
        let label_desc = if !labels.is_empty() {
            format!(" (labels: {})", labels.join(", "))
        } else {
            String::new()
        };
        let desc_text = if !description.is_empty() {
            format!(". {}", description)
        } else {
            String::new()
        };
        histograms.push(format!("- {}{}: Histogram{}. Example: histogram_quantile(0.99, {})", 
                               name, label_desc, desc_text, name));
    }
    
    let mut result = String::new();
    
    if !counters.is_empty() {
        result.push_str("COUNTERS (monotonically increasing values - MUST use irate() or rate()):\n");
        result.push_str(&counters.join("\n"));
        result.push_str("\n\n");
    }
    
    if !gauges.is_empty() {
        result.push_str("GAUGES (instantaneous values - use directly WITHOUT rate functions):\n");
        result.push_str(&gauges.join("\n"));
        result.push_str("\n\n");
    }
    
    if !histograms.is_empty() {
        result.push_str("HISTOGRAMS (distributions - use histogram_quantile()):\n");
        result.push_str(&histograms.join("\n"));
        result.push_str("\n\n");
    }
    
    result
}

/// Generate AI dashboard based on user prompt
async fn generate_ai_dashboard(
    State(state): State<Arc<QueryState>>,
    Json(request): Json<AIDashboardRequest>,
) -> Json<ApiResponse<AIDashboardResponse>> {
    // For now, create a mock implementation that we'll replace with actual LLM call
    // This will call the local llama-server on port 8080
    
    // Extract actual metrics from TSDB
    let metrics_info = extract_metrics_from_tsdb(&state.tsdb);
    
    // Get chart templates
    let chart_templates = crate::viewer::chart_templates::format_templates_for_prompt();
    
    // Build the system prompt with metric information
    let system_prompt = format!(r#"You are an expert in system performance analysis. Generate a dashboard of charts based on the user's request.

CRITICAL: These are SYSTEM-WIDE metrics that monitor the ENTIRE system, not individual applications.
- Metrics track ALL processes together - there is NO way to filter by application
- DO NOT add {{metric_type="redis"}}, {{app="nginx"}}, or ANY application-specific labels
- The ONLY valid labels are the ones explicitly listed for each metric below
- If user asks for "redis metrics", show general system metrics that would be relevant for monitoring ANY application

CORRECT examples:
✓ irate(cpu_usage[1m])
✓ irate(network_bytes{{direction="receive"}}[1m])  
✓ memory_cached

INCORRECT examples (NEVER do this):
✗ irate(cpu_usage{{metric_type="redis"}}[1m])
✗ network_bytes{{app="nginx"}}
✗ memory_cached{{service="postgresql"}}

AVAILABLE METRICS:

{}

You must return valid JSON with this exact structure:
{{
  "panels": [
    {{
      "title": "Chart Title",
      "id": "unique-id",
      "type": "line",
      "queries": [
        {{
          "expr": "PromQL expression",
          "legend": "Series name"
        }}
      ],
      "unit": "percentage"
    }}
  ]
}}

Valid unit values (lowercase): percentage, count, rate, bytes, bitrate, datarate, time, frequency
Valid type values (lowercase): line, heatmap, scatter, multi

IMPORTANT - PERCENTAGE HANDLING:
- When unit is "percentage", express values as ratios from 0.0 to 1.0
- DO NOT multiply by 100 to convert to percent - the UI handles this automatically
- CORRECT: (memory_total - memory_free) / memory_total  → shows as 0-100% in UI
- INCORRECT: (memory_total - memory_free) / memory_total * 100  → would show as 0-10000% in UI
- Example for memory usage percentage: (memory_total - memory_free) / memory_total with unit:"percentage"

CHOOSING THE RIGHT CHART TYPE:
- line: Use for trends over time with few series (< 10 lines)
- heatmap: BEST for per-CPU or per-core metrics when you have many cores (shows all CPUs as a color gradient)
- scatter: Use for percentile distributions (P50, P90, P99) of a SINGLE metric
- multi: RESERVED for special cases - avoid mixing different metrics

ABSOLUTE REQUIREMENTS - YOU MUST FOLLOW THESE:
1. NEVER add application-specific labels (NO {{metric_type="redis"}}, {{app="..."}} etc.)
2. For each metric, ONLY use the labels shown in parentheses above (if any)
3. Most metrics have NO labels - use them without any labels
4. network_bytes ONLY accepts {{direction="receive"}} or {{direction="transmit"}}
5. blockio_operations ONLY accepts {{op="read"}} or {{op="write"}}
6. cpu_usage has NO labels - use it as: irate(cpu_usage[1m])
7. memory metrics have NO labels - use them as: memory_cached, memory_free, etc.

QUERY RULES:
- COUNTERS: Must use irate() or rate() - e.g., irate(network_bytes{{direction="receive"}}[1m])
- GAUGES: Use directly WITHOUT rate() - e.g., memory_cached
- For total CPU: avg(sum by (id) (irate(cpu_usage[1m]))) / 1e9 with type:"line" and unit:"percentage" (division MUST be outside avg!)
- For per-CPU breakdown: sum by (id) (irate(cpu_usage[1m])) / 1e9 with type:"heatmap" and unit:"percentage" (MUCH better than 128 lines!)
- For network bitrate: irate(network_bytes{{direction="receive"}}[1m]) * 8
- IMPORTANT: Division and multiplication MUST be at the outermost level, not inside aggregations
- Create 4-8 relevant charts based on the user's query

CHART SEPARATION RULES:
- ALWAYS use separate charts for different metrics (one metric type per chart)
- GOOD: Multiple percentiles of the SAME metric on one chart (P50, P90, P99 of tcp_latency)
- GOOD: Multiple series of the SAME metric with different labels (receive vs transmit for network_bytes)
- BAD: Mixing different metric types (DON'T put cpu_usage and memory_free on same chart)
- BAD: Combining unrelated metrics just to reduce chart count
- Exception: CPU efficiency metrics (IPC/IPNS) can be shown together as they're directly related

HEATMAP EXAMPLE (use for many series like per-CPU metrics):
{{
  "title": "Per-CPU Usage Heatmap",
  "id": "cpu-heatmap",
  "type": "heatmap",
  "queries": [
    {{
      "expr": "sum by (id) (irate(cpu_usage[1m])) / 1e9",
      "legend": "CPU Cores"
    }}
  ],
  "unit": "percentage"
}}

CORRECT query patterns:
✓ avg(sum by (id) (irate(cpu_usage[1m]))) / 1e9  -- Division outside aggregation
✓ sum by (id) (irate(cpu_usage[1m])) / 1e9      -- Division at the end
✓ irate(network_bytes{{direction="receive"}}[1m]) * 8  -- Multiplication at the end
✓ (memory_total - memory_free) / memory_total    -- Memory usage as ratio (0.0-1.0) for percentage unit

INCORRECT patterns (will fail):
✗ avg(sum by (id) (irate(cpu_usage[1m])) / 1e9)  -- Division inside avg() fails
✗ sum(irate(network_bytes[1m]) * 8)               -- Multiplication inside sum() fails
✗ (memory_total - memory_free) / memory_total * 100  -- Don't multiply by 100 for percentages!

SMART DASHBOARD TIPS:
- When showing CPU metrics, include BOTH:
  1. A line chart with average CPU (for overall trend)
  2. A heatmap with per-CPU breakdown (to spot hot cores or imbalances)
- For systems with many cores (>8), ALWAYS prefer heatmap over multiple lines
- Heatmaps make it easy to spot: hot cores, NUMA imbalances, scheduling issues

REMEMBER: These metrics monitor the ENTIRE SYSTEM. When user asks for "redis" or any app metrics,
you show SYSTEM metrics that would be useful for monitoring system performance while that app runs.

{}
"#, metrics_info, chart_templates);

    // Print the complete prompt for debugging
    eprintln!("=== LLM SYSTEM PROMPT ===");
    eprintln!("{}", system_prompt);
    eprintln!("=== USER PROMPT ===");
    eprintln!("Create a dashboard to help with: {}", request.prompt);
    eprintln!("=== END PROMPTS ===");
    
    // Call the local LLM server
    let client = reqwest::Client::new();
    
    let llm_request = serde_json::json!({
        "model": "qwen",
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user", 
                "content": format!("Create a dashboard to help with: {}", request.prompt)
            }
        ],
        "temperature": 0.7,
        "response_format": { "type": "json_object" }
    });
    
    match client
        .post("http://localhost:8080/v1/chat/completions")
        .json(&llm_request)
        .send()
        .await
    {
        Ok(response) => {
            if let Ok(llm_response) = response.json::<serde_json::Value>().await {
                // Extract the content from the LLM response
                if let Some(content) = llm_response
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                {
                    eprintln!("LLM response content: {}", content);
                    // Parse the JSON response
                    match serde_json::from_str::<AIDashboardResponse>(content) {
                        Ok(panels_response) => {
                            eprintln!("Successfully parsed {} panels from LLM", panels_response.panels.len());
                            return Json(ApiResponse::success(panels_response));
                        }
                        Err(e) => {
                            eprintln!("Failed to parse LLM response: {}", e);
                        }
                    }
                } else {
                    eprintln!("Failed to extract content from LLM response: {:?}", llm_response);
                }
            } else {
                eprintln!("Failed to parse LLM response as JSON");
            }
        }
        Err(e) => {
            eprintln!("Error calling LLM: {}", e);
        }
    }
    
    // Fallback response if LLM fails
    let fallback_panels = vec![
        PromQLPanel {
            title: "CPU Usage".to_string(),
            id: "ai-cpu".to_string(),
            panel_type: PanelType::Line,
            queries: vec![
                PromQLQueryDef {
                    expr: "avg(sum by (id) (irate(cpu_usage[1m]))) / 1e9".to_string(),
                    legend: Some("CPU %".to_string()),
                    interval: None,
                },
            ],
            unit: Unit::Percentage,
            options: None,
        },
        PromQLPanel {
            title: "Network Traffic".to_string(),
            id: "ai-network".to_string(),
            panel_type: PanelType::Line,
            queries: vec![
                PromQLQueryDef {
                    expr: "irate(network_bytes{direction=\"receive\"}[1m]) * 8".to_string(),
                    legend: Some("Receive".to_string()),
                    interval: None,
                },
                PromQLQueryDef {
                    expr: "irate(network_bytes{direction=\"transmit\"}[1m]) * 8".to_string(),
                    legend: Some("Transmit".to_string()),
                    interval: None,
                },
            ],
            unit: Unit::Bitrate,
            options: None,
        },
    ];
    
    Json(ApiResponse::success(AIDashboardResponse {
        panels: fallback_panels,
    }))
}