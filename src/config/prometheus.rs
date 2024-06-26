use super::*;

#[derive(Deserialize)]
pub struct Prometheus {
    #[serde(default = "disabled")]
    histograms: bool,
    #[serde(default = "four")]
    histogram_grouping_power: u8,
}

impl Default for Prometheus {
    fn default() -> Self {
        Self {
            histograms: false,
            histogram_grouping_power: 4,
        }
    }
}

impl Prometheus {
    pub fn check(&self) {
        if !(2..=(crate::common::HISTOGRAM_GROUPING_POWER)).contains(&self.histogram_grouping_power)
        {
            eprintln!(
                "prometheus histogram downsample factor must be in the range 2..={}",
                crate::common::HISTOGRAM_GROUPING_POWER
            );
            std::process::exit(1);
        }
    }

    pub fn histograms(&self) -> bool {
        self.histograms
    }

    pub fn histogram_grouping_power(&self) -> u8 {
        self.histogram_grouping_power
    }
}
