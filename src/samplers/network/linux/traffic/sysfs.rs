use crate::common::{Interval, Nop};
use crate::samplers::hwinfo::hardware_info;
use crate::samplers::network::stats::*;
use crate::samplers::network::*;
use metriken::Counter;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use super::NAME;

#[distributed_slice(NETWORK_SAMPLERS)]
fn init(config: &Config) -> Box<dyn Sampler> {
    let metrics = vec![
        (&NETWORK_RX_BYTES, "rx_bytes"),
        (&NETWORK_RX_PACKETS, "rx_packets"),
        (&NETWORK_TX_BYTES, "tx_bytes"),
        (&NETWORK_TX_PACKETS, "tx_packets"),
    ];

    if let Ok(s) = SysfsNetSampler::new(config, super::NAME, metrics) {
        Box::new(s)
    } else {
        Box::new(Nop::new(config))
    }
}
