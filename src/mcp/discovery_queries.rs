/// Meaningful PromQL queries inspired by the viewer dashboards
/// These represent actual metrics that users care about, not just raw counters

#[derive(Clone)]
pub struct DiscoveryQuery {
    pub name: &'static str,
    pub category: &'static str,
    pub query: &'static str,
}

/// Core metrics that are commonly analyzed
pub const CORE_METRICS: &[DiscoveryQuery] = &[
    // CPU Utilization
    DiscoveryQuery {
        name: "CPU Utilization %",
        category: "cpu",
        query: "irate(cpu_usage[5m]) / cpu_cores / 1000000000",
    },
    DiscoveryQuery {
        name: "CPU User %",
        category: "cpu",
        query: "irate(cpu_usage{state=\"user\"}[5m]) / cpu_cores / 1000000000",
    },
    DiscoveryQuery {
        name: "CPU System %",
        category: "cpu",
        query: "irate(cpu_usage{state=\"system\"}[5m]) / cpu_cores / 1000000000",
    },
    
    // CPU Performance
    DiscoveryQuery {
        name: "Instructions per Cycle",
        category: "cpu",
        query: "irate(cpu_instructions[5m]) / irate(cpu_cycles[5m])",
    },
    DiscoveryQuery {
        name: "CPU Frequency",
        category: "cpu",
        query: "irate(cpu_tsc[5m]) * irate(cpu_aperf[5m]) / irate(cpu_mperf[5m]) / cpu_cores",
    },
    DiscoveryQuery {
        name: "L3 Cache Hit %",
        category: "cpu",
        query: "(1 - irate(cpu_l3_miss[5m]) / irate(cpu_l3_access[5m])) * 100",
    },
    
    // Network
    DiscoveryQuery {
        name: "Network TX Rate",
        category: "network",
        query: "sum(irate(network_bytes{direction=\"transmit\"}[5m]))",
    },
    DiscoveryQuery {
        name: "Network RX Rate",
        category: "network",
        query: "sum(irate(network_bytes{direction=\"receive\"}[5m]))",
    },
    DiscoveryQuery {
        name: "Network Packet TX Rate",
        category: "network",
        query: "sum(irate(network_packets{direction=\"transmit\"}[5m]))",
    },
    DiscoveryQuery {
        name: "Network Packet RX Rate",
        category: "network",
        query: "sum(irate(network_packets{direction=\"receive\"}[5m]))",
    },
    
    // Syscalls
    DiscoveryQuery {
        name: "Syscall Rate",
        category: "syscall",
        query: "sum(irate(syscall[5m]))",
    },
    DiscoveryQuery {
        name: "Read Syscall Rate",
        category: "syscall",
        query: "irate(syscall{syscall=\"read\"}[5m])",
    },
    DiscoveryQuery {
        name: "Write Syscall Rate",
        category: "syscall",
        query: "irate(syscall{syscall=\"write\"}[5m])",
    },
    
    // Block I/O
    DiscoveryQuery {
        name: "Disk Read Rate",
        category: "blockio",
        query: "sum(irate(blockio_bytes{operation=\"read\"}[5m]))",
    },
    DiscoveryQuery {
        name: "Disk Write Rate",
        category: "blockio",
        query: "sum(irate(blockio_bytes{operation=\"write\"}[5m]))",
    },
    DiscoveryQuery {
        name: "Disk Read IOPS",
        category: "blockio",
        query: "sum(irate(blockio_operations{operation=\"read\"}[5m]))",
    },
    DiscoveryQuery {
        name: "Disk Write IOPS",
        category: "blockio",
        query: "sum(irate(blockio_operations{operation=\"write\"}[5m]))",
    },
    
    // Memory
    DiscoveryQuery {
        name: "Memory Used %",
        category: "memory",
        query: "(1 - memory_available / memory_total) * 100",
    },
    DiscoveryQuery {
        name: "Memory Cache %",
        category: "memory",
        query: "memory_cached / memory_total * 100",
    },
    
    // Scheduler
    DiscoveryQuery {
        name: "Context Switch Rate",
        category: "scheduler",
        query: "irate(scheduler_context_switches[5m])",
    },
    DiscoveryQuery {
        name: "Process Creation Rate",
        category: "scheduler",
        query: "irate(scheduler_processes_created[5m])",
    },
    DiscoveryQuery {
        name: "Running Processes",
        category: "scheduler",
        query: "scheduler_processes_running",
    },
    DiscoveryQuery {
        name: "Blocked Processes",
        category: "scheduler",
        query: "scheduler_processes_blocked",
    },
];

/// Per-CPU metrics (these will use sum by (id))
pub const PER_CPU_METRICS: &[DiscoveryQuery] = &[
    DiscoveryQuery {
        name: "Per-CPU Utilization",
        category: "cpu",
        query: "sum by (id) (irate(cpu_usage[5m])) / 1000000000",
    },
    DiscoveryQuery {
        name: "Per-CPU IPC",
        category: "cpu",
        query: "sum by (id) (irate(cpu_instructions[5m])) / sum by (id) (irate(cpu_cycles[5m]))",
    },
    DiscoveryQuery {
        name: "Per-CPU Frequency",
        category: "cpu",
        query: "sum by (id) (irate(cpu_tsc[5m])) * sum by (id) (irate(cpu_aperf[5m])) / sum by (id) (irate(cpu_mperf[5m]))",
    },
    DiscoveryQuery {
        name: "Per-CPU Migrations To",
        category: "cpu",
        query: "sum by (id) (irate(cpu_migrations{direction=\"to\"}[5m]))",
    },
];

/// CGroup metrics (these will use sum by (name))
pub const CGROUP_METRICS: &[DiscoveryQuery] = &[
    DiscoveryQuery {
        name: "CGroup CPU Usage",
        category: "cgroup",
        query: "sum by (name) (irate(cgroup_cpu_usage[5m]))",
    },
    DiscoveryQuery {
        name: "CGroup CPU Cycles",
        category: "cgroup",
        query: "sum by (name) (irate(cgroup_cpu_cycles[5m]))",
    },
    DiscoveryQuery {
        name: "CGroup Instructions",
        category: "cgroup",
        query: "sum by (name) (irate(cgroup_cpu_instructions[5m]))",
    },
    DiscoveryQuery {
        name: "CGroup Syscalls",
        category: "cgroup",
        query: "sum by (name) (irate(cgroup_syscall[5m]))",
    },
    DiscoveryQuery {
        name: "CGroup Network TX",
        category: "cgroup",
        query: "sum by (name) (irate(cgroup_network_bytes{direction=\"transmit\"}[5m]))",
    },
    DiscoveryQuery {
        name: "CGroup Network RX",
        category: "cgroup",
        query: "sum by (name) (irate(cgroup_network_bytes{direction=\"receive\"}[5m]))",
    },
];

/// Get queries to check based on available metrics in the TSDB
pub fn get_discovery_queries(tsdb: &crate::viewer::tsdb::Tsdb) -> Vec<DiscoveryQuery> {
    let mut queries = Vec::new();
    
    // Check which metric types are available
    let has_cpu = tsdb.counter_names().iter().any(|n| n.starts_with("cpu_"));
    let has_network = tsdb.counter_names().iter().any(|n| n.starts_with("network_"));
    let has_cgroup = tsdb.counter_names().iter().any(|n| n.starts_with("cgroup_"));
    let has_blockio = tsdb.counter_names().iter().any(|n| n.starts_with("blockio_"));
    let has_syscall = tsdb.counter_names().iter().any(|n| n == &"syscall");
    
    // Add core metrics based on what's available
    for metric in CORE_METRICS {
        let should_add = match metric.category {
            "cpu" => has_cpu,
            "network" => has_network,
            "blockio" => has_blockio,
            "syscall" => has_syscall,
            "memory" => true, // Memory gauges are usually present
            "scheduler" => true, // Scheduler metrics are usually present
            _ => false,
        };
        
        if should_add {
            queries.push(metric.clone());
        }
    }
    
    // Add per-CPU metrics if we have CPU data
    if has_cpu {
        for metric in PER_CPU_METRICS {
            queries.push(metric.clone());
        }
    }
    
    // Add cgroup metrics if we have cgroup data
    if has_cgroup {
        for metric in CGROUP_METRICS {
            queries.push(metric.clone());
        }
    }
    
    queries
}

pub struct MetricPair {
    pub name1: &'static str,
    pub query1: &'static str,
    pub name2: &'static str, 
    pub query2: &'static str,
}

/// Get metric pairs that are known to be interesting for correlation
pub fn get_interesting_pairs() -> Vec<MetricPair> {
    vec![
        // CPU relationships
        MetricPair {
            name1: "CPU Cycles",
            query1: "irate(cpu_cycles[5m])",
            name2: "CPU Instructions",
            query2: "irate(cpu_instructions[5m])",
        },
        MetricPair {
            name1: "CPU APERF",
            query1: "irate(cpu_aperf[5m])",
            name2: "CPU MPERF",
            query2: "irate(cpu_mperf[5m])",
        },
        MetricPair {
            name1: "L3 Cache Misses",
            query1: "irate(cpu_l3_miss[5m])",
            name2: "L3 Cache Accesses",
            query2: "irate(cpu_l3_access[5m])",
        },
        
        // Network relationships
        MetricPair {
            name1: "Network TX Bytes",
            query1: "sum(irate(network_bytes{direction=\"transmit\"}[5m]))",
            name2: "Network TX Packets",
            query2: "sum(irate(network_packets{direction=\"transmit\"}[5m]))",
        },
        MetricPair {
            name1: "Network RX Bytes",
            query1: "sum(irate(network_bytes{direction=\"receive\"}[5m]))",
            name2: "Network RX Packets",
            query2: "sum(irate(network_packets{direction=\"receive\"}[5m]))",
        },
        
        // I/O relationships
        MetricPair {
            name1: "Disk Read Bytes",
            query1: "sum(irate(blockio_bytes{operation=\"read\"}[5m]))",
            name2: "Disk Read Operations",
            query2: "sum(irate(blockio_operations{operation=\"read\"}[5m]))",
        },
        MetricPair {
            name1: "Disk Write Bytes",
            query1: "sum(irate(blockio_bytes{operation=\"write\"}[5m]))",
            name2: "Disk Write Operations",
            query2: "sum(irate(blockio_operations{operation=\"write\"}[5m]))",
        },
        
        // System-wide relationships
        MetricPair {
            name1: "CPU Utilization %",
            query1: "irate(cpu_usage[5m]) / cpu_cores / 1000000000",
            name2: "Syscall Rate",
            query2: "sum(irate(syscall[5m]))",
        },
        MetricPair {
            name1: "CPU Utilization %",
            query1: "irate(cpu_usage[5m]) / cpu_cores / 1000000000",
            name2: "Network Total Throughput",
            query2: "sum(irate(network_bytes[5m]))",
        },
        MetricPair {
            name1: "Syscall Rate",
            query1: "sum(irate(syscall[5m]))",
            name2: "Network Total Throughput",
            query2: "sum(irate(network_bytes[5m]))",
        },
        
        // CGroup vs system
        MetricPair {
            name1: "CGroup Total CPU",
            query1: "sum(irate(cgroup_cpu_usage[5m]))",
            name2: "System CPU",
            query2: "irate(cpu_usage[5m])",
        },
        MetricPair {
            name1: "CGroup Total Syscalls",
            query1: "sum(irate(cgroup_syscall[5m]))",
            name2: "System Syscalls",
            query2: "sum(irate(syscall[5m]))",
        },
    ]
}