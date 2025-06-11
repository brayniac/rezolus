use super::*;

use perf_event::events::Event;
use perf_event::events::x86::{Msr, MsrId};
use perf_event::ReadFormat;

use core::hint::black_box;

pub fn run() {
    info!("perf event microbenchmark");

    if let Ok(tsc) = Msr::new(MsrId::TSC) {
        run_event("TSC", tsc);
    } else {
        warn!("perf event: TSC MSR not found");
    }

    if let Ok(aperf) = Msr::new(MsrId::APERF) {
        run_event("APERF", aperf);
    } else {
        warn!("perf event: APERF MSR not found");
    }

    if let Ok(cycles) = Event::Hardware(perf_event::events::Hardware::CPU_CYCLES) {
        run_event("Cycles", cycles);
    } else {
        warn!("perf event: Cycles Event not found");
    }
}

pub fn run_event(name: &'static str, event: impl Event + Clone) {
    // get the latency using perf event read on our local core
    match Counter::new(0, event.clone()) {
        Ok(mut counter) => {
            let iterations = 500_000;
            let start = Instant::now();

            for _ in 0..iterations {
                black_box(counter.value());
            }

            let latency = start.elapsed().as_nanos() / iterations;

            info!("perf event {name} local: {latency}ns");
        }
        Err(_) => {
            warn!("perf event {name} local: could not initialize perf counter");
            return;
        }
    }

    // get the latency using perf event read on a remote core
    match Counter::new(1, event.clone()) {
        Ok(mut counter) => {
            let iterations = 500_000;
            let start = Instant::now();

            for _ in 0..iterations {
                black_box(counter.value());
            }

            let latency = start.elapsed().as_nanos() / iterations;
            info!("perf event {name} remote: {latency}ns");
        }
        Err(_) => {
            warn!("perf event {name} remote: could not initialize perf counter");
            return;
        }
    }

    // get the latency for reading the same event on all cores
    if let Ok(cores) = logical_cores() {
        let mut counters = Vec::new();

        for id in cores {
            if let Ok(counter) = Counter::new(id, event.clone()) {
                counters.push(counter);
            } else {
                warn!("perf event {name} all: could not initialize perf counter");
                return;
            }
        }

        let iterations = 500_000;
        let iterations_per_core = iterations / counters.len();
        let iterations = iterations_per_core * counters.len();
        let start = Instant::now();

        for _ in 0..iterations {
            for counter in counters.iter() {
                black_box(counter.value());
            }
        }

        let latency = start.elapsed().as_nanos() / iterations;
        info!("perf event {name} all: {latency}ns");
    }
}

fn logical_cores() -> Result<Vec<usize>, std::io::Error> {
    let mut cores: BTreeSet<usize> = BTreeSet::new();

    // walk the cpu devices directory
    for entry in WalkDir::new("/sys/devices/system/cpu")
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let filename = path.file_name().and_then(|v| v.to_str()).unwrap_or("");

        // check if this is a cpu directory
        if filename.starts_with("cpu") && filename[3..].chars().all(char::is_numeric) {
            if let Ok(core_id) = filename[3..].parse() {
                cores.insert(core_id);
            }
        }
    }

    Ok(cores.iter().copied().collect())
}

struct Counter {
    counter: perf_event::Counter,
    core: usize,
}

impl Counter {
    pub fn new(core: usize, event: impl Event) -> Result<Self, ()> {
        match perf_event::Builder::new(event)
            .one_cpu(core)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .pinned(true)
            .read_format(
                ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING | ReadFormat::GROUP,
            )
            .build()
        {
            Ok(mut counter) => {
                if counter.enable_group().is_ok() {
                    Ok(Self {
                        counter,
                        core,
                    })
                } else {
                    Err(())
                }
            }
            Err(e) => {
                Err(())
            }
        }
    }

    pub fn value(&mut self) -> u64 {
        if let Ok(group) = self.counter.read_group() {
            if let Some(counter) = group.get(&self.counter) {
                counter.value()
            } else {
                panic!("couldn't read counter");
            }
        } else {
            panic!("perf group read failed");
        }
    }
}
