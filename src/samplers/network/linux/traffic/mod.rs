use crate::samplers::network::linux::*;
use crate::*;

const NAME: &str = "network_traffic";

#[cfg(feature = "bpf")]
mod bpf;

#[cfg(feature = "bpf")]
use bpf::*;

#[cfg(feature = "bpf")]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    // try to initialize the bpf based sampler
    if let Ok(s) = NetworkTraffic::new(config) {
        Some(Box::new(s))
    } else {
        let metrics = vec![
            (&NETWORK_RX_BYTES, "rx_bytes"),
            (&NETWORK_RX_PACKETS, "rx_packets"),
            (&NETWORK_TX_BYTES, "tx_bytes"),
            (&NETWORK_TX_PACKETS, "tx_packets"),
        ];

        if let Ok(s) = SysfsNetSampler::new(config, NAME, metrics) {
            Some(Box::new(s))
        } else {
            None
        }
    }
}

#[cfg(not(feature = "bpf"))]
#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    let metrics = vec![
        (&NETWORK_RX_BYTES, "rx_bytes"),
        (&NETWORK_RX_PACKETS, "rx_packets"),
        (&NETWORK_TX_BYTES, "tx_bytes"),
        (&NETWORK_TX_PACKETS, "tx_packets"),
    ];

    if let Ok(s) = SysfsNetSampler::new(config, NAME, metrics) {
        Some(Box::new(s))
    } else {
        None
    }
}
