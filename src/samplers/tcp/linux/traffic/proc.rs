use crate::*;

use crate::common::classic::NestedMap;
use crate::common::{Counter, Interval};
use crate::samplers::tcp::linux::stats::*;
use tokio::fs::File;

use super::NAME;

pub struct ProcNetSnmp {
    interval: Interval,
    last: Option<Instant>,
    file: File,
    counters: Vec<(Counter, &'static str, &'static str)>,
}

impl ProcNetSnmp {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let counters = vec![
            (
                Counter::new(&TCP_RX_PACKETS, Some(&TCP_RX_PACKETS_HISTOGRAM)),
                "Tcp:",
                "InSegs",
            ),
            (
                Counter::new(&TCP_TX_PACKETS, Some(&TCP_TX_PACKETS_HISTOGRAM)),
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
        let now = self.interval.tick().await;
        let elapsed = self.last.map(|v| { now.duration_since(l) });
        self.last = now;

        if let Ok(nested_map) = NestedMap::try_from_procfs(&mut self.file).await {
            for (counter, pkey, lkey) in self.counters.iter_mut() {
                if let Some(curr) = nested_map.get(pkey, lkey) {
                    counter.set(elapsed.as_secs_f64(), curr);
                }
            }
        }
    }
}
