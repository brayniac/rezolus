use crate::*;

const NAME: &str = "filesystem_descriptors";

mod procfs;

use procfs::*;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>, runtime: &Runtime) {
    runtime.spawn(async {
        if let Ok(mut s) = Procfs::init(config) {
            loop {
                s.sample().await;
            }
        }
    });
}
