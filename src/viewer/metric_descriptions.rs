use std::collections::HashMap;

/// Get hardcoded metric descriptions extracted from agent code
/// These provide context to the LLM about what each metric means
fn get_descriptions_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    
    // Block I/O metrics
    m.insert("blockio_bytes", "The number of bytes transferred for block devices");
    m.insert("blockio_operations", "The number of completed operations for block devices");
    m.insert("blockio_latency", "Distribution of block I/O operation latency in nanoseconds");
    m.insert("blockio_size", "Distribution of block I/O operation sizes in bytes");
    
    // Cgroup metrics
    m.insert("cgroup_cpu_bandwidth_period_duration", "The duration of the CFS bandwidth period in nanoseconds");
    m.insert("cgroup_cpu_bandwidth_periods", "The total number of periods in a cgroup with the CPU bandwidth set");
    m.insert("cgroup_cpu_bandwidth_quota", "The CPU bandwidth quota assigned to the cgroup in nanoseconds");
    m.insert("cgroup_cpu_bandwidth_throttled_periods", "The total number of throttled periods in a cgroup with the CPU bandwidth set");
    m.insert("cgroup_cpu_bandwidth_throttled_time", "The total throttled time of all runqueues in a cgroup");
    m.insert("cgroup_cpu_cycles", "The number of elapsed CPU cycles on a per-cgroup basis");
    m.insert("cgroup_cpu_instructions", "The number of instructions retired on a per-cgroup basis");
    m.insert("cgroup_cpu_migrations", "The number of times a process in a cgroup migrated from one CPU to another");
    m.insert("cgroup_cpu_throttled", "The number of times all runqueues in a cgroup throttled by the CPU controller");
    m.insert("cgroup_cpu_throttled_time", "The total time all runqueues in a cgroup throttled by the CPU controller");
    m.insert("cgroup_cpu_tlb_flush", "The number of TLB flush events on a per-cgroup basis");
    m.insert("cgroup_cpu_usage", "The amount of CPU time spent on a per-cgroup basis");
    m.insert("cgroup_scheduler_context_switch", "The number of context switches on a per-cgroup basis");
    m.insert("cgroup_scheduler_offcpu", "Tracks the time when tasks were off-CPU on a per-cgroup basis");
    m.insert("cgroup_scheduler_runqueue_wait", "Tracks time spent in the runqueue on a per-cgroup basis");
    m.insert("cgroup_syscall", "System call counts on a per-cgroup basis");
    
    // CPU metrics
    m.insert("cpu_cores", "The total number of logical cores that are currently online");
    m.insert("cpu_cycles", "The number of elapsed CPU cycles");
    m.insert("cpu_instructions", "The number of instructions retired");
    m.insert("cpu_l3_access", "The number of L3 cache accesses");
    m.insert("cpu_l3_miss", "The number of L3 cache misses");
    m.insert("cpu_migrations", "The number of process CPU migrations between cores");
    m.insert("cpu_tlb_flush", "The number of TLB flush events");
    m.insert("cpu_usage", "The amount of CPU time spent in different states (user, system, idle, etc.)");
    
    // GPU metrics (NVIDIA)
    m.insert("gpu_clock", "The current GPU clock speed in Hertz");
    m.insert("gpu_energy_consumption", "The GPU energy consumption in milliJoules");
    m.insert("gpu_memory", "The amount of GPU memory (free or used)");
    m.insert("gpu_memory_utilization", "Percentage of time GPU memory was being accessed (0-100)");
    m.insert("gpu_pcie_bandwidth", "The PCIe bandwidth in Bytes/s");
    m.insert("gpu_pcie_throughput", "The current PCIe throughput in Bytes/s");
    m.insert("gpu_power_usage", "The current GPU power usage in milliwatts");
    m.insert("gpu_temperature", "The current GPU temperature in degrees Celsius");
    m.insert("gpu_utilization", "Percentage of time the GPU was executing kernels (0-100)");
    
    // Memory metrics
    m.insert("memory_available", "The amount of system memory that is available for allocation");
    m.insert("memory_buffers", "The amount of system memory used for buffers");
    m.insert("memory_cached", "The amount of system memory used by the page cache");
    m.insert("memory_free", "The amount of system memory that is currently free");
    m.insert("memory_total", "The total amount of system memory");
    
    // NUMA metrics
    m.insert("memory_numa_foreign", "NUMA allocations not intended for a node but serviced by this node");
    m.insert("memory_numa_hit", "NUMA allocations that succeeded on the intended node");
    m.insert("memory_numa_interleave", "NUMA interleave policy allocations that succeeded");
    m.insert("memory_numa_local", "NUMA allocations that succeeded on the local node");
    m.insert("memory_numa_miss", "NUMA allocations that did not succeed on the intended node");
    m.insert("memory_numa_other", "NUMA allocations on this node by a process on another node");
    
    // Network metrics
    m.insert("network_bytes", "The number of bytes transferred over the network");
    m.insert("network_packets", "The number of packets transferred over the network");
    m.insert("network_drop", "Packets dropped in the network stack due to errors or resource exhaustion");
    m.insert("network_transmit_busy", "Packets encountering retryable device busy status");
    m.insert("network_transmit_complete", "Packets successfully transmitted by the driver");
    m.insert("network_transmit_timeout", "Transmit timeout events indicating hardware issues");
    m.insert("network_carrier_changes", "Number of times the network carrier status changed");
    m.insert("network_receive_dropped", "Packets dropped on receive");
    m.insert("network_transmit_dropped", "Packets dropped on transmit");
    m.insert("network_receive_errors_crc", "CRC errors on received packets");
    m.insert("network_receive_errors_missed", "Missed packets on receive");
    
    // Rezolus self-monitoring metrics
    m.insert("rezolus_blockio_operations", "Rezolus process filesystem I/O operations");
    m.insert("rezolus_bpf_run_count", "The number of times Rezolus BPF programs have been run");
    m.insert("rezolus_bpf_run_time", "The amount of time Rezolus BPF programs have been executing");
    m.insert("rezolus_context_switch", "Context switches for the Rezolus process");
    m.insert("rezolus_cpu_usage", "CPU time consumed by the Rezolus process");
    m.insert("rezolus_memory_page_faults", "Page faults requiring I/O for Rezolus process");
    m.insert("rezolus_memory_page_reclaims", "Page faults serviced by reclaiming for Rezolus process");
    m.insert("rezolus_memory_usage_resident_set_size", "The total amount of memory allocated by Rezolus");
    
    // Scheduler metrics
    m.insert("scheduler_context_switch", "The number of context switches");
    m.insert("scheduler_offcpu", "Distribution of the amount of time tasks were off-CPU");
    m.insert("scheduler_running", "Distribution of the amount of time tasks were on-CPU");
    m.insert("scheduler_runqueue_latency", "Distribution of time tasks waited in the runqueue");
    m.insert("scheduler_runqueue_wait", "Time spent in the runqueue on a per-CPU basis");
    
    // SoftIRQ metrics
    m.insert("softirq", "The count of software interrupts by type");
    m.insert("softirq_time", "The time spent in software interrupt handlers");
    
    // System call metrics
    m.insert("syscall", "System call counts by category (read, write, network, etc.)");
    m.insert("syscall_latency", "Distribution of system call latencies by category");
    
    // TCP metrics
    m.insert("tcp_bytes", "The number of bytes transferred over TCP");
    m.insert("tcp_packets", "The number of packets transferred over TCP");
    m.insert("tcp_connect_latency", "Distribution of latency for establishing TCP connections");
    m.insert("tcp_jitter", "Distribution of TCP latency jitter");
    m.insert("tcp_packet_latency", "Distribution of latency from socket readable to userspace read");
    m.insert("tcp_retransmit", "The number of TCP packets that were re-transmitted");
    m.insert("tcp_size", "Distribution of TCP packet sizes");
    m.insert("tcp_srtt", "Distribution of TCP smoothed round-trip time");
    
    m
}

/// Get a description for a metric
pub fn get_metric_description(metric_name: &str) -> Option<&'static str> {
    // Create a static map on first use
    static mut DESCRIPTIONS: Option<HashMap<&'static str, &'static str>> = None;
    static INIT: std::sync::Once = std::sync::Once::new();
    
    unsafe {
        INIT.call_once(|| {
            DESCRIPTIONS = Some(get_descriptions_map());
        });
        
        DESCRIPTIONS.as_ref()
            .and_then(|map| map.get(metric_name).copied())
    }
}