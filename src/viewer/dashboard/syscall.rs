use super::*;

/// Declarative Syscall dashboard using the Builder pattern
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(syscall_group())
        .build()
}

/// Syscall metrics group
fn syscall_group<'a>() -> GroupConfig<'a> {
    let mut group = GroupConfig::new("Syscall", "syscall")
        .plot(
            PlotConfig::line("Total", "syscall-total", Unit::Rate)
                .data(DataSource::counter("syscall"))
                .build()
        )
        .plot(
            PlotConfig::percentile_scatter(
                "Total",
                "syscall-total-latency",
                Unit::Time,
                "syscall_latency",
                (),
                true
            )
        );

    // Add per-operation metrics
    for op in &[
        "Read",
        "Write",
        "Poll",
        "Socket",
        "Lock",
        "Time",
        "Sleep",
        "Yield",
        "Filesystem",
        "Memory",
        "Process",
        "Query",
        "IPC",
        "Timer",
        "Event",
        "Other",
    ] {
        let op_lower = op.to_lowercase();
        let rate_id = format!("syscall-{op}");
        let latency_id = format!("syscall-{op}-latency");
        
        group = group
            .plot(
                PlotConfig::line(op.to_string(), rate_id, Unit::Rate)
                    .data(DataSource::counter_with_labels("syscall", [("op", op_lower.as_str())]))
                    .build()
            )
            .plot(
                PlotConfig::percentile_scatter(
                    op.to_string(),
                    latency_id,
                    Unit::Time,
                    "syscall_latency",
                    [("op", op_lower.as_str())],
                    true
                )
            );
    }

    group
}
