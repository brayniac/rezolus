use super::*;
use crate::viewer::tsdb::Labels;

mod blockio;
mod cgroups;
mod cpu;
mod network;
mod overview;
mod rezolus;
mod scheduler;
mod softirq;
mod syscall;

type Generator = fn(&Tsdb, Vec<Section>) -> View;

static SECTION_META: &[(&str, &str, Generator)] = &[
    ("Overview", "/overview", overview::generate),
    ("CPU", "/cpu", cpu::generate),
    ("Network", "/network", network::generate),
    ("Scheduler", "/scheduler", scheduler::generate),
    ("Syscall", "/syscall", syscall::generate),
    ("Softirq", "/softirq", softirq::generate),
    ("BlockIO", "/blockio", blockio::generate),
    ("cgroups", "/cgroups", cgroups::generate),
    ("Rezolus", "/rezolus", rezolus::generate),
];

pub fn generate(data: &Tsdb) -> AppState {
    let mut state = AppState::new();

    let sections: Vec<Section> = SECTION_META
        .iter()
        .map(|(name, route, _)| Section {
            name: (*name).to_string(),
            route: (*route).to_string(),
        })
        .collect();

    for (_, route, generator) in SECTION_META {
        let key = format!("{}.json", &route[1..]);
        let view = generator(data, sections.clone());
        state
            .sections
            .insert(key, serde_json::to_string(&view).unwrap());
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_expected_keys() {
        let data = Tsdb::default();
        let state = generate(&data);

        let mut keys: Vec<_> = state.sections.keys().cloned().collect();
        keys.sort();

        assert_eq!(
            keys,
            vec![
                "blockio.json",
                "cgroups.json",
                "cpu.json",
                "network.json",
                "overview.json",
                "rezolus.json",
                "scheduler.json",
                "softirq.json",
                "syscall.json",
            ]
        );
    }
}

// Dashboard Builder Pattern Implementation

/// Declarative dashboard builder following the Builder pattern
pub struct DashboardBuilder<'a> {
    data: &'a Tsdb,
    sections: Vec<Section>,
    view: View,
}

impl<'a> DashboardBuilder<'a> {
    pub fn new(data: &'a Tsdb, sections: Vec<Section>) -> Self {
        Self {
            data,
            sections: sections.clone(),
            view: View::new(data, sections),
        }
    }

    /// Add a group with declarative configuration
    pub fn group(mut self, config: GroupConfig<'a>) -> Self {
        let group = config.build(self.data);
        self.view.group(group);
        self
    }

    /// Build the final View
    pub fn build(self) -> View {
        self.view
    }
}

/// Declarative group configuration
pub struct GroupConfig<'a> {
    name: String,
    id: String,
    plots: Vec<PlotConfig<'a>>,
}

impl<'a> GroupConfig<'a> {
    pub fn new<S: Into<String>>(name: S, id: S) -> Self {
        Self {
            name: name.into(),
            id: id.into(),
            plots: Vec::new(),
        }
    }

    /// Add a plot configuration
    pub fn plot(mut self, plot: PlotConfig<'a>) -> Self {
        self.plots.push(plot);
        self
    }

    /// Build the group from configuration
    fn build(self, data: &Tsdb) -> Group {
        let mut group = Group::new(self.name, self.id);
        
        for plot_config in self.plots {
            plot_config.apply_to_group(&mut group, data);
        }
        
        group
    }
}

/// Declarative plot configuration
pub enum PlotConfig<'a> {
    Line {
        title: String,
        id: String,
        unit: Unit,
        data_source: DataSource<'a>,
    },
    Heatmap {
        title: String,
        id: String,
        unit: Unit,
        data_source: HeatmapSource<'a>,
    },
    Scatter {
        title: String,
        id: String,
        unit: Unit,
        data_sources: Vec<DataSource<'a>>,
    },
    Multi {
        title: String,
        id: String,
        unit: Unit,
        data_sources: Vec<(String, DataSource<'a>)>,
    },
    Conditional {
        condition: Box<dyn Fn(&Tsdb) -> bool + 'a>,
        plot: Box<PlotConfig<'a>>,
    },
}

impl<'a> PlotConfig<'a> {
    /// Create a line plot configuration
    pub fn line<S: Into<String>>(title: S, id: S, unit: Unit) -> PlotBuilder<'a> {
        PlotBuilder::line(title, id, unit)
    }

    /// Create a heatmap plot configuration
    pub fn heatmap<S: Into<String>>(title: S, id: S, unit: Unit) -> HeatmapBuilder<'a> {
        HeatmapBuilder::new(title, id, unit)
    }

    /// Create a scatter plot configuration
    pub fn scatter<S: Into<String>>(title: S, id: S, unit: Unit) -> ScatterBuilder<'a> {
        ScatterBuilder::new(title, id, unit)
    }

    /// Create a multi-series plot configuration
    pub fn multi<S: Into<String>>(title: S, id: S, unit: Unit) -> MultiBuilder<'a> {
        MultiBuilder::new(title, id, unit)
    }

    /// Apply this configuration to a group
    fn apply_to_group(self, group: &mut Group, data: &Tsdb) {
        match self {
            PlotConfig::Line { title, id, unit, data_source } => {
                let series = data_source.fetch(data);
                group.plot(PlotOpts::line(title, id, unit), series);
            }
            PlotConfig::Heatmap { title, id, unit, data_source } => {
                let heatmap = data_source.fetch(data);
                group.heatmap(PlotOpts::heatmap(title, id, unit), heatmap);
            }
            PlotConfig::Scatter { title, id, unit, data_sources } => {
                let data_vec: Option<Vec<_>> = data_sources
                    .into_iter()
                    .map(|source| source.fetch(data))
                    .collect::<Option<Vec<_>>>();
                group.scatter(PlotOpts::scatter(title, id, unit), data_vec);
            }
            PlotConfig::Multi { title, id, unit, data_sources } => {
                let data_vec: Option<Vec<_>> = data_sources
                    .into_iter()
                    .map(|(label, source)| source.fetch(data).map(|s| (label, s)))
                    .collect::<Option<Vec<_>>>();
                group.multi(PlotOpts::multi(title, id, unit), data_vec);
            }
            PlotConfig::Conditional { condition, plot } => {
                if condition(data) {
                    plot.apply_to_group(group, data);
                }
            }
        }
    }
}

/// Builder for line plots
pub struct PlotBuilder<'a> {
    title: String,
    id: String,
    unit: Unit,
    data_source: Option<DataSource<'a>>,
}

impl<'a> PlotBuilder<'a> {
    pub fn line<S: Into<String>>(title: S, id: S, unit: Unit) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            unit,
            data_source: None,
        }
    }

    pub fn data(mut self, source: DataSource<'a>) -> Self {
        self.data_source = Some(source);
        self
    }

    pub fn build(self) -> PlotConfig<'a> {
        PlotConfig::Line {
            title: self.title,
            id: self.id,
            unit: self.unit,
            data_source: self.data_source.expect("Data source required"),
        }
    }
}

/// Builder for heatmap plots
pub struct HeatmapBuilder<'a> {
    title: String,
    id: String,
    unit: Unit,
    data_source: Option<HeatmapSource<'a>>,
}

impl<'a> HeatmapBuilder<'a> {
    pub fn new<S: Into<String>>(title: S, id: S, unit: Unit) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            unit,
            data_source: None,
        }
    }

    pub fn data(mut self, source: HeatmapSource<'a>) -> Self {
        self.data_source = Some(source);
        self
    }

    pub fn build(self) -> PlotConfig<'a> {
        PlotConfig::Heatmap {
            title: self.title,
            id: self.id,
            unit: self.unit,
            data_source: self.data_source.expect("Data source required"),
        }
    }
}

/// Builder for scatter plots
pub struct ScatterBuilder<'a> {
    title: String,
    id: String,
    unit: Unit,
    data_sources: Vec<DataSource<'a>>,
}

impl<'a> ScatterBuilder<'a> {
    pub fn new<S: Into<String>>(title: S, id: S, unit: Unit) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            unit,
            data_sources: Vec::new(),
        }
    }

    pub fn add_series(mut self, source: DataSource<'a>) -> Self {
        self.data_sources.push(source);
        self
    }

    pub fn build(self) -> PlotConfig<'a> {
        PlotConfig::Scatter {
            title: self.title,
            id: self.id,
            unit: self.unit,
            data_sources: self.data_sources,
        }
    }
}

/// Builder for multi-series plots
pub struct MultiBuilder<'a> {
    title: String,
    id: String,
    unit: Unit,
    data_sources: Vec<(String, DataSource<'a>)>,
}

impl<'a> MultiBuilder<'a> {
    pub fn new<S: Into<String>>(title: S, id: S, unit: Unit) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            unit,
            data_sources: Vec::new(),
        }
    }

    pub fn add_series<S: Into<String>>(mut self, label: S, source: DataSource<'a>) -> Self {
        self.data_sources.push((label.into(), source));
        self
    }

    pub fn build(self) -> PlotConfig<'a> {
        PlotConfig::Multi {
            title: self.title,
            id: self.id,
            unit: self.unit,
            data_sources: self.data_sources,
        }
    }
}

/// Data source abstraction
pub enum DataSource<'a> {
    /// Simple counter with optional labels
    Counter {
        metric: &'a str,
        labels: Labels,
        transform: Option<Box<dyn Fn(UntypedSeries) -> UntypedSeries + 'a>>,
    },
    /// CPU average metric
    CpuAvg {
        metric: &'a str,
        labels: Labels,
        transform: Option<Box<dyn Fn(UntypedSeries) -> UntypedSeries + 'a>>,
    },
    /// Gauge metric
    Gauge {
        metric: &'a str,
        labels: Labels,
        transform: Option<Box<dyn Fn(UntypedSeries) -> UntypedSeries + 'a>>,
    },
    /// Computed metric from multiple sources
    Computed {
        compute: Box<dyn Fn(&Tsdb) -> Option<UntypedSeries> + 'a>,
    },
}

impl<'a> DataSource<'a> {
    pub fn counter(metric: &'a str) -> Self {
        Self::Counter {
            metric,
            labels: Labels::default(),
            transform: None,
        }
    }

    pub fn counter_with_labels<L>(metric: &'a str, labels: L) -> Self 
    where
        L: Into<Labels>,
    {
        Self::Counter {
            metric,
            labels: labels.into(),
            transform: None,
        }
    }

    pub fn cpu_avg(metric: &'a str) -> Self {
        Self::CpuAvg {
            metric,
            labels: Labels::default(),
            transform: None,
        }
    }

    pub fn cpu_avg_with_labels<L>(metric: &'a str, labels: L) -> Self 
    where
        L: Into<Labels>,
    {
        Self::CpuAvg {
            metric,
            labels: labels.into(),
            transform: None,
        }
    }

    pub fn gauge(metric: &'a str) -> Self {
        Self::Gauge {
            metric,
            labels: Labels::default(),
            transform: None,
        }
    }

    pub fn with_transform<F>(mut self, f: F) -> Self 
    where
        F: Fn(UntypedSeries) -> UntypedSeries + 'a,
    {
        match &mut self {
            Self::Counter { transform, .. } |
            Self::CpuAvg { transform, .. } |
            Self::Gauge { transform, .. } => {
                *transform = Some(Box::new(f));
            }
            _ => {}
        }
        self
    }

    pub fn computed<F>(f: F) -> Self 
    where
        F: Fn(&Tsdb) -> Option<UntypedSeries> + 'a,
    {
        Self::Computed {
            compute: Box::new(f),
        }
    }

    fn fetch(&self, data: &Tsdb) -> Option<UntypedSeries> {
        match self {
            Self::Counter { metric, labels, transform } => {
                let series = data.counters(metric, labels.clone())
                    .map(|v| v.rate().sum());
                if let Some(t) = transform {
                    series.map(t)
                } else {
                    series
                }
            }
            Self::CpuAvg { metric, labels, transform } => {
                let series = data.cpu_avg(metric, labels.clone());
                if let Some(t) = transform {
                    series.map(t)
                } else {
                    series
                }
            }
            Self::Gauge { metric, labels, transform } => {
                let series = data.gauges(metric, labels.clone())
                    .map(|v| v.sum());
                if let Some(t) = transform {
                    series.map(t)
                } else {
                    series
                }
            }
            Self::Computed { compute } => compute(data),
        }
    }
}

/// Heatmap data source abstraction
pub enum HeatmapSource<'a> {
    CpuHeatmap {
        metric: &'a str,
        labels: Labels,
        transform: Option<Box<dyn Fn(Heatmap) -> Heatmap + 'a>>,
    },
    Computed {
        compute: Box<dyn Fn(&Tsdb) -> Option<Heatmap> + 'a>,
    },
}

impl<'a> HeatmapSource<'a> {
    pub fn cpu_heatmap(metric: &'a str) -> Self {
        Self::CpuHeatmap {
            metric,
            labels: Labels::default(),
            transform: None,
        }
    }

    pub fn cpu_heatmap_with_labels<L>(metric: &'a str, labels: L) -> Self 
    where
        L: Into<Labels>,
    {
        Self::CpuHeatmap {
            metric,
            labels: labels.into(),
            transform: None,
        }
    }

    pub fn with_transform<F>(mut self, f: F) -> Self 
    where
        F: Fn(Heatmap) -> Heatmap + 'a,
    {
        match &mut self {
            Self::CpuHeatmap { transform, .. } => {
                *transform = Some(Box::new(f));
            }
            _ => {}
        }
        self
    }

    pub fn computed<F>(f: F) -> Self 
    where
        F: Fn(&Tsdb) -> Option<Heatmap> + 'a,
    {
        Self::Computed {
            compute: Box::new(f),
        }
    }

    fn fetch(&self, data: &Tsdb) -> Option<Heatmap> {
        match self {
            Self::CpuHeatmap { metric, labels, transform } => {
                let heatmap = data.cpu_heatmap(metric, labels.clone());
                if let Some(t) = transform {
                    heatmap.map(t)
                } else {
                    heatmap
                }
            }
            Self::Computed { compute } => compute(data),
        }
    }
}
