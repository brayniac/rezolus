use std::collections::HashSet;
use std::path::Path;

use crate::viewer::tsdb::Tsdb;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading kiad.parquet TSDB file...");
    
    let tsdb = Tsdb::load(Path::new("kiad.parquet"))?;
    
    println!("File loaded successfully!");
    println!("Source: {}", tsdb.source());
    println!("Version: {}", tsdb.version());
    println!("Sampling interval: {} seconds", tsdb.interval());
    println!();
    
    // Get all counter metric names
    let counter_names = tsdb.counter_names();
    println!("Total counter metrics: {}", counter_names.len());
    println!();
    
    // Show first 20 counter metrics
    println!("First 20 counter metrics:");
    for (i, name) in counter_names.iter().take(20).enumerate() {
        println!("  {}: {}", i + 1, name);
    }
    println!();
    
    // Find all cgroup-related metrics
    let cgroup_metrics: Vec<&str> = counter_names.iter()
        .filter(|name| name.contains("cgroup"))
        .copied()
        .collect();
    
    println!("Cgroup-related metrics ({} total):", cgroup_metrics.len());
    for metric in &cgroup_metrics {
        println!("  - {}", metric);
    }
    println!();
    
    // Extract cgroup names from cgroup_cpu_usage metric
    if counter_names.contains(&"cgroup_cpu_usage") {
        println!("Analyzing cgroup_cpu_usage metric for available cgroups...");
        let label_values = tsdb.get_label_values("cgroup_cpu_usage");
        
        let mut cgroups = HashSet::new();
        for label_value in &label_values {
            if label_value.starts_with("cgroup=") {
                let cgroup_name = label_value.strip_prefix("cgroup=").unwrap_or("");
                if !cgroup_name.is_empty() {
                    cgroups.insert(cgroup_name);
                }
            }
        }
        
        println!("Available cgroups ({} total):", cgroups.len());
        let mut sorted_cgroups: Vec<_> = cgroups.into_iter().collect();
        sorted_cgroups.sort();
        for cgroup in &sorted_cgroups {
            println!("  - {}", cgroup);
        }
        println!();
    }
    
    // Check for cgroup_syscall metrics
    let syscall_metrics: Vec<&str> = counter_names.iter()
        .filter(|name| name.contains("cgroup_syscall"))
        .copied()
        .collect();
        
    if !syscall_metrics.is_empty() {
        println!("Cgroup syscall metrics ({} total):", syscall_metrics.len());
        for metric in &syscall_metrics {
            println!("  - {}", metric);
            
            // Get label values to see operation types
            let labels = tsdb.get_label_values(metric);
            let mut operations = HashSet::new();
            for label in &labels {
                if label.starts_with("operation=") {
                    let op = label.strip_prefix("operation=").unwrap_or("");
                    if !op.is_empty() {
                        operations.insert(op);
                    }
                }
            }
            
            if !operations.is_empty() {
                let mut sorted_ops: Vec<_> = operations.into_iter().collect();
                sorted_ops.sort();
                println!("    Operations: {:?}", sorted_ops);
            }
        }
        println!();
    } else {
        println!("No cgroup_syscall metrics found.");
        println!();
    }
    
    // Show gauge and histogram metrics too
    let gauge_names = tsdb.gauge_names();
    let histogram_names = tsdb.histogram_names();
    
    println!("Summary:");
    println!("  - Counter metrics: {}", counter_names.len());
    println!("  - Gauge metrics: {}", gauge_names.len());
    println!("  - Histogram metrics: {}", histogram_names.len());
    
    if !gauge_names.is_empty() {
        println!("\nFirst 10 gauge metrics:");
        for (i, name) in gauge_names.iter().take(10).enumerate() {
            println!("  {}: {}", i + 1, name);
        }
    }
    
    if !histogram_names.is_empty() {
        println!("\nFirst 10 histogram metrics:");
        for (i, name) in histogram_names.iter().take(10).enumerate() {
            println!("  {}: {}", i + 1, name);
        }
    }
    
    Ok(())
}