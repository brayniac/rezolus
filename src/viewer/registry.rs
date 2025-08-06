// Chart registry with section/group hierarchy
// This provides a cleaner way to define and organize charts with rich metadata

use crate::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// === Core Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartMetadata {
    /// Short title for compact display (e.g., "Busy %")
    pub short_title: String,
    /// Full descriptive title (e.g., "CPU Busy Percentage")
    pub long_title: String,
    /// Detailed description of what this chart shows
    pub description: String,
    /// Keywords for search and matching
    pub keywords: Vec<String>,
    /// When this chart is useful
    pub use_cases: Option<String>,
    /// Related charts by title
    pub related_charts: Vec<String>,
    /// Warning/critical thresholds if applicable
    pub thresholds: Option<Thresholds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    pub warning: Option<f64>,
    pub critical: Option<f64>,
}

pub struct ChartDefinition {
    pub metadata: ChartMetadata,
    pub chart_type: ChartType,
    pub unit: Unit,
    generator: Box<dyn Fn(&Tsdb) -> Option<Plot> + Send + Sync>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Heatmap,
    Scatter,
    Multi,
}

// === Chart Builder ===

pub struct ChartBuilder {
    metadata: ChartMetadata,
    chart_type: ChartType,
    unit: Unit,
    generator: Option<Box<dyn Fn(&Tsdb) -> Option<Plot> + Send + Sync>>,
}

impl ChartBuilder {
    pub fn new(short_title: impl Into<String>) -> Self {
        Self {
            metadata: ChartMetadata {
                short_title: short_title.into(),
                long_title: String::new(),
                description: String::new(),
                keywords: Vec::new(),
                use_cases: None,
                related_charts: Vec::new(),
                thresholds: None,
            },
            chart_type: ChartType::Line,
            unit: Unit::Count,
            generator: None,
        }
    }

    pub fn long_title(mut self, title: impl Into<String>) -> Self {
        self.metadata.long_title = title.into();
        if self.metadata.long_title.is_empty() {
            self.metadata.long_title = self.metadata.short_title.clone();
        }
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.metadata.description = desc.into();
        self
    }

    pub fn keywords<S: Into<String>>(mut self, keywords: impl IntoIterator<Item = S>) -> Self {
        self.metadata.keywords = keywords.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn use_cases(mut self, cases: impl Into<String>) -> Self {
        self.metadata.use_cases = Some(cases.into());
        self
    }

    pub fn related<S: Into<String>>(mut self, charts: impl IntoIterator<Item = S>) -> Self {
        self.metadata.related_charts = charts.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn thresholds(mut self, warning: Option<f64>, critical: Option<f64>) -> Self {
        self.metadata.thresholds = Some(Thresholds { warning, critical });
        self
    }

    pub fn chart_type(mut self, chart_type: ChartType) -> Self {
        self.chart_type = chart_type;
        self
    }

    pub fn unit(mut self, unit: Unit) -> Self {
        self.unit = unit;
        self
    }

    pub fn generator<F>(mut self, f: F) -> Self
    where
        F: Fn(&Tsdb) -> Option<Plot> + Send + Sync + 'static,
    {
        self.generator = Some(Box::new(f));
        self
    }

    pub fn build(self) -> ChartDefinition {
        ChartDefinition {
            metadata: self.metadata,
            chart_type: self.chart_type,
            unit: self.unit,
            generator: self.generator.expect("generator must be set"),
        }
    }
}

// === Group and Section Types ===

pub struct ChartGroup {
    pub name: String,
    pub id: String,
    pub charts: Vec<ChartDefinition>,
}

impl ChartGroup {
    pub fn new(name: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: id.into(),
            charts: Vec::new(),
        }
    }

    pub fn add_chart(mut self, chart: ChartDefinition) -> Self {
        self.charts.push(chart);
        self
    }

    pub fn generate(&self, data: &Tsdb) -> Group {
        let mut group = Group::new(&self.name, &self.id);
        
        for chart_def in &self.charts {
            if let Some(plot) = (chart_def.generator)(data) {
                group.push(Some(plot));
            }
        }
        
        group
    }
}

pub struct ChartSection {
    pub name: String,
    pub route: String,
    pub description: String,
    pub groups: BTreeMap<String, ChartGroup>,
}

impl ChartSection {
    pub fn new(name: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            route: route.into(),
            description: String::new(),
            groups: BTreeMap::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn add_group(mut self, group: ChartGroup) -> Self {
        self.groups.insert(group.id.clone(), group);
        self
    }

    pub fn generate(&self, data: &Tsdb, sections: Vec<Section>) -> View {
        let mut view = View::new(data, sections);
        
        for group in self.groups.values() {
            view.group(group.generate(data));
        }
        
        view
    }
}

// === Chart Registry ===

pub struct ChartRegistry {
    sections: BTreeMap<String, ChartSection>,
}

impl ChartRegistry {
    pub fn new() -> Self {
        Self {
            sections: BTreeMap::new(),
        }
    }

    pub fn add_section(mut self, section: ChartSection) -> Self {
        self.sections.insert(section.route.clone(), section);
        self
    }

    pub fn get_section(&self, route: &str) -> Option<&ChartSection> {
        self.sections.get(route)
    }

    /// Export all chart metadata as JSON for frontend/LLM consumption
    pub fn export_metadata_json(&self) -> String {
        #[derive(Serialize)]
        struct Export {
            sections: Vec<SectionExport>,
        }

        #[derive(Serialize)]
        struct SectionExport {
            name: String,
            route: String,
            description: String,
            groups: Vec<GroupExport>,
        }

        #[derive(Serialize)]
        struct GroupExport {
            name: String,
            id: String,
            charts: Vec<ChartExport>,
        }

        #[derive(Serialize)]
        struct ChartExport {
            metadata: ChartMetadata,
            chart_type: ChartType,
            unit: Unit,
            section: String,
            group: String,
            full_context: String,
        }

        let mut sections = Vec::new();

        for section in self.sections.values() {
            let mut groups = Vec::new();

            for group in section.groups.values() {
                let mut charts = Vec::new();

                for chart in &group.charts {
                    charts.push(ChartExport {
                        metadata: chart.metadata.clone(),
                        chart_type: chart.chart_type,
                        unit: chart.unit,
                        section: section.name.clone(),
                        group: group.name.clone(),
                        full_context: format!("{} / {} / {}", section.name, group.name, chart.metadata.long_title),
                    });
                }

                groups.push(GroupExport {
                    name: group.name.clone(),
                    id: group.id.clone(),
                    charts,
                });
            }

            sections.push(SectionExport {
                name: section.name.clone(),
                route: section.route.clone(),
                description: section.description.clone(),
                groups,
            });
        }

        serde_json::to_string_pretty(&Export { sections }).unwrap()
    }

    /// Export chart information formatted for LLM understanding
    pub fn export_for_llm(&self) -> String {
        let mut output = String::new();

        for section in self.sections.values() {
            output.push_str(&format!("## {} Section\n", section.name));
            if !section.description.is_empty() {
                output.push_str(&format!("{}\n", section.description));
            }
            output.push('\n');

            for group in section.groups.values() {
                output.push_str(&format!("### {} ({})\n", group.name, group.id));

                for chart in &group.charts {
                    let meta = &chart.metadata;
                    output.push_str(&format!(
                        "- **{}** ({})\n",
                        meta.long_title,
                        meta.short_title
                    ));
                    output.push_str(&format!("  Description: {}\n", meta.description));
                    
                    if !meta.keywords.is_empty() {
                        output.push_str(&format!("  Keywords: {}\n", meta.keywords.join(", ")));
                    }
                    
                    if let Some(use_cases) = &meta.use_cases {
                        output.push_str(&format!("  Use cases: {}\n", use_cases));
                    }
                    
                    if let Some(thresholds) = &meta.thresholds {
                        if let Some(warning) = thresholds.warning {
                            output.push_str(&format!("  Warning threshold: {}\n", warning));
                        }
                        if let Some(critical) = thresholds.critical {
                            output.push_str(&format!("  Critical threshold: {}\n", critical));
                        }
                    }
                    
                    if !meta.related_charts.is_empty() {
                        output.push_str(&format!("  Related: {}\n", meta.related_charts.join(", ")));
                    }
                    
                    output.push('\n');
                }
            }
        }

        output
    }

    /// Get all charts as a flat list with full context
    pub fn all_charts_with_context(&self) -> Vec<(String, ChartMetadata)> {
        let mut result = Vec::new();

        for section in self.sections.values() {
            for group in section.groups.values() {
                for chart in &group.charts {
                    let context = format!(
                        "{} / {} / {}",
                        section.name,
                        group.name,
                        chart.metadata.long_title
                    );
                    result.push((context, chart.metadata.clone()));
                }
            }
        }

        result
    }
}

// === Global Registry ===

pub static REGISTRY: Lazy<ChartRegistry> = Lazy::new(build_registry);

// === Registry Builder Function ===

fn build_registry() -> ChartRegistry {
    ChartRegistry::new()
        .add_section(build_cpu_section())
        .add_section(build_network_section())
    // Add more sections as needed
}

// === CPU Section Example ===

fn build_cpu_section() -> ChartSection {
    ChartSection::new("CPU", "/cpu")
        .description("CPU utilization, performance, and scheduling metrics")
        .add_group(
            ChartGroup::new("Utilization", "utilization")
                .add_chart(
                    ChartBuilder::new("Busy %")
                        .long_title("CPU Busy Percentage")
                        .description("Percentage of CPU time spent executing processes. High sustained values (>80%) indicate CPU saturation.")
                        .keywords(["cpu", "usage", "utilization", "busy", "processor", "load"])
                        .use_cases("Monitor for performance bottlenecks, capacity planning, detecting CPU-bound workloads")
                        .related(["CPU Idle %", "CPU User %", "CPU System %"])
                        .thresholds(Some(80.0), Some(95.0))
                        .chart_type(ChartType::Line)
                        .unit(Unit::Percentage)
                        .generator(|data| {
                            Plot::line(
                                "CPU Busy %",
                                "busy-pct",
                                Unit::Percentage,
                                data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0),
                            )
                        })
                        .build(),
                )
                .add_chart(
                    ChartBuilder::new("Idle %")
                        .long_title("CPU Idle Percentage")
                        .description("Percentage of time CPU cores are idle and available for work. Low values indicate high CPU utilization.")
                        .keywords(["cpu", "idle", "available", "free", "unused"])
                        .use_cases("Identify available CPU capacity, validate load balancing")
                        .related(["CPU Busy %"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::Percentage)
                        .generator(|data| {
                            let busy = data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0)?;
                            Plot::line(
                                "CPU Idle %",
                                "idle-pct",
                                Unit::Percentage,
                                Some(100.0 - busy),
                            )
                        })
                        .build(),
                ),
        )
        .add_group(
            ChartGroup::new("Performance", "performance")
                .add_chart(
                    ChartBuilder::new("IPC")
                        .long_title("Instructions per Cycle")
                        .description("Average number of instructions executed per CPU cycle. Higher values indicate better CPU efficiency.")
                        .keywords(["cpu", "performance", "ipc", "instructions", "efficiency"])
                        .use_cases("Analyze CPU efficiency, identify cache misses or pipeline stalls")
                        .chart_type(ChartType::Line)
                        .unit(Unit::Count)
                        .generator(|data| {
                            let cycles = data.counters("cpu_cycles", ()).map(|v| v.rate().sum())?;
                            let instructions = data.counters("cpu_instructions", ()).map(|v| v.rate().sum())?;
                            let ipc = instructions / cycles;
                            Plot::line("Instructions per Cycle", "ipc", Unit::Count, Some(ipc))
                        })
                        .build(),
                ),
        )
}

// === Network Section Example ===

fn build_network_section() -> ChartSection {
    ChartSection::new("Network", "/network")
        .description("Network traffic, throughput, and error metrics")
        .add_group(
            ChartGroup::new("Throughput", "throughput")
                .add_chart(
                    ChartBuilder::new("RX Bytes/s")
                        .long_title("Network Receive Throughput")
                        .description("Incoming network traffic in bytes per second. Spikes indicate increased network activity or potential DDoS.")
                        .keywords(["network", "receive", "rx", "incoming", "traffic", "bandwidth", "throughput"])
                        .use_cases("Monitor network load, detect traffic anomalies, capacity planning")
                        .related(["TX Bytes/s", "RX Packets/s"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::BytesPerSecond)
                        .generator(|data| {
                            Plot::line(
                                "Network RX Bytes/s",
                                "network-rx-bytes",
                                Unit::BytesPerSecond,
                                data.counters("network_rx_bytes", ()).map(|v| v.rate()),
                            )
                        })
                        .build(),
                ),
        )
}