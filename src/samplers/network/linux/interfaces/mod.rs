use crate::common::*;
use crate::samplers::network::linux::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(s) = NetworkInterfaces::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

const NAME: &str = "network_interfaces";

struct NetworkInterfaces {
    inner: SysfsNetSampler,
    interval: Interval,
}

impl NetworkInterfaces {
    pub fn new(config: &Config) -> Result<Self, ()> {
        let metrics = vec![
            (&NETWORK_CARRIER_CHANGES, "../carrier_changes"),
            (&NETWORK_RX_CRC_ERRORS, "rx_crc_errors"),
            (&NETWORK_RX_DROPPED, "rx_dropped"),
            (&NETWORK_RX_MISSED_ERRORS, "rx_missed_errors"),
            (&NETWORK_TX_DROPPED, "tx_dropped"),
        ];

        Ok(Self {
            inner: SysfsNetSampler::new(config, NAME, metrics)?,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for NetworkInterfaces {
    async fn sample(&mut self) {
        self.interval.tick().await;

        let now = Instant::now();
        METADATA_NETWORK_INTERFACES_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

        let _ = self.inner.sample_now();

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_NETWORK_INTERFACES_RUNTIME.add(elapsed);
        let _ = METADATA_NETWORK_INTERFACES_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}
