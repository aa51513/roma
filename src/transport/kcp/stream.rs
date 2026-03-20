use std::pin::Pin;
use std::task::{Poll, Context};
use std::io::Result;
use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite};
use kcp_tokio::KcpStream;

pub struct KcpStreamWrapper {
    inner: KcpStream,
}

impl KcpStreamWrapper {
    pub fn new(stream: KcpStream) -> Self {
        KcpStreamWrapper { inner: stream }
    }

    pub fn peer_addr(&self) -> SocketAddr {
        *self.inner.peer_addr()
    }
}

impl AsyncRead for KcpStreamWrapper {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for KcpStreamWrapper {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

unsafe impl Send for KcpStreamWrapper {}
unsafe impl Sync for KcpStreamWrapper {}
