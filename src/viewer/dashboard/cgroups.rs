use super::*;

/// Declarative cgroups dashboard using the Builder pattern
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(cpu_cgroups_group())
        .group(performance_cgroups_group())
        .group(tlb_cgroups_group())
        .group(syscall_cgroups_group())
        .build()
}

/// CPU cgroups metrics group
fn cpu_cgroups_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("CPU", "cpu")
        .plot(
            PlotConfig::multi("Total Cores", "cgroup-total-cores", Unit::Count)
                .compute(|data| {
                    data.counters("cgroup_cpu_usage", ())
                        .map(|v| (v.rate().by_name() / NANOSECONDS_PER_SECOND).top_n(5, average))
                })
                .build()
        )
        .plot(
            PlotConfig::multi("User Cores", "cgroup-user-cores", Unit::Count)
                .compute(|data| {
                    data.counters("cgroup_cpu_usage", [("state", "user")])
                        .map(|v| (v.rate().by_name() / NANOSECONDS_PER_SECOND).top_n(5, average))
                })
                .build()
        )
        .plot(
            PlotConfig::multi("System Cores", "cgroup-system-cores", Unit::Count)
                .compute(|data| {
                    data.counters("cgroup_cpu_usage", [("state", "system")])
                        .map(|v| (v.rate().by_name() / NANOSECONDS_PER_SECOND).top_n(5, average))
                })
                .build()
        )
        .plot(
            PlotConfig::multi("CPU Migrations", "cgroup-cpu-migrations", Unit::Rate)
                .compute(|data| {
                    data.counters("cgroup_cpu_migrations", ())
                        .map(|v| v.rate().by_name().top_n(5, average))
                })
                .build()
        )
        .plot(
            PlotConfig::multi("CPU Throttled Time", "cgroup-cpu-throttled-time", Unit::Time)
                .compute(|data| {
                    data.counters("cgroup_cpu_throttled_time", ())
                        .map(|v| v.rate().by_name().top_n(5, average))
                })
                .build()
        )
        .plot(
            PlotConfig::multi("CPU Throttle Latency", "cgroup-throttle-latency", Unit::Time)
                .compute(|data| {
                    match (
                        data.counters("cgroup_cpu_throttled_time", ())
                            .map(|v| v.rate().by_name()),
                        data.counters("cgroup_cpu_throttled", ())
                            .map(|v| v.rate().by_name()),
                    ) {
                        (Some(time), Some(count)) => {
                            Some((time / count).top_n(5, average))
                        }
                        _ => None,
                    }
                })
                .build()
        )
}

/// Performance cgroups metrics group
fn performance_cgroups_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Performance", "performance")
        .plot(
            PlotConfig::multi("Highest IPC", "cgroup-ipc-high", Unit::Count)
                .compute(|data| {
                    match (
                        data.counters("cgroup_cpu_instructions", ())
                            .map(|v| v.rate().by_name()),
                        data.counters("cgroup_cpu_cycles", ())
                            .map(|v| v.rate().by_name()),
                    ) {
                        (Some(instructions), Some(cycles)) => {
                            Some((instructions / cycles).top_n(5, average))
                        }
                        _ => None,
                    }
                })
                .build()
        )
        .plot(
            PlotConfig::multi("Lowest IPC", "cgroup-ipc-low", Unit::Count)
                .compute(|data| {
                    match (
                        data.counters("cgroup_cpu_instructions", ())
                            .map(|v| v.rate().by_name()),
                        data.counters("cgroup_cpu_cycles", ())
                            .map(|v| v.rate().by_name()),
                    ) {
                        (Some(instructions), Some(cycles)) => {
                            Some((instructions / cycles).bottom_n(5, average))
                        }
                        _ => None,
                    }
                })
                .build()
        )
}

/// TLB cgroups metrics group
fn tlb_cgroups_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("TLB", "tlb")
        .plot(
            PlotConfig::multi("Total", "cgroup-tlb-flush", Unit::Count)
                .compute(|data| {
                    data.counters("cgroup_cpu_tlb_flush", ())
                        .map(|v| v.rate().by_name().top_n(5, average))
                })
                .build()
        )
}

/// Syscall cgroups metrics group
fn syscall_cgroups_group<'a>() -> GroupConfig<'a> {
    let mut group = GroupConfig::new("Syscall", "syscall")
        .plot(
            PlotConfig::multi("Total", "cgroup-syscall", Unit::Rate)
                .compute(|data| {
                    data.counters("cgroup_syscall", ())
                        .map(|v| v.rate().by_name().top_n(5, average))
                })
                .build()
        );

    // Add per-operation syscall metrics
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
        
        group = group.plot(
            PlotConfig::multi(op.to_string(), format!("syscall-{op}"), Unit::Rate)
                .compute(move |data| {
                    data.counters("cgroup_syscall", [("op", op_lower.as_str())])
                        .map(|v| v.rate().by_name().top_n(5, average))
                })
                .build()
        );
    }

    group
}