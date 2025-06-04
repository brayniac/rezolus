use crate::agent::Config;
use async_trait::async_trait;
use linkme::distributed_slice;
use std::sync::Arc;

mod blockio;
mod cpu;
mod gpu;
mod memory;
mod network;
mod rezolus;
mod scheduler;
mod syscall;
mod tcp;

#[distributed_slice]
pub static SAMPLERS: [fn(config: Arc<Config>) -> SamplerResult] = [..];

#[allow(dead_code)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Fast samplers with deterministic latency
    High,
    /// Normal samplers
    #[default]
    Medium,
    /// Slow samplers or ones with highly variable latency
    Low,
}

#[async_trait]
pub trait Sampler: Send + Sync {
    async fn refresh(&self);

    fn priority(&self) -> Priority {
        Priority::default()
    }
}

pub type SamplerResult = anyhow::Result<Option<Box<dyn Sampler>>>;
