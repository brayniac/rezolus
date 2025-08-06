// A practical implementation that works without proc macros
// This could be implemented today with minimal changes

use once_cell::sync::Lazy;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

// === Core Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartMetadata {
    pub short_title: String,
    pub long_title: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub section: String,
    pub group: String,
    pub chart_type: ChartType,
    pub unit: Unit,
    pub related_charts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Heatmap,
    Scatter,
    Multi,
}

// === Chart Builder for Declarative Definitions ===

pub struct ChartBuilder {
    metadata: ChartMetadata,
    generator: Box<dyn Fn(&Tsdb) -> Option<Plot> + Send + Sync>,
}

impl ChartBuilder {
    pub fn new(short_title: impl Into<String>) -> Self {
        Self {
            metadata: ChartMetadata {
                short_title: short_title.into(),
                long_title: String::new(),
                description: String::new(),
                keywords: Vec::new(),
                section: String::new(),
                group: String::new(),
                chart_type: ChartType::Line,
                unit: Unit::Count,
                related_charts: Vec::new(),
            },
            generator: Box::new(|_| None),
        }
    }
    
    pub fn long_title(mut self, title: impl Into<String>) -> Self {
        self.metadata.long_title = title.into();
        self
    }
    
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.metadata.description = desc.into();
        self
    }
    
    pub fn keywords(mut self, keywords: Vec<&str>) -> Self {
        self.metadata.keywords = keywords.iter().map(|s| s.to_string()).collect();
        self
    }
    
    pub fn section(mut self, section: impl Into<String>) -> Self {
        self.metadata.section = section.into();
        self
    }
    
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.metadata.group = group.into();
        self
    }
    
    pub fn chart_type(mut self, chart_type: ChartType) -> Self {
        self.metadata.chart_type = chart_type;
        self
    }
    
    pub fn unit(mut self, unit: Unit) -> Self {
        self.metadata.unit = unit;
        self
    }
    
    pub fn related(mut self, charts: Vec<&str>) -> Self {
        self.metadata.related_charts = charts.iter().map(|s| s.to_string()).collect();
        self
    }
    
    pub fn generator<F>(mut self, f: F) -> Self 
    where
        F: Fn(&Tsdb) -> Option<Plot> + Send + Sync + 'static
    {
        self.generator = Box::new(f);
        self
    }
    
    pub fn build(self) -> ChartDefinition {
        ChartDefinition {
            metadata: self.metadata,
            generator: self.generator,
        }
    }
}

pub struct ChartDefinition {
    pub metadata: ChartMetadata,
    generator: Box<dyn Fn(&Tsdb) -> Option<Plot> + Send + Sync>,
}

impl ChartDefinition {
    pub fn generate(&self, data: &Tsdb) -> Option<Plot> {
        (self.generator)(data)
    }
}

// === Chart Registry ===

pub struct ChartRegistry {
    charts: Vec<ChartDefinition>,
    by_section: HashMap<String, Vec<usize>>,
    by_title: HashMap<String, usize>,
}

impl ChartRegistry {
    pub fn new() -> Self {
        Self {
            charts: Vec::new(),
            by_section: HashMap::new(),
            by_title: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, chart: ChartDefinition) {
        let idx = self.charts.len();
        let section = chart.metadata.section.clone();
        let title = chart.metadata.short_title.clone();
        
        self.by_section.entry(section).or_default().push(idx);
        self.by_title.insert(title, idx);
        self.charts.push(chart);
    }
    
    pub fn get_metadata(&self) -> Vec<&ChartMetadata> {
        self.charts.iter().map(|c| &c.metadata).collect()
    }
    
    pub fn generate_section(&self, data: &Tsdb, section: &str) -> Vec<Plot> {
        let mut plots = Vec::new();
        
        if let Some(indices) = self.by_section.get(section) {
            for &idx in indices {
                if let Some(plot) = self.charts[idx].generate(data) {
                    plots.push(plot);
                }
            }
        }
        
        plots
    }
}

// === Global Registry ===

pub static REGISTRY: Lazy<ChartRegistry> = Lazy::new(|| {
    let mut registry = ChartRegistry::new();
    register_all_charts(&mut registry);
    registry
});

// === Macro for Easier Registration ===

macro_rules! chart {
    (
        short: $short:expr,
        long: $long:expr,
        desc: $desc:expr,
        keywords: [$($kw:expr),*],
        section: $section:expr,
        group: $group:expr,
        type: $type:expr,
        unit: $unit:expr,
        generate: $gen:expr
    ) => {
        ChartBuilder::new($short)
            .long_title($long)
            .description($desc)
            .keywords(vec![$($kw),*])
            .section($section)
            .group($group)
            .chart_type($type)
            .unit($unit)
            .generator($gen)
            .build()
    };
}

// === Registration Function ===

fn register_all_charts(registry: &mut ChartRegistry) {
    // CPU Charts
    registry.register(chart! {
        short: "CPU Busy",
        long: "CPU Busy Percentage",
        desc: "Percentage of CPU time spent executing processes. High sustained values (>80%) indicate CPU saturation.",
        keywords: ["cpu", "usage", "utilization", "busy", "processor"],
        section: "cpu",
        group: "utilization",
        type: ChartType::Line,
        unit: Unit::Percentage,
        generate: |data: &Tsdb| {
            Plot::line(
                "CPU Busy Percentage",
                "cpu-busy",
                Unit::Percentage,
                data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0),
            )
        }
    });
    
    registry.register(chart! {
        short: "CPU Idle",
        long: "CPU Idle Percentage",
        desc: "Percentage of time CPU cores are idle. Low values indicate high CPU utilization.",
        keywords: ["cpu", "idle", "available", "free"],
        section: "cpu",
        group: "utilization",
        type: ChartType::Line,
        unit: Unit::Percentage,
        generate: |data: &Tsdb| {
            Plot::line(
                "CPU Idle Percentage",
                "cpu-idle",
                Unit::Percentage,
                data.cpu_avg("cpu_idle", ()).map(|v| v / 1000000000.0),
            )
        }
    });
    
    // Network Charts
    registry.register(
        ChartBuilder::new("Network RX")
            .long_title("Network Bytes Received per Second")
            .description("Incoming network traffic in bytes per second. Spikes indicate increased network activity.")
            .keywords(vec!["network", "receive", "rx", "incoming", "traffic", "bandwidth"])
            .section("network")
            .group("throughput")
            .chart_type(ChartType::Line)
            .unit(Unit::BytesPerSecond)
            .related(vec!["Network TX", "Network Packets RX"])
            .generator(|data: &Tsdb| {
                Plot::line(
                    "Network Bytes Received per Second",
                    "network-rx",
                    Unit::BytesPerSecond,
                    data.counters("network_rx_bytes", ()).map(|v| v.rate()),
                )
            })
            .build()
    );
    
    // Add more charts here...
}

// === Export Functions ===

pub fn export_metadata_json() -> String {
    let metadata: Vec<_> = REGISTRY.get_metadata().into_iter().cloned().collect();
    serde_json::to_string_pretty(&metadata).unwrap()
}

pub fn export_metadata_for_llm() -> String {
    let mut output = String::new();
    
    for chart in REGISTRY.get_metadata() {
        output.push_str(&format!(
            "- {} ({}): {}\n  Keywords: {}\n  Section: {}, Group: {}\n\n",
            chart.long_title,
            chart.short_title,
            chart.description,
            chart.keywords.join(", "),
            chart.section,
            chart.group
        ));
    }
    
    output
}