// Simplified chart registry - single title with context from hierarchy
// The full context comes from section/group/title pattern

use super::plot::*;
use super::tsdb::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// === Core Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartMetadata {
    /// Title within the group context (e.g., "Busy %", "RX Bytes/s")
    pub title: String,
    /// Detailed description of what this chart shows
    pub description: String,
    /// Keywords for search and matching
    pub keywords: Vec<String>,
    /// When this chart is useful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_cases: Option<String>,
    /// Related charts by title
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related_charts: Vec<String>,
    /// Warning/critical thresholds if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
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

impl ChartDefinition {
    /// Get the full title with context
    pub fn full_title(&self, section: &str, group: &str) -> String {
        format!("{} / {} / {}", section, group, self.metadata.title)
    }
    
    /// Get a contextual title (e.g., "CPU Busy %" instead of just "Busy %")
    pub fn contextual_title(&self, section: &str) -> String {
        // For titles that already include context, return as-is
        if self.metadata.title.to_lowercase().contains(&section.to_lowercase()) {
            self.metadata.title.clone()
        } else {
            // Add section context to ambiguous titles
            format!("{} {}", section, self.metadata.title)
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Heatmap,
    Scatter,
    Multi,
}

// === Chart Builder (Simplified) ===

pub struct ChartBuilder {
    metadata: ChartMetadata,
    chart_type: ChartType,
    unit: Unit,
    generator: Option<Box<dyn Fn(&Tsdb) -> Option<Plot> + Send + Sync>>,
}

impl ChartBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            metadata: ChartMetadata {
                title: title.into(),
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

    /// Generate the View for this section
    pub fn generate(&self, data: &Tsdb, sections: Vec<Section>) -> View {
        let mut view = View::new(data, sections);
        
        for group in self.groups.values() {
            let mut view_group = Group::new(&group.name, &group.id);
            
            for chart in &group.charts {
                if let Some(plot) = (chart.generator)(data) {
                    view_group.push(Some(plot));
                }
            }
            
            view.group(view_group);
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

    /// Export all chart metadata as JSON for frontend/LLM consumption
    pub fn export_metadata_json(&self) -> String {
        #[derive(Serialize)]
        struct ChartExport {
            // Core metadata
            title: String,
            description: String,
            keywords: Vec<String>,
            
            // Context from hierarchy
            section: String,
            group: String,
            full_context: String,
            contextual_title: String,
            
            // Chart properties
            chart_type: ChartType,
            unit: Unit,
            
            // Optional metadata
            #[serde(skip_serializing_if = "Option::is_none")]
            use_cases: Option<String>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            related_charts: Vec<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            thresholds: Option<Thresholds>,
        }

        let mut all_charts = Vec::new();

        for section in self.sections.values() {
            for group in section.groups.values() {
                for chart in &group.charts {
                    all_charts.push(ChartExport {
                        title: chart.metadata.title.clone(),
                        description: chart.metadata.description.clone(),
                        keywords: chart.metadata.keywords.clone(),
                        section: section.name.clone(),
                        group: group.name.clone(),
                        full_context: chart.full_title(&section.name, &group.name),
                        contextual_title: chart.contextual_title(&section.name),
                        chart_type: chart.chart_type,
                        unit: chart.unit,
                        use_cases: chart.metadata.use_cases.clone(),
                        related_charts: chart.metadata.related_charts.clone(),
                        thresholds: chart.metadata.thresholds.clone(),
                    });
                }
            }
        }

        serde_json::to_string_pretty(&all_charts).unwrap()
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
                output.push_str(&format!("### {} Group\n", group.name));

                for chart in &group.charts {
                    let meta = &chart.metadata;
                    // Use contextual title for clarity
                    let contextual_title = chart.contextual_title(&section.name);
                    
                    output.push_str(&format!("- **{}**\n", contextual_title));
                    output.push_str(&format!("  Path: {} / {} / {}\n", 
                        section.name, group.name, meta.title));
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
    pub fn all_charts_with_context(&self) -> Vec<ChartInfo> {
        let mut result = Vec::new();

        for section in self.sections.values() {
            for group in section.groups.values() {
                for chart in &group.charts {
                    result.push(ChartInfo {
                        title: chart.metadata.title.clone(),
                        contextual_title: chart.contextual_title(&section.name),
                        full_path: chart.full_title(&section.name, &group.name),
                        section: section.name.clone(),
                        group: group.name.clone(),
                        description: chart.metadata.description.clone(),
                        keywords: chart.metadata.keywords.clone(),
                    });
                }
            }
        }

        result
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartInfo {
    pub title: String,
    pub contextual_title: String,  // e.g., "CPU Busy %"
    pub full_path: String,          // e.g., "CPU / Utilization / Busy %"
    pub section: String,
    pub group: String,
    pub description: String,
    pub keywords: Vec<String>,
}

// === Example Usage ===

fn build_cpu_section() -> ChartSection {
    ChartSection::new("CPU", "/cpu")
        .description("CPU utilization, performance, and scheduling metrics")
        .add_group(
            ChartGroup::new("Utilization", "utilization")
                .add_chart(
                    ChartBuilder::new("Busy %")  // Simple title - context comes from hierarchy
                        .description(
                            "Percentage of CPU time spent executing processes. \
                            High sustained values (>80%) indicate CPU saturation."
                        )
                        .keywords(["cpu", "usage", "utilization", "busy", "processor", "load"])
                        .use_cases("Monitor for performance bottlenecks, capacity planning")
                        .related(["Idle %", "User %", "System %"])
                        .thresholds(Some(80.0), Some(95.0))
                        .chart_type(ChartType::Line)
                        .unit(Unit::Percentage)
                        .generator(|data| {
                            Plot::line(
                                "Busy %",  // Keep original for backward compatibility
                                "busy-pct",
                                Unit::Percentage,
                                data.cpu_avg("cpu_usage", ()).map(|v| v / 1000000000.0),
                            )
                        })
                        .build(),
                ),
        )
        .add_group(
            ChartGroup::new("Performance", "performance")
                .add_chart(
                    ChartBuilder::new("IPC")  // Already unique enough
                        .description(
                            "Instructions executed per CPU cycle. \
                            Higher values indicate better CPU efficiency."
                        )
                        .keywords(["cpu", "performance", "ipc", "instructions", "efficiency"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::Count)
                        .generator(|data| {
                            let cycles = data.counters("cpu_cycles", ()).map(|v| v.rate().sum())?;
                            let instructions = data.counters("cpu_instructions", ()).map(|v| v.rate().sum())?;
                            Plot::line("IPC", "ipc", Unit::Count, Some(instructions / cycles))
                        })
                        .build(),
                ),
        )
}

fn build_network_section() -> ChartSection {
    ChartSection::new("Network", "/network")
        .description("Network traffic, throughput, and error metrics")
        .add_group(
            ChartGroup::new("Throughput", "throughput")
                .add_chart(
                    ChartBuilder::new("RX Bytes/s")  // Clear enough in Network context
                        .description("Incoming network traffic in bytes per second.")
                        .keywords(["network", "receive", "rx", "incoming", "traffic", "bandwidth"])
                        .related(["TX Bytes/s", "RX Packets/s"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::Datarate)
                        .generator(|data| {
                            Plot::line(
                                "RX Bytes/s",
                                "network-rx-bytes",
                                Unit::Datarate,
                                data.counters("network_bytes", [("direction", "receive")])
                                    .map(|v| v.rate().sum()),
                            )
                        })
                        .build(),
                )
                .add_chart(
                    ChartBuilder::new("TX Bytes/s")
                        .description("Outgoing network traffic in bytes per second.")
                        .keywords(["network", "transmit", "tx", "outgoing", "traffic", "bandwidth"])
                        .related(["RX Bytes/s", "TX Packets/s"])
                        .chart_type(ChartType::Line)
                        .unit(Unit::Datarate)
                        .generator(|data| {
                            Plot::line(
                                "TX Bytes/s",
                                "network-tx-bytes",
                                Unit::Datarate,
                                data.counters("network_bytes", [("direction", "transmit")])
                                    .map(|v| v.rate().sum()),
                            )
                        })
                        .build(),
                ),
        )
}