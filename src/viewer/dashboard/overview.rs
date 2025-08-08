use super::common::*;

/// Overview dashboard with key metrics
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Overview".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "System Resources".to_string(),
                id: "resources".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "CPU Utilization".to_string(),
                        id: "cpu-util".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg(sum by (id) (irate(cpu_usage[1m]))) / 1e9".to_string(),
                                legend: Some("CPU %".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Memory Cache".to_string(),
                        id: "memory".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "memory_cached".to_string(),
                                legend: Some("Cached".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "memory_buffers".to_string(),
                                legend: Some("Buffers".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bytes,
                        options: None,
                    },
                    PromQLPanel {
                        title: "IPC (Instructions per Cycle)".to_string(),
                        id: "ipc".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_instructions[1m])) / sum(irate(cpu_cycles[1m]))".to_string(),
                                legend: Some("IPC".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "Network & I/O".to_string(),
                id: "network-io".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Network Traffic".to_string(),
                        id: "network-traffic".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_bytes{direction=\"receive\"}[1m]) * 8".to_string(),
                                legend: Some("Receive".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(network_bytes{direction=\"transmit\"}[1m]) * 8".to_string(),
                                legend: Some("Transmit".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bitrate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Disk Operations".to_string(),
                        id: "disk-ops".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(blockio_operations{op=\"read\"}[1m])".to_string(),
                                legend: Some("Read".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(blockio_operations{op=\"write\"}[1m])".to_string(),
                                legend: Some("Write".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "TCP Traffic".to_string(),
                        id: "tcp-traffic".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(tcp_bytes{direction=\"receive\"}[1m]) * 8".to_string(),
                                legend: Some("Receive".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(tcp_bytes{direction=\"transmit\"}[1m]) * 8".to_string(),
                                legend: Some("Transmit".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bitrate,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "System Activity".to_string(),
                id: "activity".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Context Switches".to_string(),
                        id: "context-switches".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(scheduler_context_switch[1m]))".to_string(),
                                legend: Some("Context Switches/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "System Calls".to_string(),
                        id: "syscalls".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(syscall[1m]))".to_string(),
                                legend: Some("Syscalls/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Software Interrupts".to_string(),
                        id: "softirq".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(softirq[1m]))".to_string(),
                                legend: Some("Softirq/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                ],
            },
        ],
    }
}