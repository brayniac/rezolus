use super::common::*;

/// Network dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Network".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Traffic".to_string(),
                id: "traffic".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Transmit Bandwidth".to_string(),
                        id: "bandwidth-tx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
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
                        title: "Receive Bandwidth".to_string(),
                        id: "bandwidth-rx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
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
                        title: "Transmit Packets".to_string(),
                        id: "packets-tx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_packets{direction=\"transmit\"}[1m])".to_string(),
                                legend: Some("Transmit".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Receive Packets".to_string(),
                        id: "packets-rx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
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
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, tcp_packet_latency)".to_string(),
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