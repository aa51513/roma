use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::io::{Result, Error, ErrorKind};

use futures::ready;
use bytes::{Bytes, BytesMut, BufMut};

use tokio::io::{AsyncRead, AsyncWrite};
use h2::{SendStream, RecvStream};

use crate::utils::H2_BUF_SIZE;

pub struct GrpcStream {
    recv: RecvStream,
    send: SendStream<Bytes>,
    buffer: BytesMut,
}

impl GrpcStream {
    #[inline]
    pub fn new(recv: RecvStream, send: SendStream<Bytes>) -> Self {
        GrpcStream {
            recv,
            send,
            buffer: BytesMut::with_capacity(H2_BUF_SIZE),
        }
    }
}

impl AsyncRead for GrpcStream {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        if !self.buffer.is_empty() {
            let to_read = min(buf.remaining(), self.buffer.len());
            let data = self.buffer.split_to(to_read);
            buf.put_slice(&data[..to_read]);
            return Poll::Ready(Ok(()));
        };
        Poll::Ready(match ready!(self.recv.poll_data(cx)) {
            Some(Ok(data)) => {
                // Skip gRPC frame header (5 bytes: 1 byte flag + 4 bytes length)
                if data.len() > 5 {
                    let payload = &data[5..];
                    let to_read = min(buf.remaining(), payload.len());
                    buf.put_slice(&payload[..to_read]);
                    // copy the left payload into buffer
                    if payload.len() > to_read {
                        self.buffer.extend_from_slice(&payload[to_read..]);
                    };
                }
                // increase recv window
                self.recv
                    .flow_control()
                    .release_capacity(data.len())
                    .map_or_else(
                        |e| Err(Error::new(ErrorKind::ConnectionReset, e)),
                        |_| Ok(()),
                    )
            }
            // no more data frames
            _ => Ok(()),
        })
    }
}

impl AsyncWrite for GrpcStream {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        // Create gRPC frame: 1 byte flag (0) + 4 bytes length + data
        let frame_len = 5 + buf.len();
        self.send.reserve_capacity(frame_len);

        Poll::Ready(match ready!(self.send.poll_capacity(cx)) {
            Some(Ok(_)) => {
                let mut frame = BytesMut::with_capacity(frame_len);
                frame.put_u8(0); // flag: no compression
                frame.put_u32(buf.len() as u32); // length
                frame.extend_from_slice(buf);

                self.send.send_data(frame.freeze(), false).map_or_else(
                    |e| Err(Error::new(ErrorKind::BrokenPipe, e)),
                    |_| Ok(buf.len()),
                )
            }
            _ => Err(Error::new(ErrorKind::BrokenPipe, "broken pipe")),
        })
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        self.send.reserve_capacity(0);
        Poll::Ready(ready!(self.send.poll_capacity(cx)).map_or(
            Err(Error::new(ErrorKind::BrokenPipe, "broken pipe")),
            |_| {
                self.send.send_data(Bytes::new(), true).map_or_else(
                    |e| Err(Error::new(ErrorKind::BrokenPipe, e)),
                    |_| Ok(()),
                )
            },
        ))
    }
}
