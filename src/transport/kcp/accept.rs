use std::io::Result;
use std::net::SocketAddr;

use log::debug;
use async_trait::async_trait;

use kcp_tokio::{KcpConfig, KcpListener, KcpStream};

use super::KcpStreamWrapper;
use crate::utils::CommonAddr;
use crate::transport::{AsyncAccept, Transport};

pub struct Acceptor {
    listener: KcpListener,
    addr: CommonAddr,
}

impl Acceptor {
    pub async fn new(addr: CommonAddr) -> Result<Self> {
        let bind_addr = match &addr {
            CommonAddr::SocketAddr(sockaddr) => *sockaddr,
            CommonAddr::DomainName(_, _) => unreachable!(),
            #[cfg(all(unix, feature = "uds"))]
            CommonAddr::UnixSocketPath(_) => unreachable!(),
        };
        debug!("kcp bind {}", &bind_addr);
        let config = KcpConfig::new().fast_mode();
        let listener = KcpListener::bind(bind_addr, config).await?;
        Ok(Acceptor { listener, addr })
    }
}

#[async_trait]
impl AsyncAccept for Acceptor {
    const TRANS: Transport = Transport::KCP;

    const SCHEME: &'static str = "kcp";

    type IO = KcpStreamWrapper;

    type Base = KcpStreamWrapper;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        let (stream, peer_addr) = self.listener.accept().await?;
        debug!("kcp accept {} <- {}", &self.addr, &peer_addr);
        Ok((KcpStreamWrapper::new(stream), peer_addr))
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        Ok(base)
    }
}
