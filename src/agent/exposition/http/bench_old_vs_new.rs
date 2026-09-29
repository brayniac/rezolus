//! BENCH BRANCH ONLY: the agent's pre-metriken `create_v3` (`bench_old_v3`)
//! against metriken's `GroupBuilder` (`create_v3`), side by side, on one
//! registry.
//!
//! ```text
//! cargo test --release --locked --bin rezolus -- --ignored old_vs_new_build_cost --nocapture --test-threads=1
//! ```
//!
//! Environment:
//! - `GB_TICKS` cache-hit ticks measured (default 3000)
//! - `GB_CHG_TICKS` membership-change ticks measured (default 2000)
//! - `GB_WARMUP` warm-up ticks before measuring (default 300)
//! - `GB_ONLY=old|new` build with one builder only (for `perf record`)
//! - `GB_FIRST=old|new` which builder runs first on tick 0 and in warm-up
//!   (default old); the order alternates every tick after that
//!
//! Prints one line per run:
//! `RESULT hit_old_p50=.. hit_old_p99=.. hit_new_p50=.. hit_new_p99=..
//! hit_ratio=.. chg_old_p50=.. chg_old_p99=.. chg_new_p50=.. chg_new_p99=..
//! chg_ratio=..` in microseconds; a side not built is `NaN`.

use super::*;
use linkme::distributed_slice;
use metriken::{Counter, CounterGroup, DynBoxedMetric, Gauge, MetricBuilder};

fn md(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn in_group(name: &str, group: &str) -> MetricBuilder {
    MetricBuilder::new(name.to_string()).metadata("acq_group", group)
}

static GB_TASK: AcquisitionGroup = AcquisitionGroup::new_reader_stamped("unattributed", "gb_task");
#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static GB_TASK_ENTRY: &'static AcquisitionGroup = &GB_TASK;

static GB_CGROUP: AcquisitionGroup =
    AcquisitionGroup::new_reader_stamped("unattributed", "gb_cgroup");
#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static GB_CGROUP_ENTRY: &'static AcquisitionGroup = &GB_CGROUP;

static GB_CPU_A: AcquisitionGroup = AcquisitionGroup::new("unattributed", "gb_cpu_a");
#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static GB_CPU_A_ENTRY: &'static AcquisitionGroup = &GB_CPU_A;

static GB_CPU_B: AcquisitionGroup = AcquisitionGroup::new("unattributed", "gb_cpu_b");
#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static GB_CPU_B_ENTRY: &'static AcquisitionGroup = &GB_CPU_B;

static GB_CPU_C: AcquisitionGroup = AcquisitionGroup::new("unattributed", "gb_cpu_c");
#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static GB_CPU_C_ENTRY: &'static AcquisitionGroup = &GB_CPU_C;

static GB_SCALARS: AcquisitionGroup = AcquisitionGroup::new("unattributed", "gb_scalars");
#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static GB_SCALARS_ENTRY: &'static AcquisitionGroup = &GB_SCALARS;

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

/// `(p50, p99)` in microseconds, or `nan` for a side that was not built.
fn p50_p99(v: &mut [Duration]) -> (f64, f64) {
    if v.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    v.sort();
    let at = |p: f64| us(v[((v.len() as f64 * p) as usize).min(v.len() - 1)]);
    (at(0.5), at(0.99))
}

#[test]
#[ignore]
fn old_vs_new_build_cost() {
    const CPUS: usize = 64;
    let hit_ticks = env_usize("GB_TICKS", 3000);
    let chg_ticks = env_usize("GB_CHG_TICKS", 2000);
    let warmup = env_usize("GB_WARMUP", 300);
    let only = std::env::var("GB_ONLY").ok();
    let run_old = only.as_deref() != Some("new");
    let run_new = only.as_deref() != Some("old");
    let new_first = std::env::var("GB_FIRST").ok().as_deref() == Some("new");

    // Per-task: 4 counter groups over one 4096-slot space, 2,500 live.
    let task: Vec<DynBoxedMetric<CounterGroup>> = ["user", "system", "wait", "switches"]
        .iter()
        .map(|m| in_group(&format!("gb_task_{m}"), "gb_task").build(CounterGroup::new(4096)))
        .collect();
    for idx in 0..2_500usize {
        let labels = md(&[
            ("name", &format!("task-{idx}")),
            ("pid", &idx.to_string()),
            ("__uid__", &format!("{:016x}", idx * 7919)),
        ]);
        for g in &task {
            g.set_metadata(idx, labels.clone());
            g.add(idx, idx as u64 + 1);
        }
    }

    // Per-cgroup: 3 counter groups over 512 slots, 300 live.
    let cgroup: Vec<DynBoxedMetric<CounterGroup>> = ["cycles", "instructions", "throttled"]
        .iter()
        .map(|m| in_group(&format!("gb_cgroup_{m}"), "gb_cgroup").build(CounterGroup::new(512)))
        .collect();
    for idx in 0..300usize {
        let labels = md(&[("name", &format!("/system.slice/unit-{idx}.service"))]);
        for g in &cgroup {
            g.set_metadata(idx, labels.clone());
            g.add(idx, 1);
        }
    }

    // Per-CPU: three stamped groups of 1024-entry counter groups, bounded
    // to 64.
    let mut percpu: Vec<DynBoxedMetric<CounterGroup>> = Vec::new();
    for (group, ag, metrics) in [
        ("gb_cpu_a", &GB_CPU_A, 4usize),
        ("gb_cpu_b", &GB_CPU_B, 3),
        ("gb_cpu_c", &GB_CPU_C, 2),
    ] {
        ag.set_member_bound(CPUS);
        for m in 0..metrics {
            let g = in_group(&format!("{group}_{m}"), group).build(CounterGroup::new(1024));
            for cpu in 0..CPUS {
                g.set_metadata(cpu, md(&[("cpu", &cpu.to_string())]));
                g.add(cpu, 1);
            }
            percpu.push(g);
        }
        ag.acquire().finish();
    }

    // 200 scalars: 100 declared, 100 in the default group.
    let mut counters: Vec<DynBoxedMetric<Counter>> = Vec::new();
    let mut gauges: Vec<DynBoxedMetric<Gauge>> = Vec::new();
    for i in 0..50 {
        counters.push(in_group(&format!("gb_sc_{i}"), "gb_scalars").build(Counter::new()));
        gauges.push(in_group(&format!("gb_sg_{i}"), "gb_scalars").build(Gauge::new()));
        counters.push(MetricBuilder::new(format!("gb_dc_{i}")).build(Counter::new()));
        gauges.push(MetricBuilder::new(format!("gb_dg_{i}")).build(Gauge::new()));
    }
    for c in &counters {
        c.add(1);
    }
    for g in &gauges {
        g.set(1);
    }

    let mut old_cache = super::bench_old_v3::SkeletonCache::new();
    let mut new_builder = v3_builder();
    let mut build_old = || {
        let start = Instant::now();
        let s = super::bench_old_v3::create_v3(
            SystemTime::now(),
            Duration::ZERO,
            Vec::new(),
            &mut old_cache,
            (0, 0),
        );
        let elapsed = start.elapsed();
        std::hint::black_box(s);
        elapsed
    };
    let mut build_new = || {
        let start = Instant::now();
        let s = create_v3(Duration::ZERO, Vec::new(), &mut new_builder, (0, 0));
        let elapsed = start.elapsed();
        std::hint::black_box(s);
        elapsed
    };

    // One tick: both builders (or the one asked for), OLD first on even
    // ticks unless GB_FIRST=new, then alternating.
    let mut tick = |i: usize, old: &mut Vec<Duration>, new: &mut Vec<Duration>| {
        let old_first = i.is_multiple_of(2) != new_first;
        if old_first {
            if run_old {
                old.push(build_old());
            }
            if run_new {
                new.push(build_new());
            }
        } else {
            if run_new {
                new.push(build_new());
            }
            if run_old {
                old.push(build_old());
            }
        }
    };

    let (mut scratch_old, mut scratch_new) = (Vec::new(), Vec::new());
    for i in 0..warmup {
        tick(i, &mut scratch_old, &mut scratch_new);
    }

    let (mut hit_old, mut hit_new) = (Vec::new(), Vec::new());
    for i in 0..hit_ticks {
        for g in &task {
            g.add(i % 2_500, 1);
        }
        tick(i, &mut hit_old, &mut hit_new);
    }

    // A membership-change tick: one task slot assigned and the previous one
    // released, so the task group misses and every other group hits. The
    // live population stays at 2,501.
    let (mut chg_old, mut chg_new) = (Vec::new(), Vec::new());
    for i in 0..chg_ticks {
        let idx = 2_500 + (i % 1_500);
        let prev = 2_500 + ((i + 1_499) % 1_500);
        for g in &task {
            g.set_metadata(idx, md(&[("name", &format!("task-{idx}"))]));
            g.add(idx, 1);
            if i > 0 {
                g.clear_metadata(prev);
            }
        }
        tick(i, &mut chg_old, &mut chg_new);
    }

    let (ho50, ho99) = p50_p99(&mut hit_old);
    let (hn50, hn99) = p50_p99(&mut hit_new);
    let (co50, co99) = p50_p99(&mut chg_old);
    let (cn50, cn99) = p50_p99(&mut chg_new);
    println!(
        "RESULT hit_old_p50={ho50:.1} hit_old_p99={ho99:.1} hit_new_p50={hn50:.1} \
         hit_new_p99={hn99:.1} hit_ratio={:.4} chg_old_p50={co50:.1} chg_old_p99={co99:.1} \
         chg_new_p50={cn50:.1} chg_new_p99={cn99:.1} chg_ratio={:.4} registry={} \
         hit_ticks={hit_ticks} chg_ticks={chg_ticks} warmup={warmup} only={} first={}",
        hn50 / ho50,
        cn50 / co50,
        metriken::metrics().iter().count(),
        only.as_deref().unwrap_or("both"),
        if new_first { "new" } else { "old" },
    );

    drop((task, cgroup, percpu, counters, gauges));
}
