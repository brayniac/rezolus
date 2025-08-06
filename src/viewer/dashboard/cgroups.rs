use super::common::*;

/// Cgroups dashboard using PromQL
/// This dashboard shows aggregated metrics and can be extended with dynamic selection
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Cgroups".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "CPU".to_string(),
                id: "cpu".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Total CPU Usage (All Cgroups)".to_string(),
                        id: "cgroup-total-cpu".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cgroup_cpu_usage[1m])) / 1e9".to_string(),
                                legend: Some("Total".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Top 5 Cgroups by CPU".to_string(),
                        id: "cgroup-top-cpu".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "topk(5, avg by (name) (irate(cgroup_cpu_usage[1m]) / 1e9))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "CPU Throttled Time".to_string(),
                        id: "cgroup-throttled".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "topk(5, avg by (name) (irate(cgroup_cpu_throttled_time[1m])))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Time,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "Performance".to_string(),
                id: "performance".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Top 5 by IPC".to_string(),
                        id: "cgroup-ipc-high".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "topk(5, avg by (name) (irate(cgroup_cpu_instructions[1m]) / irate(cgroup_cpu_cycles[1m])))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Bottom 5 by IPC".to_string(),
                        id: "cgroup-ipc-low".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "bottomk(5, avg by (name) (irate(cgroup_cpu_instructions[1m]) / irate(cgroup_cpu_cycles[1m])))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                ],
            },
            PromQLGroup {
                name: "Syscalls".to_string(),
                id: "syscalls".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Top 5 by Syscall Rate".to_string(),
                        id: "cgroup-syscalls".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "topk(5, avg by (name) (irate(cgroup_syscall[1m])))".to_string(),
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