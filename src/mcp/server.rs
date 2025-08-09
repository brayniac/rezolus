use super::Config;
use ringlog::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json::{json, Value};

use crate::viewer::tsdb::Tsdb;

/// MCP server state
pub struct MCPServer {
    config: Config,
    tsdb_cache: Arc<RwLock<HashMap<String, Arc<Tsdb>>>>,
    temp_dashboards: Arc<RwLock<HashMap<String, Value>>>,
}

impl MCPServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            tsdb_cache: Arc::new(RwLock::new(HashMap::new())),
            temp_dashboards: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Run the MCP server using stdio
    pub async fn run_stdio(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        // Send initialization message
        self.send_capabilities(&mut stdout).await?;

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            match self.handle_message(&line).await {
                Ok(response) => {
                    if let Some(resp) = response {
                        let response_str = serde_json::to_string(&resp)?;
                        stdout.write_all(response_str.as_bytes()).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    }
                }
                Err(e) => {
                    eprintln!("MCP Error: {}", e);
                    let error_response = json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -1,
                            "message": e.to_string()
                        }
                    });
                    let response_str = serde_json::to_string(&error_response)?;
                    stdout.write_all(response_str.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
            }
        }

        Ok(())
    }

    async fn send_capabilities(&self, stdout: &mut io::Stdout) -> Result<(), Box<dyn std::error::Error>> {
        let capabilities = json!({
            "jsonrpc": "2.0",
            "result": {
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "rezolus-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });
        
        let response_str = serde_json::to_string(&capabilities)?;
        stdout.write_all(response_str.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        
        Ok(())
    }

    async fn handle_message(&mut self, message: &str) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let request: Value = serde_json::from_str(message)?;
        
        let method = request.get("method")
            .and_then(|m| m.as_str())
            .ok_or("Missing method")?;
        
        let id = request.get("id").cloned();

        match method {
            "initialize" => {
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "tools": {
                                "listChanged": false
                            }
                        },
                        "serverInfo": {
                            "name": "rezolus-mcp",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                })))
            }
            "tools/list" => {
                Ok(Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            {
                                "name": "list_cgroups",
                                "description": "List all cgroups and available metrics in a Rezolus parquet file",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "file_path": {
                                            "type": "string",
                                            "description": "Path to the Rezolus .parquet file"
                                        }
                                    },
                                    "required": ["file_path"]
                                }
                            },
                            {
                                "name": "analyze_correlation",
                                "description": "Analyze correlation between two metrics from a Rezolus parquet file",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "file_path": {
                                            "type": "string",
                                            "description": "Path to the Rezolus .parquet file"
                                        },
                                        "metric1": {
                                            "type": "string",
                                            "description": "First metric name"
                                        },
                                        "metric2": {
                                            "type": "string",
                                            "description": "Second metric name"
                                        }
                                    },
                                    "required": ["file_path", "metric1", "metric2"]
                                }
                            },
                            {
                                "name": "detect_anomalies",
                                "description": "Detect anomalies in a metric from a Rezolus parquet file",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "file_path": {
                                            "type": "string",
                                            "description": "Path to the Rezolus .parquet file"
                                        },
                                        "metric": {
                                            "type": "string",
                                            "description": "Metric name to analyze"
                                        },
                                        "sensitivity": {
                                            "type": "number",
                                            "description": "Anomaly detection sensitivity (1.0-5.0)",
                                            "minimum": 1.0,
                                            "maximum": 5.0,
                                            "default": 2.0
                                        }
                                    },
                                    "required": ["file_path", "metric"]
                                }
                            },
                            {
                                "name": "discover_correlations",
                                "description": "Discover strongest correlations across all metrics in a Rezolus parquet file",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "file_path": {
                                            "type": "string",
                                            "description": "Path to the Rezolus .parquet file"
                                        },
                                        "min_correlation": {
                                            "type": "number",
                                            "description": "Minimum correlation strength to report (0.0-1.0)",
                                            "minimum": 0.0,
                                            "maximum": 1.0,
                                            "default": 0.5
                                        },
                                        "max_pairs": {
                                            "type": "integer",
                                            "description": "Maximum number of metric pairs to analyze",
                                            "minimum": 10,
                                            "maximum": 10000,
                                            "default": 1000
                                        }
                                    },
                                    "required": ["file_path"]
                                }
                            }
                        ]
                    }
                })))
            }
            "tools/call" => {
                let params = request.get("params")
                    .ok_or("Missing params")?;
                
                self.handle_tool_call(id, params).await
            }
            _ => {
                Err(format!("Unknown method: {}", method).into())
            }
        }
    }

    async fn handle_tool_call(&mut self, id: Option<Value>, params: &Value) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let tool_name = params.get("name")
            .and_then(|n| n.as_str())
            .ok_or("Missing tool name")?;
        
        let arguments = params.get("arguments")
            .ok_or("Missing arguments")?;

        match tool_name {
            "list_cgroups" => {
                let result = self.list_cgroups(arguments).await?;
                Ok(Some(json!({
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
                })))
            }
            "analyze_correlation" => {
                let result = self.analyze_correlation(arguments).await?;
                Ok(Some(json!({
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
                })))
            }
            "discover_correlations" => {
                let result = self.discover_correlations(arguments).await?;
                Ok(Some(json!({
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
                })))
            }
            "detect_anomalies" => {
                let result = self.detect_anomalies(arguments).await?;
                Ok(Some(json!({
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
                })))
            }
            _ => {
                Err(format!("Unknown tool: {}", tool_name).into())
            }
        }
    }

    async fn list_cgroups(&mut self, arguments: &Value) -> Result<String, Box<dyn std::error::Error>> {
        let file_path = arguments.get("file_path")
            .and_then(|p| p.as_str())
            .ok_or("Missing file_path")?;
        
        // Load TSDB (with caching)
        let tsdb = self.get_or_load_tsdb(file_path).await?;
        
        // List cgroups
        use crate::mcp::tools::list_cgroups::list_cgroups;
        let report = list_cgroups(&tsdb)?;
        
        Ok(report.to_summary())
    }
    
    async fn analyze_correlation(&mut self, arguments: &Value) -> Result<String, Box<dyn std::error::Error>> {
        let file_path = arguments.get("file_path")
            .and_then(|p| p.as_str())
            .ok_or("Missing file_path")?;
        
        let metric1 = arguments.get("metric1")
            .and_then(|m| m.as_str())
            .ok_or("Missing metric1")?;
            
        let metric2 = arguments.get("metric2")
            .and_then(|m| m.as_str())
            .ok_or("Missing metric2")?;

        // Load TSDB (with caching)
        let tsdb = self.get_or_load_tsdb(file_path).await?;
        
        // Perform correlation analysis
        use crate::mcp::tools::correlation::analyze_correlation;
        let analysis = analyze_correlation(&tsdb, metric1, metric2)?;
        
        // Generate dashboard ID (in real implementation, would create actual dashboard)
        let dashboard_id = format!("{:x}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() & 0xFFFFFF);
        
        // Format result with recommendations
        let mut result = analysis.to_summary();
        result.push_str("\n\n");
        result.push_str(&analysis.get_dashboard_recommendation());
        result.push_str(&format!(
            "\n\nDashboard URL: http://localhost:8081/ai/{} (placeholder - dashboard creation not yet implemented)",
            dashboard_id
        ));
        
        Ok(result)
    }

    async fn discover_correlations(&mut self, arguments: &Value) -> Result<String, Box<dyn std::error::Error>> {
        let file_path = arguments.get("file_path")
            .and_then(|p| p.as_str())
            .ok_or("Missing file_path")?;
        
        let min_correlation = arguments.get("min_correlation")
            .and_then(|c| c.as_f64());
            
        let max_pairs = arguments.get("max_pairs")
            .and_then(|m| m.as_u64())
            .map(|m| m as usize);
            
        let complete = arguments.get("complete")
            .and_then(|c| c.as_bool())
            .unwrap_or(false);
            
        let deep = arguments.get("deep")
            .and_then(|d| d.as_bool())
            .unwrap_or(false);
            
        let isolate_cgroup = arguments.get("isolate_cgroup")
            .and_then(|c| c.as_str());

        // Load TSDB (with caching)
        let tsdb = self.get_or_load_tsdb(file_path).await?;
        
        // If cgroup isolation analysis requested
        if let Some(cgroup_name) = isolate_cgroup {
            use crate::mcp::tools::cgroup_isolation::analyze_cgroup_isolation;
            eprintln!("Performing cgroup isolation analysis for: {}", cgroup_name);
            let report = analyze_cgroup_isolation(&tsdb, cgroup_name)?;
            return Ok(report.to_detailed_summary());
        }
        
        // If deep analysis requested, do that
        if deep {
            use crate::mcp::tools::deep_analysis::deep_correlation_analysis;
            eprintln!("Performing DEEP correlation analysis...");
            let report = deep_correlation_analysis(&tsdb)?;
            return Ok(report.to_detailed_summary());
        }
        
        // If complete analysis requested, do that instead
        if complete {
            use crate::mcp::tools::complete_analysis::complete_correlation_analysis;
            eprintln!("Performing COMPLETE correlation analysis...");
            let report = complete_correlation_analysis(&tsdb, min_correlation.unwrap_or(0.5))?;
            return Ok(report.to_detailed_summary());
        }
        
        // Check if we have cgroup metrics
        let has_cgroups = tsdb.counter_names().iter().any(|n| n.starts_with("cgroup_"));
        
        if has_cgroups {
            // Use parallel cgroup-aware discovery
            use crate::mcp::tools::parallel_discovery::parallel_cgroup_correlations;
            use crate::mcp::tools::cgroup_discovery::format_cgroup_report;
            eprintln!("Using parallel cgroup-aware correlation discovery");
            let cgroup_results = parallel_cgroup_correlations(&tsdb, min_correlation, Some(10))?;
            return Ok(format_cgroup_report(&cgroup_results));
        }
        
        // Use parallel discovery for non-cgroup metrics
        use crate::mcp::tools::parallel_discovery::parallel_discover_correlations;
        eprintln!("Using parallel correlation discovery");
        let results = parallel_discover_correlations(&tsdb, min_correlation)?;
        
        // Format results
        let mut summary = format!("Parallel correlation discovery found {} strong correlations\n\n", results.len());
        
        summary.push_str("🔥 TOP POSITIVE CORRELATIONS:\n");
        let positive: Vec<_> = results.iter().filter(|r| r.correlation > 0.0).take(10).collect();
        for (i, r) in positive.iter().enumerate() {
            summary.push_str(&format!(
                "{}. {} vs {} (r={:.3}, n={})\n",
                i + 1, r.metric1, r.metric2, r.correlation, r.sample_count
            ));
        }
        
        summary.push_str("\n❄️ TOP NEGATIVE CORRELATIONS:\n");
        let negative: Vec<_> = results.iter().filter(|r| r.correlation < 0.0).take(10).collect();
        for (i, r) in negative.iter().enumerate() {
            summary.push_str(&format!(
                "{}. {} vs {} (r={:.3}, n={})\n",
                i + 1, r.metric1, r.metric2, r.correlation, r.sample_count
            ));
        }
        
        Ok(summary)
    }

    async fn detect_anomalies(&mut self, arguments: &Value) -> Result<String, Box<dyn std::error::Error>> {
        let file_path = arguments.get("file_path")
            .and_then(|p| p.as_str())
            .ok_or("Missing file_path")?;
        
        let metric = arguments.get("metric")
            .and_then(|m| m.as_str())
            .ok_or("Missing metric")?;
            
        let sensitivity = arguments.get("sensitivity")
            .and_then(|s| s.as_f64())
            .unwrap_or(2.0);

        // Load TSDB (with caching)
        let _tsdb = self.get_or_load_tsdb(file_path).await?;
        
        // TODO: Implement actual anomaly detection
        // For now, return a placeholder
        Ok(format!(
            "Anomaly detection for {} from {} (sensitivity: {})\n\
            Found 3 anomalies (placeholder)\n\
            Dashboard URL: http://localhost:8081/ai/def456 (placeholder)",
            metric, file_path, sensitivity
        ))
    }

    async fn get_or_load_tsdb(&mut self, file_path: &str) -> Result<Arc<Tsdb>, Box<dyn std::error::Error>> {
        // Check cache first
        {
            let cache = self.tsdb_cache.read().unwrap();
            if let Some(tsdb) = cache.get(file_path) {
                return Ok(Arc::clone(tsdb));
            }
        }
        
        // Load TSDB
        info!("Loading TSDB from: {}", file_path);
        let tsdb = Tsdb::load(Path::new(file_path))
            .map_err(|e| format!("Failed to load TSDB: {}", e))?;
        let tsdb_arc = Arc::new(tsdb);
        
        // Cache it
        {
            let mut cache = self.tsdb_cache.write().unwrap();
            cache.insert(file_path.to_string(), Arc::clone(&tsdb_arc));
        }
        
        Ok(tsdb_arc)
    }
}

/// Run the MCP server
pub fn run(config: Config) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    rt.block_on(async {
        let mut server = MCPServer::new(config);
        if let Err(e) = server.run_stdio().await {
            eprintln!("MCP Server error: {}", e);
            std::process::exit(1);
        }
    });
}