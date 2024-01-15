use crate::*;

sampler!(Syscall, "sensors", SYSCALL_SAMPLERS);

mod stats;

#[cfg(all(target_os = "linux"))]
mod lm_sensors;
