use axum::{extract::State, http::StatusCode, response::Html, routing::get, Router};
use clap::ArgMatches;
use ringlog::{Level, LogBuilder, MultiLogBuilder, Output, Stderr};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tera::{Context, Tera};
use tower_http::services::ServeDir;

// Define constants for application state
static RUNNING: usize = 0;
static STATE: AtomicUsize = AtomicUsize::new(RUNNING);

pub struct Config {
    verbose: u8,
}

impl TryFrom<ArgMatches> for Config {
    type Error = String;

    fn try_from(args: ArgMatches) -> Result<Self, Self::Error> {
        Ok(Config {
            verbose: *args.get_one::<u8>("VERBOSE").unwrap_or(&0),
        })
    }
}

pub fn command() -> clap::Command {
    clap::Command::new("view")
        .about("View a Rezolus recording")
        .arg(
            clap::Arg::new("VERBOSE")
                .long("verbose")
                .short('v')
                .help("Increase the verbosity")
                .action(clap::ArgAction::Count),
        )
}

// Helper function to create sine wave data with offset and amplitude
fn create_sine_wave(timestamps: &[f64], freq: f64, amp: f64, phase: f64, offset: f64) -> Vec<f64> {
    timestamps
        .iter()
        .map(|&t| amp * ((t * freq) + phase).sin() + offset)
        .collect()
}

fn setup_files() -> std::io::Result<()> {
    // Create directory structure if it doesn't exist
    let template_dir = Path::new("src/viewer/assets/templates");
    let static_dir = Path::new("src/viewer/assets/static");

    fs::create_dir_all(template_dir)?;
    fs::create_dir_all(static_dir)?;

    // Write the dashboard.html template (assuming we store the content in assets/templates/dashboard.html during build)
    let dashboard_html = include_str!("./assets/templates/dashboard.html");
    fs::write(template_dir.join("dashboard.html"), dashboard_html)?;

    // Write the CSS file (assuming we store the content in assets/static/styles.css during build)
    let styles_css = include_str!("./assets/static/styles.css");
    fs::write(static_dir.join("styles.css"), styles_css)?;

    // Write the JS file (assuming we store the content in assets/static/zoom-controller.js during build)
    let zoom_controller_js = include_str!("./assets/static/zoom-controller.js");
    fs::write(static_dir.join("zoom-controller.js"), zoom_controller_js)?;

    println!("Template and static files created successfully.");
    Ok(())
}

// Add these new struct definitions in the mod.rs file
// alongside the existing struct definitions

// Enum to represent different chart types
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum ChartType {
    Line,
    Scatter,
    Heatmap,
}

// Series info is the same for line and scatter charts
#[derive(Serialize)]
struct SeriesInfo {
    name: String,
    values: Vec<f64>,
    color: String,
    id: Option<String>, // For special identification, e.g., CPU core
}

// Base structure for all chart types
#[derive(Serialize)]
struct TimeSeriesData {
    chart_type: ChartType,
    timestamps: Vec<f64>,
    title: String,
    y_units: String,
    series: Vec<SeriesInfo>,
}

// Keep the existing MetricGroup struct but use the new TimeSeriesData type
#[derive(Serialize)]
struct MetricGroup {
    name: String,
    description: String,
    series: Vec<TimeSeriesData>,
}

// Helper function to create a line chart configuration
fn create_line_chart(
    timestamps: &[f64],
    title: &str,
    units: &str,
    series_data: Vec<(&str, f64, f64, f64, &str)>,
) -> TimeSeriesData {
    let mut series_vec = Vec::new();

    // Create each series
    for (name, freq, amp, phase, color) in series_data {
        let values = timestamps
            .iter()
            .map(|&t| amp * ((t * freq) + phase).sin())
            .collect();

        series_vec.push(SeriesInfo {
            name: name.to_string(),
            values,
            color: color.to_string(),
            id: None,
        });
    }

    TimeSeriesData {
        chart_type: ChartType::Line,
        timestamps: timestamps.to_owned(),
        title: title.to_string(),
        y_units: units.to_string(),
        series: series_vec,
    }
}

// Helper function to create a line chart with offset
fn create_line_chart_with_offset(
    timestamps: &[f64],
    title: &str,
    units: &str,
    series_data: Vec<(&str, f64, f64, f64, f64, &str)>,
) -> TimeSeriesData {
    let mut series_vec = Vec::new();

    // Create each series with offset
    // Parameters: (name, freq, amp, phase, offset, color)
    for (name, freq, amp, phase, offset, color) in series_data {
        let values = timestamps
            .iter()
            .map(|&t| amp * ((t * freq) + phase).sin() + offset)
            .collect();

        series_vec.push(SeriesInfo {
            name: name.to_string(),
            values,
            color: color.to_string(),
            id: None,
        });
    }

    TimeSeriesData {
        chart_type: ChartType::Line,
        timestamps: timestamps.to_owned(),
        title: title.to_string(),
        y_units: units.to_string(),
        series: series_vec,
    }
}

// Helper function to create a scatter chart
fn create_scatter_chart(
    timestamps: &[f64],
    title: &str,
    units: &str,
    series_data: Vec<(&str, f64, f64, f64, f64, &str)>,
) -> TimeSeriesData {
    let mut series_vec = Vec::new();

    // Create each series with noise for scatter effect
    // Parameters: (name, freq, amp, phase, noise_factor, color)
    for (name, freq, amp, phase, noise_factor, color) in series_data {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let values = timestamps
            .iter()
            .map(|&t| {
                let base = amp * ((t * freq) + phase).sin();
                let noise = rng.gen_range(-noise_factor..noise_factor);
                base + noise
            })
            .collect();

        series_vec.push(SeriesInfo {
            name: name.to_string(),
            values,
            color: color.to_string(),
            id: None,
        });
    }

    TimeSeriesData {
        chart_type: ChartType::Scatter,
        timestamps: timestamps.to_owned(),
        title: title.to_string(),
        y_units: units.to_string(),
        series: series_vec,
    }
}

// Helper function to create a heatmap chart for per-CPU data
fn create_cpu_heatmap(
    timestamps: &[f64],
    title: &str,
    units: &str,
    num_cpus: usize,
) -> TimeSeriesData {
    let mut series_vec = Vec::new();

    // Create data for each CPU
    for cpu_idx in 0..num_cpus {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Generate different patterns for each CPU
        let freq = 0.05 + (cpu_idx as f64 * 0.01); // Slightly different frequency per CPU
        let phase = (cpu_idx as f64) * 0.5; // Different phase per CPU
        let amplitude = 25.0 + (cpu_idx as f64 * 2.0); // Different amplitude per CPU
        let offset = 20.0 + (cpu_idx as f64 * 3.0); // Different offset per CPU

        // Create the CPU's utilization pattern
        let values = timestamps
            .iter()
            .map(|&t| {
                // Base sine wave for periodic behavior
                let base = offset + amplitude * ((t * freq) + phase).sin();

                // Add some randomness for more realistic data
                let noise = rng.gen::<f64>() * 15.0;

                // Add occasional spikes for some CPUs
                let spike = if rng.gen::<f64>() < 0.01 && cpu_idx % 2 == 0 {
                    30.0
                } else {
                    0.0
                };

                // Combine and clamp to 0-100 range
                (base + noise + spike).max(0.0).min(100.0)
            })
            .collect();

        // Each CPU gets its own series with a unique ID
        series_vec.push(SeriesInfo {
            name: format!("CPU {}", cpu_idx),
            values,
            color: "#4EC9B0".to_string(), // Default color (will be overridden by heatmap colorscale)
            id: Some(format!("cpu{}", cpu_idx)),
        });
    }

    TimeSeriesData {
        chart_type: ChartType::Heatmap,
        timestamps: timestamps.to_owned(),
        title: title.to_string(),
        y_units: units.to_string(),
        series: series_vec,
    }
}

fn generate_example_metric_groups() -> Vec<MetricGroup> {
    // Base timestamps for all series
    let timestamps: Vec<f64> = (0..3600).map(|i| (i as f64) * 1.0).collect();

    // Define our groups and their metrics
    let groups = vec![
        MetricGroup {
            name: "CPU".to_string(),
            description: "CPU utilization metrics".to_string(),
            series: vec![
                create_line_chart_with_offset(
                    &timestamps,
                    "CPU Utilization",
                    "%",
                    vec![
                        ("System", 0.1, 10.0, 0.0, 30.0, "#569CD6"),
                        ("User", 0.1, 15.0, 1.0, 20.0, "#4EC9B0"),
                        ("IO Wait", 0.2, 5.0, 0.5, 5.0, "#CE9178"),
                    ],
                ),
                create_cpu_heatmap(
                    &timestamps,
                    "Per-CPU Utilization",
                    "%",
                    8, // 8 CPU cores
                ),
                // Removed CPU Load Average chart
            ],
        },
        // Other metric groups remain the same
        MetricGroup {
            name: "Memory".to_string(),
            description: "Memory usage metrics".to_string(),
            series: vec![
                create_line_chart_with_offset(
                    &timestamps,
                    "Memory Usage",
                    "GB",
                    vec![
                        ("Used", 0.01, 1.0, 0.0, 8.0, "#CE9178"),
                        ("Cached", 0.02, 0.5, 1.0, 4.0, "#DCDCAA"),
                        ("Free", 0.015, 0.8, 0.5, 4.0, "#569CD6"),
                    ],
                ),
                create_line_chart_with_offset(
                    &timestamps,
                    "Swap Usage",
                    "MB",
                    vec![("Used", 0.08, 100.0, 0.5, 250.0, "#DCDCAA")],
                ),
            ],
        },
        MetricGroup {
            name: "Network".to_string(),
            description: "Network throughput metrics".to_string(),
            series: vec![
                create_line_chart_with_offset(
                    &timestamps,
                    "Network Throughput",
                    "Mbps",
                    vec![
                        ("Ingress", 0.05, 200.0, 0.0, 500.0, "#9CDCFE"),
                        ("Egress", 0.05, 150.0, 1.0, 300.0, "#B5CEA8"),
                    ],
                ),
                create_scatter_chart(
                    &timestamps,
                    "Network Latency",
                    "ms",
                    vec![
                        ("p50", 0.1, 2.0, 0.0, 1.0, "#9CDCFE"),
                        ("p90", 0.1, 4.0, 0.5, 2.0, "#CE9178"),
                        ("p99", 0.1, 8.0, 1.0, 5.0, "#CC6666"),
                    ],
                ),
            ],
        },
        MetricGroup {
            name: "Disk".to_string(),
            description: "Disk performance metrics".to_string(),
            series: vec![
                create_line_chart_with_offset(
                    &timestamps,
                    "Disk I/O",
                    "IOPS",
                    vec![
                        ("Read", 0.15, 500.0, 3.0, 1500.0, "#CC6666"),
                        ("Write", 0.1, 300.0, 0.0, 800.0, "#C586C0"),
                    ],
                ),
                create_scatter_chart(
                    &timestamps,
                    "Disk Latency",
                    "ms",
                    vec![
                        ("Read", 0.1, 0.5, 0.0, 0.5, "#CC6666"),
                        ("Write", 0.15, 0.8, 2.5, 1.0, "#C586C0"),
                    ],
                ),
            ],
        },
    ];

    groups
}

async fn index(State(templates): State<Arc<Tera>>) -> Html<String> {
    let mut context = Context::new();

    // Generate example data with groups
    let groups = generate_example_metric_groups();
    context.insert("metric_groups", &groups);

    let rendered = templates
        .render("dashboard.html", &context)
        .expect("Failed to render template");

    Html(rendered)
}

pub fn run(config: Config) {
    // Configure debug log
    let debug_output: Box<dyn Output> = Box::new(Stderr::new());

    let level = match config.verbose {
        0 => Level::Info,
        1 => Level::Debug,
        _ => Level::Trace,
    };

    let debug_log = if level <= Level::Info {
        LogBuilder::new().format(ringlog::default_format)
    } else {
        LogBuilder::new()
    }
    .output(debug_output)
    .build()
    .expect("failed to initialize debug log");

    let mut log = MultiLogBuilder::new()
        .level_filter(level.to_level_filter())
        .default(debug_log)
        .build()
        .start();

    // Initialize async runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .thread_name("rezolus")
        .build()
        .expect("failed to launch async runtime");

    // Spawn logging thread
    rt.spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = log.flush();
        }
    });

    // Handle Ctrl-C
    ctrlc::set_handler(move || {
        let state = STATE.load(Ordering::SeqCst);

        if state == RUNNING {
            println!("Server shutting down...");
            std::process::exit(0);
        }
    })
    .expect("Failed to set Ctrl-C handler");

    // Run the server
    rt.block_on(async move {
        // Ensure template and static files exist (create if not)
        if let Err(e) = setup_files() {
            eprintln!("Error setting up files: {}", e);
            return;
        }

        // Create Tera template engine
        let templates = match Tera::new("src/viewer/assets/templates/**/*") {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error parsing templates: {}", e);
                return;
            }
        };

        let shared_templates = Arc::new(templates);

        // Create router with endpoints
        let app = Router::new()
            .route("/", get(index))
            // Serve static files
            .nest_service("/static", ServeDir::new("src/viewer/assets/static"))
            .with_state(shared_templates);

        // Start the server
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
        println!("Listening on: http://localhost:8080");
        println!("Press Ctrl+C to stop the server");

        axum::serve(listener, app).await.unwrap();
    });
}
