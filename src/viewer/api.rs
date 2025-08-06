use super::*;
use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// API query parameters for time range selection
#[derive(Debug, Deserialize)]
pub struct TimeRangeQuery {
    /// Start time in seconds since epoch (optional, defaults to beginning)
    pub start: Option<i64>,
    /// End time in seconds since epoch (optional, defaults to end)
    pub end: Option<i64>,
    /// Refresh interval in seconds for live mode (optional)
    pub refresh: Option<u32>,
}

/// API query parameters for metric queries
#[derive(Debug, Deserialize)]
pub struct MetricQuery {
    /// Metric name
    pub metric: String,
    /// Labels to filter by (JSON encoded)
    pub labels: Option<String>,
    /// Time range parameters
    #[serde(flatten)]
    pub time_range: TimeRangeQuery,
}

/// Response for available metrics endpoint
#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub metrics: Vec<String>,
}

/// Response for dashboard configuration
#[derive(Debug, Serialize)]
pub struct DashboardConfig {
    pub sections: Vec<Section>,
    pub groups: Vec<GroupDefinition>,
}

#[derive(Debug, Serialize)]
pub struct GroupDefinition {
    pub name: String,
    pub id: String,
    pub plots: Vec<PlotDefinition>,
}

#[derive(Debug, Serialize)]
pub struct PlotDefinition {
    pub title: String,
    pub id: String,
    pub plot_type: String,
    pub metric: Option<String>,
    pub labels: Option<Vec<(String, String)>>,
    pub unit: Unit,
}

/// Get list of available metrics
pub async fn metrics_list(
    State(state): State<Arc<AppStateV2>>,
) -> Json<MetricsResponse> {
    let metrics = state.tsdb.available_metrics();
    Json(MetricsResponse { metrics })
}

/// Get dashboard configuration for a specific section
pub async fn dashboard_config(
    State(state): State<Arc<AppStateV2>>,
    axum::extract::Path(section): axum::extract::Path<String>,
) -> Json<DashboardConfig> {
    // Return the dashboard structure without data
    // This tells the frontend what plots to request
    match section.as_str() {
        "cpu" => Json(cpu_dashboard_config()),
        "network" => Json(network_dashboard_config()),
        "blockio" => Json(blockio_dashboard_config()),
        "scheduler" => Json(scheduler_dashboard_config()),
        "syscall" => Json(syscall_dashboard_config()),
        "softirq" => Json(softirq_dashboard_config()),
        "rezolus" => Json(rezolus_dashboard_config()),
        "cgroups" => Json(cgroups_dashboard_config()),
        _ => Json(overview_dashboard_config()),
    }
}

/// Query metric data
pub async fn query_metric(
    State(state): State<Arc<AppStateV2>>,
    Query(params): Query<MetricQuery>,
) -> Json<PlotData> {
    let labels = params.labels
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    
    // Apply time range filter if specified
    let data = if let (Some(start), Some(end)) = (params.time_range.start, params.time_range.end) {
        state.tsdb.query_range(&params.metric, labels, start, end)
    } else {
        state.tsdb.query(&params.metric, labels)
    };
    
    Json(data.into())
}

/// Query plot data directly
pub async fn query_plot(
    State(state): State<Arc<AppStateV2>>,
    axum::extract::Path((section, plot_id)): axum::extract::Path<(String, String)>,
    Query(time_range): Query<TimeRangeQuery>,
) -> Json<Plot> {
    // Build the specific plot based on section and plot_id
    // This approach keeps the plot building logic on the server
    let plot = match section.as_str() {
        "cpu" => build_cpu_plot(&state.tsdb, &plot_id, &time_range),
        "network" => build_network_plot(&state.tsdb, &plot_id, &time_range),
        // ... other sections
        _ => None,
    };
    
    Json(plot.unwrap_or_else(|| Plot::empty()))
}

/// Stream updates for live mode
pub async fn stream_updates(
    State(state): State<Arc<AppStateV2>>,
    Query(params): Query<MetricQuery>,
) -> impl IntoResponse {
    use axum::response::sse::{Event, Sse};
    use futures::stream::{self, Stream};
    use std::convert::Infallible;
    
    let refresh_interval = params.refresh.unwrap_or(1);
    
    let stream = stream::repeat_with(move || {
        // In real implementation, this would fetch only new data
        let data = state.tsdb.query(&params.metric, Labels::default());
        Event::default().json_data(data)
    })
    .map(Ok::<_, Infallible>)
    .throttle(Duration::from_secs(refresh_interval as u64));
    
    Sse::new(stream)
}

// Helper functions to generate dashboard configurations
fn cpu_dashboard_config() -> DashboardConfig {
    DashboardConfig {
        sections: vec![/* ... */],
        groups: vec![
            GroupDefinition {
                name: "Utilization".to_string(),
                id: "utilization".to_string(),
                plots: vec![
                    PlotDefinition {
                        title: "Busy %".to_string(),
                        id: "cpu-busy".to_string(),
                        plot_type: "line".to_string(),
                        metric: Some("cpu_usage".to_string()),
                        labels: None,
                        unit: Unit::Percentage,
                    },
                    PlotDefinition {
                        title: "Busy %".to_string(),
                        id: "cpu-busy-heatmap".to_string(),
                        plot_type: "heatmap".to_string(),
                        metric: Some("cpu_usage".to_string()),
                        labels: None,
                        unit: Unit::Percentage,
                    },
                    // ... more plots
                ],
            },
            // ... more groups
        ],
    }
}

fn network_dashboard_config() -> DashboardConfig {
    // Similar structure for network dashboard
    todo!()
}

fn blockio_dashboard_config() -> DashboardConfig {
    todo!()
}

fn scheduler_dashboard_config() -> DashboardConfig {
    todo!()
}

fn syscall_dashboard_config() -> DashboardConfig {
    todo!()
}

fn softirq_dashboard_config() -> DashboardConfig {
    todo!()
}

fn rezolus_dashboard_config() -> DashboardConfig {
    todo!()
}

fn cgroups_dashboard_config() -> DashboardConfig {
    todo!()
}

fn overview_dashboard_config() -> DashboardConfig {
    todo!()
}

fn build_cpu_plot(tsdb: &Tsdb, plot_id: &str, time_range: &TimeRangeQuery) -> Option<Plot> {
    // Build specific CPU plots based on plot_id
    match plot_id {
        "cpu-busy" => {
            let data = tsdb.cpu_avg("cpu_usage", ())
                .map(|v| v / NANOSECONDS_PER_SECOND);
            Some(Plot::from_series("Busy %", "cpu-busy", Unit::Percentage, data))
        }
        "cpu-busy-heatmap" => {
            let data = tsdb.cpu_heatmap("cpu_usage", ())
                .map(|v| v / NANOSECONDS_PER_SECOND);
            Some(Plot::from_heatmap("Busy %", "cpu-busy-heatmap", Unit::Percentage, data))
        }
        // ... other CPU plots
        _ => None,
    }
}

fn build_network_plot(tsdb: &Tsdb, plot_id: &str, time_range: &TimeRangeQuery) -> Option<Plot> {
    // Build specific network plots based on plot_id
    todo!()
}

#[derive(Debug, Serialize)]
pub struct PlotData {
    pub data: Vec<Vec<f64>>,
    pub timestamps: Vec<i64>,
}

impl From<Option<UntypedSeries>> for PlotData {
    fn from(series: Option<UntypedSeries>) -> Self {
        // Convert series to plot data format
        todo!()
    }
}

impl Plot {
    fn empty() -> Self {
        Plot {
            opts: PlotOpts::line("Empty", "empty", Unit::Count),
            data: vec![],
            min_value: None,
            max_value: None,
            time_data: None,
            formatted_time_data: None,
            series_names: None,
        }
    }
    
    fn from_series(title: &str, id: &str, unit: Unit, data: Option<UntypedSeries>) -> Self {
        if let Some(series) = data {
            Plot {
                opts: PlotOpts::line(title, id, unit),
                data: series.as_data(),
                min_value: None,
                max_value: None,
                time_data: None,
                formatted_time_data: None,
                series_names: None,
            }
        } else {
            Self::empty()
        }
    }
    
    fn from_heatmap(title: &str, id: &str, unit: Unit, data: Option<Heatmap>) -> Self {
        if let Some(heatmap) = data {
            let echarts_data = heatmap.as_data();
            Plot {
                opts: PlotOpts::heatmap(title, id, unit),
                data: echarts_data.data,
                min_value: Some(echarts_data.min_value),
                max_value: Some(echarts_data.max_value),
                time_data: Some(echarts_data.time),
                formatted_time_data: Some(echarts_data.formatted_time),
                series_names: None,
            }
        } else {
            Self::empty()
        }
    }
}

/// New app state that holds the TSDB for querying
pub struct AppStateV2 {
    pub tsdb: Tsdb,
}

impl AppStateV2 {
    pub fn new(tsdb: Tsdb) -> Self {
        Self { tsdb }
    }
}