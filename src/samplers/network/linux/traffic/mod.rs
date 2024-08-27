use crate::common::*;
use crate::samplers::network::linux::*;

const NAME: &str = "network_traffic";

#[cfg(feature = "bpf")]
mod bpf;

#[cfg(feature = "bpf")]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    // try to initialize the bpf based sampler
    if let Ok(s) = bpf::NetworkTraffic::new(config) {
        Some(Box::new(s))
    } else {
        if let Ok(s) = NetworkTraffic::new(config) {
            Some(Box::new(s))
        } else {
            None
        }
    }
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(s) = NetworkTraffic::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

struct NetworkTraffic {
    inner: SysfsNetSampler,
    interval: Interval,
}

impl NetworkTraffic {
    pub fn new(config: &Config) -> Result<Self, ()> {
        let metrics = vec![
            (&NETWORK_RX_BYTES, "rx_bytes"),
            (&NETWORK_RX_PACKETS, "rx_packets"),
            (&NETWORK_TX_BYTES, "tx_bytes"),
            (&NETWORK_TX_PACKETS, "tx_packets"),
        ];

        Ok(Self {
            inner: SysfsNetSampler::new(config, NAME, metrics)?,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for NetworkTraffic {
    async fn sample(&mut self) {
        self.interval.tick().await;

        let now = Instant::now();
        METADATA_NETWORK_TRAFFIC_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

        let _ = self.inner.sample_now();

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_NETWORK_TRAFFIC_RUNTIME.add(elapsed);
        let _ = METADATA_NETWORK_TRAFFIC_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}
