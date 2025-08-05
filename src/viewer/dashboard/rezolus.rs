use super::*;

/// Declarative Rezolus dashboard using the Builder pattern
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(rezolus_metrics_group())
        .build()
}

/// Rezolus self-monitoring metrics group
fn rezolus_metrics_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Rezolus", "rezolus")
        // CPU Usage
        .plot(
            PlotConfig::line("CPU %", "cpu", Unit::Percentage)
                .data(
                    DataSource::counter("rezolus_cpu_usage")
                        .with_transform(|v| v / NANOSECONDS_PER_SECOND)
                )
                .build()
        )
        // Memory (RSS)
        .plot(
            PlotConfig::line("Memory (RSS)", "memory", Unit::Bytes)
                .data(DataSource::gauge("rezolus_memory_usage_resident_set_size"))
                .build()
        )
        // IPC (Instructions per Cycle)
        .plot(ipc_plot())
        // Syscalls
        .plot(
            PlotConfig::line("Syscalls", "syscalls", Unit::Rate)
                .data(
                    DataSource::counter_with_labels(
                        "cgroup_syscall",
                        [("name", "/system.slice/rezolus.service")]
                    )
                )
                .build()
        )
        // Total BPF Overhead
        .plot(
            PlotConfig::line("Total BPF Overhead", "bpf-overhead", Unit::Count)
                .data(
                    DataSource::counter("rezolus_bpf_run_time")
                        .with_transform(|v| v / NANOSECONDS_PER_SECOND)
                )
                .build()
        )
        // BPF Per-Sampler Overhead
        .plot(bpf_sampler_overhead_plot())
        // BPF Per-Sampler Execution Time
        .plot(bpf_execution_time_plot())
}

/// IPC plot for rezolus service
fn ipc_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::conditional(
        |data| {
            data.counters(
                "cgroup_cpu_instructions",
                [("name", "/system.slice/rezolus.service")]
            ).is_some() &&
            data.counters(
                "cgroup_cpu_cycles",
                [("name", "/system.slice/rezolus.service")]
            ).is_some()
        },
        PlotConfig::line("IPC", "ipc", Unit::Count)
            .data(
                DataSource::computed(|data| {
                    match (
                        data.counters(
                            "cgroup_cpu_instructions",
                            [("name", "/system.slice/rezolus.service")]
                        ).map(|v| v.rate().sum()),
                        data.counters(
                            "cgroup_cpu_cycles",
                            [("name", "/system.slice/rezolus.service")]
                        ).map(|v| v.rate().sum()),
                    ) {
                        (Some(instructions), Some(cycles)) => Some(instructions / cycles),
                        _ => None,
                    }
                })
            )
            .build()
    )
}

/// BPF per-sampler overhead multi-series plot
fn bpf_sampler_overhead_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::multi("BPF Per-Sampler Overhead", "bpf-sampler-overhead", Unit::Count)
        .compute(|data| {
            data.counters("rezolus_bpf_run_time", ())
                .map(|v| v.rate().by_sampler() / NANOSECONDS_PER_SECOND)
                .map(|v| v.top_n(20, average))
        })
        .build()
}

/// BPF per-sampler execution time multi-series plot
fn bpf_execution_time_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::multi("BPF Per-Sampler Execution Time", "bpf-execution-time", Unit::Time)
        .compute(|data| {
            match (
                data.counters("rezolus_bpf_run_time", ())
                    .map(|v| v.rate().by_sampler() / NANOSECONDS_PER_SECOND),
                data.counters("rezolus_bpf_run_count", ())
                    .map(|v| v.rate().by_sampler() / NANOSECONDS_PER_SECOND),
            ) {
                (Some(run_time), Some(run_count)) => {
                    Some((run_time / run_count).top_n(20, average))
                }
                _ => None,
            }
        })
        .build()
}