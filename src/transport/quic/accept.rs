use std::io::{Result, Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;

use log::debug;
use async_trait::async_trait;

use quinn::Endpoint;

use super::QuicStream;
use crate::utils::CommonAddr;
use crate::transport::{AsyncConnect, AsyncAccept, Transport};

pub struct Acceptor<C> {
    #[allow(dead_code)]
    cc: Arc<C>,
    lis: Endpoint,
    #[allow(dead_code)]
    addr: CommonAddr,
}

impl<C> Acceptor<C> {
    pub fn new(cc: Arc<C>, lis: Endpoint, addr: CommonAddr) -> Self {
        Acceptor { cc, lis, addr }
    }
}

async fn accept_quic_conn(lis: &Endpoint) -> Result<(QuicStream, SocketAddr)> {
    let connecting = lis.accept().await.ok_or_else(|| {
        Error::new(ErrorKind::ConnectionAborted, "connection abort")
    })?;

    let new_conn = match connecting.into_0rtt() {
        Ok((new_conn, _)) => new_conn,
        Err(connecting) => connecting.await
            .map_err(|e| Error::new(ErrorKind::ConnectionRefused, e))?,
    };

    let (send, recv) = new_conn.accept_bi().await
        .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

    debug!("quic accept[new] <- {}", &new_conn.remote_address());
    Ok((QuicStream::new(send, recv), new_conn.remote_address()))
}

// Single Connection
#[async_trait]
impl AsyncAccept for Acceptor<()> {
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    type Base = QuicStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        accept_quic_conn(&self.lis).await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}

// Mux
#[async_trait]
impl<C> AsyncAccept for Acceptor<C>
where
    C: AsyncConnect + 'static,
{
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    type Base = QuicStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        accept_quic_conn(&self.lis).await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}

// Raw Acceptor, used to setup the Quic Acceptor above
pub struct RawAcceptor {
    lis: Endpoint,
    addr: CommonAddr,
}

impl RawAcceptor {
    pub fn new(lis: Endpoint, addr: CommonAddr) -> Self {
        RawAcceptor { lis, addr }
    }
    pub fn set_connector<C>(self, cc: Arc<C>) -> Acceptor<C> {
        Acceptor::new(cc, self.lis, self.addr)
    }
}

#[async_trait]
impl AsyncAccept for RawAcceptor {
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    type Base = QuicStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        accept_quic_conn(&self.lis).await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}
