use std::net::SocketAddr;
use std::io::{Result, Error, ErrorKind};
use std::sync::Arc;

use log::{debug, info, warn};
use async_trait::async_trait;
use bytes::Bytes;
use http::{StatusCode, Request, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use h2::RecvStream;
use h2::server::{self, SendResponse};

use super::GrpcStream;
use crate::utils::CommonAddr;
use crate::transport::{AsyncConnect, AsyncAccept, Transport};

pub struct Acceptor<L: AsyncAccept, C> {
    cc: Arc<C>,
    lis: L,
    path: String,
}

impl<L, C> Acceptor<L, C>
where
    L: AsyncAccept,
{
    pub fn new(cc: Arc<C>, lis: L, path: String) -> Self {
        Acceptor { cc, lis, path }
    }
}

// Single Connection
#[async_trait]
impl<L> AsyncAccept for Acceptor<L, ()>
where
    L: AsyncAccept,
{
    const TRANS: Transport = Transport::GRPC;

    const SCHEME: &'static str = "grpc";

    type IO = GrpcStream;

    type Base = L::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        debug!("grpc accept[new] <-");

        // establish a new h2 connection
        let mut conn = server::handshake(stream)
            .await
            .map_err(|e| Error::new(ErrorKind::ConnectionAborted, e))?;

        // accept a new stream
        let (request, response) = conn
            .accept()
            .await
            .ok_or_else(|| Error::new(ErrorKind::ConnectionAborted, "connection closed"))?
            .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

        // handle the gRPC request
        let grpc_stream = handle_request(&self.path, request, response).await?;

        Ok(grpc_stream)
    }
}

// Mux
#[async_trait]
impl<L, C> AsyncAccept for Acceptor<L, C>
where
    L: AsyncAccept,
    C: AsyncConnect + 'static,
{
    const TRANS: Transport = Transport::GRPC;

    const SCHEME: &'static str = "grpc";

    type IO = GrpcStream;

    type Base = L::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        debug!("grpc accept[new] <-");

        // establish a new h2 connection
        let mut conn = server::handshake(stream)
            .await
            .map_err(|e| Error::new(ErrorKind::ConnectionAborted, e))?;

        // accept a new stream
        let (request, response) = conn
            .accept()
            .await
            .ok_or_else(|| Error::new(ErrorKind::ConnectionAborted, "connection closed"))?
            .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

        // handle the gRPC request
        let grpc_stream = handle_request(&self.path, request, response).await?;

        // handle next mux requests
        tokio::spawn(handle_mux_conn(self.cc.clone(), conn, self.path.clone()));

        Ok(grpc_stream)
    }
}

async fn handle_request(
    path: &str,
    request: Request<RecvStream>,
    mut response: SendResponse<Bytes>,
) -> Result<GrpcStream> {
    // check request path
    if request.uri().path() != path {
        debug!("grpc check request path -- not found");
        let _ = response.send_response(
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(())
                .unwrap(),
            true,
        );
        return Err(Error::new(ErrorKind::NotFound, "invalid path"));
    }
    debug!("grpc check request path -- ok");

    // get recv stream from request body
    let (_, recv) = request.into_parts();

    // respond with OK status for gRPC
    let send = response
        .send_response(
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/grpc")
                .body(())
                .unwrap(),
            false,
        )
        .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

    Ok(GrpcStream::new(recv, send))
}

async fn handle_mux_conn<C, IO>(
    cc: Arc<C>,
    mut conn: server::Connection<IO, Bytes>,
    path: String,
) where
    C: AsyncConnect + 'static,
    IO: AsyncRead + AsyncWrite + Unpin,
{
    use crate::io::bidi_copy_with_stream;

    loop {
        match conn.accept().await {
            Some(x) => match x {
                Ok((request, response)) => {
                    match handle_request(&path, request, response).await {
                        Ok(stream) => {
                            info!(
                                "new grpc stream[reuse] <-> {}[{}]",
                                cc.addr(),
                                C::SCHEME
                            );
                            tokio::spawn(bidi_copy_with_stream(cc.clone(), stream));
                        }
                        Err(e) => debug!("failed to resolve grpc-mux stream, {}", e),
                    }
                }
                Err(e) => {
                    warn!("failed to recv grpc-mux response, {}", e);
                    return;
                }
            },
            None => {
                warn!("no more grpc-mux stream");
                return;
            }
        }
    }
}
