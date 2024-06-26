use super::*;

#[derive(Deserialize, Default)]
pub struct SamplerConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub bpf: Option<bool>,
    #[serde(default)]
    pub interval: Option<String>,
    #[serde(default)]
    pub distribution_interval: Option<String>,
}

impl SamplerConfig {
    pub fn check(&self, name: &str) {
        match self
            .interval
            .as_ref()
            .map(|v| v.parse::<humantime::Duration>())
        {
            Some(Err(e)) => {
                eprintln!("{name} sampler interval is not valid: {e}");
                std::process::exit(1);
            }
            Some(Ok(interval)) => {
                if Duration::from_nanos(interval.as_nanos() as u64) < Duration::from_millis(1) {
                    eprintln!("{name} sampler interval is too short. Minimum interval is: 1ms");
                    std::process::exit(1);
                }
            }
            _ => {}
        }

        match self
            .distribution_interval
            .as_ref()
            .map(|v| v.parse::<humantime::Duration>())
        {
            Some(Err(e)) => {
                eprintln!("{name} sampler distribution_interval is not valid: {e}");
                std::process::exit(1);
            }
            Some(Ok(interval)) => {
                if Duration::from_nanos(interval.as_nanos() as u64) < Duration::from_millis(1) {
                    eprintln!("{name} sampler distribution_interval is too short. Minimum interval is: 1ms");
                    std::process::exit(1);
                }
            }
            _ => {}
        }
    }
}
