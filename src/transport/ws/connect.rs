use std::borrow::Borrow;
use std::io::{Error, ErrorKind, Result};

use log::debug;
use http::Uri;
use async_trait::async_trait;

use tokio_tungstenite::tungstenite;
use tungstenite::protocol::WebSocketConfig;

use super::WebSocketStream;
use crate::transport::{AsyncConnect, Transport};

use crate::utils::CommonAddr;

struct Request<'a> {
    pub uri: &'a Uri,
    pub host: &'a str,
}

impl tungstenite::client::IntoClientRequest for Request<'_> {
    fn into_client_request(
        self,
    ) -> tungstenite::error::Result<tungstenite::handshake::client::Request> {
        let builder = http::Request::builder()
            .method("GET")
            .uri(self.uri)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/109.0")
            .header("Host", self.host);
        Ok(builder.body(())?)
    }
}

pub struct Connector<T: AsyncConnect> {
    cc: T,
    uri: Uri,
    host: String,
    config: Option<WebSocketConfig>,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(cc: T, path: String,host: String) -> Self {
        let default_authority = cc.addr().to_string();
        let authority = if String::is_empty(&host) { default_authority } else { <std::string::String as Borrow<str>>::borrow(&host).to_string() };
        let uri = Uri::builder()
            .scheme(Self::SCHEME)
            .authority(authority)
            .path_and_query(path)
            .build()
            .unwrap().clone();
        Connector {
            cc,
            uri,
            host,
            config: None,
        }
    }
}

#[async_trait]
impl<T: AsyncConnect> AsyncConnect for Connector<T> {
    const TRANS: Transport = Transport::WS;

    const SCHEME: &'static str = match T::TRANS {
        Transport::TLS => "wss",
        _ => "ws",
    };

    type IO = WebSocketStream<T::IO>;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    fn clear_reuse(&self) {}

    async fn connect(&self) -> Result<Self::IO> {
        let stream = self.cc.connect().await?;
        debug!("ws connect ->");
        let request = Request {
            uri: &self.uri,
            host: &self.host,
        };
        tokio_tungstenite::client_async_with_config(
            request,
            stream,
            self.config,
        )
        .await
        .map_or_else(
            |e| Err(Error::new(ErrorKind::ConnectionRefused, e)),
            |(ws, _)| Ok(WebSocketStream::new(ws)),
        )
    }
}
