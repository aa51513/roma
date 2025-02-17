use std::pin::Pin;
use std::task::{Context, Poll};
use std::io::Result;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
#[cfg(all(unix, feature = "uds"))]
use tokio::net::UnixStream;

use crate::transport::IOStream;

#[allow(clippy::upper_case_acronyms)]
pub enum PlainStream {
    TCP(TcpStream),
    #[cfg(all(unix, feature = "uds"))]
    UDS(UnixStream),
}

pub struct ReadHalf {
    inner: Box<dyn AsyncRead + Unpin + Send + Sync>,
}

pub struct WriteHalf {
    inner: Box<dyn AsyncWrite + Unpin + Send + Sync>,
}

impl IOStream for PlainStream {}

#[cfg(unix)]
impl AsRawFd for PlainStream {
    fn as_raw_fd(&self) -> RawFd {
        match self {
            Self::TCP(x) => x.as_raw_fd(),
            #[cfg(feature = "uds")]
            Self::UDS(x) => x.as_raw_fd(),
        }
    }
}

impl PlainStream {
    pub fn set_no_delay(&self, nodelay: bool) -> Result<()> {
        match self {
            Self::TCP(x) => x.set_nodelay(nodelay),
            #[cfg(all(unix, feature = "uds"))]
            _ => Ok(()),
        }
    }
}

// 实现 AsyncRead 和 AsyncWrite 以支持 Tokio 的 split 方法
impl AsyncRead for PlainStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        match self.get_mut() {
            Self::TCP(stream) => Pin::new(stream).poll_read(cx, buf),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for PlainStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        match self.get_mut() {
            Self::TCP(stream) => Pin::new(stream).poll_write(cx, buf),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match self.get_mut() {
            Self::TCP(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match self.get_mut() {
            Self::TCP(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl AsyncRead for ReadHalf {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for WriteHalf {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}


#[cfg(target_os = "linux")]
pub use linux_ext::*;

#[cfg(target_os = "linux")]
pub mod linux_ext {
    use super::*;
    use std::io::{Error, ErrorKind};
    use tokio::io::Interest;

    #[inline]
    pub fn split(x: &mut PlainStream) -> (ReadHalf, WriteHalf) {
        (ReadHalf(&*x), WriteHalf(&*x))
    }

    // tokio >= 1.9.0
    #[inline]
    pub fn try_io<R>(
        x: &PlainStream,
        interest: Interest,
        f: impl FnOnce() -> Result<R>,
    ) -> Result<R> {
        match x {
            PlainStream::TCP(x) => x.try_io(interest, f),
            #[cfg(feature = "uds")]
            PlainStream::UDS(x) => x.try_io(interest, f),
        }
    }

    #[inline]
    pub async fn readable(x: &PlainStream) -> Result<()> {
        match x {
            PlainStream::TCP(x) => x.readable().await,
            #[cfg(feature = "uds")]
            PlainStream::UDS(x) => x.readable().await,
        }
    }

    #[inline]
    pub async fn writable(x: &PlainStream) -> Result<()> {
        match x {
            PlainStream::TCP(x) => x.writable().await,
            #[cfg(feature = "uds")]
            PlainStream::UDS(x) => x.writable().await,
        }
    }

    #[inline]
    pub fn clear_readiness(x: &PlainStream, interest: Interest) {
        let _ = try_io(x, interest, || {
            Err(Error::new(ErrorKind::WouldBlock, "")) as Result<()>
        });
    }
}