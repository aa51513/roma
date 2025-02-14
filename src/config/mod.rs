use std::fs;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

use crate::utils::must;
use crate::transport::{AsyncConnect, AsyncAccept};

pub mod dns;
pub mod ep;
pub mod net;
pub mod tls;
pub mod trans;
// re-export
pub use dns::DnsMode;
pub use dns::DnsServerConfig;
pub use net::NetConfig;
pub use tls::TLSConfig;
pub use trans::TransportConfig;
pub use ep::{EndpointConfig, EpHalfConfig};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub dns_mode: DnsMode,

    #[serde(default = "default_dns_servers")]
    pub dns_servers: Vec<DnsServerConfig>,

    pub endpoints: Vec<EndpointConfig>,
}

fn default_dns_servers() -> Vec<DnsServerConfig>{
    let mut vec: Vec<DnsServerConfig> = Vec::with_capacity(4);
    vec.push(DnsServerConfig{addr:"8.8.8.8:53".to_string(),..Default::default()});
    vec.push(DnsServerConfig{addr:"8.8.4.4:53".to_string(),..Default::default()});
    vec.push(DnsServerConfig{addr:"[2001:4860:4860::8888]:53".to_string(),..Default::default()});
    vec.push(DnsServerConfig{addr:"[2001:4860:4860::8844]:53".to_string(),..Default::default()});

    vec
}

impl GlobalConfig {
    pub fn from_config_file(file: &str) -> Self {
        let config = must!(fs::read_to_string(file), "load {}", file);
        must!(serde_json::from_str(&config), "parse json")
    }
}

pub trait WithTransport<L, C>
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    type Acceptor: AsyncAccept;
    type Connector: AsyncConnect;

    fn apply_to_lis(&self, lis: L) -> Self::Acceptor;
    fn apply_to_conn(&self, conn: C) -> Self::Connector;
    fn apply_to_lis_with_conn(&self, conn: Arc<C>, lis: L) -> Self::Acceptor;
}
