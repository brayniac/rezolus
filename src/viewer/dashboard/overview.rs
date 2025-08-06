use super::common::*;

/// Overview dashboard with key metrics
pub fn dashboard() -> PromQLDashboard {
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
                                expr: "avg(sum by (id) (irate(cpu_usage[1m]))) / 1e9".to_string(),
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