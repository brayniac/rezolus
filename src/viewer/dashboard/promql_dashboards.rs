use super::*;
use serde::{Deserialize, Serialize};

/// Dashboard definition using PromQL queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLDashboard {
    pub name: String,
    pub sections: Vec<DashboardSection>,
    pub groups: Vec<PromQLGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSection {
    pub name: String,
    pub route: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLGroup {
    pub name: String,
    pub id: String,
    pub panels: Vec<PromQLPanel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLPanel {
    pub title: String,
    pub id: String,
    #[serde(rename = "type")]
    pub panel_type: PanelType,
    pub queries: Vec<PromQLQueryDef>,
    pub unit: Unit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<PanelOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelType {
    Line,
    Heatmap,
    Scatter,
    Multi,
    Gauge,
    Stat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLQueryDef {
    pub expr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_scale: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<bool>,
}

/// CPU dashboard using PromQL
pub fn cpu_dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "CPU".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Utilization".to_string(),
                id: "utilization".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "CPU Busy %".to_string(),
                        id: "cpu-busy".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_usage[1m]) / 1e9".to_string(),
                                legend: Some("Total".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU Busy % by Core".to_string(),
                        id: "cpu-busy-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_usage[1m]) by (cpu) / 1e9".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU User %".to_string(),
                        id: "cpu-user".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_usage{state=\"user\"}[1m]) / 1e9".to_string(),
                                legend: Some("User".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU System %".to_string(),
                        id: "cpu-system".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_usage{state=\"system\"}[1m]) / 1e9".to_string(),
                                legend: Some("System".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "Performance".to_string(),
                id: "performance".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Instructions Per Cycle".to_string(),
                        id: "ipc".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_instructions[1m]) / irate(cpu_cycles[1m])".to_string(),
                                legend: Some("IPC".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Cache Misses".to_string(),
                        id: "cache-misses".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_cache_misses[1m])".to_string(),
                                legend: Some("Misses/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Branch Mispredictions".to_string(),
                        id: "branch-misses".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_branch_misses[1m])".to_string(),
                                legend: Some("Mispredictions/sec".to_string()),
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

/// Network dashboard using PromQL
pub fn network_dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Network".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Traffic".to_string(),
                id: "traffic".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Bandwidth".to_string(),
                        id: "bandwidth".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_bytes{direction=\"transmit\"}[1m]) * 8".to_string(),
                                legend: Some("Transmit".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(network_bytes{direction=\"receive\"}[1m]) * 8".to_string(),
                                legend: Some("Receive".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bitrate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Packets".to_string(),
                        id: "packets".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_packets{direction=\"transmit\"}[1m])".to_string(),
                                legend: Some("Transmit".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(network_packets{direction=\"receive\"}[1m])".to_string(),
                                legend: Some("Receive".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "TCP".to_string(),
                id: "tcp".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "TCP Packet Latency".to_string(),
                        id: "tcp-latency".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, tcp_packet_latency)".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, tcp_packet_latency)".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, tcp_packet_latency)".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Time,
                        options: Some(PanelOptions {
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                ],
            },
        ],
    }
}

/// BlockIO dashboard using PromQL
pub fn blockio_dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "BlockIO".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Operations".to_string(),
                id: "operations".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Throughput".to_string(),
                        id: "throughput".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(blockio_bytes{op=\"read\"}[1m])".to_string(),
                                legend: Some("Read".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(blockio_bytes{op=\"write\"}[1m])".to_string(),
                                legend: Some("Write".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Datarate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "IOPS".to_string(),
                        id: "iops".to_string(),
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
                        unit: Unit::Count,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "Latency".to_string(),
                id: "latency".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Read Latency".to_string(),
                        id: "read-latency".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, blockio_latency{op=\"read\"})".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, blockio_latency{op=\"read\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Time,
                        options: Some(PanelOptions {
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                    PromQLPanel {
                        title: "Write Latency".to_string(),
                        id: "write-latency".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, blockio_latency{op=\"write\"})".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, blockio_latency{op=\"write\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Time,
                        options: Some(PanelOptions {
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                ],
            },
        ],
    }
}

/// Overview dashboard with key metrics
pub fn overview_dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Overview".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "System".to_string(),
                id: "system".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "CPU Usage".to_string(),
                        id: "cpu".to_string(),
                        panel_type: PanelType::Gauge,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg(irate(cpu_usage[1m])) / 1e9".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Network Bandwidth".to_string(),
                        id: "network".to_string(),
                        panel_type: PanelType::Stat,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(network_bytes[1m])) * 8".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Bitrate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Disk IOPS".to_string(),
                        id: "disk".to_string(),
                        panel_type: PanelType::Stat,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(blockio_operations[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                ],
            },
        ],
    }
}

fn default_sections() -> Vec<DashboardSection> {
    vec![
        DashboardSection { name: "Overview".to_string(), route: "/overview".to_string() },
        DashboardSection { name: "CPU".to_string(), route: "/cpu".to_string() },
        DashboardSection { name: "Network".to_string(), route: "/network".to_string() },
        DashboardSection { name: "Scheduler".to_string(), route: "/scheduler".to_string() },
        DashboardSection { name: "Syscall".to_string(), route: "/syscall".to_string() },
        DashboardSection { name: "Softirq".to_string(), route: "/softirq".to_string() },
        DashboardSection { name: "BlockIO".to_string(), route: "/blockio".to_string() },
        DashboardSection { name: "cgroups".to_string(), route: "/cgroups".to_string() },
        DashboardSection { name: "Rezolus".to_string(), route: "/rezolus".to_string() },
    ]
}

/// Get dashboard definition by name
pub fn get_dashboard(name: &str) -> Option<PromQLDashboard> {
    match name {
        "cpu" => Some(cpu_dashboard()),
        "network" => Some(network_dashboard()),
        "blockio" => Some(blockio_dashboard()),
        "overview" => Some(overview_dashboard()),
        _ => None,
    }
}

/// Generate all dashboard definitions
pub fn generate_all_dashboards() -> Vec<PromQLDashboard> {
    vec![
        overview_dashboard(),
        cpu_dashboard(),
        network_dashboard(),
        blockio_dashboard(),
    ]
}