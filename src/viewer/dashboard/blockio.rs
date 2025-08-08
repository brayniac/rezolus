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
                        unit: Unit::Rate,
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
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Flush Operations".to_string(),
                        id: "flush-ops".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(blockio_operations{op=\"flush\"}[1m])".to_string(),
                                legend: Some("Flush".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Discard Operations".to_string(),
                        id: "discard-ops".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(blockio_operations{op=\"discard\"}[1m])".to_string(),
                                legend: Some("Discard".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "Size Distribution".to_string(),
                id: "size".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Read Size Distribution".to_string(),
                        id: "read-size".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, blockio_size{op=\"read\"})".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, blockio_size{op=\"read\"})".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, blockio_size{op=\"read\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, blockio_size{op=\"read\"})".to_string(),
                                legend: Some("p999".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bytes,
                        options: Some(PanelOptions {
                            cgroup_filter: None,
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                    PromQLPanel {
                        title: "Write Size Distribution".to_string(),
                        id: "write-size".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, blockio_size{op=\"write\"})".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, blockio_size{op=\"write\"})".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, blockio_size{op=\"write\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, blockio_size{op=\"write\"})".to_string(),
                                legend: Some("p999".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bytes,
                        options: Some(PanelOptions {
                            cgroup_filter: None,
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
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
                            cgroup_filter: None,
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
                            cgroup_filter: None,
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                    PromQLPanel {
                        title: "Flush Latency".to_string(),
                        id: "flush-latency".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, blockio_latency{op=\"flush\"})".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, blockio_latency{op=\"flush\"})".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, blockio_latency{op=\"flush\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, blockio_latency{op=\"flush\"})".to_string(),
                                legend: Some("p999".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Time,
                        options: Some(PanelOptions {
                            cgroup_filter: None,
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