use super::*;

#[cfg(target_os = "linux")]
pub mod perf;

pub fn run() {
	info!("running microbenchmarks");
	
	#[cfg(target_os = "linux")]
	perf::run();
}