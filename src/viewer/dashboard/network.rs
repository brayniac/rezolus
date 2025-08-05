use super::*;

pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(traffic_group())
        .group(tcp_group())
        .build()
}

fn traffic_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("Traffic", "traffic")
        .plot(
            PlotConfig::line("Bandwidth Transmit", "bandwidth-tx", Unit::Bitrate)
                .data(DataSource::counter_as_bitrate("network_bytes", [("direction", "transmit")]))
                .build()
        )
        .plot(
            PlotConfig::line("Bandwidth Receive", "bandwidth-rx", Unit::Bitrate)
                .data(DataSource::counter_as_bitrate("network_bytes", [("direction", "receive")]))
                .build()
        )
        .plot(
            PlotConfig::line("Packets Transmit", "packets-tx", Unit::Rate)
                .data(
                    DataSource::counter_with_labels("network_packets", [("direction", "transmit")])
                )
                .build()
        )
        .plot(
            PlotConfig::line("Packets Receive", "packets-rx", Unit::Rate)
                .data(
                    DataSource::counter_with_labels("network_packets", [("direction", "receive")])
                )
                .build()
        )
}

fn tcp_group<'a>() -> GroupConfig<'a> {
    GroupConfig::new("TCP", "tcp")
        .plot(
            PlotConfig::percentile_scatter(
                "Packet Latency",
                "tcp-packet-latency",
                Unit::Time,
                "tcp_packet_latency",
                (),
                true
            )
        )
}