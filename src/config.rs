#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    pub bind_addr: std::net::SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        Self {
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8621),
        }
    }
}
