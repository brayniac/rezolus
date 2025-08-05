use super::*;

/// Declarative Softirq dashboard using the Builder pattern
pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    let mut builder = DashboardBuilder::new(data, sections)
        .group(softirq_total_group());

    // Add detailed groups for each softirq type
    for (label, kind) in [
        ("Hardware Interrupts", "hi"),
        ("IRQ Poll", "irq_poll"),
        ("Network Transmit", "net_tx"),
        ("Network Receive", "net_rx"),
        ("RCU", "rcu"),
        ("Sched", "sched"),
        ("Tasklet", "tasklet"),
        ("Timer", "timer"),
        ("HR Timer", "hrtimer"),
        ("Block", "block"),
    ] {
        builder = builder.group(softirq_detail_group(label, kind));
    }

    builder.build()
}

/// Total Softirq metrics group
fn softirq_total_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Softirq", "softirq")
        .plot(
            PlotConfig::line("Rate", "softirq-total-rate", Unit::Rate)
                .data(DataSource::counter("softirq"))
                .build()
        )
        .plot(
            PlotConfig::heatmap("Rate", "softirq-total-rate-heatmap", Unit::Rate)
                .data(HeatmapSource::cpu_heatmap("softirq", ()))
                .build()
        )
        .plot(
            PlotConfig::line("CPU %", "softirq-total-time", Unit::Percentage)
                .data(
                    DataSource::cpu_avg("softirq_time", ())
                        .with_transform(|v| v / NANOSECONDS_PER_SECOND)
                )
                .build()
        )
        .plot(
            PlotConfig::heatmap("CPU %", "softirq-total-time-heatmap", Unit::Percentage)
                .data(
                    HeatmapSource::cpu_heatmap("softirq_time", ())
                        .with_transform(|v| v / NANOSECONDS_PER_SECOND)
                )
                .build()
        )
}

/// Detailed Softirq metrics group for a specific type
fn softirq_detail_group<'a>(label: &'a str, kind: &'a str) -> GroupConfig<'a> {
    GroupConfig::new(label.to_string(), format!("softirq-{kind}"))
        .plot(
            PlotConfig::line("Rate".to_string(), format!("softirq-{kind}-rate"), Unit::Rate)
                .data(DataSource::counter_with_labels("softirq", [("kind", kind)]))
                .build()
        )
        .plot(
            PlotConfig::heatmap("Rate".to_string(), format!("softirq-{kind}-rate-heatmap"), Unit::Rate)
                .data(HeatmapSource::cpu_heatmap("softirq", [("kind", kind)]))
                .build()
        )
        .plot(
            PlotConfig::line("CPU %".to_string(), format!("softirq-{kind}-time"), Unit::Percentage)
                .data(
                    DataSource::cpu_avg("softirq_time", [("kind", kind)])
                        .with_transform(|v| v / NANOSECONDS_PER_SECOND)
                )
                .build()
        )
        .plot(
            PlotConfig::heatmap("CPU %".to_string(), format!("softirq-{kind}-time-heatmap"), Unit::Percentage)
                .data(
                    HeatmapSource::cpu_heatmap_as_percentage("softirq_time", [("kind", kind)])
                )
                .build()
        )
}