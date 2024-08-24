use crate::*;

const NAME: &str = "filesystem_descriptors";

mod procfs;

use procfs::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(s) = Procfs::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}
