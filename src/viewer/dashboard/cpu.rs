use super::common::*;

/// CPU dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
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
                                expr: "avg(sum by (id) (irate(cpu_usage[1m]))) / 1e9".to_string(),
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
                                expr: "sum by (id) (irate(cpu_usage[1m])) / 1e9".to_string(),
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
                                expr: "avg(irate(cpu_usage{state=\"user\"}[1m])) / 1e9".to_string(),
                                legend: Some("User".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU User % by Core".to_string(),
                        id: "cpu-user-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_usage{state=\"user\"}[1m])) / 1e9".to_string(),
                                legend: None,
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
                                expr: "avg(irate(cpu_usage{state=\"system\"}[1m])) / 1e9".to_string(),
                                legend: Some("System".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Percentage,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU System % by Core".to_string(),
                        id: "cpu-system-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_usage{state=\"system\"}[1m])) / 1e9".to_string(),
                                legend: None,
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