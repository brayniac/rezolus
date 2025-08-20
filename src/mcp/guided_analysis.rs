use crate::viewer::promql::QueryEngine;
use crate::viewer::tsdb::Tsdb;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

/// A guided analysis system that provides hierarchical, drill-down analysis
/// Designed to be LLM-friendly with clear decision trees and actionable insights
pub struct GuidedAnalyzer {
    engine: Arc<QueryEngine>,
    tsdb: Arc<Tsdb>,
}

impl GuidedAnalyzer {
    pub fn new(tsdb: Arc<Tsdb>) -> Self {
        let engine = Arc::new(QueryEngine::new(Arc::clone(&tsdb)));
        Self { engine, tsdb }
    }

    /// Start with system overview and identify problem areas
    pub fn analyze_system_health(&self) -> SystemHealthReport {
        let (start, end) = self.engine.get_time_range();
        let step = 60.0;
        
        let mut report = SystemHealthReport::default();
        
        // High-level health indicators
        report.duration_seconds = end - start;
        report.timestamp_start = start;
        report.timestamp_end = end;
        
        // CPU subsystem health
        // cpu_usage is in nanoseconds, divide by 1e9 to get utilization fraction
        if let Ok(result) = self.engine.query_range("avg(irate(cpu_usage[5m])) / cpu_cores / 1000000000", start, end, step) {
            report.cpu_health.utilization = Self::extract_average(&result);
            report.cpu_health.status = Self::assess_cpu_status(report.cpu_health.utilization);
        }
        
        // Memory subsystem health
        if let Ok(result) = self.engine.query_range("memory_used / memory_total", start, end, step) {
            report.memory_health.utilization = Self::extract_average(&result);
            report.memory_health.status = Self::assess_memory_status(report.memory_health.utilization);
        }
        
        // Network subsystem health
        if let Ok(result) = self.engine.query_range("irate(tcp_retransmit[5m])", start, end, step) {
            let retransmit_rate = Self::extract_average(&result);
            report.network_health.retransmit_rate = retransmit_rate;
            report.network_health.status = Self::assess_network_status(retransmit_rate);
        }
        
        // I/O subsystem health
        if let Ok(result) = self.engine.query_range(
            "histogram_quantile(0.99, block_io_request_latency[5m])", 
            start, end, step
        ) {
            let p99_latency = Self::extract_average(&result);
            report.io_health.latency_p99 = p99_latency;
            report.io_health.status = Self::assess_io_status(p99_latency);
        }
        
        // Identify top issues
        report.identify_problem_areas();
        
        report
    }

    /// Drill down into a specific subsystem based on initial findings
    pub fn drill_down(&self, subsystem: &str, context: &DrillDownContext) -> DrillDownReport {
        match subsystem {
            "cpu" => self.drill_down_cpu(context),
            "memory" => self.drill_down_memory(context),
            "network" => self.drill_down_network(context),
            "io" => self.drill_down_io(context),
            "container" => self.drill_down_container(context),
            _ => DrillDownReport::NotFound,
        }
    }

    fn drill_down_cpu(&self, context: &DrillDownContext) -> DrillDownReport {
        let (start, end) = (context.start_time, context.end_time);
        let step = 60.0;
        
        let mut findings = Vec::new();
        let mut recommendations = Vec::new();
        let mut next_steps = Vec::new();
        
        // Check for CPU throttling
        if let Ok(result) = self.engine.query_range(
            "irate(cpu_tsc[5m]) * irate(cpu_aperf[5m]) / irate(cpu_mperf[5m]) / cpu_cores",
            start, end, step
        ) {
            use super::anomaly::{detect_anomalies, AnomalyMethod};
            if let Ok(anomalies) = detect_anomalies(
                &self.engine,
                "irate(cpu_tsc[5m]) * irate(cpu_aperf[5m]) / irate(cpu_mperf[5m]) / cpu_cores",
                Some("CPU Frequency"),
                AnomalyMethod::ZScore,
                2.0,
                start,
                end,
                step
            ) {
                if anomalies.anomalies.len() > 5 {
                    findings.push(Finding {
                        severity: Severity::High,
                        category: "CPU Throttling".to_string(),
                        description: format!("Detected {} frequency drops, indicating thermal or power throttling", 
                                           anomalies.anomalies.len()),
                        evidence: vec![
                            format!("Frequency anomalies: {}", anomalies.anomalies.len()),
                            format!("Avg frequency: {:.2} GHz", anomalies.statistics.mean / 1e9),
                        ],
                    });
                    recommendations.push("Check CPU temperature and cooling".to_string());
                    recommendations.push("Review power management settings".to_string());
                    next_steps.push("analyze_thermal_patterns".to_string());
                }
            }
        }
        
        // Check for uneven CPU distribution
        if context.detailed {
            if let Ok(result) = self.engine.query_range("stddev(irate(cpu_usage[5m]))", start, end, step) {
                let cpu_stddev = Self::extract_average(&result);
                if cpu_stddev > 0.2 {
                    findings.push(Finding {
                        severity: Severity::Medium,
                        category: "CPU Imbalance".to_string(),
                        description: "High variance in per-CPU usage indicates poor load distribution".to_string(),
                        evidence: vec![
                            format!("CPU usage stddev: {:.2}", cpu_stddev),
                        ],
                    });
                    recommendations.push("Review CPU affinity settings".to_string());
                    recommendations.push("Check for single-threaded bottlenecks".to_string());
                    next_steps.push("analyze_per_cpu_patterns".to_string());
                }
            }
        }
        
        // Check scheduler efficiency
        if let Ok(result) = self.engine.query_range("avg(scheduler_run_queue_latency)", start, end, step) {
            let runqueue_latency = Self::extract_average(&result);
            if runqueue_latency > 1000.0 { // > 1ms
                findings.push(Finding {
                    severity: Severity::Medium,
                    category: "Scheduler Pressure".to_string(),
                    description: "High runqueue latency indicates CPU contention".to_string(),
                    evidence: vec![
                        format!("Avg runqueue latency: {:.2} μs", runqueue_latency),
                    ],
                });
                recommendations.push("Consider increasing CPU resources".to_string());
                recommendations.push("Review process priorities".to_string());
                next_steps.push("analyze_process_scheduling".to_string());
            }
        }
        
        DrillDownReport::CPU(CPUDrillDown {
            findings,
            recommendations,
            next_steps,
            metrics_to_monitor: vec![
                "irate(cpu_usage[5m])".to_string(),
                "scheduler_run_queue_latency".to_string(),
                "cpu_frequency".to_string(),
            ],
        })
    }

    fn drill_down_memory(&self, context: &DrillDownContext) -> DrillDownReport {
        let (start, end) = (context.start_time, context.end_time);
        let step = 60.0;
        
        let mut findings = Vec::new();
        let mut recommendations = Vec::new();
        let mut next_steps = Vec::new();
        
        // Check for memory leaks
        if let Ok(result) = self.engine.query_range("memory_used", start, end, step) {
            if let Ok(values) = Self::extract_time_series(&result) {
                let trend = Self::calculate_trend(&values);
                if trend > 1_000_000.0 { // Growing > 1MB/sec
                    findings.push(Finding {
                        severity: Severity::Critical,
                        category: "Memory Leak".to_string(),
                        description: "Memory usage shows continuous growth pattern".to_string(),
                        evidence: vec![
                            format!("Growth rate: {:.2} MB/hour", trend * 3600.0 / 1_000_000.0),
                        ],
                    });
                    recommendations.push("Identify processes with growing memory".to_string());
                    recommendations.push("Review application memory management".to_string());
                    next_steps.push("analyze_per_process_memory".to_string());
                }
            }
        }
        
        // Check swap usage
        if let Ok(result) = self.engine.query_range("memory_swap_used", start, end, step) {
            let swap_usage = Self::extract_average(&result);
            if swap_usage > 100_000_000.0 { // > 100MB
                findings.push(Finding {
                    severity: Severity::High,
                    category: "Swap Activity".to_string(),
                    description: "Significant swap usage detected".to_string(),
                    evidence: vec![
                        format!("Avg swap used: {:.2} MB", swap_usage / 1_000_000.0),
                    ],
                });
                recommendations.push("Increase available memory".to_string());
                recommendations.push("Reduce memory pressure from applications".to_string());
                next_steps.push("analyze_swap_patterns".to_string());
            }
        }
        
        // Check page fault rate
        if let Ok(result) = self.engine.query_range("irate(vm_page_fault[5m])", start, end, step) {
            let fault_rate = Self::extract_average(&result);
            if fault_rate > 1000.0 { // > 1000/sec
                findings.push(Finding {
                    severity: Severity::Medium,
                    category: "Page Faults".to_string(),
                    description: "High page fault rate indicates memory pressure".to_string(),
                    evidence: vec![
                        format!("Page fault rate: {:.0}/sec", fault_rate),
                    ],
                });
                recommendations.push("Review memory allocation patterns".to_string());
                next_steps.push("analyze_memory_access_patterns".to_string());
            }
        }
        
        DrillDownReport::Memory(MemoryDrillDown {
            findings,
            recommendations,
            next_steps,
            metrics_to_monitor: vec![
                "memory_used".to_string(),
                "memory_swap_used".to_string(),
                "irate(vm_page_fault[5m])".to_string(),
            ],
        })
    }

    fn drill_down_network(&self, context: &DrillDownContext) -> DrillDownReport {
        let (start, end) = (context.start_time, context.end_time);
        let step = 60.0;
        
        let mut findings = Vec::new();
        let mut recommendations = Vec::new();
        let mut next_steps = Vec::new();
        
        // Check retransmission patterns
        if let Ok(result) = self.engine.query_range("irate(tcp_retransmit[5m])", start, end, step) {
            let retransmit_rate = Self::extract_average(&result);
            let max_retransmit = Self::extract_max(&result);
            
            if retransmit_rate > 100.0 { // > 100/sec average
                findings.push(Finding {
                    severity: Severity::High,
                    category: "Network Reliability".to_string(),
                    description: "High TCP retransmission rate indicates network issues".to_string(),
                    evidence: vec![
                        format!("Avg retransmit rate: {:.0}/sec", retransmit_rate),
                        format!("Peak retransmit rate: {:.0}/sec", max_retransmit),
                    ],
                });
                recommendations.push("Check network connectivity and packet loss".to_string());
                recommendations.push("Review network congestion".to_string());
                next_steps.push("analyze_network_errors".to_string());
            }
        }
        
        // Check for traffic bursts
        use super::anomaly::{detect_anomalies, AnomalyMethod};
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
            if anomalies.anomalies.len() > 5 {
                findings.push(Finding {
                    severity: Severity::Medium,
                    category: "Traffic Bursts".to_string(),
                    description: "Detected network traffic spikes".to_string(),
                    evidence: vec![
                        format!("Number of bursts: {}", anomalies.anomalies.len()),
                    ],
                });
                recommendations.push("Implement traffic shaping if needed".to_string());
                recommendations.push("Review batch job scheduling".to_string());
                next_steps.push("analyze_traffic_patterns".to_string());
            }
        }
        
        DrillDownReport::Network(NetworkDrillDown {
            findings,
            recommendations,
            next_steps,
            metrics_to_monitor: vec![
                "irate(tcp_retransmit[5m])".to_string(),
                "irate(network_transmit_bytes[5m])".to_string(),
                "irate(network_receive_bytes[5m])".to_string(),
            ],
        })
    }

    fn drill_down_io(&self, context: &DrillDownContext) -> DrillDownReport {
        let (start, end) = (context.start_time, context.end_time);
        let step = 60.0;
        
        let mut findings = Vec::new();
        let mut recommendations = Vec::new();
        let mut next_steps = Vec::new();
        
        // Check I/O latency
        if let Ok(result) = self.engine.query_range(
            "histogram_quantile(0.99, block_io_request_latency[5m])",
            start, end, step
        ) {
            let p99_latency = Self::extract_average(&result);
            if p99_latency > 0.100 { // > 100ms
                findings.push(Finding {
                    severity: Severity::High,
                    category: "I/O Latency".to_string(),
                    description: "High disk I/O latency detected".to_string(),
                    evidence: vec![
                        format!("P99 latency: {:.2} ms", p99_latency * 1000.0),
                    ],
                });
                recommendations.push("Check disk health and performance".to_string());
                recommendations.push("Review I/O patterns and caching".to_string());
                next_steps.push("analyze_io_patterns".to_string());
            }
        }
        
        DrillDownReport::IO(IODrillDown {
            findings,
            recommendations,
            next_steps,
            metrics_to_monitor: vec![
                "histogram_quantile(0.99, block_io_request_latency[5m])".to_string(),
                "irate(block_io_requests[5m])".to_string(),
            ],
        })
    }

    fn drill_down_container(&self, context: &DrillDownContext) -> DrillDownReport {
        if let Some(cgroup_name) = &context.filter {
            let (start, end) = (context.start_time, context.end_time);
            let step = 60.0;
            
            let mut findings = Vec::new();
            let mut recommendations = Vec::new();
            let mut next_steps = Vec::new();
            
            // Check container CPU throttling
            let throttle_query = format!("cgroup_cpu_throttled{{name=\"{}\"}}", cgroup_name);
            if let Ok(result) = self.engine.query_range(&throttle_query, start, end, step) {
                let throttled = Self::extract_max(&result);
                if throttled > 0.0 {
                    findings.push(Finding {
                        severity: Severity::High,
                        category: "Container Throttling".to_string(),
                        description: format!("Container '{}' is being CPU throttled", cgroup_name),
                        evidence: vec![
                            format!("Throttled periods: {:.0}", throttled),
                        ],
                    });
                    recommendations.push("Increase container CPU limits".to_string());
                    recommendations.push("Optimize application CPU usage".to_string());
                    next_steps.push("analyze_container_cpu_patterns".to_string());
                }
            }
            
            DrillDownReport::Container(ContainerDrillDown {
                container_name: cgroup_name.clone(),
                findings,
                recommendations,
                next_steps,
                metrics_to_monitor: vec![
                    format!("cgroup_cpu_usage{{name=\"{}\"}}", cgroup_name),
                    format!("cgroup_memory_usage{{name=\"{}\"}}", cgroup_name),
                ],
            })
        } else {
            DrillDownReport::NotFound
        }
    }

    // Helper methods
    fn assess_cpu_status(utilization: f64) -> HealthStatus {
        if utilization > 0.9 {
            HealthStatus::Critical
        } else if utilization > 0.7 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }

    fn assess_memory_status(utilization: f64) -> HealthStatus {
        if utilization > 0.95 {
            HealthStatus::Critical
        } else if utilization > 0.85 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }

    fn assess_network_status(retransmit_rate: f64) -> HealthStatus {
        if retransmit_rate > 1000.0 {
            HealthStatus::Critical
        } else if retransmit_rate > 100.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }

    fn assess_io_status(p99_latency: f64) -> HealthStatus {
        if p99_latency > 0.5 { // > 500ms
            HealthStatus::Critical
        } else if p99_latency > 0.1 { // > 100ms
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }

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
        
        let n = values.len() as f64;
        let sum_x: f64 = values.iter().map(|(t, _)| t).sum();
        let sum_y: f64 = values.iter().map(|(_, v)| v).sum();
        let sum_xy: f64 = values.iter().map(|(t, v)| t * v).sum();
        let sum_xx: f64 = values.iter().map(|(t, _)| t * t).sum();
        
        (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x)
    }
}

// Data structures for hierarchical analysis

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemHealthReport {
    pub timestamp_start: f64,
    pub timestamp_end: f64,
    pub duration_seconds: f64,
    pub cpu_health: SubsystemHealth,
    pub memory_health: SubsystemHealth,
    pub network_health: NetworkHealth,
    pub io_health: IOHealth,
    pub problem_areas: Vec<ProblemArea>,
    pub recommended_next_analysis: Vec<String>,
}

impl Default for SystemHealthReport {
    fn default() -> Self {
        Self {
            timestamp_start: 0.0,
            timestamp_end: 0.0,
            duration_seconds: 0.0,
            cpu_health: SubsystemHealth::default(),
            memory_health: SubsystemHealth::default(),
            network_health: NetworkHealth::default(),
            io_health: IOHealth::default(),
            problem_areas: Vec::new(),
            recommended_next_analysis: Vec::new(),
        }
    }
}

impl SystemHealthReport {
    fn identify_problem_areas(&mut self) {
        // Prioritize problem areas
        if matches!(self.cpu_health.status, HealthStatus::Critical | HealthStatus::Warning) {
            self.problem_areas.push(ProblemArea {
                subsystem: "cpu".to_string(),
                severity: self.cpu_health.status.clone(),
                summary: format!("CPU utilization at {:.1}%", self.cpu_health.utilization * 100.0),
            });
            self.recommended_next_analysis.push("cpu".to_string());
        }
        
        if matches!(self.memory_health.status, HealthStatus::Critical | HealthStatus::Warning) {
            self.problem_areas.push(ProblemArea {
                subsystem: "memory".to_string(),
                severity: self.memory_health.status.clone(),
                summary: format!("Memory utilization at {:.1}%", self.memory_health.utilization * 100.0),
            });
            self.recommended_next_analysis.push("memory".to_string());
        }
        
        if matches!(self.network_health.status, HealthStatus::Critical | HealthStatus::Warning) {
            self.problem_areas.push(ProblemArea {
                subsystem: "network".to_string(),
                severity: self.network_health.status.clone(),
                summary: format!("TCP retransmits at {:.0}/sec", self.network_health.retransmit_rate),
            });
            self.recommended_next_analysis.push("network".to_string());
        }
        
        if matches!(self.io_health.status, HealthStatus::Critical | HealthStatus::Warning) {
            self.problem_areas.push(ProblemArea {
                subsystem: "io".to_string(),
                severity: self.io_health.status.clone(),
                summary: format!("I/O P99 latency at {:.0}ms", self.io_health.latency_p99 * 1000.0),
            });
            self.recommended_next_analysis.push("io".to_string());
        }
        
        // Sort by severity
        self.problem_areas.sort_by_key(|p| match p.severity {
            HealthStatus::Critical => 0,
            HealthStatus::Warning => 1,
            HealthStatus::Healthy => 2,
        });
    }
    
    pub fn format_for_llm(&self) -> String {
        let mut output = String::new();
        
        output.push_str("SYSTEM HEALTH OVERVIEW\n");
        output.push_str("======================\n\n");
        
        output.push_str(&format!("Recording Duration: {:.1} hours\n\n", self.duration_seconds / 3600.0));
        
        output.push_str("Subsystem Status:\n");
        output.push_str(&format!("  CPU:     {:?} ({:.1}% utilization)\n", 
            self.cpu_health.status, self.cpu_health.utilization * 100.0));
        output.push_str(&format!("  Memory:  {:?} ({:.1}% utilization)\n", 
            self.memory_health.status, self.memory_health.utilization * 100.0));
        output.push_str(&format!("  Network: {:?} ({:.0} retransmits/sec)\n", 
            self.network_health.status, self.network_health.retransmit_rate));
        output.push_str(&format!("  I/O:     {:?} ({:.0}ms P99 latency)\n", 
            self.io_health.status, self.io_health.latency_p99 * 1000.0));
        
        if !self.problem_areas.is_empty() {
            output.push_str("\nIDENTIFIED ISSUES (by priority):\n");
            for (i, problem) in self.problem_areas.iter().enumerate() {
                output.push_str(&format!("{}. [{:?}] {} - {}\n", 
                    i + 1, problem.severity, problem.subsystem.to_uppercase(), problem.summary));
            }
            
            output.push_str("\nRECOMMENDED ANALYSIS PATH:\n");
            output.push_str("To investigate these issues, analyze in this order:\n");
            for subsystem in &self.recommended_next_analysis {
                output.push_str(&format!("  - drill_down('{}') for detailed {} analysis\n", 
                    subsystem, subsystem));
            }
        } else {
            output.push_str("\n✓ No critical issues detected\n");
        }
        
        output
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemHealth {
    pub status: HealthStatus,
    pub utilization: f64,
}

impl Default for SubsystemHealth {
    fn default() -> Self {
        Self {
            status: HealthStatus::Healthy,
            utilization: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHealth {
    pub status: HealthStatus,
    pub retransmit_rate: f64,
}

impl Default for NetworkHealth {
    fn default() -> Self {
        Self {
            status: HealthStatus::Healthy,
            retransmit_rate: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOHealth {
    pub status: HealthStatus,
    pub latency_p99: f64,
}

impl Default for IOHealth {
    fn default() -> Self {
        Self {
            status: HealthStatus::Healthy,
            latency_p99: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProblemArea {
    pub subsystem: String,
    pub severity: HealthStatus,
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DrillDownContext {
    pub start_time: f64,
    pub end_time: f64,
    pub filter: Option<String>, // e.g., container name
    pub detailed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DrillDownReport {
    CPU(CPUDrillDown),
    Memory(MemoryDrillDown),
    Network(NetworkDrillDown),
    IO(IODrillDown),
    Container(ContainerDrillDown),
    NotFound,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CPUDrillDown {
    pub findings: Vec<Finding>,
    pub recommendations: Vec<String>,
    pub next_steps: Vec<String>,
    pub metrics_to_monitor: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryDrillDown {
    pub findings: Vec<Finding>,
    pub recommendations: Vec<String>,
    pub next_steps: Vec<String>,
    pub metrics_to_monitor: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkDrillDown {
    pub findings: Vec<Finding>,
    pub recommendations: Vec<String>,
    pub next_steps: Vec<String>,
    pub metrics_to_monitor: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IODrillDown {
    pub findings: Vec<Finding>,
    pub recommendations: Vec<String>,
    pub next_steps: Vec<String>,
    pub metrics_to_monitor: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerDrillDown {
    pub container_name: String,
    pub findings: Vec<Finding>,
    pub recommendations: Vec<String>,
    pub next_steps: Vec<String>,
    pub metrics_to_monitor: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub category: String,
    pub description: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl DrillDownReport {
    pub fn format_for_llm(&self) -> String {
        match self {
            DrillDownReport::CPU(report) => Self::format_subsystem_report("CPU", report.findings.as_slice(), 
                                                                          &report.recommendations, &report.next_steps),
            DrillDownReport::Memory(report) => Self::format_subsystem_report("Memory", report.findings.as_slice(),
                                                                             &report.recommendations, &report.next_steps),
            DrillDownReport::Network(report) => Self::format_subsystem_report("Network", report.findings.as_slice(),
                                                                              &report.recommendations, &report.next_steps),
            DrillDownReport::IO(report) => Self::format_subsystem_report("I/O", report.findings.as_slice(),
                                                                         &report.recommendations, &report.next_steps),
            DrillDownReport::Container(report) => {
                let header = format!("Container '{}' Analysis", report.container_name);
                Self::format_subsystem_report(&header, report.findings.as_slice(),
                                             &report.recommendations, &report.next_steps)
            }
            DrillDownReport::NotFound => "No analysis available for specified subsystem".to_string(),
        }
    }
    
    fn format_subsystem_report(name: &str, findings: &[Finding], recommendations: &[String], next_steps: &[String]) -> String {
        let mut output = format!("{} DRILL-DOWN ANALYSIS\n", name.to_uppercase());
        output.push_str(&"=".repeat(name.len() + 20));
        output.push_str("\n\n");
        
        if findings.is_empty() {
            output.push_str("✓ No significant issues found\n");
        } else {
            output.push_str("FINDINGS:\n");
            for (i, finding) in findings.iter().enumerate() {
                output.push_str(&format!("\n{}. [{:?}] {}\n", i + 1, finding.severity, finding.category));
                output.push_str(&format!("   {}\n", finding.description));
                if !finding.evidence.is_empty() {
                    output.push_str("   Evidence:\n");
                    for evidence in &finding.evidence {
                        output.push_str(&format!("   - {}\n", evidence));
                    }
                }
            }
        }
        
        if !recommendations.is_empty() {
            output.push_str("\nRECOMMENDATIONS:\n");
            for rec in recommendations {
                output.push_str(&format!("• {}\n", rec));
            }
        }
        
        if !next_steps.is_empty() {
            output.push_str("\nFURTHER INVESTIGATION:\n");
            output.push_str("For deeper analysis, consider:\n");
            for step in next_steps {
                output.push_str(&format!("• {}\n", step));
            }
        }
        
        output
    }
}