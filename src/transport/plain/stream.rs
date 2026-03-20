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

pub struct ReadHalf<'a>(*const PlainStream, std::marker::PhantomData<&'a PlainStream>);

pub struct WriteHalf<'a>(*mut PlainStream, std::marker::PhantomData<&'a mut PlainStream>);

unsafe impl Send for ReadHalf<'_> {}
unsafe impl Sync for ReadHalf<'_> {}
unsafe impl Send for WriteHalf<'_> {}
unsafe impl Sync for WriteHalf<'_> {}

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

impl AsRef<PlainStream> for ReadHalf<'_> {
    fn as_ref(&self) -> &PlainStream { unsafe { &*self.0 } }
}

impl AsRef<PlainStream> for WriteHalf<'_> {
    fn as_ref(&self) -> &PlainStream { unsafe { &*self.0 } }
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

#[cfg(target_os = "linux")]
#[allow(unused_imports)]
pub use linux_ext::*;

#[cfg(target_os = "linux")]
pub mod linux_ext {
    use super::*;
    use std::io::{Error, ErrorKind};
    use tokio::io::Interest;

    #[inline]
    pub fn split(x: &mut PlainStream) -> (ReadHalf<'_>, WriteHalf<'_>) {
        let ptr = x as *mut PlainStream;
        (
            ReadHalf(ptr, std::marker::PhantomData),
            WriteHalf(ptr, std::marker::PhantomData),
        )
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

impl AsyncRead for PlainStream {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_read(cx, buf),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(x) => Pin::new(x).poll_read(cx, buf),
        }
    }
}

impl AsyncRead for ReadHalf<'_> {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let stream = unsafe { &mut *(self.get_mut().0 as *mut PlainStream) };
        Pin::new(stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for PlainStream {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_write(cx, buf),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(x) => Pin::new(x).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_flush(cx),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(x) => Pin::new(x).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_shutdown(cx),
            #[cfg(all(unix, feature = "uds"))]
            Self::UDS(x) => Pin::new(x).poll_shutdown(cx),
        }
    }
}

impl AsyncWrite for WriteHalf<'_> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        let stream = unsafe { &mut *self.get_mut().0 };
        Pin::new(stream).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        let stream = unsafe { &mut *self.get_mut().0 };
        Pin::new(stream).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        let stream = unsafe { &mut *self.get_mut().0 };
        Pin::new(stream).poll_shutdown(cx)
    }
}
