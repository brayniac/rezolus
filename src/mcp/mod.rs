use clap::{ArgMatches, Command};

mod analysis;
mod anomaly;
mod cli;
mod correlation;
mod discovery_queries;
mod fft_analysis;
mod guided_analysis;
mod metric_helper;
mod scenarios;
mod server;

/// Run the MCP server
pub fn run(config: Config) {
    // Check if we have a subcommand
    if let Some((cmd, args)) = &config.subcommand {
        if let Err(e) = cli::handle_command(cmd, args) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Otherwise run the MCP server
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    runtime.block_on(async {
        let mut server = server::MCPServer::new(config);
        if let Err(e) = server.run_stdio().await {
            eprintln!("MCP server error: {}", e);
            std::process::exit(1);
        }
    });
}

/// MCP server configuration
pub struct Config {
    pub verbose: u8,
    pub subcommand: Option<(String, ArgMatches)>,
}

impl TryFrom<ArgMatches> for Config {
    type Error = String;

    fn try_from(args: ArgMatches) -> Result<Self, String> {
        Ok(Config {
            verbose: *args.get_one::<u8>("VERBOSE").unwrap_or(&0),
            subcommand: args
                .subcommand()
                .map(|(name, args)| (name.to_string(), args.clone())),
        })
    }
}

/// Create the MCP subcommand
pub fn command() -> Command {
    Command::new("mcp")
        .about("Run Rezolus MCP server for AI analysis")
        .arg(
            clap::Arg::new("VERBOSE")
                .long("verbose")
                .short('v')
                .help("Increase verbosity")
                .action(clap::ArgAction::Count),
        )
        .subcommand(
            Command::new("discover")
                .about("Discover correlations in metrics")
                .arg(
                    clap::Arg::new("FILE")
                        .help("Path to parquet file")
                        .required(true)
                        .value_name("FILE"),
                )
                .arg(
                    clap::Arg::new("min-correlation")
                        .long("min-correlation")
                        .short('m')
                        .help("Minimum correlation to report (0.0-1.0)")
                        .default_value("0.5")
                        .value_parser(clap::value_parser!(f64)),
                )
                .arg(
                    clap::Arg::new("isolate-cgroup")
                        .long("isolate-cgroup")
                        .help("Isolate analysis to specific cgroup")
                        .value_name("CGROUP"),
                ),
        )
        .subcommand(
            Command::new("anomaly")
                .about("Detect anomalies in a metric")
                .arg(
                    clap::Arg::new("FILE")
                        .help("Path to parquet file")
                        .required(true)
                        .value_name("FILE"),
                )
                .arg(
                    clap::Arg::new("METRIC")
                        .help("Metric name to analyze")
                        .required(true)
                        .value_name("METRIC"),
                )
                .arg(
                    clap::Arg::new("sensitivity")
                        .long("sensitivity")
                        .short('s')
                        .help("Anomaly detection sensitivity (1.0-5.0)")
                        .default_value("2.0")
                        .value_parser(clap::value_parser!(f64)),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List cgroups and metrics in a file")
                .arg(
                    clap::Arg::new("FILE")
                        .help("Path to parquet file")
                        .required(true)
                        .value_name("FILE"),
                ),
        )
        .subcommand(
            Command::new("correlation")
                .about("Analyze correlation between two metrics")
                .arg(
                    clap::Arg::new("FILE")
                        .help("Path to parquet file")
                        .required(true)
                        .value_name("FILE"),
                )
                .arg(
                    clap::Arg::new("METRIC1")
                        .help("First metric")
                        .required(true)
                        .value_name("METRIC1"),
                )
                .arg(
                    clap::Arg::new("METRIC2")
                        .help("Second metric")
                        .required(true)
                        .value_name("METRIC2"),
                ),
        )
        .subcommand(
            Command::new("trend")
                .about("Analyze long-term trends in a metric")
                .arg(
                    clap::Arg::new("FILE")
                        .help("Path to parquet file")
                        .required(true)
                        .value_name("FILE"),
                )
                .arg(
                    clap::Arg::new("METRIC")
                        .help("Metric name to analyze")
                        .required(true)
                        .value_name("METRIC"),
                )
                .arg(
                    clap::Arg::new("window-hours")
                        .long("window-hours")
                        .short('w')
                        .help("Analysis window in hours (1-168)")
                        .default_value("24.0")
                        .value_parser(clap::value_parser!(f64)),
                ),
        )
        .subcommand(
            Command::new("fft")
                .about("Analyze periodic patterns using FFT")
                .arg(
                    clap::Arg::new("FILE")
                        .help("Path to parquet file")
                        .required(true)
                        .value_name("FILE"),
                )
                .arg(
                    clap::Arg::new("METRIC")
                        .help("Metric name or PromQL query to analyze (supports labels, e.g., 'cpu_usage{cpu=\"0\"}')")
                        .required(true)
                        .value_name("METRIC"),
                )
                .arg(
                    clap::Arg::new("step")
                        .long("step")
                        .short('s')
                        .help("Step size in seconds (default: 60)")
                        .default_value("60.0")
                        .value_parser(clap::value_parser!(f64)),
                ),
        )
        .subcommand(
            Command::new("diagnose")
                .about("Run targeted diagnostic analysis")
                .arg(
                    clap::Arg::new("FILE")
                        .help("Path to parquet file")
                        .required(true)
                        .value_name("FILE"),
                )
                .arg(
                    clap::Arg::new("SCENARIO")
                        .help("Diagnostic scenario to run")
                        .required(true)
                        .value_name("SCENARIO")
                        .value_parser(["cpu", "memory", "network", "latency", "cgroup", "all"]),
                )
                .arg(
                    clap::Arg::new("cgroup-name")
                        .long("cgroup")
                        .short('c')
                        .help("Cgroup name for cgroup analysis")
                        .value_name("NAME"),
                ),
        )
}
