use crate::*;

pub struct Interval {
    inner: tokio::time::Interval,
    last: Option<Instant>,
}

impl Interval {
    pub fn new(period: Duration) -> Self {
        Self {
            inner: tokio::time::interval(period),
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
