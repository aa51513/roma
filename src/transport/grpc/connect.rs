use std::io::{Result, Error, ErrorKind};
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::{trace, debug};
use async_trait::async_trait;
use bytes::Bytes;
use http::{Uri, Version, Request};

use h2::client::{self, SendRequest};

use super::GrpcStream;
use crate::transport::{AsyncConnect, Transport};
use crate::utils::CommonAddr;

pub struct Connector<T: AsyncConnect> {
    cc: T,
    uri: Uri,
    max_concurrent: usize,
    count: AtomicUsize,
    channel: RwLock<Option<SendRequest<Bytes>>>,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(
        cc: T,
        path: String,
        max_concurrent: usize,
    ) -> Self {
        let authority = cc.addr().to_string();
        let max_concurrent = if max_concurrent == 0 {
            1000
        } else {
            max_concurrent
        };
        Connector {
            cc,
            uri: Uri::builder()
                .scheme("http")
                .authority(authority.as_str())
                .path_and_query(path)
                .build()
                .unwrap(),
            max_concurrent,
            count: AtomicUsize::new(1),
            channel: RwLock::new(None),
        }
    }
}

#[async_trait]
impl<T: AsyncConnect> AsyncConnect for Connector<T> {
    const TRANS: Transport = Transport::GRPC;

    const SCHEME: &'static str = "grpc";

    type IO = GrpcStream;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    #[inline]
    fn clear_reuse(&self) { *self.channel.write().unwrap() = None }

    async fn connect(&self) -> Result<Self::IO> {
        let mut client = new_client(self).await?;

        // request with a send stream
        let (response, send) = client
            .send_request(
                Request::builder()
                    .uri(&self.uri)
                    .version(Version::HTTP_2)
                    .header("content-type", "application/grpc")
                    .body(())
                    .unwrap(),
                false,
            )
            .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

        // get recv stream from response body
        let recv = response
            .await
            .map_err(|e| Error::new(ErrorKind::Interrupted, e))?
            .into_body();

        Ok(GrpcStream::new(recv, send))
    }
}

async fn new_client<T: AsyncConnect>(
    cc: &Connector<T>,
) -> Result<SendRequest<Bytes>> {
    // reuse existed connection
    trace!("grpc init new client");
    let channel = (*cc.channel.read().unwrap()).clone();
    if let Some(channel) = channel {
        let count = cc.count.load(Ordering::Relaxed);
        trace!("grpc reusable, current mux = {}", count);
        if count < cc.max_concurrent {
            if let Ok(client) = channel.ready().await {
                debug!("grpc connect[reuse {}] ->", count);
                cc.count.fetch_add(1, Ordering::Relaxed);
                return Ok(client);
            };
        };
    };

    // establish a new connection
    let stream = cc.cc.connect().await?;
    debug!("grpc connect[new] ->");

    let (client, conn) = client::Builder::new()
        .handshake(stream)
        .await
        .map_err(|e| Error::new(ErrorKind::ConnectionRefused, e))?;

    // store connection
    // may have conflicts
    cc.count.store(1, Ordering::Relaxed);
    *cc.channel.write().unwrap() = Some(client.clone());
    tokio::spawn(conn);

    client
        .ready()
        .await
        .map_err(|e| Error::new(ErrorKind::Interrupted, e))
}
