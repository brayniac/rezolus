use crate::common::Interval;
use crate::samplers::tcp::linux::stats::*;
use crate::*;
use metriken::Gauge;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    ConnectionState::init(config)
}

const NAME: &str = "tcp_connection_state";

pub struct ConnectionState {
    interval: Interval,
    files: Vec<File>,
    gauges: Vec<(&'static Lazy<Gauge>, i64)>,
}

impl ConnectionState {
    pub fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let gauges: Vec<(&'static Lazy<Gauge>, i64)> = vec![
            (&TCP_CONN_STATE_ESTABLISHED, 0),
            (&TCP_CONN_STATE_SYN_SENT, 0),
            (&TCP_CONN_STATE_SYN_RECV, 0),
            (&TCP_CONN_STATE_FIN_WAIT1, 0),
            (&TCP_CONN_STATE_FIN_WAIT2, 0),
            (&TCP_CONN_STATE_TIME_WAIT, 0),
            (&TCP_CONN_STATE_CLOSE, 0),
            (&TCP_CONN_STATE_CLOSE_WAIT, 0),
            (&TCP_CONN_STATE_LAST_ACK, 0),
            (&TCP_CONN_STATE_LISTEN, 0),
            (&TCP_CONN_STATE_CLOSING, 0),
            (&TCP_CONN_STATE_NEW_SYN_RECV, 0),
        ];

        let ipv4 = std::fs::File::open("/proc/net/tcp")
            .map(|f| File::from_std(f))
            .map_err(|e| {
                error!("Failed to open /proc/net/tcp: {e}");
            });

        let ipv6 = std::fs::File::open("/proc/net/tcp6")
            .map(|f| File::from_std(f))
            .map_err(|e| {
                error!("Failed to open /proc/net/tcp6: {e}");
            });

        let mut files: Vec<Result<File, ()>> = vec![ipv4, ipv6];

        let files: Vec<File> = files.drain(..).filter_map(|v| v.ok()).collect();

        if files.is_empty() {
            error!("Could not open any file in /proc/net for this sampler");
            return Err(());
        }

        Ok(Box::new(Self {
            files,
            gauges,
            interval: config.interval(NAME),
        }))
    }
}

#[async_trait]
impl Sampler for ConnectionState {
    async fn sample(&mut self) {
        self.interval.tick().await;

        let now = Instant::now();

        METADATA_TCP_CONNECTION_STATE_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

        // zero the temporary gauges
        for (_, gauge) in self.gauges.iter_mut() {
            *gauge = 0;
        }

        for file in self.files.iter_mut() {
            // seek to start to cause reload of content
            if file.rewind().await.is_ok() {
                let mut data = String::new();
                if file.read_to_string(&mut data).await.is_err() {
                    error!("error reading /proc/net/tcp");
                    return;
                }

                for line in data.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();

                    // find and increment the temporary gauge for this state
                    if let Some(Ok(state)) = parts.get(3).map(|v| u8::from_str_radix(v, 16)) {
                        if let Some((_, gauge)) = self.gauges.get_mut(state as usize - 1) {
                            *gauge += 1;
                        }
                    }
                }
            }
        }

        for (gauge, value) in self.gauges.iter() {
            gauge.set(*value);
        }

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_TCP_CONNECTION_STATE_RUNTIME.add(elapsed);
        let _ = METADATA_TCP_CONNECTION_STATE_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}
