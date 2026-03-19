use std::io::{Result, Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;

use log::{debug};
use async_trait::async_trait;

use quinn::{Endpoint};

use super::QuicStream;
use crate::utils::{self, CommonAddr};
use crate::transport::{AsyncConnect, AsyncAccept, Transport};

pub struct Acceptor<C> {
    cc: Arc<C>,
    lis: Endpoint,
    addr: CommonAddr,
}

impl<C> Acceptor<C> {
    pub fn new(cc: Arc<C>, lis: Endpoint, addr: CommonAddr) -> Self {
        Acceptor { cc, lis, addr }
    }
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
        // new connection
        let lis = unsafe { utils::const_cast(&self.lis) };
        let connecting = lis.accept().await.ok_or_else(|| {
            Error::new(ErrorKind::ConnectionAborted, "connection abort")
        })?;

        // early data
        let new_conn = match connecting.into_0rtt() {
            Ok((new_conn, _)) => new_conn,
            Err(connecting) => connecting.await?,
        };

        let new_quin_conn = new_conn.accept_bi().await;

        return match new_quin_conn {
            Ok((send, recv)) => {
                debug!("quic accept[new] <- {}", &new_conn.remote_address());
                Ok((QuicStream::new(send, recv), new_conn.remote_address()))
            }
            Err(err) => { Err(Error::new(ErrorKind::Interrupted, err)) }
        }
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
        // new connection
        let lis = unsafe { utils::const_cast(&self.lis) };
        let connecting = lis.accept().await.ok_or_else(|| {
            Error::new(ErrorKind::ConnectionAborted, "connection abort")
        })?;

        // early data
        let new_conn = match connecting.into_0rtt() {
            Ok((new_conn, _)) => new_conn,
            Err(connecting) => connecting.await?,
        };

        let new_quic_conn = new_conn.accept_bi().await;
        match new_quic_conn{
            Ok((send, recv)) => {
                debug!("quic accept[new] <- {}", &new_conn.remote_address());
                Ok((QuicStream::new(send, recv), new_conn.remote_address()))
            }
            Err(err) => {
                Err(Error::new(ErrorKind::Interrupted, err))
            }
        }
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
        // new connection
        let lis = unsafe { utils::const_cast(&self.lis) };
        let connecting = lis.accept().await.ok_or_else(|| {
            Error::new(ErrorKind::ConnectionAborted, "connection abort")
        })?;

        // early data
        let new_conn = match connecting.into_0rtt() {
            Ok((new_conn, _)) => new_conn,
            Err(connecting) => connecting.await?,
        };

        let new_quic_conn = new_conn.accept_bi().await;
        match new_quic_conn {
            Ok((send, recv)) => {
                Ok((QuicStream::new(send, recv), new_conn.remote_address()))
            }
            Err(err) => {
                Err(Error::new(ErrorKind::Interrupted, err))
            }
        }
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}
