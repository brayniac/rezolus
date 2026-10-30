use crate::common::HISTOGRAM_GROUPING_POWER;
use metriken::*;

use crate::agent::timing::AcquisitionGroup;
use linkme::distributed_slice;

// Registered here (not in mod.rs) because this file is also `include!`d
// directly on non-Linux platforms (see `tcp/mod.rs`'s
// `#[cfg(not(target_os = "linux"))] mod stats` fallback) to keep metric
// identity stable across platforms, while `mod.rs`'s BPF sampler code is
// Linux-only.
pub static LATENCY_ACQ: AcquisitionGroup = AcquisitionGroup::new(
    crate::agent::samplers::bpf_sampler_name("tcp_connect_latency"),
    "tcp_connect_latency_latency",
);

#[distributed_slice(crate::agent::samplers::ACQUISITION_GROUPS)]
static LATENCY_ACQ_REG: &'static AcquisitionGroup = &LATENCY_ACQ;

#[metric(
    name = "tcp_connect_latency",
    description = "Distribution of the time from connect() to an established connection (active open)",
    metadata = { unit = "nanoseconds", outcome = "established", acq_group = "tcp_connect_latency_latency" }
)]
pub static TCP_CONNECT_LATENCY_ESTABLISHED: RwLockCounterHistogram =
    RwLockCounterHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "tcp_connect_latency",
    description = "Distribution of the time from connect() to a RST in reply to the SYN (connection refused)",
    metadata = { unit = "nanoseconds", outcome = "refused", acq_group = "tcp_connect_latency_latency" }
)]
pub static TCP_CONNECT_LATENCY_REFUSED: RwLockCounterHistogram =
    RwLockCounterHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "tcp_connect_latency",
    description = "Distribution of the time from connect() to the kernel giving up after its SYN retries (ETIMEDOUT)",
    metadata = { unit = "nanoseconds", outcome = "timeout", acq_group = "tcp_connect_latency_latency" }
)]
pub static TCP_CONNECT_LATENCY_TIMEOUT: RwLockCounterHistogram =
    RwLockCounterHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "tcp_connect_latency",
    description = "Distribution of the time from connect() to a failure with another error, such as an ICMP host or network unreachable, including a SYN timeout after one",
    metadata = { unit = "nanoseconds", outcome = "error", acq_group = "tcp_connect_latency_latency" }
)]
pub static TCP_CONNECT_LATENCY_ERROR: RwLockCounterHistogram =
    RwLockCounterHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "tcp_connect_latency",
    description = "Distribution of the time from connect() to the socket closing in SYN_SENT with no error: closed by the caller, or a failure inside connect() after the SYN",
    metadata = { unit = "nanoseconds", outcome = "aborted", acq_group = "tcp_connect_latency_latency" }
)]
pub static TCP_CONNECT_LATENCY_ABORTED: RwLockCounterHistogram =
    RwLockCounterHistogram::new(HISTOGRAM_GROUPING_POWER, 64);

#[metric(
    name = "rezolus_bpf_run_count",
    description = "The number of times Rezolus BPF programs have been run",
    metadata = { sampler = "tcp_connect_latency"}
)]
pub static BPF_RUN_COUNT: LazyCounter = LazyCounter::new(Counter::default);

#[metric(
    name = "rezolus_bpf_run_time",
    description = "The amount of time Rezolus BPF programs have been executing",
    metadata = { unit = "nanoseconds", sampler = "tcp_connect_latency"}
)]
pub static BPF_RUN_TIME: LazyCounter = LazyCounter::new(Counter::default);
