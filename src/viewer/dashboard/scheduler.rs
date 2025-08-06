use super::common::*;

/// Scheduler dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Scheduler".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Scheduler".to_string(),
                id: "scheduler".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Runqueue Latency".to_string(),
                        id: "scheduler-runqueue-latency".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, scheduler_runqueue_latency)".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, scheduler_runqueue_latency)".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, scheduler_runqueue_latency)".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, scheduler_runqueue_latency)".to_string(),
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
                        title: "Off CPU Time".to_string(),
                        id: "off-cpu-time".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, scheduler_offcpu)".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, scheduler_offcpu)".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, scheduler_offcpu)".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, scheduler_offcpu)".to_string(),
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
                        title: "Running Time".to_string(),
                        id: "running-time".to_string(),
                        panel_type: PanelType::Scatter,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.5, scheduler_running)".to_string(),
                                legend: Some("p50".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.9, scheduler_running)".to_string(),
                                legend: Some("p90".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.99, scheduler_running)".to_string(),
                                legend: Some("p99".to_string()),
                                interval: None,
                            },
                            PromQLQueryDef {
                                expr: "histogram_quantile(0.999, scheduler_running)".to_string(),
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
                        title: "Context Switch".to_string(),
                        id: "cswitch".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(scheduler_context_switch[1m])".to_string(),
                                legend: Some("Rate".to_string()),
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