use super::common::*;

/// Syscall dashboard using PromQL
pub fn dashboard() -> PromQLDashboard {
    PromQLDashboard {
        name: "Syscall".to_string(),
        sections: default_sections(),
        groups: vec![
            PromQLGroup {
                name: "Syscall".to_string(),
                id: "syscall".to_string(),
                panels: vec![
                    create_syscall_panel("Total", "total", None),
                    create_syscall_panel("Read", "read", Some("read")),
                    create_syscall_panel("Write", "write", Some("write")),
                    create_syscall_panel("Lock", "lock", Some("lock")),
                    create_syscall_panel("Yield", "yield", Some("yield")),
                    create_syscall_panel("Poll", "poll", Some("poll")),
                    create_syscall_panel("Socket", "socket", Some("socket")),
                    create_syscall_panel("Time", "time", Some("time")),
                    create_syscall_panel("Sleep", "sleep", Some("sleep")),
                    create_syscall_panel("Filesystem", "filesystem", Some("filesystem")),
                    create_syscall_panel("Memory", "memory", Some("memory")),
                    create_syscall_panel("Process", "process", Some("process")),
                    create_syscall_panel("Query", "query", Some("query")),
                    create_syscall_panel("IPC", "ipc", Some("ipc")),
                    create_syscall_panel("Timer", "timer", Some("timer")),
                    create_syscall_panel("Event", "event", Some("event")),
                    create_syscall_panel("Other", "other", Some("other")),
                ].into_iter().flatten().collect(),
            },
        ],
    }
}

fn create_syscall_panel(name: &str, id: &str, op: Option<&str>) -> Vec<PromQLPanel> {
    let mut panels = vec![];
    
    // Rate panel
    let rate_expr = if let Some(op) = op {
        format!("sum(irate(syscall{{op=\"{}\"}}[1m]))", op)
    } else {
        "sum(irate(syscall[1m]))".to_string()
    };
    
    panels.push(PromQLPanel {
        title: if op.is_some() { name.to_string() } else { "Total".to_string() },
        id: format!("syscall-{}", id),
        panel_type: PanelType::Line,
        queries: vec![
            PromQLQueryDef {
                expr: rate_expr,
                legend: Some(format!("{} Rate", name)),
                interval: None,
            },
        ],
        unit: Unit::Rate,
        options: None,
    });
    
    // Latency panel
    let latency_base = if let Some(op) = op {
        format!("syscall_latency{{op=\"{}\"}}", op)
    } else {
        "syscall_latency".to_string()
    };
    
    panels.push(PromQLPanel {
        title: format!("{} Latency", name),
        id: format!("syscall-{}-latency", id),
        panel_type: PanelType::Scatter,
        queries: vec![
            PromQLQueryDef {
                expr: format!("histogram_quantile(0.5, {})", latency_base),
                legend: Some("p50".to_string()),
                interval: None,
            },
            PromQLQueryDef {
                expr: format!("histogram_quantile(0.9, {})", latency_base),
                legend: Some("p90".to_string()),
                interval: None,
            },
            PromQLQueryDef {
                expr: format!("histogram_quantile(0.99, {})", latency_base),
                legend: Some("p99".to_string()),
                interval: None,
            },
            PromQLQueryDef {
                expr: format!("histogram_quantile(0.999, {})", latency_base),
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
    });
    
    panels
}