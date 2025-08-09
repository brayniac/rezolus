use crate::viewer::tsdb::Tsdb;
use std::sync::Arc;
use std::collections::BTreeSet;

pub struct CgroupsReport {
    pub cgroups: Vec<CgroupInfo>,
    pub total_count: usize,
    pub has_syscall_metrics: bool,
    pub has_cpu_metrics: bool,
    pub has_memory_metrics: bool,
    pub has_network_metrics: bool,
    pub available_metrics: Vec<String>,
}

pub struct CgroupInfo {
    pub name: String,
    pub has_cpu: bool,
    pub has_memory: bool,
    pub has_syscalls: bool,
    pub has_network: bool,
    pub cpu_usage_cores: f64,
    pub cpu_usage_pct: f64,
    pub syscall_rate: f64,
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
    
    // Fallback to 8 if not found
    Ok(8)
}

pub fn list_cgroups(tsdb: &Arc<Tsdb>) -> Result<CgroupsReport, Box<dyn std::error::Error>> {
    let mut cgroup_names = BTreeSet::new();
    let mut available_metrics: BTreeSet<String> = BTreeSet::new();
    
    // Check for cgroup CPU metrics
    let has_cpu_metrics = tsdb.counter_names().iter().any(|n| n.starts_with("cgroup_cpu"));
    let has_memory_metrics = tsdb.counter_names().iter().any(|n| n.starts_with("cgroup_memory"));
    let has_syscall_metrics = tsdb.counter_names().iter().any(|n| n.starts_with("cgroup_syscall"));
    let has_network_metrics = tsdb.counter_names().iter().any(|n| n.starts_with("cgroup_network"));
    
    // Collect all cgroup-related metrics
    for metric_name in tsdb.counter_names() {
        if metric_name.starts_with("cgroup_") {
            available_metrics.insert(metric_name.to_string());
            
            // Get cgroup names from this metric
            if let Some(collection) = tsdb.counters(&metric_name, ()) {
                for labels in collection.labels() {
                    if let Some(name) = labels.inner.get("name") {
                        cgroup_names.insert(name.clone());
                    }
                }
            }
        }
    }
    
    // Get total CPU cores for percentage calculation
    let cpu_cores = get_cpu_cores(tsdb)?;
    
    // Build detailed info for each cgroup
    // Calculate rates for ALL cgroups to ensure accurate sorting
    let max_detailed = cgroup_names.len(); // Calculate for all to get correct top consumers
    
    let mut cgroups = Vec::new();
    for (idx, cgroup_name) in cgroup_names.iter().enumerate() {
        let mut cpu_usage_cores = 0.0;
        let mut cpu_usage_pct = 0.0;
        let mut syscall_rate = 0.0;
        
        // Only calculate rates for first N cgroups to avoid timeout
        let calculate_rates = idx < max_detailed;
        
        // Get CPU usage
        let has_cpu = if let Some(collection) = tsdb.counters("cgroup_cpu_usage", [("name", cgroup_name.as_str())]) {
            if calculate_rates {
                // Calculate average rate
                let rates = collection.average_rate();
                let total_rate: f64 = rates.values()
                    .filter_map(|r| *r)
                    .sum();
                
                if total_rate > 0.0 {
                    // CPU usage counter is in nanoseconds, rate is nanoseconds/nanosecond = cores
                    cpu_usage_cores = total_rate;
                    cpu_usage_pct = (cpu_usage_cores * 100.0) / cpu_cores as f64;
                }
            }
            
            collection.labels().next().is_some()
        } else {
            false
        };
        
        let has_memory = if let Some(collection) = tsdb.counters("cgroup_memory_usage", [("name", cgroup_name.as_str())]) {
            collection.labels().next().is_some()
        } else {
            false
        };
        
        // Get syscall rate
        let has_syscalls = if let Some(collection) = tsdb.counters("cgroup_syscall", [("name", cgroup_name.as_str())]) {
            if calculate_rates {
                let rates = collection.average_rate();
                syscall_rate = rates.values()
                    .filter_map(|r| *r)
                    .sum::<f64>() * 1_000_000_000.0;  // Convert to per second
            }
            
            collection.labels().next().is_some()
        } else {
            false
        };
        
        let has_network = if let Some(collection) = tsdb.counters("cgroup_network_bytes", [("name", cgroup_name.as_str())]) {
            collection.labels().next().is_some()
        } else {
            false
        };
        
        cgroups.push(CgroupInfo {
            name: cgroup_name.clone(),
            has_cpu,
            has_memory,
            has_syscalls,
            has_network,
            cpu_usage_cores,
            cpu_usage_pct,
            syscall_rate,
        });
    }
    
    // Sort cgroups by CPU usage (highest first) for better visibility
    cgroups.sort_by(|a, b| b.cpu_usage_cores.partial_cmp(&a.cpu_usage_cores).unwrap());
    
    Ok(CgroupsReport {
        total_count: cgroups.len(),
        cgroups,
        has_syscall_metrics,
        has_cpu_metrics,
        has_memory_metrics,
        has_network_metrics,
        available_metrics: available_metrics.into_iter().collect(),
    })
}

impl CgroupsReport {
    pub fn to_summary(&self) -> String {
        let mut s = String::new();
        
        s.push_str(&format!("📦 CGROUPS ANALYSIS\n"));
        s.push_str(&format!("===================\n\n"));
        
        s.push_str(&format!("Found {} cgroups in the dataset\n\n", self.total_count));
        
        s.push_str("📊 Available Metric Types:\n");
        if self.has_cpu_metrics {
            s.push_str("  ✓ CPU metrics\n");
        }
        if self.has_memory_metrics {
            s.push_str("  ✓ Memory metrics\n");
        }
        if self.has_syscall_metrics {
            s.push_str("  ✓ Syscall metrics\n");
        }
        if self.has_network_metrics {
            s.push_str("  ✓ Network metrics\n");
        }
        
        s.push_str(&format!("\n🗂️ Cgroups List (sorted by CPU usage):\n"));
        
        // Show top CPU consumers first
        let top_consumers: Vec<_> = self.cgroups.iter()
            .filter(|c| c.cpu_usage_cores > 0.01)
            .take(10)
            .collect();
        
        if !top_consumers.is_empty() {
            s.push_str("\n📊 Top CPU Consumers:\n");
            for cgroup in &top_consumers {
                s.push_str(&format!("  • {:40} {:6.2} cores ({:5.1}%)",
                    cgroup.name,
                    cgroup.cpu_usage_cores,
                    cgroup.cpu_usage_pct
                ));
                
                if cgroup.syscall_rate > 0.0 {
                    s.push_str(&format!(" | {:.0} syscalls/sec", cgroup.syscall_rate));
                }
                s.push_str("\n");
            }
        }
        
        // Show all cgroups with basic info
        s.push_str(&format!("\n📋 All Cgroups ({} total):\n", self.cgroups.len()));
        for cgroup in &self.cgroups {
            s.push_str(&format!("  • {}", cgroup.name));
            
            // Show CPU usage if significant
            if cgroup.cpu_usage_cores > 0.01 {
                s.push_str(&format!(" [{:.2} cores]", cgroup.cpu_usage_cores));
            }
            
            let mut features = Vec::new();
            if cgroup.has_cpu { features.push("CPU"); }
            if cgroup.has_memory { features.push("MEM"); }
            if cgroup.has_syscalls { features.push("SYSCALL"); }
            if cgroup.has_network { features.push("NET"); }
            
            if !features.is_empty() {
                s.push_str(&format!(" [{}]", features.join(", ")));
            }
            s.push_str("\n");
        }
        
        if self.available_metrics.len() > 0 {
            s.push_str(&format!("\n📈 Available Cgroup Metrics ({}):\n", self.available_metrics.len()));
            for metric in &self.available_metrics[..10.min(self.available_metrics.len())] {
                s.push_str(&format!("  • {}\n", metric));
            }
            if self.available_metrics.len() > 10 {
                s.push_str(&format!("  ... and {} more\n", self.available_metrics.len() - 10));
            }
        }
        
        // Add recommendations
        s.push_str("\n💡 Analysis Recommendations:\n");
        if self.total_count > 0 {
            s.push_str("  • Use --isolate-cgroup flag to analyze specific cgroup isolation\n");
            s.push_str("  • Use 'complete' analysis for exhaustive correlation discovery\n");
            
            // Suggest interesting cgroups
            let system_cgroups: Vec<_> = self.cgroups.iter()
                .filter(|c| c.name.contains("system.slice"))
                .collect();
            if !system_cgroups.is_empty() {
                s.push_str(&format!("\n  System services found ({}):\n", system_cgroups.len()));
                for cgroup in system_cgroups.iter().take(3) {
                    s.push_str(&format!("    • {}\n", cgroup.name));
                }
            }
        } else {
            s.push_str("  • No cgroups found - this dataset may not have cgroup metrics\n");
        }
        
        s
    }
}