use crate::config::*;

#[derive(Deserialize, Default)]
pub struct General {
    #[serde(default = "listen")]
    listen: String,
    #[serde(default)]
    btf_path: Option<String>,
}

impl General {
    pub fn check(&self) {}

    pub fn listen(&self) -> SocketAddr {
        self.listen
            .to_socket_addrs()
            .map_err(|e| {
                eprintln!("bad listen address: {e}");
                std::process::exit(1);
            })
            .unwrap()
            .next()
            .ok_or_else(|| {
                eprintln!("could not resolve socket addr");
                std::process::exit(1);
            })
            .unwrap()
    }

    pub fn btf_path(&self) -> Option<&str> {
        self.btf_path.as_deref()
    }
}
