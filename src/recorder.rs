use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::io::SeekFrom;
use clap::Subcommand;
use chrono::Timelike;
use chrono::Utc;
use backtrace::Backtrace;
use clap::Parser;
use clap::ValueEnum;
use metriken_exposition::{MsgpackToParquet, ParquetOptions};
use reqwest::Client;
use reqwest::Url;
use ringlog::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tempfile::tempfile_in;

static STATE: AtomicUsize = AtomicUsize::new(RUNNING);

static RUNNING: usize = 0;
static CAPTURING: usize = 1;
static TERMINATING: usize = 2;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Format {
    Parquet,
    Raw,
}

#[derive(Parser)]
#[command(version)]
#[command(about = "An on-demand tool for recording Rezolus metrics to a file", long_about = None)]
struct Config {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Record {
        #[arg(short, long)]
        duration: Option<humantime::Duration>,
        #[arg(short, long, default_value_t = humantime::Duration::from(Duration::from_secs(1)))]
        interval: humantime::Duration,
        #[arg(short, long, value_enum, default_value_t = Format::Parquet)]
        format: Format,
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
        #[arg(value_name = "SOURCE")]
        source: String,
        #[arg(value_name = "FILE")]
        destination: String,

    },
    Blackbox {
        #[arg(short, long)]
        duration: humantime::Duration,
        #[arg(short, long, default_value_t = humantime::Duration::from(Duration::from_secs(1)))]
        interval: humantime::Duration,
        #[arg(short, long, value_enum, default_value_t = Format::Parquet)]
        format: Format,
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
        #[arg(value_name = "SOURCE")]
        source: String,
        #[arg(value_name = "FILE")]
        destination: String,
    }
}

// impl Config {
//     /// Opens the destination file. This will be the final output file.
//     fn destination(&self, destination: String) -> std::fs::File {
//         match std::fs::File::create(self.destination_path(destination.clone())) {
//             Ok(f) => f,
//             Err(error) => {
//                 eprintln!(
//                     "could not open destination: {:?}\n{error}",
//                     destination
//                 );
//                 std::process::exit(1);
//             }
//         }
//     }

//     /// Get the path to the destination file.
//     fn destination_path(&self, destination: String) -> PathBuf {
//         match destination.parse() {
//             Ok(p) => p,
//             Err(error) => {
//                 eprintln!(
//                     "destination is not a valid path: {}\n{error}",
//                     destination
//                 );
//                 std::process::exit(1);
//             }
//         }
//     }

//     /// The interval between each sample.
//     fn interval(&self) -> Duration {
//         self.interval.into()
//     }

//     /// Get a temporary file in the output directory
//     fn temporary(&self, destination: String) -> std::fs::File {
//         // tempfile will be in same directory as out destination file
//         let mut temp_path = self.destination_path(destination);
//         temp_path.pop();

//         match tempfile_in(temp_path.clone()) {
//             Ok(t) => t,
//             Err(error) => {
//                 eprintln!("could not open temporary file in: {:?}\n{error}", temp_path);
//                 std::process::exit(1);
//             }
//         }
//     }

//     /// The url to request. Currently we expect that if this is a complete URL
//     /// that the path is root-level. We accept host:port, or IP:port here too.
//     /// We then sample `/metrics/binary` which is the Rezolus msgpack endpoint.
//     fn url(&self, url: String) -> Url {
//         // parse source address
//         let mut url: Url = {
//             let source = url;

//             let source = if source.starts_with("http://") || source.starts_with("https://") {
//                 source.to_string()
//             } else {
//                 format!("http://{source}")
//             };

//             match source.parse::<Url>() {
//                 Ok(c) => c,
//                 Err(error) => {
//                     eprintln!("source is not a valid URL: {source}\n{error}");
//                     std::process::exit(1);
//                 }
//             }
//         };

//         if url.path() != "/" {
//             eprintln!("URL should not have an non-root path: {url}");
//             std::process::exit(1);
//         }

//         url.set_path("/metrics/binary");

//         url
//     }
// }

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        eprintln!("{s}");
        eprintln!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // parse command line options
    let config = Config::parse();

    // configure debug log
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

    match config.command {
        Command::Record { duration, source, destination } => {
            let config = RecorderConfig {
                duration,
                source,
                destination,
            };

            rt.block_on(async move {
                recorder(config, duration).await
            });
        }
        Command::Blackbox { duration, source, destination } => {
            rt.block_on(async move {
                blackbox(config, duration).await
            });
        }
    }
}

pub struct RecorderConfig {
    duration: Option<humantime::Duration>,
    source: String,
    destination: String,
}

async fn recorder(config: RecorderConfig, duration: Option<humantime::Duration>) {
    // convert the duration
    let duration: Option<Duration> = duration.map(|v| v.into());

    // load the url to connect to
    let url = config.url();

    // open destination and (optional) temporary files
    let mut destination = Some(config.destination());

    // our http client
    let client = match Client::builder().http1_only().build() {
        Ok(c) => c,
        Err(e) => {
            error!("error connecting to Rezolus: {e}");
            std::process::exit(1);
        }
    };

    // writer will be either the temporary file or the final destination file
    // depending on the output format
    let mut writer = if config.format == Format::Raw {
        destination.take().unwrap()
    } else {
        config.temporary()
    };

    // get an aligned start time
    let start = tokio::time::Instant::now() - Duration::from_nanos(Utc::now().nanosecond() as u64)
        + config.interval();

    // sampling interval
    let mut interval = tokio::time::interval_at(start, config.interval());

    // sample in a loop until RUNNING is false or duration has completed
    while STATE.load(Ordering::Relaxed) == RUNNING {
        // check if the duration has completed
        if let Some(duration) = duration {
            if start.elapsed() >= duration {
                break;
            }
        }

        // wait to sample
        interval.tick().await;

        let start = Instant::now();

        // sample rezolus
        if let Ok(response) = client.get(url.clone()).send().await {
            if let Ok(body) = response.bytes().await {
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
    match config.format {
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
}

pub struct BlackboxConfig {
    duration: humantime::Duration,
    source: String,
    destination: String,
}

async fn blackbox(config: Config, duration: humantime::Duration) {
    // load the url to connect to
    let url = config.url();

    // open destination and temporary files
    let _ = config.destination();
    let mut temporary = config.temporary();

    // connect to rezolus
    let mut client = match Client::builder().http1_only().build() {
        Ok(c) => Some(c),
        Err(e) => {
            error!("error connecting to Rezolus: {e}");
            std::process::exit(1);
        }
    };

    // we need to estimate the snapshot size

    let c = client.take().unwrap();
    let start = Instant::now();

    let (len, latency) = if let Ok(response) = c.get(url.clone()).send().await {
        if let Ok(body) = response.bytes().await {
            let latency = start.elapsed();

            debug!("sampling latency: {} us", latency.as_micros());
            debug!("body size: {}", body.len());

            client = Some(c);

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
    if config.interval().as_micros() < (latency.as_micros() * 2) {
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
    let snapshot_len = (1 + len as u64 * 4 / 4096) * 4096;

    // the total number of snapshots
    let snapshot_count = (1 + duration.as_micros() / config.interval().as_micros()) as u64;

    // expand the temporary file to hold enough room for all the snapshots
    let _ = temporary
        .set_len(snapshot_len * snapshot_count)
        .map_err(|e| {
            error!("failed to grow temporary file: {e}");
            std::process::exit(1);
        });

    let mut idx = 0;

    // get an aligned start time
    let start = tokio::time::Instant::now() - Duration::from_nanos(Utc::now().nanosecond() as u64)
        + config.interval();

    // sampling interval
    let mut interval = tokio::time::interval_at(start, config.interval());

    loop {
        // sample in a loop until RUNNING is false or duration has completed
        while STATE.load(Ordering::Relaxed) == RUNNING {
            // connect to rezolus
            if client.is_none() {
                debug!("connecting to Rezolus at: {url}");

                match Client::builder().http1_only().build() {
                    Ok(c) => client = Some(c),
                    Err(e) => {
                        error!("error connecting to Rezolus: {e}");
                        std::process::exit(1);
                    }
                }

                continue;
            }

            let c = client.take().unwrap();

            // wait to sample
            interval.tick().await;

            let start = Instant::now();

            // sample rezolus
            if let Ok(response) = c.get(url.clone()).send().await {
                if let Ok(body) = response.bytes().await {
                    let latency = start.elapsed();

                    debug!("sampling latency: {} us", latency.as_micros());

                    debug!("body size: {}", body.len());

                    // seek to position in snapshot
                    temporary
                        .seek(SeekFrom::Start(idx * snapshot_len))
                        .expect("failed to seek");

                    // write the size of the snapshot
                    temporary
                        .write_all(&body.len().to_be_bytes())
                        .expect("failed to write snapshot size");

                    // write the actual snapshot content
                    temporary
                        .write_all(&body)
                        .expect("failed to write snapshot");

                    client = Some(c);
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
        let _ = temporary.flush();

        // handle any output format specific transforms
        match config.format {
            Format::Raw => {
                debug!("capturing ringbuffer and writing to raw");

                let mut packed = config.destination();

                for offset in 1..=snapshot_count {
                    // we start at the last recorded index + 1 to get the oldest
                    // record first
                    let mut i = idx + offset;

                    // handle wrap-around in the ring-buffer
                    if i > snapshot_len {
                        i -= snapshot_len;
                    }

                    // seek to the start of the snapshot slot
                    temporary
                        .seek(SeekFrom::Start(i * snapshot_len))
                        .expect("failed to seek");

                    // read the size of the snapshot
                    let mut len = [0, 0, 0, 0, 0, 0, 0, 0];
                    temporary
                        .read_exact(&mut len)
                        .expect("failed to read snapshot len");

                    // read the contents of the snapshot
                    let mut buf = vec![0; u64::from_be_bytes(len) as usize];
                    temporary
                        .read_exact(&mut buf)
                        .expect("failed to read snapshot");

                    // write the contents of the snapshot to the packed file
                    packed
                        .write_all(&buf)
                        .expect("failed to write to packed file");
                }

                let _ = packed.flush();

                debug!("finished");
            }
            Format::Parquet => {
                debug!("capturing ringbuffer and writing to parquet");

                let _ = temporary.rewind();

                let mut packed = config.temporary();

                for offset in 1..=snapshot_count {
                    // we start at the last recorded index + 1 to get the oldest
                    // record first
                    let mut i = idx + offset;

                    // handle wrap-around in the ring-buffer
                    if i >= snapshot_count {
                        i -= snapshot_count;
                    }

                    // seek to the start of the snapshot slot
                    temporary
                        .seek(SeekFrom::Start(i * snapshot_len))
                        .expect("failed to seek");

                    // read the size of the snapshot
                    let mut len = [0, 0, 0, 0, 0, 0, 0, 0];
                    temporary
                        .read_exact(&mut len)
                        .expect("failed to read snapshot len");

                    // read the contents of the snapshot
                    let mut buf = vec![0; u64::from_be_bytes(len) as usize];
                    temporary
                        .read_exact(&mut buf)
                        .expect("failed to read snapshot");

                    // write the contents of the snapshot to the packed file
                    packed
                        .write_all(&buf)
                        .expect("failed to write to packed file");
                }

                let _ = packed.flush();

                let _ = packed.rewind();

                let destination = config.destination();

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
}
