use crate::viewer::tsdb::{Tsdb, UntypedSeries};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use rayon::prelude::*;
use std::time::Instant;

pub struct DeepAnalysisReport {
    pub cross_core_analysis: CrossCoreAnalysis,
    pub temporal_patterns: TemporalPatterns,
    pub workload_classification: WorkloadClassification,
    pub advanced_metrics: AdvancedMetrics,
    pub total_analysis_time_ms: u128,
}

pub struct CrossCoreAnalysis {
    pub total_cores_analyzed: usize,
    pub inter_core_correlations: Vec<InterCoreCorrelation>,
    pub core_imbalance_score: f64,
    pub core_synchronization_patterns: Vec<String>,
    pub hottest_cores: Vec<(usize, f64)>,
}

pub struct InterCoreCorrelation {
    pub core1: usize,
    pub core2: usize,
    pub metric: String,
    pub correlation: f64,
    pub interpretation: String,
}

pub struct TemporalPatterns {
    pub periodicity_detected: Vec<PeriodicPattern>,
    pub burst_patterns: Vec<BurstPattern>,
    pub trend_analysis: TrendAnalysis,
}

pub struct PeriodicPattern {
    pub metric: String,
    pub period_seconds: f64,
    pub strength: f64,
}

pub struct BurstPattern {
    pub metric: String,
    pub burst_duration_avg: f64,
    pub burst_interval_avg: f64,
    pub burst_intensity: f64,
}

pub struct TrendAnalysis {
    pub increasing_metrics: Vec<(String, f64)>,
    pub decreasing_metrics: Vec<(String, f64)>,
    pub stable_metrics: Vec<String>,
}

pub struct WorkloadClassification {
    pub dominant_workload_type: String,
    pub workload_mix: HashMap<String, f64>,
    pub phase_transitions: Vec<PhaseTransition>,
}

pub struct PhaseTransition {
    pub timestamp: u64,
    pub from_phase: String,
    pub to_phase: String,
    pub trigger_metrics: Vec<String>,
}

pub struct AdvancedMetrics {
    pub mutual_information_pairs: Vec<(String, String, f64)>,
    pub causal_relationships: Vec<CausalRelation>,
    pub outlier_correlations: Vec<OutlierCorrelation>,
}

pub struct CausalRelation {
    pub cause: String,
    pub effect: String,
    pub lag_ms: u64,
    pub confidence: f64,
}

pub struct OutlierCorrelation {
    pub metric1: String,
    pub metric2: String,
    pub normal_correlation: f64,
    pub outlier_correlation: f64,
    pub difference: f64,
}

/// Perform deep analysis including cross-core, temporal, and advanced analytics
pub fn deep_correlation_analysis(
    tsdb: &Arc<Tsdb>,
) -> Result<DeepAnalysisReport, Box<dyn std::error::Error>> {
    let start = Instant::now();
    
    eprintln!("Starting DEEP correlation analysis...");
    
    // Perform cross-core analysis
    let cross_core = analyze_cross_core(tsdb)?;
    
    // Detect temporal patterns
    let temporal = detect_temporal_patterns(tsdb)?;
    
    // Classify workload
    let workload = classify_workload(tsdb)?;
    
    // Calculate advanced metrics
    let advanced = calculate_advanced_metrics(tsdb)?;
    
    let total_time = start.elapsed().as_millis();
    
    Ok(DeepAnalysisReport {
        cross_core_analysis: cross_core,
        temporal_patterns: temporal,
        workload_classification: workload,
        advanced_metrics: advanced,
        total_analysis_time_ms: total_time,
    })
}

fn analyze_cross_core(tsdb: &Arc<Tsdb>) -> Result<CrossCoreAnalysis, Box<dyn std::error::Error>> {
    eprintln!("Analyzing cross-core correlations...");
    
    // Identify per-core metrics
    let mut core_metrics: HashMap<usize, HashMap<String, Arc<UntypedSeries>>> = HashMap::new();
    
    // Check for per-core metrics (counters)
    for metric_name in tsdb.counter_names() {
        // Include cpu_, softirq_, syscall_ and other per-core metrics
        if metric_name.starts_with("cpu_") || 
           metric_name.starts_with("softirq_") ||
           metric_name.starts_with("syscall_") ||
           metric_name.starts_with("scheduler_") {
            // Try to get per-core data using "id" label
            for core_id in 0..256 {  // Check up to 256 cores
                let core_str = core_id.to_string();
                if let Some(collection) = tsdb.counters(metric_name, [("id", core_str.as_str())]) {
                    let rate = collection.rate();
                    let series = rate.sum();
                    if !series.inner.is_empty() {
                        core_metrics.entry(core_id)
                            .or_default()
                            .insert(metric_name.to_string(), Arc::new(series));
                    }
                }
            }
        }
    }
    
    // Also check gauges with per-core data
    for metric_name in tsdb.gauge_names() {
        if metric_name.starts_with("cpu_") || 
           metric_name.starts_with("softirq_") ||
           metric_name.starts_with("scheduler_") {
            for core_id in 0..256 {
                let core_str = core_id.to_string();
                if let Some(collection) = tsdb.gauges(metric_name, [("id", core_str.as_str())]) {
                    let untyped = collection.untyped();
                    let series = untyped.sum();
                    if !series.inner.is_empty() {
                        core_metrics.entry(core_id)
                            .or_default()
                            .insert(metric_name.to_string(), Arc::new(series));
                    }
                }
            }
        }
    }
    
    let total_cores = core_metrics.len();
    eprintln!("Found {} cores with metrics", total_cores);
    
    // Analyze inter-core correlations
    let mut inter_core_correlations = Vec::new();
    
    if total_cores > 1 {
        // Get all core pairs
        let core_ids: Vec<_> = core_metrics.keys().cloned().collect();
        let mut pairs = Vec::new();
        for i in 0..core_ids.len() {
            for j in i+1..core_ids.len() {
                pairs.push((core_ids[i], core_ids[j]));
            }
        }
        
        // Analyze correlations between cores for same metric
        let correlations: Vec<_> = pairs.par_iter()
            .flat_map(|(core1, core2)| {
                let metrics1 = &core_metrics[core1];
                let metrics2 = &core_metrics[core2];
                
                let mut results = Vec::new();
                
                // Find common metrics
                for (metric_name, series1) in metrics1 {
                    if let Some(series2) = metrics2.get(metric_name) {
                        if let Ok((corr, count)) = compute_correlation(series1, series2) {
                            if count >= 10 && corr.abs() >= 0.5 {
                                results.push(InterCoreCorrelation {
                                    core1: *core1,
                                    core2: *core2,
                                    metric: metric_name.clone(),
                                    correlation: corr,
                                    interpretation: interpret_cross_core_correlation(corr),
                                });
                            }
                        }
                    }
                }
                
                results
            })
            .collect();
        
        inter_core_correlations = correlations;
    }
    
    // Calculate core imbalance
    let core_imbalance_score = calculate_core_imbalance(&core_metrics)?;
    
    // Identify hottest cores
    let hottest_cores = identify_hottest_cores(&core_metrics)?;
    
    // Detect synchronization patterns
    let sync_patterns = detect_synchronization_patterns(&inter_core_correlations);
    
    Ok(CrossCoreAnalysis {
        total_cores_analyzed: total_cores,
        inter_core_correlations,
        core_imbalance_score,
        core_synchronization_patterns: sync_patterns,
        hottest_cores,
    })
}

fn detect_temporal_patterns(tsdb: &Arc<Tsdb>) -> Result<TemporalPatterns, Box<dyn std::error::Error>> {
    eprintln!("Detecting temporal patterns...");
    
    // This is a simplified implementation
    // In reality, we'd use FFT or autocorrelation for periodicity detection
    
    let periodic_patterns = Vec::new();
    let burst_patterns = Vec::new();
    
    // Analyze trends
    let mut increasing = Vec::new();
    let mut decreasing = Vec::new();
    let mut stable = Vec::new();
    
    // Sample implementation for trend detection
    for metric_name in tsdb.counter_names().iter().take(10) {
        if let Some(collection) = tsdb.counters(metric_name, ()) {
            let rate = collection.rate();
            let series = rate.sum();
            
            if let Some(trend) = calculate_trend(&series) {
                if trend > 0.1 {
                    increasing.push((metric_name.to_string(), trend));
                } else if trend < -0.1 {
                    decreasing.push((metric_name.to_string(), trend));
                } else {
                    stable.push(metric_name.to_string());
                }
            }
        }
    }
    
    Ok(TemporalPatterns {
        periodicity_detected: periodic_patterns,
        burst_patterns,
        trend_analysis: TrendAnalysis {
            increasing_metrics: increasing,
            decreasing_metrics: decreasing,
            stable_metrics: stable,
        },
    })
}

fn classify_workload(tsdb: &Arc<Tsdb>) -> Result<WorkloadClassification, Box<dyn std::error::Error>> {
    eprintln!("Classifying workload patterns...");
    
    let mut workload_mix = HashMap::new();
    
    // Analyze CPU vs I/O balance
    let cpu_intensity = calculate_cpu_intensity(tsdb)?;
    let io_intensity = calculate_io_intensity(tsdb)?;
    let network_intensity = calculate_network_intensity(tsdb)?;
    
    workload_mix.insert("CPU-bound".to_string(), cpu_intensity);
    workload_mix.insert("I/O-bound".to_string(), io_intensity);
    workload_mix.insert("Network-bound".to_string(), network_intensity);
    
    // Determine dominant type
    let dominant = workload_mix.iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "Mixed".to_string());
    
    Ok(WorkloadClassification {
        dominant_workload_type: dominant,
        workload_mix,
        phase_transitions: Vec::new(),
    })
}

fn calculate_advanced_metrics(tsdb: &Arc<Tsdb>) -> Result<AdvancedMetrics, Box<dyn std::error::Error>> {
    eprintln!("Calculating advanced metrics...");
    
    // Placeholder for advanced metrics
    // In a full implementation, we'd calculate:
    // - Mutual information between metric pairs
    // - Granger causality tests
    // - Outlier-specific correlations
    
    Ok(AdvancedMetrics {
        mutual_information_pairs: Vec::new(),
        causal_relationships: Vec::new(),
        outlier_correlations: Vec::new(),
    })
}

// Helper functions

fn compute_correlation(
    series1: &Arc<UntypedSeries>,
    series2: &Arc<UntypedSeries>,
) -> Result<(f64, usize), Box<dyn std::error::Error>> {
    let mut values1 = Vec::new();
    let mut values2 = Vec::new();
    
    let map2: HashMap<u64, f64> = series2.inner.iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    
    for (timestamp, value1) in series1.inner.iter() {
        if let Some(&value2) = map2.get(timestamp) {
            values1.push(*value1);
            values2.push(value2);
        }
    }
    
    if values1.len() < 3 {
        return Err("Not enough data points".into());
    }
    
    let n = values1.len() as f64;
    let mean1 = values1.iter().sum::<f64>() / n;
    let mean2 = values2.iter().sum::<f64>() / n;
    
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
    
    let correlation = if denominator1 > 0.0 && denominator2 > 0.0 {
        numerator / (denominator1.sqrt() * denominator2.sqrt())
    } else {
        0.0
    };
    
    Ok((correlation, values1.len()))
}

fn interpret_cross_core_correlation(corr: f64) -> String {
    if corr > 0.9 {
        "Cores highly synchronized - possible shared workload".to_string()
    } else if corr > 0.7 {
        "Strong core coupling - likely related tasks".to_string()
    } else if corr > 0.5 {
        "Moderate core interaction".to_string()
    } else if corr < -0.5 {
        "Cores anti-correlated - possible load balancing".to_string()
    } else {
        "Independent core behavior".to_string()
    }
}

fn calculate_core_imbalance(
    core_metrics: &HashMap<usize, HashMap<String, Arc<UntypedSeries>>>
) -> Result<f64, Box<dyn std::error::Error>> {
    if core_metrics.is_empty() {
        return Ok(0.0);
    }
    
    // Calculate average load per core
    let mut core_loads = Vec::new();
    
    for (_, metrics) in core_metrics {
        if let Some(usage) = metrics.get("cpu_usage") {
            let avg = usage.inner.values().sum::<f64>() / usage.inner.len() as f64;
            // Convert to percentage
            let percentage = (avg / 1_000_000_000.0 * 100.0).min(100.0);
            core_loads.push(percentage);
        }
    }
    
    if core_loads.is_empty() {
        return Ok(0.0);
    }
    
    let mean = core_loads.iter().sum::<f64>() / core_loads.len() as f64;
    let variance = core_loads.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / core_loads.len() as f64;
    
    // Coefficient of variation as imbalance score
    if mean > 0.0 {
        Ok((variance.sqrt() / mean).min(1.0))
    } else {
        Ok(0.0)
    }
}

fn identify_hottest_cores(
    core_metrics: &HashMap<usize, HashMap<String, Arc<UntypedSeries>>>
) -> Result<Vec<(usize, f64)>, Box<dyn std::error::Error>> {
    let mut core_temps = Vec::new();
    
    for (core_id, metrics) in core_metrics {
        if let Some(usage) = metrics.get("cpu_usage") {
            // CPU usage is likely in nanoseconds, convert to percentage
            let avg = usage.inner.values().sum::<f64>() / usage.inner.len() as f64;
            // Assuming values are in nanoseconds per second, convert to percentage
            let percentage = (avg / 1_000_000_000.0 * 100.0).min(100.0);
            core_temps.push((*core_id, percentage));
        }
    }
    
    core_temps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    Ok(core_temps.into_iter().take(5).collect())
}

fn detect_synchronization_patterns(correlations: &[InterCoreCorrelation]) -> Vec<String> {
    let mut patterns = Vec::new();
    
    // Group highly correlated cores
    let high_corr: Vec<_> = correlations.iter()
        .filter(|c| c.correlation > 0.9)
        .collect();
    
    if !high_corr.is_empty() {
        patterns.push(format!("Found {} core pairs with high synchronization", high_corr.len()));
    }
    
    // Check for symmetric patterns
    let mut core_pairs: HashSet<(usize, usize)> = HashSet::new();
    for corr in correlations {
        let pair = if corr.core1 < corr.core2 {
            (corr.core1, corr.core2)
        } else {
            (corr.core2, corr.core1)
        };
        core_pairs.insert(pair);
    }
    
    if core_pairs.len() > 10 {
        patterns.push("Complex multi-core interaction detected".to_string());
    }
    
    patterns
}

fn calculate_trend(series: &UntypedSeries) -> Option<f64> {
    if series.inner.len() < 10 {
        return None;
    }
    
    let values: Vec<f64> = series.inner.values().cloned().collect();
    let n = values.len() as f64;
    
    // Simple linear regression for trend
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = values.iter().sum::<f64>() / n;
    
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    
    for (i, y) in values.iter().enumerate() {
        let x = i as f64;
        numerator += (x - x_mean) * (y - y_mean);
        denominator += (x - x_mean).powi(2);
    }
    
    if denominator > 0.0 {
        Some(numerator / denominator)
    } else {
        None
    }
}

fn calculate_cpu_intensity(tsdb: &Arc<Tsdb>) -> Result<f64, Box<dyn std::error::Error>> {
    if let Some(collection) = tsdb.counters("cpu_usage", ()) {
        let rate = collection.rate();
        let series = rate.sum();
        let avg = series.inner.values().sum::<f64>() / series.inner.len().max(1) as f64;
        Ok((avg / 100.0).min(1.0))
    } else {
        Ok(0.0)
    }
}

fn calculate_io_intensity(tsdb: &Arc<Tsdb>) -> Result<f64, Box<dyn std::error::Error>> {
    let mut io_score = 0.0;
    let mut count = 0;
    
    for metric in ["blockio_read", "blockio_write"] {
        if let Some(collection) = tsdb.counters(metric, ()) {
            let rate = collection.rate();
            let series = rate.sum();
            if !series.inner.is_empty() {
                io_score += series.inner.values().sum::<f64>() / series.inner.len() as f64;
                count += 1;
            }
        }
    }
    
    if count > 0 {
        Ok((io_score / (count as f64 * 1_000_000.0)).min(1.0)) // Normalize to 0-1
    } else {
        Ok(0.0)
    }
}

fn calculate_network_intensity(tsdb: &Arc<Tsdb>) -> Result<f64, Box<dyn std::error::Error>> {
    let mut net_score = 0.0;
    let mut count = 0;
    
    for metric in ["network_rx_bytes", "network_tx_bytes"] {
        if let Some(collection) = tsdb.counters(metric, ()) {
            let rate = collection.rate();
            let series = rate.sum();
            if !series.inner.is_empty() {
                net_score += series.inner.values().sum::<f64>() / series.inner.len() as f64;
                count += 1;
            }
        }
    }
    
    if count > 0 {
        Ok((net_score / (count as f64 * 1_000_000_000.0)).min(1.0)) // Normalize to 0-1
    } else {
        Ok(0.0)
    }
}

impl DeepAnalysisReport {
    pub fn to_detailed_summary(&self) -> String {
        let mut s = String::new();
        
        s.push_str("🔬 DEEP CORRELATION ANALYSIS REPORT\n");
        s.push_str("====================================\n\n");
        
        // Cross-core analysis
        s.push_str(&format!("🖥️ CROSS-CORE ANALYSIS:\n"));
        s.push_str(&format!("  • Cores analyzed: {}\n", self.cross_core_analysis.total_cores_analyzed));
        s.push_str(&format!("  • Core imbalance score: {:.2}%\n", self.cross_core_analysis.core_imbalance_score * 100.0));
        s.push_str(&format!("  • Inter-core correlations found: {}\n", self.cross_core_analysis.inter_core_correlations.len()));
        
        if !self.cross_core_analysis.hottest_cores.is_empty() {
            s.push_str("\n  Hottest cores:\n");
            for (core, load) in &self.cross_core_analysis.hottest_cores {
                s.push_str(&format!("    Core {}: {:.1}% avg usage\n", core, load));
            }
        }
        
        if !self.cross_core_analysis.inter_core_correlations.is_empty() {
            s.push_str("\n  Top inter-core correlations:\n");
            for corr in self.cross_core_analysis.inter_core_correlations.iter().take(5) {
                s.push_str(&format!("    Core {} <-> Core {} on {} (r={:.3})\n    {}\n",
                    corr.core1, corr.core2, corr.metric, corr.correlation, corr.interpretation));
            }
        }
        
        // Temporal patterns
        s.push_str(&format!("\n⏱️ TEMPORAL PATTERNS:\n"));
        
        if !self.temporal_patterns.trend_analysis.increasing_metrics.is_empty() {
            s.push_str("  Increasing trends:\n");
            for (metric, trend) in &self.temporal_patterns.trend_analysis.increasing_metrics {
                s.push_str(&format!("    {} (trend: {:.3})\n", metric, trend));
            }
        }
        
        if !self.temporal_patterns.trend_analysis.decreasing_metrics.is_empty() {
            s.push_str("  Decreasing trends:\n");
            for (metric, trend) in &self.temporal_patterns.trend_analysis.decreasing_metrics {
                s.push_str(&format!("    {} (trend: {:.3})\n", metric, trend));
            }
        }
        
        // Workload classification
        s.push_str(&format!("\n📊 WORKLOAD CLASSIFICATION:\n"));
        s.push_str(&format!("  • Dominant type: {}\n", self.workload_classification.dominant_workload_type));
        s.push_str("  • Workload mix:\n");
        for (wtype, intensity) in &self.workload_classification.workload_mix {
            s.push_str(&format!("    {}: {:.1}%\n", wtype, intensity * 100.0));
        }
        
        s.push_str(&format!("\n⏰ Total analysis time: {}ms\n", self.total_analysis_time_ms));
        
        s
    }
}