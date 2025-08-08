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
                        title: "CPU Usage - Unselected Cgroups (Sum)".to_string(),
                        id: "cgroup-unselected-cpu".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                // Use {{CGROUP_FILTER}} placeholder that will be replaced by the backend
                                // Divide by 1e9 to convert nanoseconds to CPU cores
                                expr: "sum(sum by (name) (irate(cgroup_cpu_usage[1m]{{CGROUP_FILTER}}))) / 1e9".to_string(),
                                legend: Some("Unselected Cgroups".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count, // CPU cores
                        options: Some(PanelOptions {
                            cgroup_filter: Some("unselected".to_string()),
                            ..Default::default()
                        }),
                    },
                    PromQLPanel {
                        title: "CPU Usage - Selected Cgroups (Individual)".to_string(),
                        id: "cgroup-selected-cpu".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                // Divide by 1e9 to convert nanoseconds to CPU cores
                                expr: "sum by (name) (irate(cgroup_cpu_usage[1m]{{CGROUP_FILTER}})) / 1e9".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count, // CPU cores
                        options: Some(PanelOptions {
                            cgroup_filter: Some("selected".to_string()),
                            ..Default::default()
                        }),
                    },
                    PromQLPanel {
                        title: "CPU Throttled Time - Unselected Cgroups (Sum)".to_string(),
                        id: "cgroup-throttled-unselected".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                // Divide by 1e9 to convert nanoseconds to seconds
                                expr: "sum(sum by (name) (irate(cgroup_cpu_throttled_time[1m]{{CGROUP_FILTER}}))) / 1e9".to_string(),
                                legend: Some("Unselected Throttled".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count, // Fraction of time throttled (seconds/second)
                        options: Some(PanelOptions {
                            cgroup_filter: Some("unselected".to_string()),
                            ..Default::default()
                        }),
                    },
                    PromQLPanel {
                        title: "CPU Throttled Time - Selected Cgroups".to_string(),
                        id: "cgroup-throttled-selected".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                // Divide by 1e9 to convert nanoseconds to seconds
                                expr: "sum by (name) (irate(cgroup_cpu_throttled_time[1m]{{CGROUP_FILTER}})) / 1e9".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count, // Fraction of time throttled (seconds/second)
                        options: Some(PanelOptions {
                            cgroup_filter: Some("selected".to_string()),
                            ..Default::default()
                        }),
                    },
                ],
            },
            PromQLGroup {
                name: "Performance".to_string(),
                id: "performance".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "IPC - Unselected Cgroups (Average)".to_string(),
                        id: "cgroup-ipc-unselected".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(sum by (name) (irate(cgroup_cpu_instructions[1m]{{CGROUP_FILTER}}))) / sum(sum by (name) (irate(cgroup_cpu_cycles[1m]{{CGROUP_FILTER}})))".to_string(),
                                legend: Some("Unselected Avg IPC".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: Some(PanelOptions {
                            cgroup_filter: Some("unselected".to_string()),
                            ..Default::default()
                        }),
                    },
                    PromQLPanel {
                        title: "IPC - Selected Cgroups".to_string(),
                        id: "cgroup-ipc-selected".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "(sum by (name) (irate(cgroup_cpu_instructions[1m]{{CGROUP_FILTER}}))) / (sum by (name) (irate(cgroup_cpu_cycles[1m]{{CGROUP_FILTER}})))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: Some(PanelOptions {
                            cgroup_filter: Some("selected".to_string()),
                            ..Default::default()
                        }),
                    },
                    PromQLPanel {
                        title: "Instruction Rate - Unselected Cgroups (Sum)".to_string(),
                        id: "cgroup-instructions-unselected".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(sum by (name) (irate(cgroup_cpu_instructions[1m]{{CGROUP_FILTER}})))".to_string(),
                                legend: Some("Unselected Instructions/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: Some(PanelOptions {
                            cgroup_filter: Some("unselected".to_string()),
                            ..Default::default()
                        }),
                    },
                    PromQLPanel {
                        title: "Instruction Rate - Selected Cgroups".to_string(),
                        id: "cgroup-instructions-selected".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (name) (irate(cgroup_cpu_instructions[1m]{{CGROUP_FILTER}}))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: Some(PanelOptions {
                            cgroup_filter: Some("selected".to_string()),
                            ..Default::default()
                        }),
                    },
                ],
            },
            PromQLGroup {
                name: "Syscalls".to_string(),
                id: "syscalls".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "Syscall Rate - Unselected Cgroups (Sum)".to_string(),
                        id: "cgroup-syscalls-unselected".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(sum by (name) (irate(cgroup_syscall[1m]{{CGROUP_FILTER}})))".to_string(),
                                legend: Some("Unselected Syscalls/sec".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: Some(PanelOptions {
                            cgroup_filter: Some("unselected".to_string()),
                            ..Default::default()
                        }),
                    },
                    PromQLPanel {
                        title: "Syscall Rate - Selected Cgroups".to_string(),
                        id: "cgroup-syscalls-selected".to_string(),
                        panel_type: PanelType::Multi,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum by (name) (irate(cgroup_syscall[1m]{{CGROUP_FILTER}}))".to_string(),
                                legend: None,
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: Some(PanelOptions {
                            cgroup_filter: Some("selected".to_string()),
                            ..Default::default()
                        }),
                    },
                ],
            },
        ],
    }
}