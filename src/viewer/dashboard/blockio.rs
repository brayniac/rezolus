use super::*;

pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(operations_group())
        .group(latency_group())
        .group(io_size_group())
        .build()
}

fn operations_group<'a>() -> GroupConfig<'a> {
    let mut group = GroupConfig::new("Operations", "operations")
        .plot(
            PlotConfig::line("Total Throughput", "blockio-throughput-total", Unit::Datarate)
                .data(DataSource::counter("blockio_bytes"))
                .build()
        )
        .plot(
            PlotConfig::line("Total IOPS", "blockio-iops-total", Unit::Count)
                .data(DataSource::counter("blockio_operations"))
                .build()
        );

    for op in ["Read", "Write"] {
        let op_lower = op.to_lowercase();
        
        group = group.plot(
            PlotConfig::line(
                format!("{} Throughput", op),
                format!("throughput-{}", op_lower),
                Unit::Datarate
            )
            .data(
                DataSource::counter_with_labels("blockio_bytes", [("op", op_lower.as_str())])
            )
            .build()
        );
        
        group = group.plot(
            PlotConfig::line(
                format!("{} IOPS", op),
                format!("iops-{}", op_lower),
                Unit::Count
            )
            .data(
                DataSource::counter_with_labels("blockio_operations", [("op", op_lower.as_str())])
            )
            .build()
        );
    }

    group
}

fn latency_group<'a>() -> GroupConfig<'a> {
    let mut group = GroupConfig::new("Latency", "latency");

    for op in ["Read", "Write"] {
        let op_lower = op.to_lowercase();
        let plot_id = format!("latency-{}", op_lower);
        
        group = group.plot(
            PlotConfig::percentile_scatter(
                op.to_string(),
                plot_id,
                Unit::Time,
                "blockio_latency",
                [("op", op_lower.as_str())],
                true
            )
        );
    }

    group
}

fn io_size_group<'a>() -> GroupConfig<'a> {
    let mut group = GroupConfig::new("Size", "size");

    for op in ["Read", "Write"] {
        let op_lower = op.to_lowercase();
        let plot_id = format!("size-{}", op_lower);
        
        group = group.plot(
            PlotConfig::percentile_scatter(
                op.to_string(),
                plot_id,
                Unit::Bytes,
                "blockio_size",
                [("op", op_lower.as_str())],
                true
            )
        );
    }

    group
}