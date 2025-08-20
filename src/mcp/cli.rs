use crate::viewer::promql::QueryEngine;
use crate::viewer::tsdb::Tsdb;
use clap::ArgMatches;
use std::path::Path;
use std::sync::Arc;

/// Handle MCP CLI commands
pub fn handle_command(cmd: &str, args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "discover" => handle_discover(args),
        "anomaly" => handle_anomaly(args),
        "list" => handle_list(args),
        "correlation" => handle_correlation(args),
        "trend" => handle_trend(args),
        "fft" => handle_fft(args),
        "diagnose" => handle_diagnose(args),
        _ => {
            eprintln!("Unknown command: {}", cmd);
            std::process::exit(1);
        }
    }
}

fn handle_discover(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = args.get_one::<String>("FILE").unwrap();
    let min_correlation = args.get_one::<f64>("min-correlation").copied().unwrap_or(0.5);
    let isolate_cgroup = args.get_one::<String>("isolate-cgroup");

    eprintln!("Loading TSDB from: {}", file_path);
    let tsdb = Arc::new(Tsdb::load(Path::new(file_path))?);
    let engine = Arc::new(QueryEngine::new(Arc::clone(&tsdb)));

    // Get time range
    let (start, end) = engine.get_time_range();
    let step = 60.0; // 1 minute resolution
    
    use crate::mcp::correlation::{calculate_correlation, format_correlation_result};
    
    if let Some(cgroup) = isolate_cgroup {
        println!("Analyzing cgroup: {}", cgroup);
        println!("Finding correlations with minimum threshold: {}", min_correlation);
        println!();
        
        // Build cgroup-specific query
        let cgroup_cpu = format!("sum(irate(cgroup_cpu_usage{{name=\"{}\"}}[5m]))", cgroup);
        
        // Compare against per-CPU usage
        println!("Checking correlation with per-CPU usage...");
        let cpu_query = "sum by (id) (irate(cpu_usage[5m]))";
        
        if let Ok(result) = calculate_correlation(&engine, &cgroup_cpu, cpu_query, start, end, step) {
            if result.correlation.abs() >= min_correlation {
                println!("{}", format_correlation_result(&result));
            }
        }
        
        // Compare against total CPU cycles
        let cycles_query = "irate(cpu_cycles[5m])";
        if let Ok(result) = calculate_correlation(&engine, &cgroup_cpu, cycles_query, start, end, step) {
            if result.correlation.abs() >= min_correlation {
                println!("\n{}", format_correlation_result(&result));
            }
        }
    } else {
        println!("Discovering correlations with threshold: {}", min_correlation);
        println!("Time range: {:.0} to {:.0} seconds", start, end);
        println!();
        
        // Use meaningful queries from the viewer dashboards
        use crate::mcp::discovery_queries::{get_interesting_pairs, get_discovery_queries};
        use crate::mcp::correlation::calculate_correlation_with_names;
        
        // First check known interesting pairs
        println!("Checking known interesting metric pairs...");
        let mut found_correlations = Vec::new();
        
        for pair in get_interesting_pairs() {
            if let Ok(result) = calculate_correlation_with_names(
                &engine, 
                pair.query1, 
                pair.query2,
                Some(pair.name1),
                Some(pair.name2),
                start, 
                end, 
                step
            ) {
                if result.correlation.abs() >= min_correlation {
                    found_correlations.push(result);
                }
            }
        }
        
        // Then check discovery queries against each other
        println!("Checking dashboard metrics for correlations...");
        let discovery_queries = get_discovery_queries(&tsdb);
        
        for i in 0..discovery_queries.len().min(30) {
            for j in i+1..discovery_queries.len().min(30) {
                let metric1 = &discovery_queries[i];
                let metric2 = &discovery_queries[j];
                
                // Skip if both are the same category of per-CPU metrics
                if metric1.name.contains("Per-CPU") && metric2.name.contains("Per-CPU") {
                    continue;
                }
                
                if let Ok(result) = calculate_correlation_with_names(
                    &engine,
                    metric1.query,
                    metric2.query,
                    Some(metric1.name),
                    Some(metric2.name),
                    start,
                    end,
                    step
                ) {
                    if result.correlation.abs() >= min_correlation {
                        found_correlations.push(result);
                    }
                }
            }
        }
        
        // Sort by absolute correlation
        found_correlations.sort_by(|a, b| {
            b.correlation.abs()
                .partial_cmp(&a.correlation.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        if found_correlations.is_empty() {
            println!("No correlations found above threshold {}", min_correlation);
        } else {
            println!("Found {} correlations above threshold {}:\n", 
                     found_correlations.len(), min_correlation);
            
            for (i, result) in found_correlations.iter().take(10).enumerate() {
                println!("{}. r={:.4} between:", i + 1, result.correlation);
                
                // Display human-readable names
                let name1 = result.metric1_name.as_ref().unwrap_or(&result.metric1);
                let name2 = result.metric2_name.as_ref().unwrap_or(&result.metric2);
                
                println!("   {} vs {}", name1, name2);
                
                // Show queries if they're complex (have names)
                if result.metric1_name.is_some() || result.metric2_name.is_some() {
                    if result.metric1_name.is_some() {
                        println!("     [{}]", result.metric1);
                    }
                    if result.metric2_name.is_some() {
                        println!("     [{}]", result.metric2);
                    }
                }
                println!();
            }
            
            if found_correlations.len() > 10 {
                println!("... and {} more correlations", found_correlations.len() - 10);
            }
        }
    }

    Ok(())
}

fn handle_anomaly(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = args.get_one::<String>("FILE").unwrap();
    let metric_input = args.get_one::<String>("METRIC").unwrap();
    let sensitivity = args.get_one::<f64>("sensitivity").copied().unwrap_or(2.0);

    eprintln!("Loading TSDB from: {}", file_path);
    let tsdb = Arc::new(Tsdb::load(Path::new(file_path))?);
    let engine = Arc::new(QueryEngine::new(Arc::clone(&tsdb)));

    // Get time range
    let (start, end) = engine.get_time_range();
    let step = 60.0; // 1 minute resolution
    
    use crate::mcp::anomaly::{detect_anomalies, format_anomaly_result, AnomalyMethod};
    use crate::mcp::discovery_queries::get_discovery_queries;
    
    // Check if the input is a metric name or a query
    let discovery_queries = get_discovery_queries(&tsdb);
    
    // Try to find a matching metric name first
    let (query, name) = if let Some(discovered) = discovery_queries.iter()
        .find(|m| m.name.to_lowercase().contains(&metric_input.to_lowercase()) || 
                   metric_input.to_lowercase().contains(&m.name.to_lowercase())) {
        (discovered.query, Some(discovered.name))
    } else {
        // Treat input as a direct query
        (metric_input.as_str(), None)
    };
    
    // Determine the anomaly detection method based on sensitivity
    let (method, threshold) = if sensitivity <= 1.5 {
        (AnomalyMethod::InterquartileRange, sensitivity)
    } else if sensitivity <= 3.0 {
        (AnomalyMethod::ZScore, sensitivity)
    } else {
        (AnomalyMethod::MedianAbsoluteDeviation, sensitivity)
    };
    
    println!("Detecting anomalies in: {}", name.unwrap_or(query));
    if name.is_some() {
        println!("Query: {}", query);
    }
    println!("Method: {} (threshold: {})", method, threshold);
    println!();
    
    match detect_anomalies(&engine, query, name, method, threshold, start, end, step) {
        Ok(result) => {
            println!("{}", format_anomaly_result(&result));
        }
        Err(e) => {
            eprintln!("Error detecting anomalies: {}", e);
            eprintln!("\nAvailable metrics:");
            for metric in discovery_queries.iter().take(10) {
                eprintln!("  - {} ({})", metric.name, metric.category);
            }
            if discovery_queries.len() > 10 {
                eprintln!("  ... and {} more", discovery_queries.len() - 10);
            }
            eprintln!("\nYou can also use a direct PromQL query.");
        }
    }

    Ok(())
}

fn handle_list(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = args.get_one::<String>("FILE").unwrap();

    eprintln!("Loading TSDB from: {}", file_path);
    let tsdb = Arc::new(Tsdb::load(Path::new(file_path))?);
    let engine = QueryEngine::new(Arc::clone(&tsdb));

    println!("Listing metrics in: {}", file_path);
    println!("Data source: {} {}", tsdb.source(), tsdb.version());
    
    // Get time range
    let (start, end) = engine.get_time_range();
    println!("Time range: {:.0} to {:.0} seconds", start, end);
    
    // List actual metrics
    let counters = tsdb.counter_names();
    let gauges = tsdb.gauge_names();
    let histograms = tsdb.histogram_names();
    
    if !counters.is_empty() {
        println!("\nCounters ({}):", counters.len());
        for name in counters.iter().take(10) {
            println!("  - {}", name);
        }
        if counters.len() > 10 {
            println!("  ... and {} more", counters.len() - 10);
        }
    }
    
    if !gauges.is_empty() {
        println!("\nGauges ({}):", gauges.len());
        for name in gauges.iter().take(10) {
            println!("  - {}", name);
        }
        if gauges.len() > 10 {
            println!("  ... and {} more", gauges.len() - 10);
        }
    }
    
    if !histograms.is_empty() {
        println!("\nHistograms ({}):", histograms.len());
        for name in histograms.iter().take(10) {
            println!("  - {}", name);
        }
        if histograms.len() > 10 {
            println!("  ... and {} more", histograms.len() - 10);
        }
    }
    
    println!("\nTotal metrics: {}", counters.len() + gauges.len() + histograms.len());

    Ok(())
}

fn handle_correlation(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = args.get_one::<String>("FILE").unwrap();
    let metric1 = args.get_one::<String>("METRIC1").unwrap();
    let metric2 = args.get_one::<String>("METRIC2").unwrap();

    eprintln!("Loading TSDB from: {}", file_path);
    let tsdb = Arc::new(Tsdb::load(Path::new(file_path))?);
    let engine = Arc::new(QueryEngine::new(tsdb));

    // Get time range
    let (start, end) = engine.get_time_range();
    let step = 60.0; // 1 minute resolution
    
    // Calculate correlation
    use crate::mcp::correlation::{calculate_correlation, format_correlation_result};
    
    match calculate_correlation(&engine, metric1, metric2, start, end, step) {
        Ok(result) => {
            println!("{}", format_correlation_result(&result));
        }
        Err(e) => {
            eprintln!("Error calculating correlation: {}", e);
            eprintln!("\nTip: Make sure both metrics exist and have overlapping data.");
            eprintln!("You can use 'rezolus mcp list {}' to see available metrics.", file_path);
        }
    }

    Ok(())
}

fn handle_trend(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = args.get_one::<String>("FILE").unwrap();
    let metric = args.get_one::<String>("METRIC").unwrap();
    let window_hours = args.get_one::<f64>("window-hours").copied().unwrap_or(24.0);

    eprintln!("Loading TSDB from: {}", file_path);
    let tsdb = Arc::new(Tsdb::load(Path::new(file_path))?);
    let engine = QueryEngine::new(tsdb);

    println!("Analyzing trends in metric: {}", metric);
    println!("Window: {} hours", window_hours);

    // Get time range
    let (start, end) = engine.get_time_range();
    
    if let Ok(_result) = engine.query_range(metric, start, end, 60.0) {
        println!("Successfully queried metric data");
        // Simplified trend analysis
    } else {
        println!("Could not query metric: {}", metric);
    }

    Ok(())
}

fn handle_fft(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = args.get_one::<String>("FILE").unwrap();
    let metric_input = args.get_one::<String>("METRIC").unwrap();
    let step = args.get_one::<f64>("step").copied().unwrap_or(60.0);

    eprintln!("Loading TSDB from: {}", file_path);
    let tsdb = Arc::new(Tsdb::load(Path::new(file_path))?);
    let engine = Arc::new(QueryEngine::new(Arc::clone(&tsdb)));

    // Get time range
    let (start, end) = engine.get_time_range();
    
    use crate::mcp::fft_analysis::{analyze_fft_patterns, format_fft_result};
    use crate::mcp::discovery_queries::get_discovery_queries;
    
    // Check if the input is a metric name or a query
    let discovery_queries = get_discovery_queries(&tsdb);
    
    // Try to find a matching metric name first
    let (query, name) = if let Some(discovered) = discovery_queries.iter()
        .find(|m| m.name.to_lowercase().contains(&metric_input.to_lowercase()) || 
                   metric_input.to_lowercase().contains(&m.name.to_lowercase())) {
        (discovered.query, Some(discovered.name))
    } else {
        // Treat input as a direct query
        (metric_input.as_str(), None)
    };
    
    println!("Analyzing periodic patterns in: {}", name.unwrap_or(query));
    if name.is_some() {
        println!("Query: {}", query);
    }
    println!("Step size: {}s", step);
    println!();
    
    match analyze_fft_patterns(&engine, query, name, start, end, step) {
        Ok(result) => {
            println!("{}", format_fft_result(&result));
        }
        Err(e) => {
            eprintln!("Error analyzing FFT patterns: {}", e);
            eprintln!("\nAvailable metrics:");
            for metric in discovery_queries.iter().take(10) {
                eprintln!("  - {} ({})", metric.name, metric.category);
            }
            if discovery_queries.len() > 10 {
                eprintln!("  ... and {} more", discovery_queries.len() - 10);
            }
            eprintln!("\nYou can also use a direct PromQL query.");
        }
    }

    Ok(())
}

fn handle_diagnose(args: &ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = args.get_one::<String>("FILE").unwrap();
    let scenario = args.get_one::<String>("SCENARIO").unwrap();
    let cgroup_name = args.get_one::<String>("cgroup-name");
    
    eprintln!("Loading TSDB from: {}", file_path);
    let tsdb = Arc::new(Tsdb::load(Path::new(file_path))?);
    
    use crate::mcp::scenarios::ScenarioAnalyzer;
    let analyzer = ScenarioAnalyzer::new(tsdb);
    
    match scenario.as_str() {
        "cpu" => {
            println!("Running CPU performance diagnosis...\n");
            match analyzer.analyze_cpu_performance() {
                Ok(report) => println!("{}", report.format()),
                Err(e) => eprintln!("CPU analysis failed: {}", e),
            }
        }
        "memory" => {
            println!("Running memory pressure diagnosis...\n");
            match analyzer.analyze_memory_pressure() {
                Ok(report) => println!("{}", report.format()),
                Err(e) => eprintln!("Memory analysis failed: {}", e),
            }
        }
        "network" => {
            println!("Running network performance diagnosis...\n");
            match analyzer.analyze_network_performance() {
                Ok(report) => println!("{}", report.format()),
                Err(e) => eprintln!("Network analysis failed: {}", e),
            }
        }
        "latency" => {
            println!("Running latency diagnosis...\n");
            match analyzer.analyze_latency_issues() {
                Ok(report) => println!("{}", report.format()),
                Err(e) => eprintln!("Latency analysis failed: {}", e),
            }
        }
        "cgroup" => {
            if let Some(name) = cgroup_name {
                println!("Running cgroup '{}' diagnosis...\n", name);
                match analyzer.analyze_cgroup_performance(name) {
                    Ok(report) => println!("{}", report.format()),
                    Err(e) => eprintln!("Cgroup analysis failed: {}", e),
                }
            } else {
                eprintln!("Error: --cgroup NAME required for cgroup diagnosis");
                std::process::exit(1);
            }
        }
        "all" => {
            println!("Running comprehensive system diagnosis...\n");
            println!("{}", "=".repeat(60));
            
            // CPU Analysis
            match analyzer.analyze_cpu_performance() {
                Ok(report) => println!("\n{}", report.format()),
                Err(e) => eprintln!("CPU analysis failed: {}", e),
            }
            
            // Memory Analysis
            match analyzer.analyze_memory_pressure() {
                Ok(report) => println!("\n{}", report.format()),
                Err(e) => eprintln!("Memory analysis failed: {}", e),
            }
            
            // Network Analysis  
            match analyzer.analyze_network_performance() {
                Ok(report) => println!("\n{}", report.format()),
                Err(e) => eprintln!("Network analysis failed: {}", e),
            }
            
            // Latency Analysis
            match analyzer.analyze_latency_issues() {
                Ok(report) => println!("\n{}", report.format()),
                Err(e) => eprintln!("Latency analysis failed: {}", e),
            }
            
            println!("\n{}", "=".repeat(60));
            println!("Diagnosis complete.");
        }
        _ => {
            eprintln!("Unknown scenario: {}", scenario);
            std::process::exit(1);
        }
    }
    
    Ok(())
}