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
                        title: "Frequency".to_string(),
                        id: "frequency".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg(irate(cpu_tsc[1m]) * irate(cpu_aperf[1m]) / irate(cpu_mperf[1m]))".to_string(),
                                legend: Some("Frequency".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Frequency,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Frequency by Core".to_string(),
                        id: "frequency-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_tsc[1m]) * irate(cpu_aperf[1m]) / irate(cpu_mperf[1m])".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Frequency,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Frequency Scaling".to_string(),
                        id: "frequency-scaling".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg(irate(cpu_aperf[1m]) / irate(cpu_mperf[1m]))".to_string(),
                                legend: Some("APERF/MPERF Ratio".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Frequency Scaling by Core".to_string(),
                        id: "frequency-scaling-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_aperf[1m]) / irate(cpu_mperf[1m])".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Instructions Per Cycle".to_string(),
                        id: "ipc".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg(irate(cpu_instructions[1m]) / irate(cpu_cycles[1m]))".to_string(),
                                legend: Some("IPC".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "IPC by Core".to_string(),
                        id: "ipc-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_instructions[1m]) / irate(cpu_cycles[1m])".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Instructions Per Nanosecond".to_string(),
                        id: "ipns".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg(irate(cpu_instructions[1m]) / irate(cpu_cycles[1m]) * irate(cpu_tsc[1m]) * irate(cpu_aperf[1m]) / irate(cpu_mperf[1m]) / 1e9)".to_string(),
                                legend: Some("Instructions/ns".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Instructions/ns by Core".to_string(),
                        id: "ipns-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cpu_instructions[1m]) / irate(cpu_cycles[1m]) * irate(cpu_tsc[1m]) * irate(cpu_aperf[1m]) / irate(cpu_mperf[1m]) / 1e9".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "L3 Cache Hit Rate".to_string(),
                        id: "l3-hitrate".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "avg((irate(cpu_l3_access[1m]) - irate(cpu_l3_miss[1m])) / irate(cpu_l3_access[1m]))".to_string(),
                                legend: Some("L3 Hit Rate".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "L3 Hit Rate by Core".to_string(),
                        id: "l3-hitrate-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "(irate(cpu_l3_access[1m]) - irate(cpu_l3_miss[1m])) / irate(cpu_l3_access[1m])".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "L3 Cache Misses".to_string(),
                        id: "l3-misses".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_l3_miss[1m]))".to_string(),
                                legend: Some("L3 Misses/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "L3 Misses by Core".to_string(),
                        id: "l3-misses-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_l3_miss[1m]))".to_string(),
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
                name: "TLB Flush".to_string(),
                id: "tlb-flush".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Total".to_string(),
                        id: "tlb-total".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_tlb_flush[1m]))".to_string(),
                                legend: Some("Total".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Total by Core".to_string(),
                        id: "tlb-total-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_tlb_flush[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Local MM Shootdown".to_string(),
                        id: "tlb-local-mm-shootdown".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_tlb_flush{reason=\"local_mm_shootdown\"}[1m]))".to_string(),
                                legend: Some("Local MM Shootdown".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Local MM Shootdown by Core".to_string(),
                        id: "tlb-local-mm-shootdown-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_tlb_flush{reason=\"local_mm_shootdown\"}[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Remote Send IPI".to_string(),
                        id: "tlb-remote-send-ipi".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_tlb_flush{reason=\"remote_send_ipi\"}[1m]))".to_string(),
                                legend: Some("Remote Send IPI".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Remote Send IPI by Core".to_string(),
                        id: "tlb-remote-send-ipi-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_tlb_flush{reason=\"remote_send_ipi\"}[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Remote Shootdown".to_string(),
                        id: "tlb-remote-shootdown".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_tlb_flush{reason=\"remote_shootdown\"}[1m]))".to_string(),
                                legend: Some("Remote Shootdown".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Remote Shootdown by Core".to_string(),
                        id: "tlb-remote-shootdown-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_tlb_flush{reason=\"remote_shootdown\"}[1m]))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Task Switch".to_string(),
                        id: "tlb-task-switch".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_tlb_flush{reason=\"task_switch\"}[1m]))".to_string(),
                                legend: Some("Task Switch".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Task Switch by Core".to_string(),
                        id: "tlb-task-switch-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_tlb_flush{reason=\"task_switch\"}[1m]))".to_string(),
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
                name: "Migrations".to_string(),
                id: "migrations".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "CPU Migrations".to_string(),
                        id: "cpu-migrations".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cpu_migrations{direction=\"to\"}[1m]))".to_string(),
                                legend: Some("Migrations".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU Migrations by Core".to_string(),
                        id: "cpu-migrations-heatmap".to_string(),
                        panel_type: PanelType::Heatmap,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (id) (irate(cpu_migrations{direction=\"to\"}[1m]))".to_string(),
                                legend: None,
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