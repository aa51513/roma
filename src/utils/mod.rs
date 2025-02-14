use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[macro_use]
pub mod macros;
pub use must;

pub mod consts;
pub use consts::*;

pub mod types;
pub use types::{CommonAddr, MaybeQuic};

#[cfg(feature = "tls")]
pub mod cert;
#[cfg(feature = "tls")]
pub use cert::{load_certs, load_keys, generate_cert_key};

#[allow(dead_code)]
#[inline]
pub fn empty_sockaddr_v4() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0)
}

#[allow(dead_code)]
#[inline]
pub fn empty_sockaddr_v6() -> SocketAddr {
    SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0)
}
