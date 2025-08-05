use super::*;

pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(cpu_group())
        .group(network_group())
        .group(scheduler_group())
        .group(syscall_group())
        .group(softirq_group())
        .group(blockio_group())
        .build()
}

fn cpu_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("CPU", "cpu")
        .plot(
            PlotConfig::line("Busy %", "cpu-busy", Unit::Percentage)
                .data(
                    DataSource::cpu_avg("cpu_usage", ())
                        .with_transform(|v| v / NANOSECONDS_PER_SECOND)
                )
                .build()
        )
        .plot(
            PlotConfig::heatmap("Busy %", "cpu-busy-heatmap", Unit::Percentage)
                .data(HeatmapSource::cpu_heatmap_as_percentage("cpu_usage", ()))
                .build()
        )
}

fn network_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Network", "network")
        .plot(
            PlotConfig::line("Transmit Bandwidth", "network-transmit-bandwidth", Unit::Bitrate)
                .data(DataSource::counter_as_bitrate("network_bytes", [("direction", "transmit")]))
                .build()
        )
        .plot(
            PlotConfig::line("Receive Bandwidth", "network-receive-bandwidth", Unit::Bitrate)
                .data(DataSource::counter_as_bitrate("network_bytes", [("direction", "receive")]))
                .build()
        )
        .plot(
            PlotConfig::line("Transmit Packets", "network-transmit-packets", Unit::Rate)
                .data(DataSource::counter_with_labels("network_packets", [("direction", "transmit")]))
                .build()
        )
        .plot(
            PlotConfig::line("Receive Packets", "network-receive-packets", Unit::Rate)
                .data(DataSource::counter_with_labels("network_packets", [("direction", "receive")]))
                .build()
        )
        .plot(
            PlotConfig::percentile_scatter(
                "TCP Packet Latency",
                "tcp-packet-latency",
                Unit::Time,
                "tcp_packet_latency",
                (),
                true
            )
        )
}

fn scheduler_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Scheduler", "scheduler")
        .plot(
            PlotConfig::percentile_scatter(
                "Runqueue Latency",
                "scheduler-runqueue-latency",
                Unit::Time,
                "scheduler_runqueue_latency",
                (),
                true
            )
        )
}

fn syscall_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Syscall", "syscall")
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
        )
}

fn softirq_group<'a>() -> GroupConfig<'a> {
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
                .data(HeatmapSource::cpu_heatmap_as_percentage("softirq_time", ()))
                .build()
        )
}

fn blockio_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("BlockIO", "blockio")
        .plot(
            PlotConfig::line("Read Throughput", "blockio-throughput-read", Unit::Datarate)
                .data(DataSource::counter_with_labels("blockio_bytes", [("op", "read")]))
                .build()
        )
        .plot(
            PlotConfig::line("Write Throughput", "blockio-throughput-write", Unit::Datarate)
                .data(DataSource::counter_with_labels("blockio_bytes", [("op", "write")]))
                .build()
        )
        .plot(
            PlotConfig::line("Read IOPS", "blockio-iops-read", Unit::Count)
                .data(DataSource::counter_with_labels("blockio_operations", [("op", "read")]))
                .build()
        )
        .plot(
            PlotConfig::line("Write IOPS", "blockio-iops-write", Unit::Count)
                .data(DataSource::counter_with_labels("blockio_operations", [("op", "write")]))
                .build()
        )
}
