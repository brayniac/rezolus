use crate::*;

use crate::common::classic::NestedMap;
use crate::samplers::tcp::linux::stats::*;
use tokio::fs::File;

use super::NAME;

pub struct ProcNetSnmp {
    interval: Interval,
    file: File,
    counters: Vec<(CounterWithHist, &'static str, &'static str)>,
}

impl ProcNetSnmp {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let counters = vec![
            (
                CounterWithHist::new(&TCP_RX_PACKETS, &TCP_RX_PACKETS_HISTOGRAM),
                "Tcp:",
                "InSegs",
            ),
            (
                CounterWithHist::new(&TCP_TX_PACKETS, &TCP_TX_PACKETS_HISTOGRAM),
                "Tcp:",
                "OutSegs",
            ),
        ];

        let file = std::fs::File::open("/proc/net/snmp").map(|f| File::from_std(f)).map_err(|e| {
            error!("Failed to open /proc/net/snmp: {e}");
        })?;

        Ok(Self {
            file,
            counters,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for ProcNetSnmp {
    async fn sample(&mut self) {
        let elapsed = self.interval.tick().await;

        if let Ok(nested_map) = NestedMap::try_from_procfs(&mut self.file).await {
            for (counter, pkey, lkey) in self.counters.iter_mut() {
                if let Some(curr) = nested_map.get(pkey, lkey) {
                    counter.set(elapsed, curr);
                }
            }
        }
    }
}
