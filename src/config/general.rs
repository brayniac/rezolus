use super::*;

#[derive(Deserialize)]
pub struct General {
    listen: String,
    #[serde(default = "disabled")]
    compression: bool,
}

impl General {
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

    pub fn compression(&self) -> bool {
        self.compression
    }
}
