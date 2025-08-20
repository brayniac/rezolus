use super::Config;
use ringlog::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::viewer::promql::{QueryEngine, QueryResult};
use crate::viewer::tsdb::Tsdb;

/// MCP server state
pub struct MCPServer {
    config: Config,
    tsdb_cache: Arc<RwLock<HashMap<String, Arc<Tsdb>>>>,
    query_engine_cache: Arc<RwLock<HashMap<String, Arc<QueryEngine>>>>,
}

impl MCPServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            tsdb_cache: Arc::new(RwLock::new(HashMap::new())),
            query_engine_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Run the MCP server using stdio
    pub async fn run_stdio(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        eprintln!("MCP DEBUG: MCP server ready, waiting for messages...");
        info!("MCP server ready, waiting for messages...");
        loop {
            debug!("Waiting for next line...");
            let line = match lines.next_line().await? {
                Some(line) => {
                    if line.trim().is_empty() {
                        debug!("Received empty line, continuing");
                        continue;
                    }
                    debug!("Received message: {}", line);
                    line
                }
                None => {
                    eprintln!("MCP DEBUG: stdin closed, no more messages");
                    info!("stdin closed, no more messages");
                    break;
                }
            };

            // Try to parse as JSON-RPC message
            let message: Value = match serde_json::from_str(&line) {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("MCP DEBUG: Failed to parse JSON: {}", e);
                    warn!("Failed to parse JSON: {}", e);
                    continue;
                }
            };

            // Handle the message and get response
            if let Some(response) = self.handle_message(message).await? {
                let response_str = serde_json::to_string(&response)?;
                eprintln!("MCP DEBUG: Sending response: {}", response_str);
                debug!("Sending response: {}", response_str);
                stdout.write_all(response_str.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        eprintln!("MCP DEBUG: MCP server shutting down");
        info!("MCP server shutting down");
        Ok(())
    }

    /// Handle a JSON-RPC message
    async fn handle_message(
        &mut self,
        message: Value,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let method = message.get("method").and_then(|m| m.as_str());
        let id = message.get("id").cloned();
        let params = message.get("params");

        match method {
            Some("initialize") => {
                eprintln!("MCP DEBUG: Received initialize request");
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2025-06-18",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "rezolus-mcp",
                            "version": "1.0.0"
                        }
                    }
                })))
            }
            Some("tools/list") => {
                eprintln!("MCP DEBUG: Received tools/list request");
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            {
                                "name": "list_metrics",
                                "description": "List all available metrics in the parquet file",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "parquet_file": {
                                            "type": "string",
                                            "description": "Path to the parquet file"
                                        },
                                        "pattern": {
                                            "type": "string",
                                            "description": "Optional regex pattern to filter metrics"
                                        }
                                    },
                                    "required": ["parquet_file"]
                                }
                            },
                            {
                                "name": "query_metrics",
                                "description": "Execute a PromQL query on the metrics data",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "parquet_file": {
                                            "type": "string",
                                            "description": "Path to the parquet file"
                                        },
                                        "query": {
                                            "type": "string",
                                            "description": "PromQL query to execute (e.g., 'irate(cpu_cycles[5m])')"
                                        },
                                        "start_time": {
                                            "type": "number",
                                            "description": "Optional start time in seconds (Unix timestamp)"
                                        },
                                        "end_time": {
                                            "type": "number",
                                            "description": "Optional end time in seconds (Unix timestamp)"
                                        },
                                        "step": {
                                            "type": "number",
                                            "description": "Optional step size in seconds for range queries (default: 60)"
                                        }
                                    },
                                    "required": ["parquet_file", "query"]
                                }
                            },
                            {
                                "name": "analyze_correlation",
                                "description": "Analyze correlation between two metrics using PromQL",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "parquet_file": {
                                            "type": "string",
                                            "description": "Path to the parquet file"
                                        },
                                        "metric1": {
                                            "type": "string",
                                            "description": "First metric PromQL expression"
                                        },
                                        "metric2": {
                                            "type": "string",
                                            "description": "Second metric PromQL expression"
                                        }
                                    },
                                    "required": ["parquet_file", "metric1", "metric2"]
                                }
                            },
                            {
                                "name": "analyze_fft_patterns",
                                "description": "Analyze periodic patterns in metrics using FFT",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "parquet_file": {
                                            "type": "string",
                                            "description": "Path to the parquet file"
                                        },
                                        "metric": {
                                            "type": "string",
                                            "description": "PromQL expression for the metric to analyze (supports labels, e.g., 'cpu_usage{cpu=\"0\"}' or 'cgroup_cpu_usage{name=\"web\"}')"
                                        },
                                        "start_time": {
                                            "type": "number",
                                            "description": "Optional start time in seconds (Unix timestamp)"
                                        },
                                        "end_time": {
                                            "type": "number",
                                            "description": "Optional end time in seconds (Unix timestamp)"
                                        },
                                        "step": {
                                            "type": "number",
                                            "description": "Optional step size in seconds (default: 60)"
                                        }
                                    },
                                    "required": ["parquet_file", "metric"]
                                }
                            },
                            {
                                "name": "detect_anomalies",
                                "description": "Detect anomalies in a metric using statistical methods",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "parquet_file": {
                                            "type": "string",
                                            "description": "Path to the parquet file"
                                        },
                                        "metric": {
                                            "type": "string",
                                            "description": "PromQL expression for the metric to analyze"
                                        },
                                        "method": {
                                            "type": "string",
                                            "enum": ["zscore", "iqr", "mad"],
                                            "description": "Anomaly detection method (default: zscore)"
                                        },
                                        "threshold": {
                                            "type": "number",
                                            "description": "Threshold for anomaly detection (default: 3 for zscore, 1.5 for iqr)"
                                        }
                                    },
                                    "required": ["parquet_file", "metric"]
                                }
                            },
                            {
                                "name": "system_health",
                                "description": "Get high-level system health overview - START HERE for analysis",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "parquet_file": {
                                            "type": "string",
                                            "description": "Path to the parquet file"
                                        }
                                    },
                                    "required": ["parquet_file"]
                                }
                            },
                            {
                                "name": "drill_down",
                                "description": "Drill down into specific subsystem issues identified by system_health",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "parquet_file": {
                                            "type": "string",
                                            "description": "Path to the parquet file"
                                        },
                                        "subsystem": {
                                            "type": "string",
                                            "enum": ["cpu", "memory", "network", "io", "container"],
                                            "description": "Subsystem to analyze (based on system_health findings)"
                                        },
                                        "filter": {
                                            "type": "string",
                                            "description": "Optional filter (e.g., container name for container analysis)"
                                        },
                                        "detailed": {
                                            "type": "boolean",
                                            "description": "Include detailed analysis (default: false)"
                                        }
                                    },
                                    "required": ["parquet_file", "subsystem"]
                                }
                            }
                        ]
                    }
                })))
            }
            Some("tools/call") => {
                eprintln!("MCP DEBUG: Received tools/call request");
                if let Some(params) = params {
                    self.handle_tool_call(id, params).await
                } else {
                    Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32602,
                            "message": "Invalid params"
                        }
                    })))
                }
            }
            Some("resources/list") => {
                eprintln!("MCP DEBUG: Received resources/list request");
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "resources": []
                    }
                })))
            }
            Some("resources/read") => {
                eprintln!("MCP DEBUG: Received resources/read request");
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": "Resources not implemented"
                    }
                })))
            }
            Some("prompts/list") => {
                eprintln!("MCP DEBUG: Received prompts/list request");
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "prompts": []
                    }
                })))
            }
            Some("notifications/initialized") => {
                eprintln!("MCP DEBUG: Received notifications/initialized (no response needed)");
                Ok(None) // Notifications don't get responses
            }
            _ => {
                eprintln!("MCP DEBUG: Unknown method: {:?}", method);
                // Only send error response if this is a request (has id), not a notification
                if id.is_some() {
                    Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": "Method not found"
                        }
                    })))
                } else {
                    Ok(None) // Don't respond to unknown notifications
                }
            }
        }
    }

    /// Handle a tool call
    async fn handle_tool_call(
        &mut self,
        id: Option<Value>,
        params: &Value,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let tool_name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or("Missing tool name")?;

        let arguments = params.get("arguments").ok_or("Missing arguments")?;

        match tool_name {
            "list_metrics" => {
                match self.list_metrics(arguments).await {
                    Ok(result) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }
                    }))),
                    Err(e) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("List metrics error: {}", e)
                        }
                    })))
                }
            }
            "query_metrics" => {
                match self.query_metrics(arguments).await {
                    Ok(result) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }
                    }))),
                    Err(e) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("Query error: {}", e)
                        }
                    })))
                }
            }
            "analyze_correlation" => {
                match self.analyze_correlation(arguments).await {
                    Ok(result) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }
                    }))),
                    Err(e) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("Correlation error: {}", e)
                        }
                    })))
                }
            }
            "detect_anomalies" => {
                match self.detect_anomalies(arguments).await {
                    Ok(result) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }
                    }))),
                    Err(e) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("Anomaly detection error: {}", e)
                        }
                    })))
                }
            }
            "analyze_fft_patterns" => {
                match self.analyze_fft_patterns(arguments).await {
                    Ok(result) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }
                    }))),
                    Err(e) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("FFT analysis error: {}", e)
                        }
                    })))
                }
            }
            "system_health" => {
                match self.analyze_system_health(arguments).await {
                    Ok(result) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }
                    }))),
                    Err(e) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("System health analysis error: {}", e)
                        }
                    })))
                }
            }
            "drill_down" => {
                match self.drill_down_analysis(arguments).await {
                    Ok(result) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result
                                }
                            ]
                        }
                    }))),
                    Err(e) => Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("Drill down analysis error: {}", e)
                        }
                    })))
                }
            }
            _ => Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Unknown tool: {}", tool_name)
                }
            }))),
        }
    }

    /// Load or get cached TSDB and QueryEngine
    async fn get_query_engine(
        &self,
        parquet_file: &str,
    ) -> Result<Arc<QueryEngine>, Box<dyn std::error::Error>> {
        // Check cache first
        {
            let cache = self.query_engine_cache.read().unwrap();
            if let Some(engine) = cache.get(parquet_file) {
                return Ok(Arc::clone(engine));
            }
        }

        // Load TSDB
        let path = Path::new(parquet_file);
        if !path.exists() {
            return Err(format!("Parquet file not found: {}", parquet_file).into());
        }

        let tsdb = Arc::new(Tsdb::load(path)?);
        let engine = Arc::new(QueryEngine::new(Arc::clone(&tsdb)));

        // Cache both
        {
            let mut tsdb_cache = self.tsdb_cache.write().unwrap();
            tsdb_cache.insert(parquet_file.to_string(), tsdb);

            let mut engine_cache = self.query_engine_cache.write().unwrap();
            engine_cache.insert(parquet_file.to_string(), Arc::clone(&engine));
        }

        Ok(engine)
    }

    /// Get cached TSDB
    async fn get_tsdb(
        &self,
        parquet_file: &str,
    ) -> Result<Arc<Tsdb>, Box<dyn std::error::Error>> {
        // Ensure it's loaded
        self.get_query_engine(parquet_file).await?;
        
        // Get from cache
        let cache = self.tsdb_cache.read().unwrap();
        cache.get(parquet_file)
            .map(Arc::clone)
            .ok_or_else(|| "TSDB not in cache".into())
    }

    /// List available metrics
    async fn list_metrics(&self, arguments: &Value) -> Result<String, Box<dyn std::error::Error>> {
        let parquet_file = arguments
            .get("parquet_file")
            .and_then(|f| f.as_str())
            .ok_or("Missing parquet_file")?;

        let pattern = arguments.get("pattern").and_then(|p| p.as_str());

        let tsdb = self.get_tsdb(parquet_file).await?;
        let engine = self.get_query_engine(parquet_file).await?;

        // Get actual metric names from TSDB
        let mut all_metrics = Vec::new();
        
        // Add counter metrics
        for name in tsdb.counter_names() {
            all_metrics.push(format!("{} (counter)", name));
            // Also add as rate metric
            all_metrics.push(format!("irate({}[5m]) (rate)", name));
        }
        
        // Add gauge metrics  
        for name in tsdb.gauge_names() {
            all_metrics.push(format!("{} (gauge)", name));
        }
        
        // Add histogram metrics
        for name in tsdb.histogram_names() {
            all_metrics.push(format!("{} (histogram)", name));
            // Also add percentile queries
            all_metrics.push(format!("histogram_quantile(0.99, {}[5m]) (p99)", name));
        }

        // Filter by pattern if provided
        let mut filtered_metrics = all_metrics;
        if let Some(pat) = pattern {
            filtered_metrics.retain(|m| m.contains(pat));
        }

        // Sort alphabetically
        filtered_metrics.sort();

        let mut output = format!("Found {} metrics", filtered_metrics.len());
        if let Some(pat) = pattern {
            output.push_str(&format!(" matching pattern '{}'", pat));
        }
        output.push_str(":\n\n");

        for metric in filtered_metrics {
            output.push_str(&format!("- {}\n", metric));
        }

        // Get time range
        let (start, end) = engine.get_time_range();
        output.push_str(&format!("\nTime range: {:.0} to {:.0} seconds\n", start, end));
        output.push_str(&format!("Data source: {} {}\n", tsdb.source(), tsdb.version()));

        Ok(output)
    }

    /// Execute a PromQL query
    async fn query_metrics(
        &self,
        arguments: &Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let parquet_file = arguments
            .get("parquet_file")
            .and_then(|f| f.as_str())
            .ok_or("Missing parquet_file")?;

        let query = arguments
            .get("query")
            .and_then(|q| q.as_str())
            .ok_or("Missing query")?;

        let engine = self.get_query_engine(parquet_file).await?;
        let tsdb = self.get_tsdb(parquet_file).await?;

        // Validate metrics exist before querying
        use crate::mcp::metric_helper::MetricHelper;
        let metric_names = MetricHelper::extract_metric_names(query);
        if !metric_names.is_empty() {
            let validation = MetricHelper::validate_metrics(&tsdb, &metric_names);
            let missing: Vec<String> = validation
                .iter()
                .filter(|(_, exists)| !**exists)
                .map(|(name, _)| name.clone())
                .collect();
            
            if !missing.is_empty() {
                let error_msg = MetricHelper::generate_metric_error_message(&tsdb, &missing);
                return Err(error_msg.into());
            }
        }

        // Get time parameters
        let (default_start, default_end) = engine.get_time_range();
        let start_time = arguments
            .get("start_time")
            .and_then(|t| t.as_f64())
            .unwrap_or(default_start);
        let end_time = arguments
            .get("end_time")
            .and_then(|t| t.as_f64())
            .unwrap_or(default_end);
        let step = arguments
            .get("step")
            .and_then(|s| s.as_f64())
            .unwrap_or(60.0);

        // Execute query
        let result = if start_time == end_time {
            // Point query
            engine.query(query, Some(start_time))?
        } else {
            // Range query
            engine.query_range(query, start_time, end_time, step)?
        };

        // Format result
        let output = match result {
            QueryResult::Vector { result } => {
                let mut out = format!("Query: {}\nResult type: Vector\n\n", query);
                for sample in result {
                    out.push_str(&format!(
                        "Timestamp: {:.0}, Value: {:.6}\n",
                        sample.value.0, sample.value.1
                    ));
                    if !sample.metric.is_empty() {
                        out.push_str("Labels: ");
                        for (k, v) in &sample.metric {
                            out.push_str(&format!("{}={} ", k, v));
                        }
                        out.push_str("\n");
                    }
                }
                out
            }
            QueryResult::Matrix { result } => {
                let mut out = format!("Query: {}\nResult type: Matrix\n\n", query);
                for sample in result {
                    if !sample.metric.is_empty() {
                        out.push_str("Labels: ");
                        for (k, v) in &sample.metric {
                            out.push_str(&format!("{}={} ", k, v));
                        }
                        out.push_str("\n");
                    }
                    out.push_str(&format!("Values ({} samples):\n", sample.values.len()));
                    
                    // Show first and last few values
                    let show_count = 5;
                    for (i, (ts, val)) in sample.values.iter().enumerate() {
                        if i < show_count || i >= sample.values.len() - show_count {
                            out.push_str(&format!("  {:.0}: {:.6}\n", ts, val));
                        } else if i == show_count {
                            out.push_str(&format!("  ... {} more values ...\n", sample.values.len() - 2 * show_count));
                        }
                    }
                }
                out
            }
            QueryResult::Scalar { result } => {
                format!(
                    "Query: {}\nResult type: Scalar\nTimestamp: {:.0}, Value: {:.6}\n",
                    query, result.0, result.1
                )
            }
        };

        Ok(output)
    }

    /// Analyze correlation between two metrics
    async fn analyze_correlation(
        &self,
        arguments: &Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let parquet_file = arguments
            .get("parquet_file")
            .and_then(|f| f.as_str())
            .ok_or("Missing parquet_file")?;

        let metric1 = arguments
            .get("metric1")
            .and_then(|m| m.as_str())
            .ok_or("Missing metric1")?;

        let metric2 = arguments
            .get("metric2")
            .and_then(|m| m.as_str())
            .ok_or("Missing metric2")?;

        let engine = self.get_query_engine(parquet_file).await?;

        // Get time range
        let (start, end) = engine.get_time_range();
        let step = 60.0; // 1 minute step

        // Use the shared correlation module
        use crate::mcp::correlation::{calculate_correlation, format_correlation_result};
        
        let result = calculate_correlation(&engine, metric1, metric2, start, end, step)?;
        Ok(format_correlation_result(&result))
    }

    /// Detect anomalies in a metric
    async fn detect_anomalies(
        &self,
        arguments: &Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let parquet_file = arguments
            .get("parquet_file")
            .and_then(|f| f.as_str())
            .ok_or("Missing parquet_file")?;

        let metric = arguments
            .get("metric")
            .and_then(|m| m.as_str())
            .ok_or("Missing metric")?;

        let method = arguments
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("zscore");

        let threshold = arguments
            .get("threshold")
            .and_then(|t| t.as_f64())
            .unwrap_or(if method == "iqr" { 1.5 } else { 3.0 });

        let engine = self.get_query_engine(parquet_file).await?;

        // Get time range
        let (start, end) = engine.get_time_range();
        let step = 60.0; // 1 minute step

        // Query metric
        let result = engine.query_range(metric, start, end, step)?;
        let values = Self::extract_values_with_time(&result)?;

        // Detect anomalies based on method
        let anomalies = match method {
            "zscore" => Self::detect_anomalies_zscore(&values, threshold),
            "iqr" => Self::detect_anomalies_iqr(&values, threshold),
            "mad" => Self::detect_anomalies_mad(&values, threshold),
            _ => return Err(format!("Unknown method: {}", method).into()),
        };

        let mut output = format!(
            "Anomaly Detection\n\
             =================\n\
             Metric: {}\n\
             Method: {} (threshold: {})\n\
             Total samples: {}\n\
             Anomalies found: {}\n\n",
            metric,
            method,
            threshold,
            values.len(),
            anomalies.len()
        );

        if anomalies.is_empty() {
            output.push_str("No anomalies detected.\n");
        } else {
            output.push_str("Anomalies:\n");
            for (i, &idx) in anomalies.iter().enumerate() {
                if i >= 10 {
                    output.push_str(&format!("... and {} more\n", anomalies.len() - 10));
                    break;
                }
                let (timestamp, value) = values[idx];
                output.push_str(&format!(
                    "  Timestamp: {:.0}, Value: {:.6}\n",
                    timestamp, value
                ));
            }
        }

        Ok(output)
    }

    /// Analyze system health - high-level overview
    async fn analyze_system_health(
        &self,
        arguments: &Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let parquet_file = arguments
            .get("parquet_file")
            .and_then(|f| f.as_str())
            .ok_or("Missing parquet_file")?;

        let tsdb = self.get_tsdb(parquet_file).await?;
        
        use crate::mcp::guided_analysis::GuidedAnalyzer;
        let analyzer = GuidedAnalyzer::new(tsdb);
        let report = analyzer.analyze_system_health();
        
        Ok(report.format_for_llm())
    }

    /// Drill down into specific subsystem
    async fn drill_down_analysis(
        &self,
        arguments: &Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let parquet_file = arguments
            .get("parquet_file")
            .and_then(|f| f.as_str())
            .ok_or("Missing parquet_file")?;

        let subsystem = arguments
            .get("subsystem")
            .and_then(|s| s.as_str())
            .ok_or("Missing subsystem")?;

        let filter = arguments
            .get("filter")
            .and_then(|f| f.as_str())
            .map(|s| s.to_string());

        let detailed = arguments
            .get("detailed")
            .and_then(|d| d.as_bool())
            .unwrap_or(false);

        let tsdb = self.get_tsdb(parquet_file).await?;
        let engine = self.get_query_engine(parquet_file).await?;
        
        use crate::mcp::guided_analysis::{GuidedAnalyzer, DrillDownContext};
        let analyzer = GuidedAnalyzer::new(tsdb);
        
        let (start, end) = engine.get_time_range();
        let context = DrillDownContext {
            start_time: start,
            end_time: end,
            filter,
            detailed,
        };
        
        let report = analyzer.drill_down(subsystem, &context);
        Ok(report.format_for_llm())
    }

    /// Analyze FFT patterns in a metric
    async fn analyze_fft_patterns(
        &self,
        arguments: &Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let parquet_file = arguments
            .get("parquet_file")
            .and_then(|f| f.as_str())
            .ok_or("Missing parquet_file")?;

        let metric = arguments
            .get("metric")
            .and_then(|m| m.as_str())
            .ok_or("Missing metric")?;

        let engine = self.get_query_engine(parquet_file).await?;

        // Get time parameters
        let (default_start, default_end) = engine.get_time_range();
        let start_time = arguments
            .get("start_time")
            .and_then(|t| t.as_f64())
            .unwrap_or(default_start);
        let end_time = arguments
            .get("end_time")
            .and_then(|t| t.as_f64())
            .unwrap_or(default_end);
        let step = arguments
            .get("step")
            .and_then(|s| s.as_f64())
            .unwrap_or(60.0);

        // Use the FFT analysis module
        use crate::mcp::fft_analysis::{analyze_fft_patterns, format_fft_result};
        
        let result = analyze_fft_patterns(&engine, metric, None, start_time, end_time, step)?;
        Ok(format_fft_result(&result))
    }

    // Helper functions

    fn extract_values(result: &QueryResult) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        match result {
            QueryResult::Matrix { result } => {
                if result.is_empty() {
                    return Err("No data in result".into());
                }
                Ok(result[0].values.iter().map(|(_, v)| *v).collect())
            }
            QueryResult::Vector { result } => {
                if result.is_empty() {
                    return Err("No data in result".into());
                }
                Ok(vec![result[0].value.1])
            }
            QueryResult::Scalar { result } => Ok(vec![result.1]),
        }
    }

    fn extract_values_with_time(
        result: &QueryResult,
    ) -> Result<Vec<(f64, f64)>, Box<dyn std::error::Error>> {
        match result {
            QueryResult::Matrix { result } => {
                if result.is_empty() {
                    return Err("No data in result".into());
                }
                Ok(result[0].values.clone())
            }
            QueryResult::Vector { result } => {
                if result.is_empty() {
                    return Err("No data in result".into());
                }
                Ok(vec![result[0].value])
            }
            QueryResult::Scalar { result } => Ok(vec![*result]),
        }
    }


    fn detect_anomalies_zscore(values: &[(f64, f64)], threshold: f64) -> Vec<usize> {
        let data: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
        let mean: f64 = data.iter().sum::<f64>() / data.len() as f64;
        let variance: f64 = data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / data.len() as f64;
        let std_dev = variance.sqrt();

        let mut anomalies = Vec::new();
        for (i, &value) in data.iter().enumerate() {
            let z_score = (value - mean).abs() / std_dev;
            if z_score > threshold {
                anomalies.push(i);
            }
        }
        anomalies
    }

    fn detect_anomalies_iqr(values: &[(f64, f64)], threshold: f64) -> Vec<usize> {
        let mut data: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let q1_idx = data.len() / 4;
        let q3_idx = 3 * data.len() / 4;
        let q1 = data[q1_idx];
        let q3 = data[q3_idx];
        let iqr = q3 - q1;

        let lower_bound = q1 - threshold * iqr;
        let upper_bound = q3 + threshold * iqr;

        let mut anomalies = Vec::new();
        for (i, (_, value)) in values.iter().enumerate() {
            if *value < lower_bound || *value > upper_bound {
                anomalies.push(i);
            }
        }
        anomalies
    }

    fn detect_anomalies_mad(values: &[(f64, f64)], threshold: f64) -> Vec<usize> {
        let data: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
        let mut sorted_data = data.clone();
        sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let median = if sorted_data.len() % 2 == 0 {
            (sorted_data[sorted_data.len() / 2 - 1] + sorted_data[sorted_data.len() / 2]) / 2.0
        } else {
            sorted_data[sorted_data.len() / 2]
        };

        let mut deviations: Vec<f64> = data.iter().map(|v| (v - median).abs()).collect();
        deviations.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mad = if deviations.len() % 2 == 0 {
            (deviations[deviations.len() / 2 - 1] + deviations[deviations.len() / 2]) / 2.0
        } else {
            deviations[deviations.len() / 2]
        };

        let mut anomalies = Vec::new();
        for (i, &value) in data.iter().enumerate() {
            let modified_z_score = 0.6745 * (value - median) / mad;
            if modified_z_score.abs() > threshold {
                anomalies.push(i);
            }
        }
        anomalies
    }
}