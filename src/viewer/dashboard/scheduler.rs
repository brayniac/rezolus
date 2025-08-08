use super::common::*;

/// Scheduler dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Scheduler".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Context Switches".to_string(),
                id: "context-switches".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Context Switch Rate".to_string(),
                        id: "cswitch-rate".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(scheduler_context_switch[1m]))".to_string(),
                                legend: Some("Total".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Context Switches by Core".to_string(),
                        id: "cswitch-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(scheduler_context_switch[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "Runqueue Wait".to_string(),
                id: "runqueue-wait".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Runqueue Wait Time".to_string(),
                        id: "runqueue-wait-time".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(scheduler_runqueue_wait[1m]))".to_string(),
                                legend: Some("Total".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Time,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Runqueue Wait by Core".to_string(),
                        id: "runqueue-wait-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(scheduler_runqueue_wait[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Time,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Runqueue Latency Distribution".to_string(),
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
                            cgroup_filter: None,
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                ],
            },
            PromQLGroup {
                name: "CPU Time".to_string(),
                id: "cpu-time".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Running Time Distribution".to_string(),
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
                            cgroup_filter: None,
                            log_scale: Some(true),
                            stack: None,
                            fill: None,
                        }),
                    },
                    PromQLPanel {
                        title: "Off-CPU Time Distribution".to_string(),
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