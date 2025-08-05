use super::*;

pub fn generate(data: &Tsdb, sections: Vec<Section>) -> View {
    DashboardBuilder::new(data, sections)
        .group(scheduler_group())
        .build()
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
        .plot(
            PlotConfig::percentile_scatter(
                "Off CPU Time",
                "off-cpu-time",
                Unit::Time,
                "scheduler_offcpu",
                (),
                true
            )
        )
        .plot(
            PlotConfig::percentile_scatter(
                "Running Time",
                "running-time",
                Unit::Time,
                "scheduler_running",
                (),
                true
            )
        )
        .plot(
            PlotConfig::line("Context Switch", "cswitch", Unit::Rate)
                .data(DataSource::counter("scheduler_context_switch"))
                .build()
        )
}