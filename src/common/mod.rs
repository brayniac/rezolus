mod counters;
mod gauges;

#[cfg(target_os = "linux")]
mod sync_primitive;

pub use counters::*;
pub use gauges::*;

#[cfg(target_os = "linux")]
pub use sync_primitive::SyncPrimitive;

#[cfg(target_os = "linux")]
pub mod bpf;

#[cfg(target_os = "linux")]
pub use bpf::*;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub mod perf_events;

pub static HISTOGRAM_GROUPING_POWER: u8 = 3;

// Time units with base unit as nanoseconds
pub const SECONDS: u64 = 1_000 * MILLISECONDS;
pub const MILLISECONDS: u64 = 1_000 * MICROSECONDS;
pub const MICROSECONDS: u64 = 1_000 * NANOSECONDS;
pub const NANOSECONDS: u64 = 1;

// Data (IEC) with base unit as bytes - typically used for memory
pub const KIBIBYTES: u64 = 1024 * BYTES;
pub const BYTES: u64 = 1;
