use super::*;
use clap::ArgMatches;

mod microbenchmark;

static THREAD_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Config {
    duration: humantime::Duration,
    verbose: u8,
}
impl TryFrom<ArgMatches> for Config {
    type Error = String;

    fn try_from(
        args: ArgMatches,
    ) -> Result<Self, <Self as std::convert::TryFrom<clap::ArgMatches>>::Error> {
        Ok(Config {
            verbose: *args.get_one::<u8>("VERBOSE").unwrap_or(&0),
            duration: args
                .get_one::<humantime::Duration>("DURATION")
                .copied()
                .unwrap(),
        })
    }
}

pub fn command() -> Command {
    Command::new("benchmark")
        .alias("bench")
        .about("On-demand recording to a file")
        .arg(
            clap::Arg::new("VERBOSE")
                .long("verbose")
                .short('v')
                .help("Increase the verbosity")
                .action(clap::ArgAction::Count),
        )
        .arg(
            clap::Arg::new("DURATION")
                .long("duration")
                .short('d')
                .help("Sets the per-test duration")
                .action(clap::ArgAction::Set)
                .required(true)
                .value_parser(value_parser!(humantime::Duration)),
        )
}

/// Runs the Rezolus `recorder` which is a Rezolus client that pulls data from
/// the msgpack endpoint and writes it to disk. The caller may use either timed
/// collection or terminate the process to finalize the recording.
///
/// This is intended to be run as ad-hoc collection of high-resolution metrics
/// or in situations where Rezolus is being used outside of a full observability
/// stack, for example in lab environments where experiments are being run using
/// either manual or automated processes.
pub fn run(config: Config) {
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

        println!();

        if state == RUNNING {
            info!("finalizing recording... ctrl+c to terminate early");
            STATE.store(TERMINATING, Ordering::SeqCst);
        } else {
            info!("terminating immediately");
            std::process::exit(2);
        }
    })
    .expect("failed to set ctrl-c handler");

    // pin the main thread and execute the benchmark

    let cores = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(cores[0]);
    info!("Pinned main thread to core {}", cores[0].id);

    rt.block_on(async move {
        let _duration: Duration = config.duration.into();

        info!("proceeding with microbenchmark suite...");

        microbenchmark::run();
    });

    // delay before exit to allow logging thread to flush

    std::thread::sleep(core::time::Duration::from_secs(1));
}
