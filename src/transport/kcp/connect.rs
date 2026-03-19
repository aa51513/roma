use std::io::Result;
use std::net::SocketAddr;

use log::debug;
use async_trait::async_trait;

use kcp_tokio::{KcpConfig, KcpStream};

use super::KcpStreamWrapper;
use crate::transport::{AsyncConnect, Transport};
use crate::dns;
use crate::utils::CommonAddr;

pub struct Connector {
    addr: CommonAddr,
}

impl Connector {
    pub fn new(addr: CommonAddr) -> Self {
        Connector { addr }
    }
}

#[async_trait]
impl AsyncConnect for Connector {
    const TRANS: Transport = Transport::KCP;

    const SCHEME: &'static str = "kcp";

    type IO = KcpStreamWrapper;

    fn addr(&self) -> &CommonAddr { &self.addr }

    fn clear_reuse(&self) {}

    async fn connect(&self) -> Result<Self::IO> {
        let connect_addr = match &self.addr {
            CommonAddr::SocketAddr(sockaddr) => *sockaddr,
            CommonAddr::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                SocketAddr::new(ip, *port)
            }
            #[cfg(all(unix, feature = "uds"))]
            CommonAddr::UnixSocketPath(_) => unreachable!(),
        };
        debug!("kcp connect -> {}", &connect_addr);
        let config = KcpConfig::new().fast_mode();
        let stream = KcpStream::connect(connect_addr, config).await?;
        Ok(KcpStreamWrapper::new(stream))
    }
}
