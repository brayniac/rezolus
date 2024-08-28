use crate::*;

pub struct Interval {
    inner: tokio::time::Interval,
    last: Option<Instant>,
}

impl Interval {
    pub fn new(period: Duration) -> Self {
        let mut inner = tokio::time::interval(period);
        inner.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        Self {
            inner,
            last: None,
        }
    }

    pub async fn tick(&mut self) -> Option<Duration> {
        let now = self.inner.tick().await;
        let elapsed = self.last.map(|v| now.duration_since(v));
        self.last = Some(now);

        elapsed
    }
}
