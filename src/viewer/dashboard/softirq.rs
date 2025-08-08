use super::common::*;

/// SoftIRQ dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "SoftIRQ".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Softirq".to_string(),
                id: "softirq".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Rate".to_string(),
                        id: "softirq-total-rate".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(softirq[1m]))".to_string(),
                                legend: Some("Total".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Rate by Core".to_string(),
                        id: "softirq-rate-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(softirq[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU %".to_string(),
                        id: "softirq-total-time".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg(irate(softirq_time[1m])) / 1e9".to_string(),
                                legend: Some("Average".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU % by Core".to_string(),
                        id: "softirq-time-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(softirq_time[1m])) / 1e9".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "By Type".to_string(),
                        id: "softirq-by-type".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"hi\"}[1m])".to_string(),
                                legend: Some("Hardware Interrupts".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"timer\"}[1m])".to_string(),
                                legend: Some("Timer".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"net_tx\"}[1m])".to_string(),
                                legend: Some("Network TX".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"net_rx\"}[1m])".to_string(),
                                legend: Some("Network RX".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"block\"}[1m])".to_string(),
                                legend: Some("Block".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"tasklet\"}[1m])".to_string(),
                                legend: Some("Tasklet".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"sched\"}[1m])".to_string(),
                                legend: Some("Scheduler".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"hrtimer\"}[1m])".to_string(),
                                legend: Some("HR Timer".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "irate(softirq{kind=\"rcu\"}[1m])".to_string(),
                                legend: Some("RCU".to_string()),
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