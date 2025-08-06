use super::common::*;

/// BlockIO dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "BlockIO".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Operations".to_string(),
                id: "operations".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Read Throughput".to_string(),
                        id: "throughput-read".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(blockio_bytes{op=\"read\"}[1m])".to_string(),
                                legend: Some("Read".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Datarate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Write Throughput".to_string(),
                        id: "throughput-write".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
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
                        title: "Read IOPS".to_string(),
                        id: "iops-read".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(blockio_operations{op=\"read\"}[1m])".to_string(),
                                legend: Some("Read".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Write IOPS".to_string(),
                        id: "iops-write".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
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
                                expr: "histogram_quantile(0.9, blockio_latency{op=\"read\"})".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, blockio_latency{op=\"read\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, blockio_latency{op=\"read\"})".to_string(),
                                legend: Some("p999".to_string()),
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
                                expr: "histogram_quantile(0.9, blockio_latency{op=\"write\"})".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, blockio_latency{op=\"write\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, blockio_latency{op=\"write\"})".to_string(),
                                legend: Some("p999".to_string()),
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