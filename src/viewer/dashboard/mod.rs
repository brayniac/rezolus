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


/// Builder for constructing dashboard views from declarative group configurations
pub struct DashboardBuilder<'a> {
    data: &'a Tsdb,
    view: View,
}

/// Conversion factor from nanoseconds to seconds, commonly used for CPU percentage calculations
const NANOSECONDS_PER_SECOND: f64 = 1e9;
/// Conversion factor from bytes to bits, used for network bandwidth calculations
const BITS_PER_BYTE: f64 = 8.0;

impl<'a> DashboardBuilder<'a> {
    pub fn new(data: &'a Tsdb, sections: Vec<Section>) -> Self {
        Self {
            data,
            view: View::new(data, sections),
        }
    }

    /// Adds a metrics group to the dashboard
    pub fn group(mut self, config: GroupConfig<'a>) -> Self {
        let group = config.build(self.data);
        self.view.group(group);
        self
    }

    /// Consumes the builder and returns the constructed View
    pub fn build(self) -> View {
        self.view
    }
}

/// Configuration for a group of related metrics plots
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

    pub fn plot(mut self, plot: PlotConfig<'a>) -> Self {
        self.plots.push(plot);
        self
    }

    /// Converts configuration into a Group by applying all plot configurations
    fn build(self, data: &Tsdb) -> Group {
        let mut group = Group::new(self.name, self.id);
        
        for plot_config in self.plots {
            plot_config.apply_to_group(&mut group, data);
        }
        
        group
    }
}

/// Configuration for individual plot types within a metrics group
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
        compute: Box<dyn Fn(&Tsdb) -> Option<Vec<UntypedSeries>> + 'a>,
        log_scale: bool,
    },
    Multi {
        title: String,
        id: String,
        unit: Unit,
        compute: Box<dyn Fn(&Tsdb) -> Option<Vec<(String, UntypedSeries)>> + 'a>,
    },
    Conditional {
        condition: Box<dyn Fn(&Tsdb) -> bool + 'a>,
        plot: Box<PlotConfig<'a>>,
    },
}

impl<'a> PlotConfig<'a> {
    pub fn line<S: Into<String>>(title: S, id: S, unit: Unit) -> PlotBuilder<'a> {
        PlotBuilder::line(title, id, unit)
    }

    pub fn heatmap<S: Into<String>>(title: S, id: S, unit: Unit) -> HeatmapBuilder<'a> {
        HeatmapBuilder::new(title, id, unit)
    }
    
    pub fn scatter<S: Into<String>>(title: S, id: S, unit: Unit) -> ScatterBuilder<'a> {
        ScatterBuilder::new(title, id, unit)
    }
    
    pub fn multi<S: Into<String>>(title: S, id: S, unit: Unit) -> MultiBuilder<'a> {
        MultiBuilder::new(title, id, unit)
    }
    
    /// Wraps a plot configuration with a conditional check that determines whether the plot should be rendered
    pub fn conditional<F>(condition: F, plot: PlotConfig<'a>) -> PlotConfig<'a>
    where
        F: Fn(&Tsdb) -> bool + 'a,
    {
        PlotConfig::Conditional {
            condition: Box::new(condition),
            plot: Box::new(plot),
        }
    }
    
    /// Creates a scatter plot displaying percentile distributions for the specified metric
    pub fn percentile_scatter<S, L>(title: S, id: S, unit: Unit, metric: &'a str, labels: L, log_scale: bool) -> PlotConfig<'a>
    where
        S: Into<String>,
        L: Into<Labels>,
    {
        let labels_val = labels.into();
        PlotConfig::scatter(title, id, unit)
            .compute(move |data| {
                data.percentiles(metric, labels_val.clone(), PERCENTILES)
            })
            .log_scale(log_scale)
            .build()
    }


    /// Applies this plot configuration to the specified group, fetching data and adding the appropriate plot type
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
            PlotConfig::Scatter { title, id, unit, compute, log_scale } => {
                if let Some(data_vec) = compute(data) {
                    let mut opts = PlotOpts::scatter(title, id, unit);
                    if log_scale {
                        opts = opts.with_log_scale(true);
                    }
                    group.scatter(opts, Some(data_vec));
                }
            }
            PlotConfig::Multi { title, id, unit, compute } => {
                if let Some(data_vec) = compute(data) {
                    group.multi(PlotOpts::multi(title, id, unit), Some(data_vec));
                }
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
    compute: Option<Box<dyn Fn(&Tsdb) -> Option<Vec<UntypedSeries>> + 'a>>,
    log_scale: bool,
}

impl<'a> ScatterBuilder<'a> {
    pub fn new<S: Into<String>>(title: S, id: S, unit: Unit) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            unit,
            compute: None,
            log_scale: false,
        }
    }

    pub fn compute<F>(mut self, f: F) -> Self
    where
        F: Fn(&Tsdb) -> Option<Vec<UntypedSeries>> + 'a,
    {
        self.compute = Some(Box::new(f));
        self
    }

    pub fn log_scale(mut self, enabled: bool) -> Self {
        self.log_scale = enabled;
        self
    }

    pub fn build(self) -> PlotConfig<'a> {
        PlotConfig::Scatter {
            title: self.title,
            id: self.id,
            unit: self.unit,
            compute: self.compute.expect("Compute function required"),
            log_scale: self.log_scale,
        }
    }
}

/// Builder for multi-series plots
pub struct MultiBuilder<'a> {
    title: String,
    id: String,
    unit: Unit,
    compute: Option<Box<dyn Fn(&Tsdb) -> Option<Vec<(String, UntypedSeries)>> + 'a>>,
}

impl<'a> MultiBuilder<'a> {
    pub fn new<S: Into<String>>(title: S, id: S, unit: Unit) -> Self {
        Self {
            title: title.into(),
            id: id.into(),
            unit,
            compute: None,
        }
    }

    pub fn compute<F>(mut self, f: F) -> Self
    where
        F: Fn(&Tsdb) -> Option<Vec<(String, UntypedSeries)>> + 'a,
    {
        self.compute = Some(Box::new(f));
        self
    }

    pub fn build(self) -> PlotConfig<'a> {
        PlotConfig::Multi {
            title: self.title,
            id: self.id,
            unit: self.unit,
            compute: self.compute.expect("Compute function required"),
        }
    }
}


/// Abstraction for fetching and transforming time series data from various metric types
pub enum DataSource<'a> {
    /// Counter metric that converts cumulative values to rates
    Counter {
        metric: &'a str,
        labels: Labels,
        transform: Option<Box<dyn Fn(UntypedSeries) -> UntypedSeries + 'a>>,
    },
    /// CPU utilization metric averaged across cores
    CpuAvg {
        metric: &'a str,
        labels: Labels,
        transform: Option<Box<dyn Fn(UntypedSeries) -> UntypedSeries + 'a>>,
    },
    /// Point-in-time value metric
    Gauge {
        metric: &'a str,
        labels: Labels,
        transform: Option<Box<dyn Fn(UntypedSeries) -> UntypedSeries + 'a>>,
    },
    /// Derived metric computed from arbitrary data sources
    Computed {
        compute: Box<dyn Fn(&Tsdb) -> Option<UntypedSeries> + 'a>,
    },
}

impl<'a> DataSource<'a> {
    /// Creates a counter that converts nanosecond CPU time to percentage utilization
    pub fn counter_as_percentage(metric: &'a str) -> Self {
        Self::counter(metric).with_transform(|v| v / NANOSECONDS_PER_SECOND)
    }
    
    /// Creates a labeled counter that converts nanosecond CPU time to percentage utilization
    pub fn counter_with_labels_as_percentage<L>(metric: &'a str, labels: L) -> Self
    where
        L: Into<Labels>,
    {
        Self::counter_with_labels(metric, labels)
            .with_transform(|v| v / NANOSECONDS_PER_SECOND)
    }
    
    /// Creates a counter that converts byte rates to bit rates for network bandwidth
    pub fn counter_as_bitrate<L>(metric: &'a str, labels: L) -> Self
    where
        L: Into<Labels>,
    {
        Self::counter_with_labels(metric, labels)
            .with_transform(|v| v * BITS_PER_BYTE)
    }
    
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

    pub fn cpu_avg<L>(metric: &'a str, labels: L) -> Self 
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
                match transform {
                    Some(t) => series.map(t),
                    None => series,
                }
            }
            Self::CpuAvg { metric, labels, transform } => {
                let series = data.cpu_avg(metric, labels.clone());
                match transform {
                    Some(t) => series.map(t),
                    None => series,
                }
            }
            Self::Gauge { metric, labels, transform } => {
                let series = data.gauges(metric, labels.clone())
                    .map(|v| v.sum());
                match transform {
                    Some(t) => series.map(t),
                    None => series,
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
    /// Create a CPU heatmap with labels (use () for no labels)
    pub fn cpu_heatmap<L>(metric: &'a str, labels: L) -> Self 
    where
        L: Into<Labels>,
    {
        Self::CpuHeatmap {
            metric,
            labels: labels.into(),
            transform: None,
        }
    }
    
    /// Helper for creating a CPU heatmap with nanoseconds to percentage transform
    pub fn cpu_heatmap_as_percentage<L>(metric: &'a str, labels: L) -> Self
    where
        L: Into<Labels>,
    {
        Self::cpu_heatmap(metric, labels)
            .with_transform(|v| v / NANOSECONDS_PER_SECOND)
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
                match transform {
                    Some(t) => heatmap.map(t),
                    None => heatmap,
                }
            }
            Self::Computed { compute } => compute(data),
        }
    }
}
