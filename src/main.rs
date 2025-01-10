use std::io::Read;
use std::io::SeekFrom;
use reqwest::blocking::Client;
use std::io::Seek;
use std::io::Write;
use metriken_exposition::MsgpackToParquet;
use metriken_exposition::ParquetOptions;
use chrono::Timelike;
use chrono::Utc;
use std::time::Instant;
use std::path::PathBuf;
use tempfile::tempfile_in;
use reqwest::Url;
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use clap::ValueEnum;
use clap::Args;
use clap::Parser;
use async_trait::async_trait;
use backtrace::Backtrace;
use linkme::distributed_slice;
use ringlog::*;
use clap::Subcommand;

use std::sync::Arc;

mod common;
mod config;
mod exposition;
mod samplers;

use config::Config;
use samplers::{Sampler, SamplerResult};

#[distributed_slice]
pub static SAMPLERS: [fn(config: Arc<Config>) -> SamplerResult] = [..];

static STATE: AtomicUsize = AtomicUsize::new(RUNNING);

static RUNNING: usize = 0;
static CAPTURING: usize = 1;
static TERMINATING: usize = 2;

#[derive(Parser)]
#[command(subcommand_negates_reqs = true)]
#[command(args_conflicts_with_subcommands = true)]
#[command(version)]
#[command(about = "High-resolution systems performance telemetry.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(value_name = "CONFIG")]
    config: String
}

#[derive(Subcommand)]
#[command(subcommand_negates_reqs = true)]
#[command(args_conflicts_with_subcommands = true)]
enum Command {
    /// Run the Rezolus agent to gather and expose metrics. (Default)
    Agent(AgentArgs),
    /// Run a flight recorder pulls data from a Rezolus agent into a local
    /// on-disk ring buffer.
    FlightRecorder(FlightRecorderArgs),
    /// Run ad-hoc collection of data from a Rezolus agent to disk.
    Record(RecordArgs),
}

#[derive(Debug, Args)]
struct AgentArgs {
    #[arg(value_name = "CONFIG")]
    config: String
}

#[derive(Debug, Args)]
struct FlightRecorderArgs {
    #[arg(short, long, default_value_t = humantime::Duration::from(Duration::from_secs(1)))]
    interval: humantime::Duration,
    #[arg(short, long, default_value_t = humantime::Duration::from(Duration::from_secs(900)))]
    duration: humantime::Duration,
    #[arg(short, long, value_enum, default_value_t = Format::Parquet)]
    format: Format,
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    #[arg(value_name = "SOURCE")]
    source: String,
    #[arg(value_name = "DESTINATION")]
    destination: String,
}

#[derive(Debug, Args)]
struct RecordArgs {
    #[arg(short, long, default_value_t = humantime::Duration::from(Duration::from_secs(1)))]
    interval: humantime::Duration,
    #[arg(short, long)]
    duration: Option<humantime::Duration>,
    #[arg(short, long, value_enum, default_value_t = Format::Parquet)]
    format: Format,
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    #[arg(value_name = "SOURCE")]
    source: String,
    #[arg(value_name = "DESTINATION")]
    destination: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Format {
    Parquet,
    Raw,
}

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        eprintln!("{s}");
        eprintln!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // read cli options
    let cli = Cli::parse();

    match cli.command {
        // the default is to run as the telemetry agent
        None => {
            agent(AgentArgs { config: cli.config })
        }
        Some(Command::Agent(a)) => {
            agent(a)
        }
        Some(Command::FlightRecorder(a)) => {
            flight_recorder(a)
        }
        Some(Command::Record(a)) => {
            record(a)
        }
    }
}

fn agent(args: AgentArgs) {
    // load config from file
    let config: Arc<Config> = {
        let file = args.config;
        debug!("loading config: {}", file);
        match Config::load(&file) {
            Ok(c) => c.into(),
            Err(error) => {
                eprintln!("error loading config file: {file}\n{error}");
                std::process::exit(1);
            }
        }
    };

    // configure debug log
    let debug_output: Box<dyn Output> = Box::new(Stderr::new());

    let level = config.log().level();

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

    // initialize async runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .thread_name("rezolus")
        .build()
        .expect("failed to launch async runtime");

    // spawn logging thread
    rt.spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = log.flush();
        }
    });

    let mut samplers = Vec::new();

    for init in SAMPLERS {
        if let Ok(Some(s)) = init(config.clone()) {
            samplers.push(s);
        }
    }

    let samplers = Arc::new(samplers.into_boxed_slice());

    rt.spawn(async move {
        exposition::http::serve(config, samplers).await;
    });

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn flight_recorder(args: FlightRecorderArgs) {
    // configure debug log
    let debug_output: Box<dyn Output> = Box::new(Stderr::new());

    let level = match args.verbose {
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

    // initialize async runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .thread_name("rezolus")
        .build()
        .expect("failed to launch async runtime");

    // spawn logging thread
    rt.spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = log.flush();
        }
    });

    ctrlc::set_handler(move || {
        let state = STATE.load(Ordering::SeqCst);

        if state == RUNNING {
            info!("triggering ringbuffer capture");
            STATE.store(CAPTURING, Ordering::SeqCst);
        } else if state == CAPTURING {
            info!("waiting for capture to complete before exiting");
            STATE.store(TERMINATING, Ordering::SeqCst);
        } else {
            info!("terminating immediately");
            std::process::exit(2);
        }
    })
    .expect("failed to set ctrl-c handler");

    // parse source address
    let mut url: Url = {
        let source = args.source.clone();

        let source = if source.starts_with("http://") || source.starts_with("https://") {
            source.to_string()
        } else {
            format!("http://{source}")
        };

        match source.parse::<Url>() {
            Ok(c) => c,
            Err(error) => {
                eprintln!("source is not a valid URL: {source}\n{error}");
                std::process::exit(1);
            }
        }
    };

    if url.path() != "/" {
        eprintln!("URL should not have an non-root path: {url}");
        std::process::exit(1);
    }

    url.set_path("/metrics/binary");

    // our http client
    let client = match Client::builder().http1_only().build() {
        Ok(c) => c,
        Err(e) => {
            error!("error connecting to Rezolus: {e}");
            std::process::exit(1);
        }
    };

    // open our destination file to make sure we can
    let _ = std::fs::File::create(args.destination.clone()).map_err(|e| {
        error!("failed to open destination file: {e}");
        std::process::exit(1);
    }).unwrap();

    // our writer will always be a temporary file
    let mut writer = {
        let mut path: PathBuf = args.destination.clone().parse().map_err(|e| {
            eprintln!("destination is not a valid path: {e}");
            std::process::exit(1);
        }).unwrap();
        path.pop();

        match tempfile_in(path.clone()) {
            Ok(t) => t,
            Err(error) => {
                eprintln!("could not open temporary file in: {:?}\n{error}", path);
                std::process::exit(1);
            }
        }
    };

    // estimate the snapshot size and latency
    let start = Instant::now();

    let (snap_len, latency) = if let Ok(response) = client.get(url.clone()).send() {
        if let Ok(body) = response.bytes() {
            let latency = start.elapsed();

            debug!("sampling latency: {} us", latency.as_micros());
            debug!("body size: {}", body.len());

            (body.len(), latency)
        } else {
            error!("error reading metrics endpoint");
            std::process::exit(1);
        }
    } else {
        error!("error reading metrics endpoint");
        std::process::exit(1);
    };

    // check that the sampling interval and sample latency are compatible
    if args.interval.as_micros() < (latency.as_micros() * 2) {
        error!("the sampling interval is too short to reliably record");
        error!(
            "set the interval to at least: {} us",
            latency.as_micros() * 2
        );
        std::process::exit(1);
    }

    // the snapshot len in blocks
    // note: we allow for more capacity than we need and round to the next
    // nearest whole number of blocks
    let snapshot_len = (1 + snap_len as u64 * 4 / 4096) * 4096;

    // the total number of snapshots
    let snapshot_count = (1 + args.duration.as_micros() / args.interval.as_micros()) as u64;

    // expand the temporary file to hold enough room for all the snapshots
    let _ = writer
        .set_len(snapshot_len * snapshot_count)
        .map_err(|e| {
            error!("failed to grow temporary file: {e}");
            std::process::exit(1);
        });

    let mut idx = 0;


    rt.block_on(async move {
        // get an aligned start time
        let start = tokio::time::Instant::now() - Duration::from_nanos(Utc::now().nanosecond() as u64)
            + args.interval.into();

        // sampling interval
        let mut interval = tokio::time::interval_at(start, args.interval.into());
        loop {
            let mut destination = std::fs::File::create(args.destination.clone()).map_err(|e| {
                error!("failed to open destination file: {e}");
                std::process::exit(1);
            }).unwrap();

            // sample in a loop until RUNNING is false or duration has completed
            while STATE.load(Ordering::Relaxed) == RUNNING {
                // wait to sample
                interval.tick().await;

                let start = Instant::now();

                // sample rezolus
                if let Ok(response) = client.get(url.clone()).send() {
                    if let Ok(body) = response.bytes() {
                        let latency = start.elapsed();

                        debug!("sampling latency: {} us", latency.as_micros());

                        debug!("body size: {}", body.len());

                        // seek to position in snapshot
                        writer
                            .seek(SeekFrom::Start(idx * snapshot_len))
                            .expect("failed to seek");

                        // write the size of the snapshot
                        writer
                            .write_all(&body.len().to_be_bytes())
                            .expect("failed to write snapshot size");

                        // write the actual snapshot content
                        writer
                            .write_all(&body)
                            .expect("failed to write snapshot");
                    } else {
                        error!("failed to read response");
                        std::process::exit(1);
                    }
                } else {
                    error!("failed to get metrics");
                    std::process::exit(1);
                }

                idx += 1;

                if idx >= snapshot_count {
                    idx = 0;
                }
            }


            debug!("flushing writer");
            let _ = writer.flush();

            // handle any output format specific transforms
            match args.format {
                Format::Raw => {
                    debug!("capturing ringbuffer and writing to raw");

                    for offset in 1..=snapshot_count {
                        // we start at the last recorded index + 1 to get the oldest
                        // record first
                        let mut i = idx + offset;

                        // handle wrap-around in the ring-buffer
                        if i > snapshot_len {
                            i -= snapshot_len;
                        }

                        // seek to the start of the snapshot slot
                        writer
                            .seek(SeekFrom::Start(i * snapshot_len))
                            .expect("failed to seek");

                        // read the size of the snapshot
                        let mut len = [0, 0, 0, 0, 0, 0, 0, 0];
                        writer
                            .read_exact(&mut len)
                            .expect("failed to read snapshot len");

                        // read the contents of the snapshot
                        let mut buf = vec![0; u64::from_be_bytes(len) as usize];
                        writer
                            .read_exact(&mut buf)
                            .expect("failed to read snapshot");

                        // write the contents of the snapshot to the packed file
                        destination
                            .write_all(&buf)
                            .expect("failed to write to packed file");
                    }

                    let _ = destination.flush();

                    debug!("finished");
                }
                Format::Parquet => {
                    debug!("capturing ringbuffer and writing to parquet");

                    let _ = writer.rewind();

                    // we need another temporary file to consume the empty space
                    // between snapshots

                    // TODO(bmartin): we can probably remove this by using our
                    // own msgpack -> parquet conversion

                    // our writer will always be a temporary file
                    let mut packed = {
                        let mut path: PathBuf = args.destination.clone().parse().map_err(|e| {
                            eprintln!("destination is not a valid path: {e}");
                            std::process::exit(1);
                        }).unwrap();
                        path.pop();

                        match tempfile_in(path.clone()) {
                            Ok(t) => t,
                            Err(error) => {
                                eprintln!("could not open temporary file in: {:?}\n{error}", path);
                                std::process::exit(1);
                            }
                        }
                    };

                    for offset in 1..=snapshot_count {
                        // we start at the last recorded index + 1 to get the oldest
                        // record first
                        let mut i = idx + offset;

                        // handle wrap-around in the ring-buffer
                        if i >= snapshot_count {
                            i -= snapshot_count;
                        }

                        // seek to the start of the snapshot slot
                        writer
                            .seek(SeekFrom::Start(i * snapshot_len))
                            .expect("failed to seek");

                        // read the size of the snapshot
                        let mut len = [0, 0, 0, 0, 0, 0, 0, 0];
                        writer
                            .read_exact(&mut len)
                            .expect("failed to read snapshot len");

                        // read the contents of the snapshot
                        let mut buf = vec![0; u64::from_be_bytes(len) as usize];
                        writer
                            .read_exact(&mut buf)
                            .expect("failed to read snapshot");

                        // write the contents of the snapshot to the packed file
                        packed
                            .write_all(&buf)
                            .expect("failed to write to packed file");
                    }

                    let _ = packed.flush();
                    let _ = packed.rewind();

                    if let Err(e) = MsgpackToParquet::with_options(ParquetOptions::new())
                        .convert_file_handle(packed, destination)
                    {
                        eprintln!("error saving parquet file: {e}");
                    }
                }
            }

            debug!("ringbuffer capture complete");

            if STATE.load(Ordering::SeqCst) == TERMINATING {
                return;
            } else {
                STATE.store(RUNNING, Ordering::SeqCst);
            }
        }
    });
}

fn record(args: RecordArgs) {
    // configure debug log
    let debug_output: Box<dyn Output> = Box::new(Stderr::new());

    let level = match args.verbose {
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

    // initialize async runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .thread_name("rezolus")
        .build()
        .expect("failed to launch async runtime");

    // spawn logging thread
    rt.spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = log.flush();
        }
    });

    ctrlc::set_handler(move || {
        let state = STATE.load(Ordering::SeqCst);

        if state == RUNNING {
            info!("triggering ringbuffer capture");
            STATE.store(CAPTURING, Ordering::SeqCst);
        } else if state == CAPTURING {
            info!("waiting for capture to complete before exiting");
            STATE.store(TERMINATING, Ordering::SeqCst);
        } else {
            info!("terminating immediately");
            std::process::exit(2);
        }
    })
    .expect("failed to set ctrl-c handler");

    // parse source address
    let mut url: Url = {
        let source = args.source.clone();

        let source = if source.starts_with("http://") || source.starts_with("https://") {
            source.to_string()
        } else {
            format!("http://{source}")
        };

        match source.parse::<Url>() {
            Ok(c) => c,
            Err(error) => {
                eprintln!("source is not a valid URL: {source}\n{error}");
                std::process::exit(1);
            }
        }
    };

    if url.path() != "/" {
        eprintln!("URL should not have an non-root path: {url}");
        std::process::exit(1);
    }

    url.set_path("/metrics/binary");

    // our http client
    let client = match Client::builder().http1_only().build() {
        Ok(c) => c,
        Err(e) => {
            error!("error connecting to Rezolus: {e}");
            std::process::exit(1);
        }
    };

    // open our destination file
    let mut destination = std::fs::File::create(args.destination.clone()).map_err(|e| {
        error!("failed to open destination file: {e}");
        std::process::exit(1);
    }).ok();

    // our writer will either be our destination if the output is raw msgpack or
    // it will be some tempfile
    let mut writer = match args.format {
        Format::Raw => {
            destination.take().unwrap()
        }
        Format::Parquet => {
            let mut path: PathBuf = args.destination.clone().parse().map_err(|e| {
                eprintln!("destination is not a valid path: {e}");
                std::process::exit(1);
            }).unwrap();
            path.pop();

            match tempfile_in(path.clone()) {
                Ok(t) => t,
                Err(error) => {
                    eprintln!("could not open temporary file in: {:?}\n{error}", path);
                    std::process::exit(1);
                }
            }
        }
    };

    rt.block_on(async move {
        // get an aligned start time
        let start = tokio::time::Instant::now() - Duration::from_nanos(Utc::now().nanosecond() as u64)
            + args.interval.into();

        // sampling interval
        let mut interval = tokio::time::interval_at(start, args.interval.into());

        // sample in a loop until RUNNING is false or duration has completed
        while STATE.load(Ordering::Relaxed) == RUNNING {
            // check if the duration has completed
            if let Some(duration) = args.duration.map(Into::<Duration>::into) {
                if start.elapsed() >= duration {
                    break;
                }
            }

            // wait to sample
            interval.tick().await;

            let start = Instant::now();

            // sample rezolus
            if let Ok(response) = client.get(url.clone()).send() {
                if let Ok(body) = response.bytes() {
                    let latency = start.elapsed();

                    debug!("sampling latency: {} us", latency.as_micros());

                    if let Err(e) = writer.write_all(&body) {
                        error!("error writing to temporary file: {e}");
                        std::process::exit(1);
                    }
                } else {
                    error!("failed read response. terminating early");
                    break;
                }
            } else {
                error!("failed to get metrics. terminating early");
                break;
            }
        }

        debug!("flushing writer");
        let _ = writer.flush();

        // handle any output format specific transforms
        match args.format {
            Format::Raw => {
                debug!("finished");
            }
            Format::Parquet => {
                debug!("converting temp file to parquet");

                let _ = writer.rewind();

                if let Err(e) = MsgpackToParquet::with_options(ParquetOptions::new())
                    .convert_file_handle(writer, destination.unwrap())
                {
                    eprintln!("error saving parquet file: {e}");
                }
            }
        }
    })
}
