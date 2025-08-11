use crate::viewer::tsdb::Tsdb;
use crate::mcp::tools::correlation::{analyze_correlation, CorrelationAnalysis};
use std::sync::Arc;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct CorrelationDiscoveryResult {
    pub metric1: String,
    pub metric2: String,
    pub correlation: f64,
    pub p_value: f64,
    pub series_names: (String, String),
    pub is_aggregated: bool,
}

pub struct DiscoveryReport {
    pub top_positive: Vec<CorrelationDiscoveryResult>,
    pub top_negative: Vec<CorrelationDiscoveryResult>,
    pub surprising: Vec<CorrelationDiscoveryResult>,
    pub total_analyzed: usize,
    pub metrics_count: usize,
}

impl DiscoveryReport {
    pub fn to_summary(&self) -> String {
        let mut summary = format!(
            "Analyzed {} metric pairs ({} unique metrics)\n\n",
            self.total_analyzed, self.metrics_count
        );
        
        summary.push_str("🔥 STRONGEST POSITIVE CORRELATIONS:\n");
        for (i, result) in self.top_positive.iter().take(10).enumerate() {
            summary.push_str(&format!(
                "{}. {} vs {} (r={:.3})\n   {} ↔ {}\n",
                i + 1, 
                result.metric1, 
                result.metric2, 
                result.correlation,
                result.series_names.0,
                result.series_names.1
            ));
        }
        
        summary.push_str("\n❄️ STRONGEST NEGATIVE CORRELATIONS:\n");
        for (i, result) in self.top_negative.iter().take(10).enumerate() {
            summary.push_str(&format!(
                "{}. {} vs {} (r={:.3})\n   {} ↔ {}\n",
                i + 1,
                result.metric1,
                result.metric2,
                result.correlation,
                result.series_names.0,
                result.series_names.1
            ));
        }
        
        if !self.surprising.is_empty() {
            summary.push_str("\n SURPRISING DISCOVERIES:\n");
            summary.push_str("(Unexpected strong correlations between unrelated metrics)\n");
            for result in &self.surprising {
                summary.push_str(&format!(
                    "- {} vs {} (r={:.3})\n",
                    result.metric1,
                    result.metric2,
                    result.correlation
                ));
            }
        }
        
        summary
    }
}

/// Discover correlations across all metrics
pub fn discover_correlations(
    tsdb: &Arc<Tsdb>,
    min_correlation: Option<f64>,
    max_pairs: Option<usize>,
) -> Result<DiscoveryReport, Box<dyn std::error::Error>> {
    let min_correlation = min_correlation.unwrap_or(0.5);
    let max_pairs = max_pairs.unwrap_or(1000); // Limit for performance
    
    // Collect all available metrics
    let mut all_metrics = Vec::new();
    
    // Add counter metrics
    for name in tsdb.counter_names() {
        all_metrics.push((name.to_string(), "counter"));
    }
    
    // Add gauge metrics  
    for name in tsdb.gauge_names() {
        all_metrics.push((name.to_string(), "gauge"));
    }
    
    // TODO: Add histogram metrics if needed
    
    let metrics_count = all_metrics.len();
    let mut results = Vec::new();
    let mut analyzed_pairs = HashSet::new();
    let mut pairs_analyzed = 0;
    
    // Progress tracking
    eprintln!("Starting correlation discovery with {} metrics", metrics_count);
    eprintln!("Maximum pairs to analyze: {}", max_pairs);
    
    // Analyze all unique pairs
    for i in 0..all_metrics.len() {
        if pairs_analyzed >= max_pairs {
            eprintln!("Reached max pairs limit ({}), stopping", max_pairs);
            break;
        }
        
        for j in i+1..all_metrics.len() {
            // Create sorted pair key to avoid duplicates
            let pair_key = if all_metrics[i].0 < all_metrics[j].0 {
                format!("{}:{}", all_metrics[i].0, all_metrics[j].0)
            } else {
                format!("{}:{}", all_metrics[j].0, all_metrics[i].0)
            };
            
            if analyzed_pairs.contains(&pair_key) {
                continue;
            }
            
            if pairs_analyzed >= max_pairs {
                eprintln!("Reached max pairs limit ({}), stopping early", max_pairs);
                break;
            }
            
            analyzed_pairs.insert(pair_key);
            pairs_analyzed += 1;
            
            if pairs_analyzed % 10 == 0 {
                eprintln!("Analyzed {} pairs...", pairs_analyzed);
            }
            
            // Skip certain obvious pairs
            if should_skip_pair(&all_metrics[i].0, &all_metrics[j].0) {
                continue;
            }
            
            // Analyze correlation
            match analyze_correlation(tsdb, &all_metrics[i].0, &all_metrics[j].0) {
                Ok(analysis) => {
                    let primary = analysis.get_primary_correlation();
                    
                    // Only keep significant correlations
                    if primary.coefficient.abs() >= min_correlation {
                        let is_aggregated = matches!(
                            analysis,
                            CorrelationAnalysis::Multiple { aggregated: Some(_), .. }
                        );
                        
                        results.push(CorrelationDiscoveryResult {
                            metric1: all_metrics[i].0.clone(),
                            metric2: all_metrics[j].0.clone(),
                            correlation: primary.coefficient,
                            p_value: primary.p_value,
                            series_names: (
                                primary.series1_name.clone(),
                                primary.series2_name.clone()
                            ),
                            is_aggregated,
                        });
                    }
                }
                Err(e) => {
                    // Skip pairs that can't be correlated
                    eprintln!("Skipping {} vs {}: {}", all_metrics[i].0, all_metrics[j].0, e);
                }
            }
        }
    }
    
    // Sort by absolute correlation strength
    results.sort_by(|a, b| {
        b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap()
    });
    
    // Separate positive and negative correlations
    let top_positive: Vec<_> = results.iter()
        .filter(|r| r.correlation > 0.0)
        .cloned()
        .collect();
        
    let top_negative: Vec<_> = results.iter()
        .filter(|r| r.correlation < 0.0)
        .cloned()
        .collect();
    
    // Find surprising correlations (unrelated metrics with strong correlation)
    let surprising = find_surprising_correlations(&results);
    
    Ok(DiscoveryReport {
        top_positive,
        top_negative,
        surprising,
        total_analyzed: pairs_analyzed,
        metrics_count,
    })
}

/// Determine if a pair should be skipped (obvious correlations)
fn should_skip_pair(metric1: &str, metric2: &str) -> bool {
    // Skip same metric
    if metric1 == metric2 {
        return true;
    }
    
    // Skip obvious pairs like total vs free for same resource
    let obvious_pairs = [
        ("memory_total", "memory_free"),
        ("memory_total", "memory_available"),
        ("memory_total", "memory_cached"),
        // These are somewhat obvious but might be interesting
        // ("cpu_instructions", "cpu_cycles"),
        // ("network_bytes", "network_packets"),
    ];
    
    for (a, b) in &obvious_pairs {
        if (metric1 == *a && metric2 == *b) || (metric1 == *b && metric2 == *a) {
            return true;
        }
    }
    
    false
}

/// Find surprising correlations between seemingly unrelated metrics
fn find_surprising_correlations(results: &[CorrelationDiscoveryResult]) -> Vec<CorrelationDiscoveryResult> {
    let mut surprising = Vec::new();
    
    for result in results {
        if result.correlation.abs() < 0.7 {
            continue; // Only strong correlations are surprising
        }
        
        // Check if metrics are from different subsystems
        let subsystems1 = get_metric_subsystem(&result.metric1);
        let subsystems2 = get_metric_subsystem(&result.metric2);
        
        if subsystems1 != subsystems2 {
            // Different subsystems with strong correlation is surprising
            // Unless it's an expected cross-system correlation
            if !is_expected_cross_system(&result.metric1, &result.metric2) {
                surprising.push(result.clone());
            }
        }
    }
    
    surprising
}

/// Get the subsystem a metric belongs to
fn get_metric_subsystem(metric: &str) -> &str {
    if metric.starts_with("cpu_") {
        "cpu"
    } else if metric.starts_with("memory_") {
        "memory"
    } else if metric.starts_with("network_") || metric.starts_with("tcp_") {
        "network"
    } else if metric.starts_with("blockio_") {
        "disk"
    } else if metric.starts_with("scheduler_") {
        "scheduler"
    } else if metric.starts_with("cgroup_") {
        "cgroup"
    } else {
        "other"
    }
}

/// Check if a cross-system correlation is expected
fn is_expected_cross_system(metric1: &str, metric2: &str) -> bool {
    // CPU and network often correlate (interrupt handling)
    if (metric1.starts_with("cpu_") && metric2.starts_with("network_")) ||
       (metric1.starts_with("network_") && metric2.starts_with("cpu_")) {
        return true;
    }
    
    // CPU and scheduler metrics are related
    if (metric1.starts_with("cpu_") && metric2.starts_with("scheduler_")) ||
       (metric1.starts_with("scheduler_") && metric2.starts_with("cpu_")) {
        return true;
    }
    
    // Memory and disk I/O can correlate (page cache)
    if (metric1.starts_with("memory_") && metric2.starts_with("blockio_")) ||
       (metric1.starts_with("blockio_") && metric2.starts_with("memory_")) {
        return true;
    }
    
    false
}