use async_trait::async_trait;
use backtrace::Backtrace;
use clap::{Arg, Command};
use linkme::distributed_slice;
use metriken::{metric, Lazy, LazyCounter};
use metriken_exposition::Histogram;
use ringlog::*;
use tokio::sync::RwLock;
use clocksource::precise::UnixInstant;
use crate::common::{CounterWithHist, Interval};

use tokio::time::Instant;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const VERSION: &str = env!("CARGO_PKG_VERSION");

type HistogramSnapshots = HashMap<String, Histogram>;

static SNAPSHOTS: Lazy<Arc<RwLock<Snapshots>>> =
    Lazy::new(|| Arc::new(RwLock::new(Snapshots::new())));

pub struct Snapshots {
    timestamp: SystemTime,
    previous: HistogramSnapshots,
    delta: HistogramSnapshots,
}

impl Default for Snapshots {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshots {
    pub fn new() -> Self {
        let snapshot = metriken_exposition::Snapshotter::default().snapshot();

        let previous: HistogramSnapshots = snapshot
            .histograms
            .into_iter()
            .map(|v| (v.name.clone(), v))
            .collect();
        let delta = previous.clone();

        Self {
            timestamp: snapshot.systemtime,
            previous,
            delta,
        }
    }

    pub fn update(&mut self) {
        let snapshot = metriken_exposition::Snapshotter::default().snapshot();
        self.timestamp = snapshot.systemtime;

        let current: HistogramSnapshots = snapshot
            .histograms
            .into_iter()
            .map(|v| (v.name.clone(), v))
            .collect();
        let mut delta = HistogramSnapshots::new();

        for (name, mut previous) in self.previous.drain() {
            if let Some(histogram) = current.get(&name).cloned() {
                if let Ok(d) = histogram.value.wrapping_sub(&previous.value) {
                    previous.value = d;
                    delta.insert(name.to_string(), previous);
                }
            }
        }

        self.previous = current;
        self.delta = delta;
    }
}

mod common;
mod config;
mod exposition;
mod samplers;

use config::Config;

pub static PERCENTILES: &[(&str, f64)] = &[
    ("p25", 25.0),
    ("p50", 50.0),
    ("p75", 75.0),
    ("p90", 90.0),
    ("p99", 99.0),
    ("p999", 99.9),
    ("p9999", 99.99),
];

#[distributed_slice]
pub static SAMPLERS: [fn(config: &Config) -> Result<Box<dyn Sampler>, ()>] = [..];

#[metric(
    name = "runtime/sample/loop",
    description = "The total number sampler loops executed"
)]
pub static RUNTIME_SAMPLE_LOOP: LazyCounter = LazyCounter::new(metriken::Counter::default);

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        eprintln!("{s}");
        eprintln!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // parse command line options
    let matches = Command::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .long_about("Rezolus provides high-resolution systems performance telemetry.")
        .arg(
            Arg::new("CONFIG")
                .help("Server configuration file")
                .action(clap::ArgAction::Set)
                .required(true)
                .index(1),
        )
        .get_matches();

    // load config from file
    let config: Arc<Config> = {
        let file = matches.get_one::<String>("CONFIG").unwrap();
        debug!("loading config: {}", file);
        match Config::load(file) {
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

    // initialize main async runtime. This is used for logging, snapshots, and
    // HTTP exposition.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .expect("failed to launch async runtime");

    // spawn logging thread
    rt.spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = log.flush();
        }
    });

    // spawn thread to maintain histogram snapshots
    let snapshot_interval = config.general().snapshot_interval();
    rt.spawn(async move {
        loop {
            // acquire a lock and update the snapshots
            {
                let mut snapshots = SNAPSHOTS.write().await;
                snapshots.update();
            }

            // delay until next update
            tokio::time::sleep(snapshot_interval).await;
        }
    });

    info!("rezolus {VERSION}");

    // spawn http exposition thread
    rt.spawn(exposition::http(config.clone()));

    // initialize fast sampler async runtime
    let fast_sampler_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .expect("failed to launch async runtime");

    // initialize normal sampler async runtime
    let normal_sampler_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .expect("failed to launch async runtime");

    for sampler in SAMPLERS {
        if let Ok(mut sampler) = sampler(&config) {
            if sampler.is_fast() {
                fast_sampler_rt.spawn(async move {
                    loop {
                        sampler.sample().await;
                    }
                });
            } else {
                normal_sampler_rt.spawn(async move {
                    loop {
                        sampler.sample().await;
                    }
                });
            }
        }
    }
}

#[async_trait]
pub trait Sampler: Send + Sync {
    /// Do some sampling and updating of stats
    async fn sample(&mut self);

    fn is_fast(&self) -> bool {
        false
    }
}
