use super::common::*;

/// Rezolus dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Rezolus".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Rezolus".to_string(),
                id: "rezolus".to_string(),
                panels: vec![
                    PromQLPanel {
                        title: "CPU Cores".to_string(),
                        id: "cpu".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(rezolus_cpu_usage[1m])) / 1e9".to_string(),
                                legend: Some("CPU Usage".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Memory RSS".to_string(),
                        id: "memory".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "rezolus_memory_usage_resident_set_size".to_string(),
                                legend: Some("RSS".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Bytes,
                        options: None,
                    },
                    PromQLPanel {
                        title: "IPC".to_string(),
                        id: "ipc".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "irate(cgroup_cpu_instructions{name=\"/system.slice/rezolus.service\"}[1m]) / irate(cgroup_cpu_cycles{name=\"/system.slice/rezolus.service\"}[1m])".to_string(),
                                legend: Some("IPC".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                    PromQLPanel {
                        title: "Syscall Rate".to_string(),
                        id: "syscalls".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(cgroup_syscall{name=\"/system.slice/rezolus.service\"}[1m]))".to_string(),
                                legend: Some("Rate".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Rate,
                        options: None,
                    },
                    PromQLPanel {
                        title: "BPF Overhead".to_string(),
                        id: "bpf-overhead".to_string(),
                        panel_type: PanelType::Line,
                        queries: vec![
                            PromQLQueryDef {
                                expr: "sum(irate(rezolus_bpf_run_time[1m])) / 1e9".to_string(),
                                legend: Some("Overhead".to_string()),
                                interval: None,
                            },
                        ],
                        unit: Unit::Count,
                        options: None,
                    },
                ],
            },
        ],
    }
}