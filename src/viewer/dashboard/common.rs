use serde::{Deserialize, Serialize};

/// Unit types for dashboard panels
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Count,
    Rate,
    Time,
    Bytes,
    Datarate,
    Bitrate,
    Percentage,
    Frequency,
}

/// Dashboard definition using PromQL queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLDashboard {
    pub name: String,
    pub sections: Vec<DashboardSection>,
    pub groups: Vec<PromQLGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSection {
    pub name: String,
    pub route: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLGroup {
    pub name: String,
    pub id: String,
    pub panels: Vec<PromQLPanel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLPanel {
    pub title: String,
    pub id: String,
    #[serde(rename = "type")]
    pub panel_type: PanelType,
    pub queries: Vec<PromQLQueryDef>,
    pub unit: Unit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<PanelOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelType {
    Line,
    Heatmap,
    Scatter,
    Multi,
    Gauge,
    Stat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromQLQueryDef {
    pub expr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PanelOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_scale: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroup_filter: Option<String>,
}

pub fn default_sections() -> Vec<DashboardSection> {
    vec![
        DashboardSection { name: "Overview".to_string(), route: "/overview".to_string() },
        DashboardSection { name: "CPU".to_string(), route: "/cpu".to_string() },
        DashboardSection { name: "Network".to_string(), route: "/network".to_string() },
        DashboardSection { name: "Scheduler".to_string(), route: "/scheduler".to_string() },
        DashboardSection { name: "Syscall".to_string(), route: "/syscall".to_string() },
        DashboardSection { name: "Softirq".to_string(), route: "/softirq".to_string() },
        DashboardSection { name: "BlockIO".to_string(), route: "/blockio".to_string() },
        DashboardSection { name: "cgroups".to_string(), route: "/cgroups".to_string() },
        DashboardSection { name: "Rezolus".to_string(), route: "/rezolus".to_string() },
        DashboardSection { name: "AI".to_string(), route: "/ai".to_string() },
    ]
}