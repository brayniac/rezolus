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
                        title: "TCP Transmit Bandwidth".to_string(),
                        id: "tcp-bandwidth-tx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(tcp_bytes{direction=\"transmit\"}[1m]) * 8".to_string(),
                                legend: Some("TCP Transmit".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bitrate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "TCP Receive Bandwidth".to_string(),
                        id: "tcp-bandwidth-rx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(tcp_bytes{direction=\"receive\"}[1m]) * 8".to_string(),
                                legend: Some("TCP Receive".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bitrate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "TCP Transmit Packets".to_string(),
                        id: "tcp-packets-tx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(tcp_packets{direction=\"transmit\"}[1m])".to_string(),
                                legend: Some("TCP TX Packets/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "TCP Receive Packets".to_string(),
                        id: "tcp-packets-rx".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(tcp_packets{direction=\"receive\"}[1m])".to_string(),
                                legend: Some("TCP RX Packets/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "TCP Retransmits".to_string(),
                        id: "tcp-retransmits".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(tcp_retransmit[1m])".to_string(),
                                legend: Some("Retransmits/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
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
                            cgroup_filter: None,
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                    PromQLPanel {
                        title: "TCP Transmit Size Distribution".to_string(),
                        id: "tcp-size-tx".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, tcp_size{direction=\"transmit\"})".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, tcp_size{direction=\"transmit\"})".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, tcp_size{direction=\"transmit\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, tcp_size{direction=\"transmit\"})".to_string(),
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
                        title: "TCP Receive Size Distribution".to_string(),
                        id: "tcp-size-rx".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, tcp_size{direction=\"receive\"})".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, tcp_size{direction=\"receive\"})".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, tcp_size{direction=\"receive\"})".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, tcp_size{direction=\"receive\"})".to_string(),
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
                        title: "TCP Connect Latency".to_string(),
                        id: "tcp-connect-latency".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, tcp_connect_latency)".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, tcp_connect_latency)".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, tcp_connect_latency)".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, tcp_connect_latency)".to_string(),
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
                        title: "TCP Jitter".to_string(),
                        id: "tcp-jitter".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, tcp_jitter)".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, tcp_jitter)".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, tcp_jitter)".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, tcp_jitter)".to_string(),
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
                        title: "TCP Smoothed RTT".to_string(),
                        id: "tcp-srtt".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, tcp_srtt)".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, tcp_srtt)".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, tcp_srtt)".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, tcp_srtt)".to_string(),
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
            PromQLGroup {
                name: "Network Errors".to_string(),
                id: "network-errors".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Network Drops".to_string(),
                        id: "network-drops".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_drop[1m])".to_string(),
                                legend: Some("Drops/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Transmit Busy".to_string(),
                        id: "tx-busy".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_transmit_busy[1m])".to_string(),
                                legend: Some("Busy/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Transmit Complete".to_string(),
                        id: "tx-complete".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_transmit_complete[1m])".to_string(),
                                legend: Some("Complete/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Transmit Timeout".to_string(),
                        id: "tx-timeout".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(network_transmit_timeout[1m])".to_string(),
                                legend: Some("Timeouts/sec".to_string()),
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