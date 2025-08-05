use super::*;

/// Declarative CPU dashboard using the Builder pattern
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(utilization_group())
        .group(performance_group())
        .group(migrations_group())
        .group(tlb_flush_group())
        .build()
}

/// CPU Utilization metrics group
fn utilization_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Utilization", "utilization")
        // Busy percentage line plot
        .plot(
            PlotConfig::line("Busy %", "busy-pct", Unit::Percentage)
                .data(
                    DataSource::cpu_avg("cpu_usage")
                        .with_transform(|v| v / 1000000000.0)
                )
                .build()
        )
        // Busy percentage heatmap
        .plot(
            PlotConfig::heatmap("Busy %", "busy-pct-heatmap", Unit::Percentage)
                .data(
                    HeatmapSource::cpu_heatmap("cpu_usage")
                        .with_transform(|v| v / 1000000000.0)
                )
                .build()
        )
        // User percentage line plot
        .plot(
            PlotConfig::line("User %", "user-pct", Unit::Percentage)
                .data(
                    DataSource::cpu_avg_with_labels("cpu_usage", [("state", "user")])
                        .with_transform(|v| v / 1000000000.0)
                )
                .build()
        )
        // User percentage heatmap
        .plot(
            PlotConfig::heatmap("User %", "user-pct-heatmap", Unit::Percentage)
                .data(
                    HeatmapSource::cpu_heatmap_with_labels("cpu_usage", [("state", "user")])
                        .with_transform(|v| v / 1000000000.0)
                )
                .build()
        )
        // System percentage line plot
        .plot(
            PlotConfig::line("System %", "system-pct", Unit::Percentage)
                .data(
                    DataSource::cpu_avg_with_labels("cpu_usage", [("state", "system")])
                        .with_transform(|v| v / 1000000000.0)
                )
                .build()
        )
        // System percentage heatmap
        .plot(
            PlotConfig::heatmap("System %", "system-pct-heatmap", Unit::Percentage)
                .data(
                    HeatmapSource::cpu_heatmap_with_labels("cpu_usage", [("state", "system")])
                        .with_transform(|v| v / 1000000000.0)
                )
                .build()
        )
}

/// CPU Performance metrics group
fn performance_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Performance", "performance")
        // Instructions per Cycle (IPC)
        .plot(ipc_plot())
        .plot(ipc_heatmap())
        // Instructions per Nanosecond (IPNS)
        .plot(ipns_plot())
        .plot(ipns_heatmap())
        // L3 Cache Hit Rate
        .plot(l3_hit_plot())
        .plot(l3_hit_heatmap())
        // CPU Frequency
        .plot(frequency_plot())
        .plot(frequency_heatmap())
}

/// IPC line plot
fn ipc_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::line("Instructions per Cycle (IPC)", "ipc", Unit::Count)
        .data(
            DataSource::computed(|data| {
                match (
                    data.counters("cpu_cycles", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_instructions", ()).map(|v| v.rate().sum()),
                ) {
                    (Some(cycles), Some(instructions)) => Some(instructions / cycles),
                    _ => None,
                }
            })
        )
        .build()
}

/// IPC heatmap
fn ipc_heatmap<'a>() -> PlotConfig<'a> {
    PlotConfig::heatmap("Instructions per Cycle (IPC)", "ipc-heatmap", Unit::Count)
        .data(
            HeatmapSource::computed(|data| {
                match (
                    data.cpu_heatmap("cpu_cycles", ()),
                    data.cpu_heatmap("cpu_instructions", ()),
                ) {
                    (Some(cycles), Some(instructions)) => Some(instructions / cycles),
                    _ => None,
                }
            })
        )
        .build()
}

/// IPNS line plot
fn ipns_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::line("Instructions per Nanosecond (IPNS)", "ipns", Unit::Count)
        .data(
            DataSource::computed(|data| {
                match (
                    data.counters("cpu_cycles", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_instructions", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_aperf", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_mperf", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_tsc", ()).map(|v| v.rate().sum()),
                    data.gauges("cpu_cores", ()).map(|v| v.sum()),
                ) {
                    (Some(cycles), Some(instructions), Some(aperf), Some(mperf), Some(tsc), Some(cores)) => {
                        Some(instructions / cycles * tsc * aperf / mperf / 1000000000.0 / cores)
                    }
                    _ => None,
                }
            })
        )
        .build()
}

/// IPNS heatmap
fn ipns_heatmap<'a>() -> PlotConfig<'a> {
    PlotConfig::heatmap("Instructions per Nanosecond (IPNS)", "ipns-heatmap", Unit::Count)
        .data(
            HeatmapSource::computed(|data| {
                match (
                    data.cpu_heatmap("cpu_cycles", ()),
                    data.cpu_heatmap("cpu_instructions", ()),
                    data.cpu_heatmap("cpu_aperf", ()),
                    data.cpu_heatmap("cpu_mperf", ()),
                    data.cpu_heatmap("cpu_tsc", ()),
                ) {
                    (Some(cycles), Some(instructions), Some(aperf), Some(mperf), Some(tsc)) => {
                        Some(instructions / cycles * tsc * aperf / mperf / 1000000000.0)
                    }
                    _ => None,
                }
            })
        )
        .build()
}

/// L3 cache hit rate plot
fn l3_hit_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::line("L3 Hit %", "l3-hit", Unit::Percentage)
        .data(
            DataSource::computed(|data| {
                match (
                    data.counters("cpu_l3_access", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_l3_miss", ()).map(|v| v.rate().sum()),
                ) {
                    (Some(access), Some(miss)) => Some(miss / access),
                    _ => None,
                }
            })
        )
        .build()
}

/// L3 cache hit rate heatmap
fn l3_hit_heatmap<'a>() -> PlotConfig<'a> {
    PlotConfig::heatmap("L3 Hit %", "l3-hit-heatmap", Unit::Percentage)
        .data(
            HeatmapSource::computed(|data| {
                match (
                    data.cpu_heatmap("cpu_l3_access", ()),
                    data.cpu_heatmap("cpu_l3_miss", ()),
                ) {
                    (Some(access), Some(miss)) => Some(miss / access),
                    _ => None,
                }
            })
        )
        .build()
}

/// CPU frequency plot
fn frequency_plot<'a>() -> PlotConfig<'a> {
    PlotConfig::line("Frequency", "frequency", Unit::Frequency)
        .data(
            DataSource::computed(|data| {
                match (
                    data.counters("cpu_aperf", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_mperf", ()).map(|v| v.rate().sum()),
                    data.counters("cpu_tsc", ()).map(|v| v.rate().sum()),
                    data.gauges("cpu_cores", ()).map(|v| v.sum()),
                ) {
                    (Some(aperf), Some(mperf), Some(tsc), Some(cores)) => {
                        Some(tsc * aperf / mperf / cores)
                    }
                    _ => None,
                }
            })
        )
        .build()
}

/// CPU frequency heatmap
fn frequency_heatmap<'a>() -> PlotConfig<'a> {
    PlotConfig::heatmap("Frequency", "frequency-heatmap", Unit::Frequency)
        .data(
            HeatmapSource::computed(|data| {
                match (
                    data.cpu_heatmap("cpu_aperf", ()),
                    data.cpu_heatmap("cpu_mperf", ()),
                    data.cpu_heatmap("cpu_tsc", ()),
                ) {
                    (Some(aperf), Some(mperf), Some(tsc)) => Some(tsc * aperf / mperf),
                    _ => None,
                }
            })
        )
        .build()
}

/// CPU Migrations metrics group
fn migrations_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Migrations", "migrations")
        // Migrations To
        .plot(
            PlotConfig::line("To", "cpu-migrations-to", Unit::Rate)
                .data(
                    DataSource::counter_with_labels("cpu_migrations", [("direction", "to")])
                )
                .build()
        )
        .plot(
            PlotConfig::heatmap("To", "cpu-migrations-to-heatmap", Unit::Rate)
                .data(
                    HeatmapSource::cpu_heatmap_with_labels("cpu_migrations", [("direction", "to")])
                )
                .build()
        )
        // Migrations From
        .plot(
            PlotConfig::line("From", "cpu-migrations-from", Unit::Rate)
                .data(
                    DataSource::counter_with_labels("cpu_migrations", [("direction", "from")])
                )
                .build()
        )
        .plot(
            PlotConfig::heatmap("From", "cpu-migrations-from-heatmap", Unit::Rate)
                .data(
                    HeatmapSource::cpu_heatmap_with_labels("cpu_migrations", [("direction", "from")])
                )
                .build()
        )
}

/// TLB Flush metrics group
fn tlb_flush_group<'a>() -> GroupConfig<'a> {
    let mut group = GroupConfig::new("TLB Flush", "tlb-flush")
        // Total TLB flushes
        .plot(
            PlotConfig::line("Total", "tlb-total", Unit::Rate)
                .data(DataSource::counter("cpu_tlb_flush"))
                .build()
        )
        .plot(
            PlotConfig::heatmap("Total", "tlb-total-heatmap", Unit::Rate)
                .data(HeatmapSource::cpu_heatmap("cpu_tlb_flush"))
                .build()
        );

    // Add plots for each TLB flush reason
    for (label, metric_suffix) in [
        ("Local MM Shootdown", "local_mm_shootdown"),
        ("Remote Send IPI", "remote_send_ipi"),
        ("Remote Shootdown", "remote_shootdown"),
        ("Task Switch", "task_switch"),
    ] {
        let id = format!("tlb-{}", metric_suffix.replace('_', "-"));
        
        group = group
            .plot(
                PlotConfig::line(label, &id, Unit::Rate)
                    .data(
                        DataSource::counter_with_labels("cpu_tlb_flush", [("reason", metric_suffix)])
                    )
                    .build()
            )
            .plot(
                PlotConfig::heatmap(label, &format!("{}-heatmap", id), Unit::Rate)
                    .data(
                        HeatmapSource::cpu_heatmap_with_labels("cpu_tlb_flush", [("reason", metric_suffix)])
                    )
                    .build()
            );
    }

    group
}