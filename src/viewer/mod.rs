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

#[derive(Serialize)]
struct TimeSeriesData {
    timestamps: Vec<f64>,
    values: Vec<f64>,
    title: String,
    y_units: String,
    color: String,
    y_min: Option<f64>,
    y_max: Option<f64>,
    group: String, // Added group field
}

#[derive(Serialize)]
struct MetricGroup {
    name: String,
    description: String,
    series: Vec<TimeSeriesData>,
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

fn generate_example_metric_groups() -> Vec<MetricGroup> {
    // Base timestamps for all series
    let timestamps: Vec<f64> = (0..3600).map(|i| (i as f64) * 1.0).collect();

    // Define our groups and their metrics
    let groups = vec![
        MetricGroup {
            name: "CPU".to_string(),
            description: "CPU utilization metrics".to_string(),
            series: vec![
                create_time_series(
                    &timestamps,
                    0.1,
                    1.0,
                    0.0,
                    "CPU Utilization",
                    "%",
                    "#569CD6",
                    "CPU",
                ),
                create_time_series(
                    &timestamps,
                    0.2,
                    0.8,
                    0.3,
                    "CPU Load Average",
                    "load",
                    "#4EC9B0",
                    "CPU",
                ),
            ],
        },
        MetricGroup {
            name: "Memory".to_string(),
            description: "Memory usage metrics".to_string(),
            series: vec![
                create_time_series(
                    &timestamps,
                    0.05,
                    2.0,
                    1.0,
                    "Memory Usage",
                    "MB",
                    "#CE9178",
                    "Memory",
                ),
                create_time_series(
                    &timestamps,
                    0.08,
                    1.2,
                    0.5,
                    "Swap Usage",
                    "MB",
                    "#DCDCAA",
                    "Memory",
                ),
            ],
        },
        MetricGroup {
            name: "Network".to_string(),
            description: "Network throughput metrics".to_string(),
            series: vec![
                create_time_series(
                    &timestamps,
                    0.2,
                    0.5,
                    2.0,
                    "Network Throughput",
                    "Mbps",
                    "#9CDCFE",
                    "Network",
                ),
                create_time_series(
                    &timestamps,
                    0.15,
                    0.7,
                    1.5,
                    "Packet Rate",
                    "pps",
                    "#B5CEA8",
                    "Network",
                ),
            ],
        },
        MetricGroup {
            name: "Disk".to_string(),
            description: "Disk performance metrics".to_string(),
            series: vec![
                create_time_series(
                    &timestamps,
                    0.15,
                    1.5,
                    3.0,
                    "Disk I/O",
                    "IOPS",
                    "#CC6666",
                    "Disk",
                ),
                create_time_series(
                    &timestamps,
                    0.1,
                    1.3,
                    2.5,
                    "Disk Latency",
                    "ms",
                    "#C586C0",
                    "Disk",
                ),
            ],
        },
    ];

    groups
}

fn create_time_series(
    timestamps: &Vec<f64>,
    freq: f64,
    amp: f64,
    phase: f64,
    title: &str,
    units: &str,
    color: &str,
    group: &str,
) -> TimeSeriesData {
    let values: Vec<f64> = timestamps
        .iter()
        .map(|&t| amp * ((t * freq) + phase).sin())
        .collect();

    TimeSeriesData {
        timestamps: timestamps.clone(),
        values,
        title: title.to_string(),
        y_units: units.to_string(),
        color: color.to_string(),
        y_min: None,
        y_max: None,
        group: group.to_string(),
    }
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
