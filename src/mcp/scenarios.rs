use crate::viewer::promql::QueryEngine;
use crate::viewer::tsdb::Tsdb;
use std::sync::Arc;

use super::anomaly::{detect_anomalies, AnomalyMethod};
use super::correlation::calculate_correlation_with_names;
use super::fft_analysis::analyze_fft_patterns;

/// High-level analysis scenarios that combine multiple techniques
pub struct ScenarioAnalyzer {
    engine: Arc<QueryEngine>,
    tsdb: Arc<Tsdb>,
}

impl ScenarioAnalyzer {
    pub fn new(tsdb: Arc<Tsdb>) -> Self {
        let engine = Arc::new(QueryEngine::new(Arc::clone(&tsdb)));
        Self { engine, tsdb }
    }

    /// Analyze CPU performance issues
    /// Checks for: throttling, frequency scaling issues, scheduling problems
    pub fn analyze_cpu_performance(&self) -> Result<CPUAnalysisReport, Box<dyn std::error::Error>> {
        let (start, end) = self.engine.get_time_range();
        let step = 60.0;
        
        let mut report = CPUAnalysisReport::default();
        
        // 1. Check CPU utilization patterns
        // cpu_usage is in nanoseconds, divide by 1e9 to get fraction
        if let Ok(result) = self.engine.query_range("avg(irate(cpu_usage[5m])) / 1000000000", start, end, step) {
            report.avg_utilization = Self::extract_average(&result);
        }
        
        // 2. Detect CPU throttling via frequency analysis
        let freq_query = "irate(cpu_tsc[5m]) * irate(cpu_aperf[5m]) / irate(cpu_mperf[5m]) / cpu_cores";
        if let Ok(anomalies) = detect_anomalies(
            &self.engine, 
            freq_query,
            Some("CPU Frequency"),
            AnomalyMethod::ZScore,
            2.5,
            start,
            end,
            step
        ) {
            report.frequency_drops = anomalies.anomalies.len();
            report.likely_throttling = anomalies.anomalies.len() > 5;
        }
        
        // 3. Check for periodic patterns (could indicate batch jobs or GC)
        if let Ok(fft_result) = analyze_fft_patterns(
            &self.engine,
            "sum(irate(cpu_usage[5m]))",
            Some("Total CPU Usage"),
            start,
            end,
            step
        ) {
            report.periodic_patterns = fft_result.dominant_frequencies
                .into_iter()
                .filter(|f| f.confidence > 2.0)
                .map(|f| (f.period_seconds, f.confidence))
                .collect();
        }
        
        // 4. Check scheduler efficiency
        if let Ok(result) = self.engine.query_range("avg(scheduler_run_queue_latency)", start, end, step) {
            report.avg_runqueue_latency = Self::extract_average(&result);
        }
        
        // 5. Correlation between CPU and memory (might indicate swapping)
        if let Ok(corr) = calculate_correlation_with_names(
            &self.engine,
            "sum(irate(cpu_usage[5m]))",
            "memory_used",
            Some("CPU Usage"),
            Some("Memory Used"),
            start,
            end,
            step
        ) {
            report.cpu_memory_correlation = corr.correlation;
        }
        
        Ok(report)
    }

    /// Analyze container/cgroup performance
    pub fn analyze_cgroup_performance(
        &self, 
        cgroup_name: &str
    ) -> Result<CgroupAnalysisReport, Box<dyn std::error::Error>> {
        let (start, end) = self.engine.get_time_range();
        let step = 60.0;
        
        let mut report = CgroupAnalysisReport {
            cgroup_name: cgroup_name.to_string(),
            ..Default::default()
        };
        
        // 1. CPU usage and throttling
        let cpu_query = format!("sum(irate(cgroup_cpu_usage{{name=\"{}\"}}[5m]))", cgroup_name);
        if let Ok(result) = self.engine.query_range(&cpu_query, start, end, step) {
            report.cpu_usage = Self::extract_average(&result);
        }
        
        let throttle_query = format!("cgroup_cpu_throttled{{name=\"{}\"}}", cgroup_name);
        if let Ok(result) = self.engine.query_range(&throttle_query, start, end, step) {
            report.throttled_periods = Self::extract_max(&result) as usize;
        }
        
        // 2. Memory usage and limits
        let mem_query = format!("cgroup_memory_usage{{name=\"{}\"}}", cgroup_name);
        if let Ok(result) = self.engine.query_range(&mem_query, start, end, step) {
            report.memory_usage = Self::extract_average(&result);
        }
        
        // 3. Detect memory pressure via anomalies
        if let Ok(anomalies) = detect_anomalies(
            &self.engine,
            &mem_query,
            Some(&format!("{} Memory", cgroup_name)),
            AnomalyMethod::InterquartileRange,
            1.5,
            start,
            end,
            step
        ) {
            report.memory_spikes = anomalies.anomalies.len();
        }
        
        // 4. Check for periodic behavior (GC patterns, batch processing)
        if let Ok(fft_result) = analyze_fft_patterns(
            &self.engine,
            &cpu_query,
            Some(&format!("{} CPU", cgroup_name)),
            start,
            end,
            step
        ) {
            report.periodic_patterns = fft_result.dominant_frequencies
                .into_iter()
                .filter(|f| f.confidence > 2.0)
                .map(|f| (f.period_seconds, f.confidence))
                .collect();
        }
        
        // 5. Compare to system-wide metrics
        if let Ok(corr) = calculate_correlation_with_names(
            &self.engine,
            &cpu_query,
            "sum(irate(cpu_usage[5m]))",
            Some(&format!("{} CPU", cgroup_name)),
            Some("System CPU"),
            start,
            end,
            step
        ) {
            report.correlation_with_system = corr.correlation;
        }
        
        Ok(report)
    }

    /// Analyze network performance issues
    pub fn analyze_network_performance(&self) -> Result<NetworkAnalysisReport, Box<dyn std::error::Error>> {
        let (start, end) = self.engine.get_time_range();
        let step = 60.0;
        
        let mut report = NetworkAnalysisReport::default();
        
        // 1. Overall throughput
        if let Ok(tx_result) = self.engine.query_range(
            "sum(irate(network_transmit_bytes[5m]))", 
            start, end, step
        ) {
            report.avg_tx_throughput = Self::extract_average(&tx_result);
        }
        
        if let Ok(rx_result) = self.engine.query_range(
            "sum(irate(network_receive_bytes[5m]))", 
            start, end, step
        ) {
            report.avg_rx_throughput = Self::extract_average(&rx_result);
        }
        
        // 2. Retransmission analysis
        if let Ok(result) = self.engine.query_range("irate(tcp_retransmit[5m])", start, end, step) {
            report.retransmit_rate = Self::extract_average(&result);
        }
        
        // 3. Detect network bursts
        if let Ok(anomalies) = detect_anomalies(
            &self.engine,
            "sum(irate(network_transmit_bytes[5m]))",
            Some("Network TX"),
            AnomalyMethod::ZScore,
            3.0,
            start,
            end,
            step
        ) {
            report.traffic_bursts = anomalies.anomalies.len();
        }
        
        // 4. Check for periodic traffic patterns
        if let Ok(fft_result) = analyze_fft_patterns(
            &self.engine,
            "sum(irate(network_transmit_bytes[5m]) + irate(network_receive_bytes[5m]))",
            Some("Total Network Traffic"),
            start,
            end,
            step
        ) {
            report.periodic_patterns = fft_result.dominant_frequencies
                .into_iter()
                .filter(|f| f.confidence > 2.0)
                .map(|f| (f.period_seconds, f.confidence))
                .collect();
        }
        
        // 5. TCP latency if available
        if let Ok(result) = self.engine.query_range(
            "histogram_quantile(0.99, tcp_packet_latency[5m])", 
            start, end, step
        ) {
            report.p99_latency = Self::extract_average(&result);
        }
        
        Ok(report)
    }

    /// Analyze latency and responsiveness issues
    pub fn analyze_latency_issues(&self) -> Result<LatencyAnalysisReport, Box<dyn std::error::Error>> {
        let (start, end) = self.engine.get_time_range();
        let step = 60.0;
        
        let mut report = LatencyAnalysisReport::default();
        
        // 1. Disk I/O latency
        if let Ok(result) = self.engine.query_range(
            "histogram_quantile(0.99, block_io_request_latency[5m])",
            start, end, step
        ) {
            report.disk_p99_latency = Self::extract_average(&result);
        }
        
        // 2. Scheduler latency
        if let Ok(result) = self.engine.query_range(
            "avg(scheduler_run_queue_latency)",
            start, end, step
        ) {
            report.scheduler_latency = Self::extract_average(&result);
        }
        
        // 3. System call latency
        if let Ok(result) = self.engine.query_range(
            "histogram_quantile(0.99, syscall_latency[5m])",
            start, end, step
        ) {
            report.syscall_p99_latency = Self::extract_average(&result);
        }
        
        // 4. Find correlation between latency and load
        if let Ok(corr) = calculate_correlation_with_names(
            &self.engine,
            "histogram_quantile(0.99, block_io_request_latency[5m])",
            "sum(irate(cpu_usage[5m]))",
            Some("Disk Latency"),
            Some("CPU Usage"),
            start,
            end,
            step
        ) {
            report.latency_load_correlation = corr.correlation;
        }
        
        // 5. Detect latency spikes
        let latency_metrics = [
            ("block_io_request_latency", "Disk I/O"),
            ("tcp_packet_latency", "Network"),
            ("syscall_latency", "System Call"),
        ];
        
        for (metric, name) in &latency_metrics {
            let query = format!("histogram_quantile(0.99, {}[5m])", metric);
            if let Ok(anomalies) = detect_anomalies(
                &self.engine,
                &query,
                Some(name),
                AnomalyMethod::ZScore,
                2.5,
                start,
                end,
                step
            ) {
                report.latency_spikes.push((
                    name.to_string(),
                    anomalies.anomalies.len()
                ));
            }
        }
        
        Ok(report)
    }

    /// Memory pressure analysis
    pub fn analyze_memory_pressure(&self) -> Result<MemoryAnalysisReport, Box<dyn std::error::Error>> {
        let (start, end) = self.engine.get_time_range();
        let step = 60.0;
        
        let mut report = MemoryAnalysisReport::default();
        
        // 1. Memory utilization
        if let Ok(result) = self.engine.query_range("memory_used / memory_total", start, end, step) {
            report.avg_utilization = Self::extract_average(&result);
        }
        
        // 2. Page fault rate
        if let Ok(result) = self.engine.query_range("irate(vm_page_fault[5m])", start, end, step) {
            report.page_fault_rate = Self::extract_average(&result);
        }
        
        // 3. Detect memory leaks (steadily increasing usage)
        if let Ok(result) = self.engine.query_range("memory_used", start, end, step) {
            // Simple linear regression to detect trend
            if let Ok(values) = Self::extract_time_series(&result) {
                let trend = Self::calculate_trend(&values);
                report.memory_growth_rate = trend;
                report.possible_leak = trend > 1_000_000.0; // Growing > 1MB/sec
            }
        }
        
        // 4. OOM kill events
        if let Ok(result) = self.engine.query_range("vm_oom_kill", start, end, step) {
            report.oom_kills = Self::extract_max(&result) as usize;
        }
        
        // 5. Correlation with swap usage
        if let Ok(corr) = calculate_correlation_with_names(
            &self.engine,
            "memory_used",
            "memory_swap_used",
            Some("Memory Used"),
            Some("Swap Used"),
            start,
            end,
            step
        ) {
            report.swap_correlation = corr.correlation;
        }
        
        Ok(report)
    }

    // Helper methods
    fn extract_average(result: &crate::viewer::promql::QueryResult) -> f64 {
        use crate::viewer::promql::QueryResult;
        match result {
            QueryResult::Matrix { result } => {
                if !result.is_empty() && !result[0].values.is_empty() {
                    let sum: f64 = result[0].values.iter().map(|(_, v)| v).sum();
                    sum / result[0].values.len() as f64
                } else {
                    0.0
                }
            }
            QueryResult::Vector { result } => {
                if !result.is_empty() {
                    result[0].value.1
                } else {
                    0.0
                }
            }
            QueryResult::Scalar { result } => result.1,
        }
    }

    fn extract_max(result: &crate::viewer::promql::QueryResult) -> f64 {
        use crate::viewer::promql::QueryResult;
        match result {
            QueryResult::Matrix { result } => {
                if !result.is_empty() && !result[0].values.is_empty() {
                    result[0].values.iter()
                        .map(|(_, v)| *v)
                        .fold(f64::NEG_INFINITY, f64::max)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    fn extract_time_series(result: &crate::viewer::promql::QueryResult) -> Result<Vec<(f64, f64)>, Box<dyn std::error::Error>> {
        use crate::viewer::promql::QueryResult;
        match result {
            QueryResult::Matrix { result } => {
                if !result.is_empty() {
                    Ok(result[0].values.clone())
                } else {
                    Err("No data".into())
                }
            }
            _ => Err("Not a matrix result".into()),
        }
    }

    fn calculate_trend(values: &[(f64, f64)]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        
        // Simple linear regression
        let n = values.len() as f64;
        let sum_x: f64 = values.iter().map(|(t, _)| t).sum();
        let sum_y: f64 = values.iter().map(|(_, v)| v).sum();
        let sum_xy: f64 = values.iter().map(|(t, v)| t * v).sum();
        let sum_xx: f64 = values.iter().map(|(t, _)| t * t).sum();
        
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
        slope
    }
}

// Report structures

#[derive(Debug, Default)]
pub struct CPUAnalysisReport {
    pub avg_utilization: f64,
    pub frequency_drops: usize,
    pub likely_throttling: bool,
    pub periodic_patterns: Vec<(f64, f64)>, // (period_seconds, confidence)
    pub avg_runqueue_latency: f64,
    pub cpu_memory_correlation: f64,
}

#[derive(Debug, Default)]
pub struct CgroupAnalysisReport {
    pub cgroup_name: String,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub throttled_periods: usize,
    pub memory_spikes: usize,
    pub periodic_patterns: Vec<(f64, f64)>,
    pub correlation_with_system: f64,
}

#[derive(Debug, Default)]
pub struct NetworkAnalysisReport {
    pub avg_tx_throughput: f64,
    pub avg_rx_throughput: f64,
    pub retransmit_rate: f64,
    pub traffic_bursts: usize,
    pub periodic_patterns: Vec<(f64, f64)>,
    pub p99_latency: f64,
}

#[derive(Debug, Default)]
pub struct LatencyAnalysisReport {
    pub disk_p99_latency: f64,
    pub scheduler_latency: f64,
    pub syscall_p99_latency: f64,
    pub latency_load_correlation: f64,
    pub latency_spikes: Vec<(String, usize)>, // (metric_name, spike_count)
}

#[derive(Debug, Default)]
pub struct MemoryAnalysisReport {
    pub avg_utilization: f64,
    pub page_fault_rate: f64,
    pub memory_growth_rate: f64,
    pub possible_leak: bool,
    pub oom_kills: usize,
    pub swap_correlation: f64,
}

// Format reports for display

impl CPUAnalysisReport {
    pub fn format(&self) -> String {
        let mut output = String::from("CPU Performance Analysis\n");
        output.push_str("========================\n\n");
        
        output.push_str(&format!("Average Utilization: {:.2}%\n", self.avg_utilization * 100.0));
        output.push_str(&format!("Runqueue Latency: {:.2} μs\n", self.avg_runqueue_latency));
        
        if self.likely_throttling {
            output.push_str(&format!("\n⚠️  CPU Throttling Detected!\n"));
            output.push_str(&format!("   Frequency drops: {}\n", self.frequency_drops));
        }
        
        if !self.periodic_patterns.is_empty() {
            output.push_str("\nPeriodic Patterns Found:\n");
            for (period, confidence) in &self.periodic_patterns {
                output.push_str(&format!("  • {:.1}s period (confidence: {:.1})\n", period, confidence));
            }
        }
        
        if self.cpu_memory_correlation.abs() > 0.7 {
            output.push_str(&format!("\n⚠️  High CPU-Memory correlation ({:.2}) - possible swapping\n", 
                self.cpu_memory_correlation));
        }
        
        output
    }
}

impl CgroupAnalysisReport {
    pub fn format(&self) -> String {
        let mut output = format!("Cgroup '{}' Performance Analysis\n", self.cgroup_name);
        output.push_str("=====================================\n\n");
        
        output.push_str(&format!("CPU Usage: {:.2}%\n", self.cpu_usage * 100.0));
        output.push_str(&format!("Memory Usage: {:.2} MB\n", self.memory_usage / 1_000_000.0));
        
        if self.throttled_periods > 0 {
            output.push_str(&format!("\n⚠️  CPU Throttling: {} periods\n", self.throttled_periods));
        }
        
        if self.memory_spikes > 0 {
            output.push_str(&format!("⚠️  Memory Spikes: {}\n", self.memory_spikes));
        }
        
        if !self.periodic_patterns.is_empty() {
            output.push_str("\nPeriodic Patterns (likely GC or batch jobs):\n");
            for (period, confidence) in &self.periodic_patterns {
                output.push_str(&format!("  • {:.1}s period (confidence: {:.1})\n", period, confidence));
            }
        }
        
        output.push_str(&format!("\nCorrelation with system CPU: {:.2}\n", self.correlation_with_system));
        
        output
    }
}

impl NetworkAnalysisReport {
    pub fn format(&self) -> String {
        let mut output = String::from("Network Performance Analysis\n");
        output.push_str("============================\n\n");
        
        output.push_str(&format!("Avg TX: {:.2} MB/s\n", self.avg_tx_throughput / 1_000_000.0));
        output.push_str(&format!("Avg RX: {:.2} MB/s\n", self.avg_rx_throughput / 1_000_000.0));
        output.push_str(&format!("Retransmit Rate: {:.2}/s\n", self.retransmit_rate));
        
        if self.p99_latency > 0.0 {
            output.push_str(&format!("P99 Latency: {:.2} ms\n", self.p99_latency * 1000.0));
        }
        
        if self.traffic_bursts > 0 {
            output.push_str(&format!("\n⚠️  Traffic Bursts Detected: {}\n", self.traffic_bursts));
        }
        
        if !self.periodic_patterns.is_empty() {
            output.push_str("\nPeriodic Traffic Patterns:\n");
            for (period, confidence) in &self.periodic_patterns {
                output.push_str(&format!("  • {:.1}s period (confidence: {:.1})\n", period, confidence));
            }
        }
        
        output
    }
}

impl LatencyAnalysisReport {
    pub fn format(&self) -> String {
        let mut output = String::from("Latency Analysis\n");
        output.push_str("================\n\n");
        
        if self.disk_p99_latency > 0.0 {
            output.push_str(&format!("Disk P99: {:.2} ms\n", self.disk_p99_latency * 1000.0));
        }
        if self.scheduler_latency > 0.0 {
            output.push_str(&format!("Scheduler: {:.2} μs\n", self.scheduler_latency));
        }
        if self.syscall_p99_latency > 0.0 {
            output.push_str(&format!("Syscall P99: {:.2} μs\n", self.syscall_p99_latency));
        }
        
        if !self.latency_spikes.is_empty() {
            output.push_str("\nLatency Spikes Detected:\n");
            for (metric, count) in &self.latency_spikes {
                output.push_str(&format!("  • {}: {} spikes\n", metric, count));
            }
        }
        
        if self.latency_load_correlation.abs() > 0.7 {
            output.push_str(&format!("\n⚠️  Latency correlates with load (r={:.2})\n", 
                self.latency_load_correlation));
        }
        
        output
    }
}

impl MemoryAnalysisReport {
    pub fn format(&self) -> String {
        let mut output = String::from("Memory Pressure Analysis\n");
        output.push_str("========================\n\n");
        
        output.push_str(&format!("Avg Utilization: {:.2}%\n", self.avg_utilization * 100.0));
        output.push_str(&format!("Page Fault Rate: {:.2}/s\n", self.page_fault_rate));
        
        if self.possible_leak {
            output.push_str(&format!("\n⚠️  Possible Memory Leak Detected!\n"));
            output.push_str(&format!("   Growth rate: {:.2} MB/hour\n", 
                self.memory_growth_rate * 3600.0 / 1_000_000.0));
        }
        
        if self.oom_kills > 0 {
            output.push_str(&format!("\n⚠️  OOM Kills: {}\n", self.oom_kills));
        }
        
        if self.swap_correlation > 0.7 {
            output.push_str(&format!("\n⚠️  High swap usage correlation ({:.2})\n", self.swap_correlation));
        }
        
        output
    }
}