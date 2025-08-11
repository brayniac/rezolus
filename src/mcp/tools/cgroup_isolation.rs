use crate::viewer::tsdb::Tsdb;
use std::sync::Arc;
use std::collections::HashMap;
use rayon::prelude::*;
use std::time::Instant;

pub struct CgroupIsolationReport {
    pub target_cgroup: String,
    pub target_metrics: CgroupMetrics,
    pub system_metrics: SystemMetrics,
    pub other_cgroups: Vec<CgroupComparison>,
    pub target_vs_system: TargetSystemAnalysis,
    pub resource_attribution: ResourceAttribution,
    pub interference_analysis: InterferenceAnalysis,
    pub irq_isolation_analysis: IrqIsolationAnalysis,
    pub syscall_comparison: SyscallComparison,
    pub analysis_time_ms: u128,
}

pub struct SyscallComparison {
    pub target_profile: Vec<(String, f64)>,  // Target cgroup's syscall profile
    pub system_profile: Vec<(String, f64)>,  // System-wide syscall profile
    pub profile_similarity: f64,             // How similar the profiles are (0-1)
    pub dominant_operations: Vec<String>,    // Top syscall types for target
    pub insights: Vec<String>,                // Analysis insights
}

pub struct IrqIsolationAnalysis {
    pub softirq_total_cores: f64,
    pub irq_isolated_cores: Vec<usize>,  // Cores handling most softirq
    pub application_cores: Vec<usize>,   // Cores with minimal softirq
    pub isolation_quality: String,
    pub network_irq_cores: Vec<usize>,
    pub findings: Vec<String>,
}

pub struct CgroupMetrics {
    pub cpu_usage_avg: f64,
    pub cpu_usage_pct: f64,
    pub cpu_usage_cores: f64,  // Number of CPU cores worth of usage
    pub memory_usage_avg: f64,
    pub syscall_rate: f64,
    pub syscall_by_type: Vec<(String, f64)>,  // Syscalls broken down by type
    pub cpu_throttled_pct: f64,
    pub instruction_rate: f64,
    pub cycles_rate: f64,
    pub context_switches: f64,
    pub workload_characterization: WorkloadCharacterization,
}

pub struct WorkloadCharacterization {
    pub primary_type: String,
    pub io_intensity: f64,      // read+write syscalls per second
    pub network_intensity: f64,  // socket syscalls per second
    pub lock_contention: f64,    // lock+poll syscalls per second
    pub memory_pressure: f64,    // memory syscalls per second
    pub process_activity: f64,   // process syscalls per second
    pub filesystem_activity: f64, // filesystem syscalls per second
    pub characterization: String, // High-level description
}

pub struct SystemMetrics {
    pub total_cpu_usage: f64,
    pub total_cpu_cores: f64,  // Number of CPU cores worth of usage
    pub total_memory_usage: f64,
    pub total_syscalls: f64,
    pub total_network_bytes: f64,
    pub total_disk_io: f64,
    pub softirq_cores: f64,  // CPU cores spent in softirq
    pub softirq_by_core: Vec<(usize, f64)>,  // Per-core softirq usage
    pub softirq_by_kind: Vec<(String, f64)>,  // Breakdown by kind (net_rx, timer, etc)
}

pub struct CgroupComparison {
    pub cgroup_name: String,
    pub cpu_usage_pct: f64,
    pub correlation_with_target: f64,
    pub resource_competition: String,
}

pub struct TargetSystemAnalysis {
    pub target_cpu_share: f64,
    pub target_syscall_share: f64,
    pub cpu_efficiency: f64,
    pub correlations_with_system: Vec<(String, f64)>,
    pub bottleneck_analysis: Vec<String>,
}

pub struct ResourceAttribution {
    pub target_attributed_cpu: f64,
    pub system_overhead_cpu: f64,
    pub other_services_cpu: f64,
    pub idle_cpu: f64,
}

pub struct InterferenceAnalysis {
    pub competing_cgroups: Vec<String>,
    pub interference_score: f64,
    pub recommendations: Vec<String>,
}

pub fn analyze_cgroup_isolation(
    tsdb: &Arc<Tsdb>,
    target_cgroup: &str,
) -> Result<CgroupIsolationReport, Box<dyn std::error::Error>> {
    let start = Instant::now();
    
    eprintln!("Analyzing cgroup isolation for: {}", target_cgroup);
    
    // Get target cgroup metrics
    let target_metrics = collect_cgroup_metrics(tsdb, target_cgroup)?;
    
    // Get system-wide metrics
    let system_metrics = collect_system_metrics(tsdb)?;
    
    // Analyze other cgroups
    let other_cgroups = analyze_other_cgroups(tsdb, target_cgroup)?;
    
    // Target vs System analysis
    let target_vs_system = analyze_target_vs_system(tsdb, target_cgroup, &target_metrics, &system_metrics)?;
    
    // Get CPU cores for calculations
    let cpu_cores = get_cpu_cores(tsdb)?;
    
    // Resource attribution
    let resource_attribution = calculate_resource_attribution(&target_metrics, &system_metrics, &other_cgroups, cpu_cores)?;
    
    // Interference analysis
    let interference_analysis = analyze_interference(&other_cgroups, &target_vs_system)?;
    
    // IRQ isolation analysis
    let irq_isolation_analysis = analyze_irq_isolation(&system_metrics, cpu_cores)?;
    
    // Syscall comparison analysis
    let syscall_comparison = analyze_syscall_patterns(tsdb, &target_metrics)?;
    
    Ok(CgroupIsolationReport {
        target_cgroup: target_cgroup.to_string(),
        target_metrics,
        system_metrics,
        other_cgroups,
        target_vs_system,
        resource_attribution,
        interference_analysis,
        irq_isolation_analysis,
        syscall_comparison,
        analysis_time_ms: start.elapsed().as_millis(),
    })
}

fn get_cpu_cores(tsdb: &Arc<Tsdb>) -> Result<usize, Box<dyn std::error::Error>> {
    // Get CPU cores from gauge
    if let Some(collection) = tsdb.gauges("cpu_cores", ()) {
        let untyped = collection.untyped();
        let series = untyped.sum();
        if !series.inner.is_empty() {
            // Get the last value (most recent)
            if let Some((_timestamp, value)) = series.inner.iter().last() {
                return Ok(*value as usize);
            }
        }
    }
    
    // Fallback to 1 if not found
    eprintln!("Warning: cpu_cores gauge not found, defaulting to 1");
    Ok(1)
}

fn collect_cgroup_metrics(tsdb: &Arc<Tsdb>, cgroup_name: &str) -> Result<CgroupMetrics, Box<dyn std::error::Error>> {
    let mut metrics = CgroupMetrics {
        cpu_usage_avg: 0.0,
        cpu_usage_pct: 0.0,
        cpu_usage_cores: 0.0,
        memory_usage_avg: 0.0,
        syscall_rate: 0.0,
        syscall_by_type: Vec::new(),
        cpu_throttled_pct: 0.0,
        instruction_rate: 0.0,
        cycles_rate: 0.0,
        context_switches: 0.0,
        workload_characterization: WorkloadCharacterization {
            primary_type: String::new(),
            io_intensity: 0.0,
            network_intensity: 0.0,
            lock_contention: 0.0,
            memory_pressure: 0.0,
            process_activity: 0.0,
            filesystem_activity: 0.0,
            characterization: String::new(),
        },
    };
    
    // Get number of CPU cores from gauge
    let cpu_cores = get_cpu_cores(tsdb)?;
    
    // Collect CPU usage using the new average_rate method
    if let Some(collection) = tsdb.counters("cgroup_cpu_usage", [("name", cgroup_name)]) {
        // Get average rate across all labels (should be just one for this cgroup)
        let rates = collection.average_rate();
        
        // Sum all rates (in case there are multiple series)
        let total_rate: f64 = rates.values()
            .filter_map(|r| *r)
            .sum();
        
        if total_rate > 0.0 {
            // average_rate returns rate in units per nanosecond
            // CPU usage counter is in nanoseconds, so rate is nanoseconds/nanosecond = cores
            metrics.cpu_usage_avg = total_rate * 1_000_000_000.0;  // Convert to nanoseconds per second
            metrics.cpu_usage_cores = total_rate;  // Already in cores
            metrics.cpu_usage_pct = (metrics.cpu_usage_cores * 100.0) / cpu_cores as f64;
        }
    }
    
    // Collect syscalls
    if let Some(collection) = tsdb.counters("cgroup_syscall", [("name", cgroup_name)]) {
        let rates = collection.average_rate();
        metrics.syscall_rate = rates.values()
            .filter_map(|r| *r)
            .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
    }
    
    // Collect syscalls by type
    let syscall_types = vec![
        "read", "write", "poll", "lock", "time", "sleep", "socket", "yield",
        "filesystem", "memory", "process", "query", "ipc", "timer", "event", "other"
    ];
    
    let mut syscall_by_type = Vec::new();
    let mut total_typed_syscalls = 0.0;
    
    for syscall_type in &syscall_types {
        if let Some(collection) = tsdb.counters("cgroup_syscall", [("name", cgroup_name), ("op", *syscall_type)]) {
            let rates = collection.average_rate();
            let rate: f64 = rates.values()
                .filter_map(|r| *r)
                .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
            
            if rate > 0.0 {
                syscall_by_type.push((syscall_type.to_string(), rate));
                total_typed_syscalls += rate;
                
                // Update workload characterization metrics
                match *syscall_type {
                    "read" | "write" => metrics.workload_characterization.io_intensity += rate,
                    "socket" => metrics.workload_characterization.network_intensity = rate,
                    "lock" | "poll" => metrics.workload_characterization.lock_contention += rate,
                    "memory" => metrics.workload_characterization.memory_pressure = rate,
                    "process" => metrics.workload_characterization.process_activity = rate,
                    "filesystem" => metrics.workload_characterization.filesystem_activity = rate,
                    _ => {}
                }
            }
        }
    }
    
    // Sort syscalls by rate (highest first)
    syscall_by_type.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    metrics.syscall_by_type = syscall_by_type;
    
    // Determine primary workload type based on syscall patterns
    let wc = &mut metrics.workload_characterization;
    
    // Find dominant syscall type
    if let Some((primary_type, _)) = metrics.syscall_by_type.first() {
        wc.primary_type = primary_type.clone();
    }
    
    // Characterize the workload based on patterns
    wc.characterization = characterize_workload(wc, total_typed_syscalls);
    
    // Collect instructions
    if let Some(collection) = tsdb.counters("cgroup_cpu_instructions", [("name", cgroup_name)]) {
        let rates = collection.average_rate();
        metrics.instruction_rate = rates.values()
            .filter_map(|r| *r)
            .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
    }
    
    // Collect cycles
    if let Some(collection) = tsdb.counters("cgroup_cpu_cycles", [("name", cgroup_name)]) {
        let rates = collection.average_rate();
        metrics.cycles_rate = rates.values()
            .filter_map(|r| *r)
            .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
    }
    
    // Collect context switches
    if let Some(collection) = tsdb.counters("cgroup_scheduler_context_switch", [("name", cgroup_name)]) {
        let rates = collection.average_rate();
        metrics.context_switches = rates.values()
            .filter_map(|r| *r)
            .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
    }
    
    // Collect throttling
    if let Some(collection) = tsdb.counters("cgroup_cpu_throttled_time", [("name", cgroup_name)]) {
        let rate = collection.rate();
        let series = rate.sum();
        if !series.inner.is_empty() {
            let throttled = series.inner.values().sum::<f64>() / series.inner.len() as f64;
            // Calculate throttled percentage
            if metrics.cpu_usage_avg > 0.0 {
                metrics.cpu_throttled_pct = (throttled / (metrics.cpu_usage_avg + throttled)) * 100.0;
            }
        }
    }
    
    Ok(metrics)
}

fn collect_system_metrics(tsdb: &Arc<Tsdb>) -> Result<SystemMetrics, Box<dyn std::error::Error>> {
    let mut metrics = SystemMetrics {
        total_cpu_usage: 0.0,
        total_cpu_cores: 0.0,
        total_memory_usage: 0.0,
        total_syscalls: 0.0,
        total_network_bytes: 0.0,
        total_disk_io: 0.0,
        softirq_cores: 0.0,
        softirq_by_core: Vec::new(),
        softirq_by_kind: Vec::new(),
    };
    
    // Get number of CPU cores
    let cpu_cores = get_cpu_cores(tsdb)?;
    
    // System CPU usage
    if let Some(collection) = tsdb.counters("cpu_usage", ()) {
        let rates = collection.average_rate();
        let total_rate: f64 = rates.values()
            .filter_map(|r| *r)
            .sum();
        
        // CPU usage counter is in nanoseconds, rate is nanoseconds/nanosecond = cores
        metrics.total_cpu_cores = total_rate;
        metrics.total_cpu_usage = (metrics.total_cpu_cores * 100.0) / cpu_cores as f64;
    }
    
    // System syscalls
    if let Some(collection) = tsdb.counters("syscall", ()) {
        let rates = collection.average_rate();
        metrics.total_syscalls = rates.values()
            .filter_map(|r| *r)
            .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
    }
    
    // Network bytes
    if let Some(collection) = tsdb.counters("network_bytes", ()) {
        let rates = collection.average_rate();
        metrics.total_network_bytes = rates.values()
            .filter_map(|r| *r)
            .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
    }
    
    // Disk I/O
    if let Some(collection) = tsdb.counters("blockio_bytes", ()) {
        let rates = collection.average_rate();
        metrics.total_disk_io = rates.values()
            .filter_map(|r| *r)
            .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
    }
    
    // Collect softirq time - total across all cores
    if let Some(collection) = tsdb.counters("softirq_time", ()) {
        let rates = collection.average_rate();
        let total_rate: f64 = rates.values()
            .filter_map(|r| *r)
            .sum();
        // softirq_time counter is in nanoseconds, rate is nanoseconds/nanosecond = cores
        metrics.softirq_cores = total_rate;
    }
    
    // Collect per-core softirq time
    let mut core_softirq = std::collections::HashMap::new();
    for core_id in 0..cpu_cores {
        let core_str = core_id.to_string();
        if let Some(collection) = tsdb.counters("softirq_time", [("id", core_str.as_str())]) {
            let rates = collection.average_rate();
            let core_usage: f64 = rates.values()
                .filter_map(|r| *r)
                .sum();
            
            if core_usage > 0.001 {  // Only include if significant
                core_softirq.insert(core_id, core_usage);
            }
        }
    }
    
    // Sort by core ID and convert to vec
    let mut sorted_cores: Vec<_> = core_softirq.into_iter().collect();
    sorted_cores.sort_by_key(|&(core, _)| core);
    metrics.softirq_by_core = sorted_cores;
    
    // Collect softirq by kind
    let softirq_kinds = vec!["net_rx", "net_tx", "timer", "sched", "rcu", "block"];
    let mut kind_softirq = Vec::new();
    
    for kind in softirq_kinds {
        if let Some(collection) = tsdb.counters("softirq_time", [("kind", kind)]) {
            let rates = collection.average_rate();
            let cores: f64 = rates.values()
                .filter_map(|r| *r)
                .sum();
            
            if cores > 0.001 {  // Only include if significant
                kind_softirq.push((kind.to_string(), cores));
            }
        }
    }
    
    // Sort by usage
    kind_softirq.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    metrics.softirq_by_kind = kind_softirq;
    
    Ok(metrics)
}

fn analyze_other_cgroups(
    tsdb: &Arc<Tsdb>,
    target_cgroup: &str,
) -> Result<Vec<CgroupComparison>, Box<dyn std::error::Error>> {
    let mut comparisons = Vec::new();
    
    // Get all cgroup names
    let mut cgroup_names = std::collections::HashSet::new();
    if let Some(collection) = tsdb.counters("cgroup_cpu_usage", ()) {
        for labels in collection.labels() {
            if let Some(name) = labels.inner.get("name") {
                if name != target_cgroup {
                    cgroup_names.insert(name.clone());
                }
            }
        }
    }
    
    // Analyze each cgroup
    for cgroup_name in cgroup_names {
        let metrics = collect_cgroup_metrics(tsdb, &cgroup_name)?;
        
        // Calculate correlation with target cgroup
        let correlation = calculate_cgroup_correlation(tsdb, target_cgroup, &cgroup_name)?;
        
        // Determine competition level
        let competition = if correlation.abs() > 0.7 {
            "High competition".to_string()
        } else if correlation.abs() > 0.4 {
            "Moderate competition".to_string()
        } else {
            "Low competition".to_string()
        };
        
        comparisons.push(CgroupComparison {
            cgroup_name,
            cpu_usage_pct: metrics.cpu_usage_pct,
            correlation_with_target: correlation,
            resource_competition: competition,
        });
    }
    
    // Sort by CPU usage
    comparisons.sort_by(|a, b| b.cpu_usage_pct.partial_cmp(&a.cpu_usage_pct).unwrap());
    
    Ok(comparisons)
}

fn calculate_cgroup_correlation(
    tsdb: &Arc<Tsdb>,
    cgroup1: &str,
    cgroup2: &str,
) -> Result<f64, Box<dyn std::error::Error>> {
    // Get CPU usage for both cgroups
    let series1 = if let Some(collection) = tsdb.counters("cgroup_cpu_usage", [("name", cgroup1)]) {
        let rate = collection.rate();
        rate.sum()
    } else {
        return Ok(0.0);
    };
    
    let series2 = if let Some(collection) = tsdb.counters("cgroup_cpu_usage", [("name", cgroup2)]) {
        let rate = collection.rate();
        rate.sum()
    } else {
        return Ok(0.0);
    };
    
    // Calculate correlation
    compute_correlation(&series1, &series2)
}

fn compute_correlation(
    series1: &crate::viewer::tsdb::UntypedSeries,
    series2: &crate::viewer::tsdb::UntypedSeries,
) -> Result<f64, Box<dyn std::error::Error>> {
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
        return Ok(0.0);
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
    
    if denominator1 > 0.0 && denominator2 > 0.0 {
        Ok(numerator / (denominator1.sqrt() * denominator2.sqrt()))
    } else {
        Ok(0.0)
    }
}

fn analyze_target_vs_system(
    tsdb: &Arc<Tsdb>,
    target_cgroup: &str,
    target_metrics: &CgroupMetrics,
    system_metrics: &SystemMetrics,
) -> Result<TargetSystemAnalysis, Box<dyn std::error::Error>> {
    let mut analysis = TargetSystemAnalysis {
        target_cpu_share: 0.0,
        target_syscall_share: 0.0,
        cpu_efficiency: 0.0,
        correlations_with_system: Vec::new(),
        bottleneck_analysis: Vec::new(),
    };
    
    // Calculate target cgroup share of system resources
    if system_metrics.total_cpu_usage > 0.0 {
        analysis.target_cpu_share = (target_metrics.cpu_usage_pct / system_metrics.total_cpu_usage) * 100.0;
    }
    
    if system_metrics.total_syscalls > 0.0 {
        analysis.target_syscall_share = (target_metrics.syscall_rate / system_metrics.total_syscalls) * 100.0;
    }
    
    // Calculate CPU efficiency (instructions per cycle)
    if target_metrics.cycles_rate > 0.0 {
        analysis.cpu_efficiency = target_metrics.instruction_rate / target_metrics.cycles_rate;
    }
    
    // Analyze correlations with system metrics
    let system_metrics_list = vec![
        ("cpu_usage", "System CPU"),
        ("syscall", "System Syscalls"),
        ("network_bytes", "Network Traffic"),
        ("memory_free", "Free Memory"),
    ];
    
    for (metric_name, label) in system_metrics_list {
        if let Ok(correlation) = calculate_system_correlation(tsdb, target_cgroup, metric_name) {
            if correlation.abs() > 0.5 {
                analysis.correlations_with_system.push((label.to_string(), correlation));
            }
        }
    }
    
    // Bottleneck analysis
    if target_metrics.cpu_usage_pct > 80.0 {
        analysis.bottleneck_analysis.push("CPU bottleneck detected (>80% usage)".to_string());
    }
    
    if target_metrics.cpu_throttled_pct > 5.0 {
        analysis.bottleneck_analysis.push(format!("CPU throttling detected ({:.1}%)", target_metrics.cpu_throttled_pct));
    }
    
    if target_metrics.context_switches > 10000.0 {
        analysis.bottleneck_analysis.push("High context switching rate".to_string());
    }
    
    if analysis.cpu_efficiency < 1.0 {
        analysis.bottleneck_analysis.push("Low CPU efficiency (IPC < 1.0)".to_string());
    }
    
    Ok(analysis)
}

fn calculate_system_correlation(
    tsdb: &Arc<Tsdb>,
    target_cgroup: &str,
    system_metric: &str,
) -> Result<f64, Box<dyn std::error::Error>> {
    // Get cgroup CPU usage
    let cgroup_series = if let Some(collection) = tsdb.counters("cgroup_cpu_usage", [("name", target_cgroup)]) {
        let rate = collection.rate();
        rate.sum()
    } else {
        return Ok(0.0);
    };
    
    // Get system metric
    let system_series = if let Some(collection) = tsdb.counters(system_metric, ()) {
        let rate = collection.rate();
        rate.sum()
    } else if let Some(collection) = tsdb.gauges(system_metric, ()) {
        let untyped = collection.untyped();
        untyped.sum()
    } else {
        return Ok(0.0);
    };
    
    compute_correlation(&cgroup_series, &system_series)
}

fn calculate_resource_attribution(
    target_metrics: &CgroupMetrics,
    system_metrics: &SystemMetrics,
    other_cgroups: &[CgroupComparison],
    cpu_cores: usize,
) -> Result<ResourceAttribution, Box<dyn std::error::Error>> {
    let target_cpu = target_metrics.cpu_usage_cores;
    
    // Calculate other services CPU in cores
    let mut other_services_cores = 0.0;
    for cgroup in other_cgroups {
        // Convert percentage back to cores
        other_services_cores += cgroup.cpu_usage_pct * cpu_cores as f64 / 100.0;
    }
    
    let total_cgroup_cpu = target_cpu + other_services_cores;
    
    // System overhead is the difference between total system CPU and cgroup CPU
    let system_overhead = if system_metrics.total_cpu_cores > total_cgroup_cpu {
        system_metrics.total_cpu_cores - total_cgroup_cpu
    } else {
        0.0
    };
    
    // Idle CPU is what's left
    let idle_cpu = cpu_cores as f64 - system_metrics.total_cpu_cores;
    
    Ok(ResourceAttribution {
        target_attributed_cpu: target_cpu,
        system_overhead_cpu: system_overhead,
        other_services_cpu: other_services_cores,
        idle_cpu: idle_cpu.max(0.0),
    })
}

fn analyze_irq_isolation(
    system_metrics: &SystemMetrics,
    cpu_cores: usize,
) -> Result<IrqIsolationAnalysis, Box<dyn std::error::Error>> {
    let mut analysis = IrqIsolationAnalysis {
        softirq_total_cores: system_metrics.softirq_cores,
        irq_isolated_cores: Vec::new(),
        application_cores: Vec::new(),
        isolation_quality: String::new(),
        network_irq_cores: Vec::new(),
        findings: Vec::new(),
    };
    
    // Identify cores with high softirq (>10% of a core)
    let mut high_softirq_cores = Vec::new();
    let mut low_softirq_cores = Vec::new();
    
    for core_id in 0..cpu_cores {
        let softirq_usage = system_metrics.softirq_by_core.iter()
            .find(|(id, _)| *id == core_id)
            .map(|(_, usage)| *usage)
            .unwrap_or(0.0);
        
        if softirq_usage > 0.1 {  // More than 10% of a core
            high_softirq_cores.push(core_id);
            analysis.irq_isolated_cores.push(core_id);
        } else if softirq_usage < 0.01 {  // Less than 1% of a core
            low_softirq_cores.push(core_id);
            analysis.application_cores.push(core_id);
        }
    }
    
    // Look for network IRQ concentration
    for (kind, cores) in &system_metrics.softirq_by_kind {
        if kind.starts_with("net_") && cores > &0.05 {
            // Network softirq is significant
            // Find which cores are handling it
            for (core_id, usage) in &system_metrics.softirq_by_core {
                if *usage > 0.05 {
                    if !analysis.network_irq_cores.contains(core_id) {
                        analysis.network_irq_cores.push(*core_id);
                    }
                }
            }
        }
    }
    
    // Analyze isolation quality
    if !high_softirq_cores.is_empty() && !low_softirq_cores.is_empty() {
        if high_softirq_cores.len() <= 2 && low_softirq_cores.len() >= 4 {
            analysis.isolation_quality = "Excellent".to_string();
            analysis.findings.push(format!(
                "IRQ isolation detected: Cores {:?} handling interrupts, cores {:?} available for applications",
                high_softirq_cores, low_softirq_cores
            ));
        } else if high_softirq_cores.len() <= 3 {
            analysis.isolation_quality = "Good".to_string();
            analysis.findings.push(format!(
                "Partial IRQ isolation: {} cores handling most interrupts",
                high_softirq_cores.len()
            ));
        } else {
            analysis.isolation_quality = "Poor".to_string();
            analysis.findings.push("No clear IRQ isolation detected".to_string());
        }
    } else {
        analysis.isolation_quality = "None".to_string();
        analysis.findings.push("IRQ load distributed across all cores".to_string());
    }
    
    // Add specific findings about network IRQ
    if !analysis.network_irq_cores.is_empty() {
        analysis.findings.push(format!(
            "Network IRQs concentrated on cores: {:?}",
            analysis.network_irq_cores
        ));
        
        // Check if these are the expected cores (0 and 1)
        if analysis.network_irq_cores == vec![0, 1] || analysis.network_irq_cores == vec![0] || analysis.network_irq_cores == vec![1] {
            analysis.findings.push("Network IRQ affinity properly configured to cores 0-1".to_string());
        }
    }
    
    // Analyze softirq breakdown
    if let Some((top_kind, top_usage)) = system_metrics.softirq_by_kind.first() {
        analysis.findings.push(format!(
            "Dominant softirq type: {} ({:.2} cores)",
            top_kind, top_usage
        ));
    }
    
    // Calculate softirq percentage of total CPU
    if system_metrics.total_cpu_cores > 0.0 {
        let softirq_pct = (system_metrics.softirq_cores / system_metrics.total_cpu_cores) * 100.0;
        analysis.findings.push(format!(
            "Softirq consuming {:.1}% of total CPU",
            softirq_pct
        ));
    }
    
    Ok(analysis)
}

fn analyze_interference(
    other_cgroups: &[CgroupComparison],
    target_analysis: &TargetSystemAnalysis,
) -> Result<InterferenceAnalysis, Box<dyn std::error::Error>> {
    let mut analysis = InterferenceAnalysis {
        competing_cgroups: Vec::new(),
        interference_score: 0.0,
        recommendations: Vec::new(),
    };
    
    // Identify competing cgroups
    for cgroup in other_cgroups {
        if cgroup.correlation_with_target.abs() > 0.5 || cgroup.cpu_usage_pct > 10.0 {
            analysis.competing_cgroups.push(cgroup.cgroup_name.clone());
        }
    }
    
    // Calculate interference score
    let high_correlation_count = other_cgroups.iter()
        .filter(|c| c.correlation_with_target.abs() > 0.7)
        .count();
    
    let total_other_cpu: f64 = other_cgroups.iter()
        .map(|c| c.cpu_usage_pct)
        .sum();
    
    analysis.interference_score = (high_correlation_count as f64 * 0.2 + total_other_cpu / 100.0).min(1.0);
    
    // Generate recommendations
    if analysis.interference_score > 0.7 {
        analysis.recommendations.push("High interference detected - consider dedicated node for target service".to_string());
    }
    
    if target_analysis.target_cpu_share < 50.0 && target_analysis.bottleneck_analysis.iter().any(|b| b.contains("CPU")) {
        analysis.recommendations.push("Target cgroup has less than 50% CPU share but is CPU bottlenecked - reduce competing workloads".to_string());
    }
    
    if !analysis.competing_cgroups.is_empty() {
        analysis.recommendations.push(format!(
            "Consider CPU affinity/isolation from: {}",
            analysis.competing_cgroups.join(", ")
        ));
    }
    
    if target_analysis.cpu_efficiency < 1.5 {
        analysis.recommendations.push("Low IPC suggests cache contention - consider NUMA pinning".to_string());
    }
    
    Ok(analysis)
}

impl CgroupIsolationReport {
    pub fn to_detailed_summary(&self) -> String {
        let mut s = String::new();
        
        s.push_str(&format!(" CGROUP ISOLATION ANALYSIS\n"));
        s.push_str(&format!("=====================================\n"));
        s.push_str(&format!("Target: {}\n\n", self.target_cgroup));
        
        s.push_str(&format!(" TARGET CGROUP METRICS:\n"));
        s.push_str(&format!("  CPU Usage: {:.2} cores ({:.1}%)\n", self.target_metrics.cpu_usage_cores, self.target_metrics.cpu_usage_pct));
        s.push_str(&format!("  Syscall Rate: {:.0}/sec\n", self.target_metrics.syscall_rate));
        s.push_str(&format!("  Instruction Rate: {:.2}M/sec\n", self.target_metrics.instruction_rate / 1_000_000.0));
        s.push_str(&format!("  CPU Cycles: {:.2}M/sec\n", self.target_metrics.cycles_rate / 1_000_000.0));
        s.push_str(&format!("  Context Switches: {:.0}/sec\n", self.target_metrics.context_switches));
        if self.target_metrics.cpu_throttled_pct > 0.0 {
            s.push_str(&format!("  CPU Throttled: {:.1}%\n", self.target_metrics.cpu_throttled_pct));
        }
        
        s.push_str(&format!("\n SYSTEM METRICS:\n"));
        s.push_str(&format!("  Total CPU: {:.2} cores ({:.1}%)\n", self.system_metrics.total_cpu_cores, self.system_metrics.total_cpu_usage));
        s.push_str(&format!("  Softirq CPU: {:.2} cores\n", self.system_metrics.softirq_cores));
        s.push_str(&format!("  Total Syscalls: {:.0}/sec\n", self.system_metrics.total_syscalls));
        s.push_str(&format!("  Network: {:.2} MB/sec\n", self.system_metrics.total_network_bytes / 1_000_000.0));
        s.push_str(&format!("  Disk I/O: {:.2} MB/sec\n", self.system_metrics.total_disk_io / 1_000_000.0));
        
        s.push_str(&format!("\n TARGET VS SYSTEM:\n"));
        s.push_str(&format!("  Target CPU Share: {:.1}% of system\n", self.target_vs_system.target_cpu_share));
        s.push_str(&format!("  Target Syscall Share: {:.1}% of system\n", self.target_vs_system.target_syscall_share));
        s.push_str(&format!("  CPU Efficiency (IPC): {:.2}\n", self.target_vs_system.cpu_efficiency));
        
        if !self.target_vs_system.correlations_with_system.is_empty() {
            s.push_str(&format!("\n  System Correlations:\n"));
            for (metric, corr) in &self.target_vs_system.correlations_with_system {
                s.push_str(&format!("    {} : r={:.3}\n", metric, corr));
            }
        }
        
        if !self.target_vs_system.bottleneck_analysis.is_empty() {
            s.push_str(&format!("\n   Bottlenecks:\n"));
            for bottleneck in &self.target_vs_system.bottleneck_analysis {
                s.push_str(&format!("    {}\n", bottleneck));
            }
        }
        
        s.push_str(&format!("\n RESOURCE ATTRIBUTION:\n"));
        s.push_str(&format!("  Target cgroup: {:.2} cores\n", self.resource_attribution.target_attributed_cpu));
        s.push_str(&format!("  Other Services: {:.2} cores\n", self.resource_attribution.other_services_cpu));
        s.push_str(&format!("  System Overhead: {:.2} cores\n", self.resource_attribution.system_overhead_cpu));
        s.push_str(&format!("  Idle: {:.2} cores\n", self.resource_attribution.idle_cpu));
        
        if !self.other_cgroups.is_empty() {
            s.push_str(&format!("\n OTHER CGROUPS (Top 5):\n"));
            for cgroup in self.other_cgroups.iter().take(5) {
                s.push_str(&format!("  {}\n", cgroup.cgroup_name));
                s.push_str(&format!("    CPU: {:.1}%, Correlation: r={:.3}, {}\n",
                    cgroup.cpu_usage_pct, cgroup.correlation_with_target, cgroup.resource_competition));
            }
        }
        
        s.push_str(&format!("\n INTERFERENCE ANALYSIS:\n"));
        s.push_str(&format!("  Interference Score: {:.1}%\n", self.interference_analysis.interference_score * 100.0));
        if !self.interference_analysis.competing_cgroups.is_empty() {
            s.push_str(&format!("  Competing Services: {}\n", 
                self.interference_analysis.competing_cgroups.join(", ")));
        }
        
        if !self.interference_analysis.recommendations.is_empty() {
            s.push_str(&format!("\n RECOMMENDATIONS:\n"));
            for rec in &self.interference_analysis.recommendations {
                s.push_str(&format!("  {}\n", rec));
            }
        }
        
        s.push_str(&format!("\n IRQ ISOLATION ANALYSIS:\n"));
        s.push_str(&format!("  Isolation Quality: {}\n", self.irq_isolation_analysis.isolation_quality));
        s.push_str(&format!("  Softirq Total: {:.2} cores\n", self.irq_isolation_analysis.softirq_total_cores));
        
        if !self.irq_isolation_analysis.irq_isolated_cores.is_empty() {
            s.push_str(&format!("  IRQ Cores: {:?}\n", self.irq_isolation_analysis.irq_isolated_cores));
        }
        
        if !self.irq_isolation_analysis.application_cores.is_empty() {
            s.push_str(&format!("  Application Cores: {:?}\n", self.irq_isolation_analysis.application_cores));
        }
        
        if !self.system_metrics.softirq_by_core.is_empty() {
            s.push_str(&format!("\n  Per-Core Softirq Usage:\n"));
            for (core_id, usage) in &self.system_metrics.softirq_by_core {
                s.push_str(&format!("    Core {}: {:.3} cores ({:.1}%)\n", 
                    core_id, usage, usage * 100.0));
            }
        }
        
        if !self.system_metrics.softirq_by_kind.is_empty() {
            s.push_str(&format!("\n  Softirq Breakdown:\n"));
            for (kind, usage) in &self.system_metrics.softirq_by_kind {
                s.push_str(&format!("    {}: {:.3} cores\n", kind, usage));
            }
        }
        
        if !self.irq_isolation_analysis.findings.is_empty() {
            s.push_str(&format!("\n  Findings:\n"));
            for finding in &self.irq_isolation_analysis.findings {
                s.push_str(&format!("    {}\n", finding));
            }
        }
        
        // Add workload characterization
        s.push_str(&format!("\n WORKLOAD CHARACTERIZATION:\n"));
        s.push_str(&format!("  Primary Type: {}\n", self.target_metrics.workload_characterization.primary_type));
        s.push_str(&format!("  Characterization: {}\n", self.target_metrics.workload_characterization.characterization));
        s.push_str(&format!("  I/O Intensity: {:.0} ops/sec\n", self.target_metrics.workload_characterization.io_intensity));
        s.push_str(&format!("  Network Intensity: {:.0} ops/sec\n", self.target_metrics.workload_characterization.network_intensity));
        s.push_str(&format!("  Lock Contention: {:.0} ops/sec\n", self.target_metrics.workload_characterization.lock_contention));
        
        // Add syscall breakdown
        if !self.target_metrics.syscall_by_type.is_empty() {
            s.push_str(&format!("\n  Syscall Breakdown:\n"));
            for (syscall_type, rate) in &self.target_metrics.syscall_by_type[..5.min(self.target_metrics.syscall_by_type.len())] {
                let pct = (rate / self.target_metrics.syscall_rate) * 100.0;
                s.push_str(&format!("    {}: {:.0}/sec ({:.1}%)\n", syscall_type, rate, pct));
            }
        }
        
        // Add syscall comparison insights
        s.push_str(&format!("\n SYSCALL PATTERN ANALYSIS:\n"));
        s.push_str(&format!("  Profile Similarity: {:.1}%\n", self.syscall_comparison.profile_similarity * 100.0));
        if !self.syscall_comparison.dominant_operations.is_empty() {
            s.push_str(&format!("  Dominant Operations: {}\n", self.syscall_comparison.dominant_operations.join(", ")));
        }
        if !self.syscall_comparison.insights.is_empty() {
            s.push_str(&format!("\n  Insights:\n"));
            for insight in &self.syscall_comparison.insights {
                s.push_str(&format!("    {}\n", insight));
            }
        }
        
        s.push_str(&format!("\n Analysis time: {}ms\n", self.analysis_time_ms));
        
        s
    }
}

fn characterize_workload(wc: &WorkloadCharacterization, total_syscalls: f64) -> String {
    if total_syscalls == 0.0 {
        return "Unknown - no syscalls detected".to_string();
    }
    
    let io_pct = (wc.io_intensity / total_syscalls) * 100.0;
    let network_pct = (wc.network_intensity / total_syscalls) * 100.0;
    let lock_pct = (wc.lock_contention / total_syscalls) * 100.0;
    let memory_pct = (wc.memory_pressure / total_syscalls) * 100.0;
    let filesystem_pct = (wc.filesystem_activity / total_syscalls) * 100.0;
    
    // Determine primary workload characteristics
    let mut characteristics = Vec::new();
    
    if network_pct > 30.0 {
        characteristics.push("Network-intensive");
    }
    if io_pct > 40.0 {
        characteristics.push("I/O-heavy");
    }
    if lock_pct > 20.0 {
        characteristics.push("High lock contention");
    }
    if memory_pct > 15.0 {
        characteristics.push("Memory-intensive");
    }
    if filesystem_pct > 20.0 {
        characteristics.push("Filesystem-heavy");
    }
    
    // Identify workload type patterns
    if network_pct > 40.0 && io_pct > 30.0 {
        "Network service with high I/O (likely database or cache)".to_string()
    } else if lock_pct > 30.0 && io_pct < 10.0 {
        "CPU-bound with synchronization overhead".to_string()
    } else if filesystem_pct > 40.0 {
        "Storage/filesystem service".to_string()
    } else if network_pct > 50.0 {
        "Network-bound service (likely web/API server)".to_string()
    } else if io_pct > 50.0 && network_pct < 10.0 {
        "Local I/O intensive (likely data processing)".to_string()
    } else if !characteristics.is_empty() {
        characteristics.join(", ")
    } else {
        "Balanced workload".to_string()
    }
}

fn analyze_syscall_patterns(
    tsdb: &Arc<Tsdb>,
    target_metrics: &CgroupMetrics,
) -> Result<SyscallComparison, Box<dyn std::error::Error>> {
    let mut comparison = SyscallComparison {
        target_profile: target_metrics.syscall_by_type.clone(),
        system_profile: Vec::new(),
        profile_similarity: 0.0,
        dominant_operations: Vec::new(),
        insights: Vec::new(),
    };
    
    // Collect system-wide syscall profile
    let syscall_types = vec![
        "read", "write", "poll", "lock", "time", "sleep", "socket", "yield",
        "filesystem", "memory", "process", "query", "ipc", "timer", "event", "other"
    ];
    
    let mut system_profile = Vec::new();
    let mut total_system_syscalls = 0.0;
    
    for syscall_type in &syscall_types {
        if let Some(collection) = tsdb.counters("syscall", [("op", *syscall_type)]) {
            let rates = collection.average_rate();
            let rate: f64 = rates.values()
                .filter_map(|r| *r)
                .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
            
            if rate > 0.0 {
                system_profile.push((syscall_type.to_string(), rate));
                total_system_syscalls += rate;
            }
        }
    }
    
    // Sort by rate
    system_profile.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    comparison.system_profile = system_profile;
    
    // Calculate profile similarity
    if !comparison.target_profile.is_empty() && !comparison.system_profile.is_empty() {
        let mut similarity = 0.0;
        let mut weight_sum = 0.0;
        
        for (target_type, target_rate) in &comparison.target_profile {
            let target_pct = target_rate / target_metrics.syscall_rate;
            
            if let Some((_, system_rate)) = comparison.system_profile.iter()
                .find(|(t, _)| t == target_type) {
                let system_pct = system_rate / total_system_syscalls;
                let diff = (target_pct - system_pct).abs();
                similarity += (1.0 - diff) * target_pct;  // Weight by importance
                weight_sum += target_pct;
            }
        }
        
        if weight_sum > 0.0 {
            comparison.profile_similarity = similarity / weight_sum;
        }
    }
    
    // Identify dominant operations (top 3)
    for (syscall_type, _) in &comparison.target_profile[..3.min(comparison.target_profile.len())] {
        comparison.dominant_operations.push(syscall_type.clone());
    }
    
    // Generate insights
    if let Some((primary, rate)) = comparison.target_profile.first() {
        let pct = (rate / target_metrics.syscall_rate) * 100.0;
        if pct > 50.0 {
            comparison.insights.push(format!(
                "Heavily dominated by {} operations ({:.1}% of syscalls)",
                primary, pct
            ));
        }
    }
    
    // Compare with system profile
    if comparison.profile_similarity < 0.3 {
        comparison.insights.push("Syscall profile significantly different from system average".to_string());
        comparison.insights.push("Consider dedicated resources for this unique workload".to_string());
    } else if comparison.profile_similarity > 0.8 {
        comparison.insights.push("Syscall profile similar to system average".to_string());
    }
    
    // Check for specific patterns
    let wc = &target_metrics.workload_characterization;
    if wc.lock_contention > wc.io_intensity {
        comparison.insights.push("Lock contention exceeds I/O operations - possible synchronization bottleneck".to_string());
    }
    
    if wc.network_intensity > target_metrics.syscall_rate * 0.4 {
        comparison.insights.push("Network-heavy workload - ensure proper network isolation/QoS".to_string());
    }
    
    Ok(comparison)
}