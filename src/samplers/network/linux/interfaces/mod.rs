use crate::samplers::network::linux::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    let metrics = vec![
        (&NETWORK_CARRIER_CHANGES, "../carrier_changes"),
        (&NETWORK_RX_CRC_ERRORS, "rx_crc_errors"),
        (&NETWORK_RX_DROPPED, "rx_dropped"),
        (&NETWORK_RX_MISSED_ERRORS, "rx_missed_errors"),
        (&NETWORK_TX_DROPPED, "tx_dropped"),
    ];

    if let Ok(s) = SysfsNetSampler::new(config, NAME, metrics) {
        Some(Box::new(s))
    } else {
        None
    }
}

const NAME: &str = "network_interfaces";
