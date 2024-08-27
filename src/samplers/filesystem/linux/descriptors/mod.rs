use crate::*;

const NAME: &str = "filesystem_descriptors";

mod procfs;

use procfs::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    Procfs::init(config)
}
