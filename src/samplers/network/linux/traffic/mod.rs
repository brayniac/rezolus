use crate::*;

use crate::samplers::network::linux::*;
use crate::samplers::network::linux::stats::*;

const NAME: &str = "network_traffic";

#[cfg(feature = "bpf")]
mod bpf;

#[cfg(feature = "bpf")]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    // check if sampler should be enabled
    if !config.enabled(NAME) {
        return Err(());
    }

    bpf::NetworkTraffic::init(config).or_else(|_| NetworkTraffic::init(config))
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    NetworkTraffic::init(config)
}

struct NetworkTraffic {
    inner: SysfsNetSampler,
    interval: Interval,
}

impl NetworkTraffic {
    pub fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
        let metrics = vec![
            (&NETWORK_RX_BYTES, "rx_bytes"),
            (&NETWORK_RX_PACKETS, "rx_packets"),
            (&NETWORK_TX_BYTES, "tx_bytes"),
            (&NETWORK_TX_PACKETS, "tx_packets"),
        ];

        Ok(Box::new(Self {
            inner: SysfsNetSampler::new(config, NAME, metrics)?,
            interval: config.interval(NAME),
        }))
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
