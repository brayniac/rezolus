#[cfg(all(feature = "bpf", target_os = "linux"))]
pub mod bpf;

pub mod classic;
pub mod units;

mod counter;
mod interval;

pub use counter::CounterWithHist;
pub use interval::Interval;

pub const HISTOGRAM_GROUPING_POWER: u8 = 7;
