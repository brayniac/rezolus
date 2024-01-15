use crate::*;

sampler!(Sensors, "sensors", SENSOR_SAMPLERS);

mod stats;

#[cfg(all(target_os = "linux"))]
mod lm_sensors;
