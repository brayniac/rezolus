/// Pre-defined chart templates that demonstrate best practices and common patterns
/// These are provided to the LLM as examples with explanations

pub struct ChartTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub significance: &'static str,
    pub example_json: &'static str,
}

pub const CHART_TEMPLATES: &[ChartTemplate] = &[
    ChartTemplate {
        name: "IPC (Instructions Per Cycle)",
        description: "Measures CPU efficiency by calculating the ratio of instructions executed to CPU cycles consumed",
        significance: "IPC > 1.0 indicates good CPU utilization with instruction-level parallelism. IPC < 0.5 may indicate memory stalls, branch mispredictions, or other pipeline inefficiencies. Modern CPUs can execute multiple instructions per cycle.",
        example_json: r#"{
  "title": "IPC (Instructions per Cycle)",
  "id": "ipc",
  "type": "line",
  "queries": [
    {
      "expr": "sum(irate(cpu_instructions[1m])) / sum(irate(cpu_cycles[1m]))",
      "legend": "System IPC"
    }
  ],
  "unit": "count"
}"#
    },
    
    ChartTemplate {
        name: "Per-Core IPC Heatmap",
        description: "Shows IPC for each CPU core as a heatmap to identify hot cores or imbalanced workloads",
        significance: "Helps identify: 1) Single-threaded bottlenecks (one core with different IPC), 2) NUMA effects (groups of cores with different IPC), 3) Thermal throttling (degraded IPC on specific cores)",
        example_json: r#"{
  "title": "IPC by Core",
  "id": "ipc-heatmap",
  "type": "heatmap",
  "queries": [
    {
      "expr": "irate(cpu_instructions[1m]) / irate(cpu_cycles[1m])",
      "legend": "Per-Core IPC"
    }
  ],
  "unit": "count"
}"#
    },
    
    ChartTemplate {
        name: "Memory Pressure",
        description: "Shows memory utilization and pressure indicators",
        significance: "High memory pressure (>80% usage) can lead to swapping and performance degradation. Monitor both used memory and available memory (which includes reclaimable caches).",
        example_json: r#"{
  "title": "Memory Pressure",
  "id": "memory-pressure",
  "type": "line",
  "queries": [
    {
      "expr": "(memory_total - memory_available) / memory_total",
      "legend": "Memory Pressure %"
    }
  ],
  "unit": "percentage"
}"#
    },
    
    ChartTemplate {
        name: "Network Throughput",
        description: "Monitors bidirectional network throughput in bits per second",
        significance: "Network saturation occurs when throughput plateaus near link capacity. Asymmetric traffic patterns may indicate issues. Sudden drops indicate connectivity problems.",
        example_json: r#"{
  "title": "Network Throughput",
  "id": "network-throughput",
  "type": "line",
  "queries": [
    {
      "expr": "irate(network_bytes{direction=\"receive\"}[1m]) * 8",
      "legend": "Rx Throughput"
    },
    {
      "expr": "irate(network_bytes{direction=\"transmit\"}[1m]) * 8",
      "legend": "Tx Throughput"
    }
  ],
  "unit": "bitrate"
}"#
    },
    
    ChartTemplate {
        name: "TCP Retransmissions",
        description: "Tracks TCP retransmission rate indicating network reliability issues",
        significance: "Retransmissions indicate packet loss or network congestion. Even small rates can significantly impact latency. Rates >0.1% of traffic suggest problems.",
        example_json: r#"{
  "title": "TCP Retransmissions",
  "id": "tcp-retransmissions",
  "type": "line",
  "queries": [
    {
      "expr": "irate(tcp_retransmit[1m])",
      "legend": "Retransmit Rate"
    }
  ],
  "unit": "rate"
}"#
    },
    
    ChartTemplate {
        name: "Disk I/O Latency Percentiles",
        description: "Shows distribution of disk I/O latencies using percentiles",
        significance: "P50 shows typical latency, P99 shows worst-case latency for 99% of operations. Large gaps between P50 and P99 indicate inconsistent performance. SSDs: <1ms typical, HDDs: 5-20ms typical.",
        example_json: r#"{
  "title": "Disk I/O Latency",
  "id": "disk-latency",
  "type": "scatter",
  "queries": [
    {
      "expr": "histogram_quantile(0.5, blockio_latency{op=\"read\"})",
      "legend": "P50 Read"
    },
    {
      "expr": "histogram_quantile(0.99, blockio_latency{op=\"read\"})",
      "legend": "P99 Read"
    },
    {
      "expr": "histogram_quantile(0.5, blockio_latency{op=\"write\"})",
      "legend": "P50 Write"
    },
    {
      "expr": "histogram_quantile(0.99, blockio_latency{op=\"write\"})",
      "legend": "P99 Write"
    }
  ],
  "unit": "time"
}"#
    },
    
    ChartTemplate {
        name: "Context Switch Rate",
        description: "Monitors both voluntary and involuntary context switches",
        significance: "High involuntary context switches (>10k/sec per core) indicate CPU contention. High voluntary switches may indicate lock contention or excessive I/O waiting.",
        example_json: r#"{
  "title": "Context Switches",
  "id": "context-switches",
  "type": "line",
  "queries": [
    {
      "expr": "irate(scheduler_context_switch{kind=\"voluntary\"}[1m])",
      "legend": "Voluntary"
    },
    {
      "expr": "irate(scheduler_context_switch{kind=\"involuntary\"}[1m])",
      "legend": "Involuntary"
    }
  ],
  "unit": "rate"
}"#
    },
    
    ChartTemplate {
        name: "NUMA Efficiency",
        description: "Tracks NUMA (Non-Uniform Memory Access) hit/miss ratios",
        significance: "NUMA misses cause remote memory access with 2-3x latency penalty. Hit ratio should be >90% for good performance. Low hit rates indicate poor memory locality.",
        example_json: r#"{
  "title": "NUMA Efficiency",
  "id": "numa-efficiency",
  "type": "line",
  "queries": [
    {
      "expr": "irate(memory_numa_hit[1m]) / (irate(memory_numa_hit[1m]) + irate(memory_numa_miss[1m]))",
      "legend": "NUMA Hit Ratio"
    }
  ],
  "unit": "percentage"
}"#
    },
    
    ChartTemplate {
        name: "Cgroup CPU Throttling",
        description: "Shows CPU throttling for containerized workloads",
        significance: "Throttling indicates that containers are hitting their CPU limits. Any throttling can cause latency spikes. Consider increasing CPU limits if throttling is frequent.",
        example_json: r#"{
  "title": "Container CPU Throttling",
  "id": "cgroup-throttling",
  "type": "line",
  "queries": [
    {
      "expr": "sum by (name) (irate(cgroup_cpu_throttled_time[1m])) / 1e9",
      "legend": "Throttled Time (seconds)"
    }
  ],
  "unit": "time"
}"#
    },
    
    ChartTemplate {
        name: "TCP Connection Latency",
        description: "Monitors TCP connection establishment time and smoothed RTT",
        significance: "Connection latency impacts application responsiveness. Local connections: <1ms, same datacenter: <5ms, cross-region: 20-100ms. High variance indicates network instability.",
        example_json: r#"{
  "title": "TCP Connection Performance",
  "id": "tcp-performance",
  "type": "line",
  "queries": [
    {
      "expr": "histogram_quantile(0.5, tcp_connect_latency)",
      "legend": "P50 Connect Time"
    },
    {
      "expr": "histogram_quantile(0.99, tcp_connect_latency)",
      "legend": "P99 Connect Time"
    },
    {
      "expr": "histogram_quantile(0.5, tcp_srtt)",
      "legend": "P50 RTT"
    }
  ],
  "unit": "time"
}"#
    },
    
    ChartTemplate {
        name: "SoftIRQ Distribution",
        description: "Shows time spent handling software interrupts by type",
        significance: "High softirq time (>10% CPU) can cause latency issues. NET_RX/TX indicates network processing overhead, SCHED indicates scheduler overhead. Imbalanced distribution suggests interrupt affinity issues.",
        example_json: r#"{
  "title": "SoftIRQ Time",
  "id": "softirq-time",
  "type": "multi",
  "queries": [
    {
      "expr": "irate(softirq_time{kind=\"net_rx\"}[1m])",
      "legend": "Network RX"
    },
    {
      "expr": "irate(softirq_time{kind=\"net_tx\"}[1m])",
      "legend": "Network TX"
    },
    {
      "expr": "irate(softirq_time{kind=\"sched\"}[1m])",
      "legend": "Scheduler"
    },
    {
      "expr": "irate(softirq_time{kind=\"timer\"}[1m])",
      "legend": "Timer"
    }
  ],
  "unit": "time"
}"#
    },
    
    ChartTemplate {
        name: "CPU Frequency Scaling",
        description: "Shows actual CPU frequency relative to base frequency using APERF/MPERF ratio",
        significance: "APERF/MPERF ratio indicates frequency scaling: 1.0 = base frequency, >1.0 = turbo boost active, <1.0 = throttling or power saving. Values typically range from 0.8 (power save) to 3.0+ (max turbo). Per-core variations indicate thermal or power constraints.",
        example_json: r#"{
  "title": "CPU Frequency Scaling",
  "id": "cpu-frequency-scaling",
  "type": "heatmap",
  "queries": [
    {
      "expr": "irate(cpu_aperf[1m]) / irate(cpu_mperf[1m])",
      "legend": "Frequency Multiplier"
    }
  ],
  "unit": "count"
}"#
    },
    
    ChartTemplate {
        name: "IPNS (Instructions Per Nanosecond)",
        description: "Measures CPU efficiency by calculating instructions executed per nanosecond, showing true throughput",
        significance: "IPNS directly measures CPU throughput accounting for frequency variations. Modern CPUs can achieve 2-4+ IPNS with good code. Low IPNS (<1) indicates stalls from memory access, branch mispredictions, or inefficient code. Unlike IPC, IPNS accounts for frequency scaling.",
        example_json: r#"{
  "title": "IPNS (Instructions per Nanosecond)",
  "id": "ipns",
  "type": "line",
  "queries": [
    {
      "expr": "sum(irate(cpu_instructions[1m])) / 1e9",
      "legend": "System IPNS"
    }
  ],
  "unit": "count"
}"#
    },
    
    ChartTemplate {
        name: "Per-Core IPNS Heatmap",
        description: "Shows Instructions Per Nanosecond for each CPU core to identify performance variations",
        significance: "Reveals per-core performance differences accounting for frequency. Cores running at different frequencies will show different IPNS even with same IPC. Helps identify: throttled cores, NUMA effects, uneven workload distribution, and cores stuck at low frequency.",
        example_json: r#"{
  "title": "IPNS by Core",
  "id": "ipns-heatmap",
  "type": "heatmap",
  "queries": [
    {
      "expr": "irate(cpu_instructions[1m]) / 1e9",
      "legend": "Per-Core IPNS"
    }
  ],
  "unit": "count"
}"#
    },
    
    ChartTemplate {
        name: "Scheduler Runqueue Wait Time",
        description: "Measures time tasks spend waiting in the runqueue before getting CPU time",
        significance: "Runqueue wait time directly impacts application latency. Values >1ms indicate CPU saturation. >10ms suggests severe overload. This metric is critical for understanding scheduling delays that affect response times.",
        example_json: r#"{
  "title": "Scheduler Runqueue Wait Time",
  "id": "runqueue-wait",
  "type": "scatter",
  "queries": [
    {
      "expr": "histogram_quantile(0.5, scheduler_runqueue_wait_time)",
      "legend": "P50"
    },
    {
      "expr": "histogram_quantile(0.9, scheduler_runqueue_wait_time)",
      "legend": "P90"
    },
    {
      "expr": "histogram_quantile(0.99, scheduler_runqueue_wait_time)",
      "legend": "P99"
    },
    {
      "expr": "histogram_quantile(0.999, scheduler_runqueue_wait_time)",
      "legend": "P99.9"
    }
  ],
  "unit": "time"
}"#
    },
    
    ChartTemplate {
        name: "Runqueue Wait by Core",
        description: "Shows per-core runqueue wait times to identify scheduling hotspots",
        significance: "Uneven runqueue wait times across cores indicate poor load balancing or CPU affinity issues. Cores with consistently higher wait times are overloaded. This helps optimize thread placement and identify cores that need load redistribution.",
        example_json: r#"{
  "title": "Runqueue Wait by Core",
  "id": "runqueue-wait-heatmap",
  "type": "heatmap",
  "queries": [
    {
      "expr": "histogram_quantile(0.99, scheduler_runqueue_wait_time)",
      "legend": "P99 Wait Time"
    }
  ],
  "unit": "time"
}"#
    },
];

/// Format chart templates for inclusion in LLM prompt
pub fn format_templates_for_prompt() -> String {
    let mut result = String::new();
    result.push_str("\nEXPERT CHART TEMPLATES AND PATTERNS:\n");
    result.push_str("These are proven monitoring patterns with explanations:\n\n");
    
    for template in CHART_TEMPLATES {
        result.push_str(&format!("### {}\n", template.name));
        result.push_str(&format!("**What it measures**: {}\n", template.description));
        result.push_str(&format!("**Why it matters**: {}\n", template.significance));
        result.push_str(&format!("**Chart definition**:\n```json\n{}\n```\n\n", template.example_json));
    }
    
    result.push_str("\nUSE THESE PATTERNS when relevant to the user's request. ");
    result.push_str("They represent best practices for system observability.\n");
    
    result
}