use crate::viewer::tsdb::Tsdb;
use crate::mcp::tools::parallel_discovery::{parallel_discover_correlations, parallel_cgroup_correlations};
use crate::mcp::tools::cgroup_discovery::CgroupCorrelationResult;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;

pub struct CompleteAnalysisReport {
    pub total_metrics: usize,
    pub total_pairs_analyzed: usize,
    pub analysis_time_ms: u128,
    pub strongest_positive: Vec<CorrelationSummary>,
    pub strongest_negative: Vec<CorrelationSummary>,
    pub surprising_discoveries: Vec<CorrelationSummary>,
    pub metric_connectivity: Vec<MetricConnectivity>,
    pub cgroup_analysis: Option<CgroupAnalysisSummary>,
}

#[derive(Clone)]
pub struct CorrelationSummary {
    pub metric1: String,
    pub metric2: String,
    pub correlation: f64,
    pub interpretation: String,
}

pub struct MetricConnectivity {
    pub metric: String,
    pub strong_correlations: usize,
    pub avg_correlation: f64,
    pub most_correlated_with: String,
}

pub struct CgroupAnalysisSummary {
    pub total_cgroups: usize,
    pub most_active_cgroup: String,
    pub highest_internal_correlation: String,
    pub cross_cgroup_patterns: Vec<String>,
}

/// Perform complete correlation analysis without limits
pub fn complete_correlation_analysis(
    tsdb: &Arc<Tsdb>,
    min_correlation: f64,
) -> Result<CompleteAnalysisReport, Box<dyn std::error::Error>> {
    let start = Instant::now();
    
    eprintln!("Starting COMPLETE correlation analysis...");
    
    // Check if we have cgroups
    let has_cgroups = tsdb.counter_names().iter().any(|n| n.starts_with("cgroup_"));
    
    let mut report = if has_cgroups {
        analyze_with_cgroups(tsdb, min_correlation)?
    } else {
        analyze_without_cgroups(tsdb, min_correlation)?
    };
    
    report.analysis_time_ms = start.elapsed().as_millis();
    
    // Find surprising discoveries
    report.surprising_discoveries = find_surprising_correlations(&report.strongest_positive, &report.strongest_negative);
    
    // Calculate metric connectivity
    report.metric_connectivity = calculate_metric_connectivity(&report.strongest_positive, &report.strongest_negative);
    
    Ok(report)
}

fn analyze_without_cgroups(
    tsdb: &Arc<Tsdb>,
    min_correlation: f64,
) -> Result<CompleteAnalysisReport, Box<dyn std::error::Error>> {
    // Get ALL correlations (set threshold to 0 to get everything)
    let all_correlations = parallel_discover_correlations(tsdb, Some(0.0))?;
    
    let total_metrics = tsdb.counter_names().len() + tsdb.gauge_names().len();
    let total_pairs = (total_metrics * (total_metrics - 1)) / 2;
    
    eprintln!("Analyzed {} unique metric pairs from {} metrics", total_pairs, total_metrics);
    eprintln!("Total correlations computed: {}", all_correlations.len());
    
    // Now filter by threshold for reporting
    let filtered: Vec<_> = all_correlations.into_iter()
        .filter(|r| r.correlation.abs() >= min_correlation)
        .collect();
    
    eprintln!("Found {} correlations above threshold {}", filtered.len(), min_correlation);
    
    // Separate positive and negative
    let mut positive: Vec<_> = filtered.iter()
        .filter(|r| r.correlation > 0.0)
        .map(|r| CorrelationSummary {
            metric1: r.metric1.clone(),
            metric2: r.metric2.clone(),
            correlation: r.correlation,
            interpretation: interpret_correlation(r.correlation),
        })
        .collect();
    
    let mut negative: Vec<_> = filtered.iter()
        .filter(|r| r.correlation < 0.0)
        .map(|r| CorrelationSummary {
            metric1: r.metric1.clone(),
            metric2: r.metric2.clone(),
            correlation: r.correlation,
            interpretation: interpret_correlation(r.correlation),
        })
        .collect();
    
    // Sort by strength
    positive.sort_by(|a, b| b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap());
    negative.sort_by(|a, b| b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap());
    
    Ok(CompleteAnalysisReport {
        total_metrics,
        total_pairs_analyzed: total_pairs,
        analysis_time_ms: 0, // Will be set by caller
        strongest_positive: positive.into_iter().take(20).collect(),
        strongest_negative: negative.into_iter().take(20).collect(),
        surprising_discoveries: Vec::new(), // Will be filled by caller
        metric_connectivity: Vec::new(), // Will be filled by caller
        cgroup_analysis: None,
    })
}

fn analyze_with_cgroups(
    tsdb: &Arc<Tsdb>,
    min_correlation: f64,
) -> Result<CompleteAnalysisReport, Box<dyn std::error::Error>> {
    // Analyze ALL cgroups with no filtering (threshold = 0)
    let all_cgroup_correlations = parallel_cgroup_correlations(tsdb, Some(0.0), None)?;
    
    // Also get ALL system-wide correlations
    let all_system_correlations = parallel_discover_correlations(tsdb, Some(0.0))?;
    
    let total_metrics = tsdb.counter_names().len() + tsdb.gauge_names().len();
    let total_analyzed = all_cgroup_correlations.len() + all_system_correlations.len();
    
    eprintln!("Total correlations computed: {} (cgroup: {}, system: {})", 
             total_analyzed, all_cgroup_correlations.len(), all_system_correlations.len());
    
    // Filter by threshold
    let cgroup_correlations: Vec<_> = all_cgroup_correlations.into_iter()
        .filter(|r| r.correlation.abs() >= min_correlation)
        .collect();
    
    let system_correlations: Vec<_> = all_system_correlations.into_iter()
        .filter(|r| r.correlation.abs() >= min_correlation)
        .collect();
    
    eprintln!("Found {} correlations above threshold {} (cgroup: {}, system: {})", 
             cgroup_correlations.len() + system_correlations.len(), min_correlation,
             cgroup_correlations.len(), system_correlations.len());
    
    // Group by cgroup for analysis
    let mut by_cgroup: HashMap<String, Vec<&CgroupCorrelationResult>> = HashMap::new();
    for result in &cgroup_correlations {
        by_cgroup.entry(result.cgroup_name.clone())
            .or_default()
            .push(result);
    }
    
    // Find most active cgroup
    let most_active = by_cgroup.iter()
        .max_by_key(|(_, results)| results.len())
        .map(|(name, _)| name.clone())
        .unwrap_or_default();
    
    // Find highest internal correlation
    let highest_internal = cgroup_correlations.iter()
        .max_by(|a, b| a.correlation.abs().partial_cmp(&b.correlation.abs()).unwrap())
        .map(|r| format!("[{}] {} vs {} (r={:.3})", r.cgroup_name, r.metric1, r.metric2, r.correlation))
        .unwrap_or_default();
    
    // Combine cgroup and system correlations for reporting
    let mut all_positive = Vec::new();
    let mut all_negative = Vec::new();
    
    // Add cgroup correlations
    for r in &cgroup_correlations {
        let summary = CorrelationSummary {
            metric1: format!("{}[{}]", r.metric1, r.cgroup_name),
            metric2: format!("{}[{}]", r.metric2, r.cgroup_name),
            correlation: r.correlation,
            interpretation: interpret_correlation(r.correlation),
        };
        
        if r.correlation > 0.0 {
            all_positive.push(summary);
        } else {
            all_negative.push(summary);
        }
    }
    
    // Add system correlations
    for r in &system_correlations {
        let summary = CorrelationSummary {
            metric1: r.metric1.clone(),
            metric2: r.metric2.clone(),
            correlation: r.correlation,
            interpretation: interpret_correlation(r.correlation),
        };
        
        if r.correlation > 0.0 {
            all_positive.push(summary);
        } else {
            all_negative.push(summary);
        }
    }
    
    // Sort by strength
    all_positive.sort_by(|a, b| b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap());
    all_negative.sort_by(|a, b| b.correlation.abs().partial_cmp(&a.correlation.abs()).unwrap());
    
    Ok(CompleteAnalysisReport {
        total_metrics,
        total_pairs_analyzed: total_analyzed,
        analysis_time_ms: 0,
        strongest_positive: all_positive.into_iter().take(20).collect(),
        strongest_negative: all_negative.into_iter().take(20).collect(),
        surprising_discoveries: Vec::new(),
        metric_connectivity: Vec::new(),
        cgroup_analysis: Some(CgroupAnalysisSummary {
            total_cgroups: by_cgroup.len(),
            most_active_cgroup: most_active,
            highest_internal_correlation: highest_internal,
            cross_cgroup_patterns: Vec::new(), // TODO: Implement cross-cgroup analysis
        }),
    })
}

fn interpret_correlation(r: f64) -> String {
    let abs_r = r.abs();
    let strength = if abs_r >= 0.9 {
        "Very strong"
    } else if abs_r >= 0.7 {
        "Strong"
    } else if abs_r >= 0.5 {
        "Moderate"
    } else if abs_r >= 0.3 {
        "Weak"
    } else {
        "Very weak"
    };
    
    let direction = if r > 0.0 { "positive" } else { "negative" };
    
    format!("{} {} correlation", strength, direction)
}

fn find_surprising_correlations(
    positive: &[CorrelationSummary],
    negative: &[CorrelationSummary],
) -> Vec<CorrelationSummary> {
    let mut surprising = Vec::new();
    
    // Look for unexpected cross-subsystem correlations
    for corr in positive.iter().chain(negative.iter()) {
        if corr.correlation.abs() < 0.7 {
            continue;
        }
        
        let subsystem1 = get_subsystem(&corr.metric1);
        let subsystem2 = get_subsystem(&corr.metric2);
        
        // Different subsystems with strong correlation
        if subsystem1 != subsystem2 && !is_expected_cross_system(&subsystem1, &subsystem2) {
            surprising.push(corr.clone());
        }
    }
    
    surprising.truncate(10);
    surprising
}

fn get_subsystem(metric: &str) -> &str {
    let base_metric = metric.split('[').next().unwrap_or(metric);
    
    if base_metric.starts_with("cpu_") || base_metric.starts_with("cgroup_cpu_") {
        "cpu"
    } else if base_metric.starts_with("memory_") || base_metric.starts_with("cgroup_memory_") {
        "memory"
    } else if base_metric.starts_with("network_") || base_metric.starts_with("tcp_") {
        "network"
    } else if base_metric.starts_with("blockio_") {
        "disk"
    } else if base_metric.starts_with("scheduler_") || base_metric.starts_with("cgroup_scheduler_") {
        "scheduler"
    } else {
        "other"
    }
}

fn is_expected_cross_system(sys1: &str, sys2: &str) -> bool {
    matches!(
        (sys1, sys2),
        ("cpu", "scheduler") | ("scheduler", "cpu") |
        ("cpu", "network") | ("network", "cpu") |
        ("memory", "disk") | ("disk", "memory")
    )
}

fn calculate_metric_connectivity(
    positive: &[CorrelationSummary],
    negative: &[CorrelationSummary],
) -> Vec<MetricConnectivity> {
    let mut connectivity_map: HashMap<String, Vec<f64>> = HashMap::new();
    let mut best_correlation: HashMap<String, (String, f64)> = HashMap::new();
    
    // Build connectivity map
    for corr in positive.iter().chain(negative.iter()) {
        // Track both metrics
        connectivity_map.entry(corr.metric1.clone())
            .or_default()
            .push(corr.correlation.abs());
        
        connectivity_map.entry(corr.metric2.clone())
            .or_default()
            .push(corr.correlation.abs());
        
        // Track best correlation for each metric
        update_best_correlation(&mut best_correlation, &corr.metric1, &corr.metric2, corr.correlation);
        update_best_correlation(&mut best_correlation, &corr.metric2, &corr.metric1, corr.correlation);
    }
    
    // Calculate connectivity scores
    let mut connectivity: Vec<_> = connectivity_map.iter()
        .map(|(metric, correlations)| {
            let avg = correlations.iter().sum::<f64>() / correlations.len() as f64;
            let (best_with, _) = best_correlation.get(metric)
                .cloned()
                .unwrap_or((String::new(), 0.0));
            
            MetricConnectivity {
                metric: metric.clone(),
                strong_correlations: correlations.iter().filter(|&&r| r >= 0.7).count(),
                avg_correlation: avg,
                most_correlated_with: best_with,
            }
        })
        .collect();
    
    // Sort by number of strong correlations
    connectivity.sort_by(|a, b| b.strong_correlations.cmp(&a.strong_correlations));
    connectivity.truncate(20);
    
    connectivity
}

fn update_best_correlation(
    best: &mut HashMap<String, (String, f64)>,
    metric: &str,
    other: &str,
    correlation: f64,
) {
    let abs_corr = correlation.abs();
    best.entry(metric.to_string())
        .and_modify(|e| {
            if abs_corr > e.1.abs() {
                *e = (other.to_string(), correlation);
            }
        })
        .or_insert((other.to_string(), correlation));
}

impl CompleteAnalysisReport {
    pub fn to_detailed_summary(&self) -> String {
        let mut s = String::new();
        
        s.push_str(&format!("📊 COMPLETE CORRELATION ANALYSIS REPORT\n"));
        s.push_str(&format!("=====================================\n\n"));
        
        s.push_str(&format!("📈 Analysis Summary:\n"));
        s.push_str(&format!("  • Total metrics: {}\n", self.total_metrics));
        s.push_str(&format!("  • Pairs analyzed: {}\n", self.total_pairs_analyzed));
        s.push_str(&format!("  • Analysis time: {}ms\n", self.analysis_time_ms));
        
        if let Some(cgroup) = &self.cgroup_analysis {
            s.push_str(&format!("\n📦 Cgroup Analysis:\n"));
            s.push_str(&format!("  • Total cgroups: {}\n", cgroup.total_cgroups));
            s.push_str(&format!("  • Most active: {}\n", cgroup.most_active_cgroup));
            s.push_str(&format!("  • Strongest: {}\n", cgroup.highest_internal_correlation));
        }
        
        s.push_str(&format!("\n🔥 TOP POSITIVE CORRELATIONS:\n"));
        for (i, corr) in self.strongest_positive.iter().take(10).enumerate() {
            s.push_str(&format!("  {}. {} vs {} (r={:.3})\n     {}\n", 
                i + 1, corr.metric1, corr.metric2, corr.correlation, corr.interpretation));
        }
        
        s.push_str(&format!("\n❄️ TOP NEGATIVE CORRELATIONS:\n"));
        for (i, corr) in self.strongest_negative.iter().take(10).enumerate() {
            s.push_str(&format!("  {}. {} vs {} (r={:.3})\n     {}\n",
                i + 1, corr.metric1, corr.metric2, corr.correlation, corr.interpretation));
        }
        
        if !self.surprising_discoveries.is_empty() {
            s.push_str(&format!("\n🎯 SURPRISING DISCOVERIES:\n"));
            for corr in &self.surprising_discoveries {
                s.push_str(&format!("  • {} vs {} (r={:.3})\n", 
                    corr.metric1, corr.metric2, corr.correlation));
            }
        }
        
        if !self.metric_connectivity.is_empty() {
            s.push_str(&format!("\n🕸️ MOST CONNECTED METRICS:\n"));
            for conn in self.metric_connectivity.iter().take(10) {
                s.push_str(&format!("  • {} ({} strong correlations, avg r={:.3})\n    Most correlated with: {}\n",
                    conn.metric, conn.strong_correlations, conn.avg_correlation, conn.most_correlated_with));
            }
        }
        
        s
    }
}