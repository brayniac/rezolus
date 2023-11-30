use crate::common::classic::NestedMap;
use crate::common::Counter;
use crate::common::Nop;
use crate::samplers::tcp::stats::*;
use crate::samplers::tcp::*;
use std::fs::File;

#[distributed_slice(TCP_SAMPLERS)]
fn init(config: &Config) -> Box<dyn Sampler> {
    if let Ok(s) = Snmp::new(config) {
        Box::new(s)
    } else {
        Box::new(Nop::new(config))
    }
}

const NAME: &str = "tcp_netstat";

pub struct Netstat {
    prev: Instant,
    next: Instant,
    interval: Duration,
    file: File,
    counters: Vec<(Counter, &'static str, &'static str)>,
}

impl Netstat {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let now = Instant::now();

        // NOTE: we're assuming that IP traffic is all TCP
        let counters = vec![
            (
                Counter::new(&TCP_RX_BYTES_SNMP, Some(&TCP_RX_BYTES_SNMP_HISTOGRAM)),
                "IpExt:",
                "InOctets",
            ),
            (
                Counter::new(&TCP_TX_BYTES_SNMP, Some(&TCP_TX_BYTES_SNMP_HISTOGRAM)),
                "IpExt:",
                "OutOctets",
            ),
        ];

        Ok(Self {
            file: File::open("/proc/net/netstat").expect("file not found"),
            counters,
            prev: now,
            next: now,
            interval: config.interval(NAME),
        })
    }
}

impl Sampler for Netstat {
    fn sample(&mut self) {
        let now = Instant::now();

        if now < self.next {
            return;
        }

        let elapsed = (now - self.prev).as_secs_f64();

        if let Ok(nested_map) = NestedMap::try_from_procfs(&mut self.file) {
            for (counter, pkey, lkey) in self.counters.iter_mut() {
                if let Some(curr) = nested_map.get(pkey, lkey) {
                    counter.set(elapsed, curr);
                }
            }
        }

        // determine when to sample next
        let next = self.next + self.interval;

        // it's possible we fell behind
        if next > now {
            // if we didn't, sample at the next planned time
            self.next = next;
        } else {
            // if we did, sample after the interval has elapsed
            self.next = now + self.interval;
        }

        // mark when we last sampled
        self.prev = now;
    }
}
