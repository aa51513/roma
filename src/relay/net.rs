use std::io;
use log::{info, debug};
use tokio::task::JoinHandle;

use super::common;
use super::transport;
use crate::utils::{must, MaybeQuic};
use crate::config::{EpHalfConfig, NetConfig};
use crate::transport::AsyncConnect;
use crate::transport::plain::{self, PlainListener};

// ===== TCP or UDS =====
pub fn new_plain_conn(addr: &str, net: &NetConfig) -> plain::Connector {
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use crate::utils::CommonAddr;
    match net {
        NetConfig::TCP => {
            let (sockaddr, _) = must!(common::parse_socket_addr(addr, true));
            plain::Connector::new(sockaddr)
        }
        #[cfg(all(unix, feature = "uds"))]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            plain::Connector::new(path)
        }
        _ => unreachable!(),
    }
}

pub fn new_plain_lis(addr: &str, net: &NetConfig) -> plain::Acceptor {
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use crate::utils::CommonAddr;
    match net {
        NetConfig::TCP => {
            let (sockaddr, _) = must!(common::parse_socket_addr(addr, false));
            let lis =
                must!(PlainListener::bind(&sockaddr), "bind {}", &sockaddr);
            info!("bind {}[tcp]", &sockaddr);
            plain::Acceptor::new(lis, sockaddr)
        }
        #[cfg(all(unix, feature = "uds"))]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            let lis = must!(PlainListener::bind(&path), "bind {}", &path);
            info!("bind {}[uds]", &path);
            plain::Acceptor::new(lis, path)
        }
        _ => unreachable!(),
    }
}

// ===== UDP =====
#[cfg(feature = "udp")]
use udp_ext::*;
#[cfg(feature = "udp")]
pub mod udp_ext {
    use super::*;
    use futures::executor::block_on;
    use tokio::net::UdpSocket;
    use crate::transport::udp;
    use crate::utils::CommonAddr::*;

    pub fn new_udp_conn(addr: &str, _: &NetConfig) -> udp::Connector {
        let (sockaddr, _) = must!(common::parse_socket_addr(addr, true));
        udp::Connector::new(sockaddr)
    }

    #[cfg(feature = "udp")]
    pub fn new_udp_lis(addr: &str, _: &NetConfig) -> udp::Acceptor {
        let (sockaddr, _) = must!(common::parse_socket_addr(addr, false));
        let socket = match sockaddr {
            SocketAddr(sockaddr) => {
                block_on(UdpSocket::bind(sockaddr)).unwrap()
            }
            _ => unreachable!(),
        };
        udp::Acceptor::new(socket, sockaddr)
    }
}

// ===== QUIC =====
#[cfg(feature = "quic")]
use quic_ext::*;
#[cfg(feature = "quic")]
pub mod quic_ext {
    use std::fs;
    use std::io::BufReader;
    use super::*;
    use quinn::{Endpoint, ClientConfig, ServerConfig};
    use crate::utils;
    use crate::transport::quic;
    use crate::config::{TransportConfig, TLSConfig};

    pub fn new_quic_conn(
        addr: &str,
        _: &NetConfig,
        trans: &TransportConfig,
        tlsc: &TLSConfig,
    ) -> quic::Connector {
        // check transport
        let trans = match trans {
            TransportConfig::QUIC(x) => x,
            _ => unreachable!(),
        };
        // check tls
        let tlsc = match tlsc {
            TLSConfig::Client(x) => x,
            _ => unreachable!(),
        };

        let (sockaddr, is_ipv6) = must!(common::parse_socket_addr(addr, true));
        let mut client_tls = tlsc.to_tls();
        let sni = tlsc.set_sni(&mut client_tls, &sockaddr);

        let client_config = ClientConfig::with_native_roots();
        // default:
        // set ciphersuits = QUIC_CIPHER_SUITES
        // set versions = TLSv1_3
        // set enable_early_data = true
        // client_tls.ciphersuites = client_config.crypto.ciphersuites.clone();
        // client_tls.versions = client_config.crypto.versions.clone();
        // client_tls.enable_early_data = client_config.crypto.enable_early_data;

        //client_config.crypto = Arc::new(client_tls);

        let bind_addr = if is_ipv6 {
            utils::empty_sockaddr_v6()
        } else {
            utils::empty_sockaddr_v4()
        };

        //let mut builder = Endpoint::builder();
        //builder.default_client_config(client_config);
        //let (ep, _) = must!(builder.bind(&bind_addr), "bind {}", &bind_addr);

        let mut ep: Endpoint =
            Endpoint::client(bind_addr).expect("address error");
        ep.set_default_client_config(client_config);
        quic::Connector::new(ep, sockaddr, sni, trans.mux)
    }

    fn load_certs(filename: &str) -> Vec<rustls::Certificate> {
        let certfile =
            fs::File::open(filename).expect("cannot open certificate file");
        let mut reader = BufReader::new(certfile);
        rustls_pemfile::certs(&mut reader)
            .unwrap()
            .iter()
            .map(|v| rustls::Certificate(v.clone()))
            .collect()
    }

    fn load_private_key(filename: &str) -> rustls::PrivateKey {
        let keyfile =
            fs::File::open(filename).expect("cannot open private key file");
        let mut reader = BufReader::new(keyfile);

        loop {
            match rustls_pemfile::read_one(&mut reader)
                .expect("cannot parse private key .pem file")
            {
                Some(rustls_pemfile::Item::RSAKey(key)) => {
                    return rustls::PrivateKey(key)
                }
                Some(rustls_pemfile::Item::PKCS8Key(key)) => {
                    return rustls::PrivateKey(key)
                }
                Some(rustls_pemfile::Item::ECKey(key)) => {
                    return rustls::PrivateKey(key)
                }
                None => break,
                _ => {}
            }
        }

        panic!(
            "no keys found in {:?} (encrypted keys not supported)",
            filename
        );
    }

    pub fn new_quic_raw_lis(
        addr: &str,
        _: &NetConfig,
        trans: &TransportConfig,
        tlsc: &TLSConfig,
    ) -> quic::RawAcceptor {
        // check transport
        match trans {
            TransportConfig::QUIC(x) => x,
            _ => unreachable!(),
        };
        // check tls
        let tlsc = match tlsc {
            TLSConfig::Server(x) => x,
            _ => unreachable!(),
        };

        let (sockaddr, _) = must!(common::parse_socket_addr(addr, false));
        let bind_addr = match sockaddr {
            utils::CommonAddr::SocketAddr(ref x) => x,
            _ => unreachable!(),
        };

        let certs: Vec<rustls::Certificate> = load_certs(&tlsc.cert);
        let key: rustls::PrivateKey = load_private_key(&tlsc.key);

        let server_config =
            ServerConfig::with_single_cert(certs, key).expect("bad cert file");
        let (_, incoming) = Endpoint::server(server_config, *bind_addr)
            .expect("failed to bind");
        info!("bind {}[quic]", &bind_addr);
        quic::RawAcceptor::new(incoming, sockaddr)
    }
}

// ===== KCP =====
#[cfg(feature = "kcp")]
use kcp_ext::*;
#[cfg(feature = "kcp")]
pub mod kcp_ext {
    use super::*;
    use futures::executor::block_on;
    use crate::transport::kcp;
    use crate::utils::CommonAddr::*;

    pub fn new_kcp_conn(addr: &str, _: &NetConfig) -> kcp::Connector {
        let (sockaddr, _) = must!(common::parse_socket_addr(addr, true));
        kcp::Connector::new(sockaddr)
    }

    pub fn new_kcp_lis(addr: &str, _: &NetConfig) -> kcp::Acceptor {
        let (sockaddr, _) = must!(common::parse_socket_addr(addr, false));
        info!("bind {}[kcp]", &sockaddr);
        block_on(kcp::Acceptor::new(sockaddr)).unwrap()
    }
}

pub fn spawn_lis_half_with_net<C>(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
    conn: C,
) where
    C: AsyncConnect + 'static,
{
    use NetConfig::*;
    #[cfg(feature = "quic")]
    use crate::config::TransportConfig::QUIC;
    #[cfg(feature = "kcp")]
    use crate::config::TransportConfig::KCP;

    debug!("load listen network[{}]", &listen.net);

    match &listen.net {
        TCP => {
            let lis =
                MaybeQuic::Other(new_plain_lis(&listen.addr, &listen.net));
            transport::spawn_with_trans(workers, listen, remote, lis, conn)
        }
        #[cfg(all(unix, feature = "uds"))]
        UDS => {
            let lis =
                MaybeQuic::Other(new_plain_lis(&listen.addr, &listen.net));
            transport::spawn_with_trans(workers, listen, remote, lis, conn)
        }
        #[cfg(feature = "quic")]
        UDP if matches!(listen.trans, QUIC(_)) => {
            use crate::transport::quic;
            let lis = MaybeQuic::<quic::RawAcceptor>::Quic(new_quic_raw_lis(
                &listen.addr,
                &listen.net,
                &listen.trans,
                &listen.tls,
            ));
            transport::spawn_with_trans(workers, listen, remote, lis, conn)
        }
        #[cfg(feature = "kcp")]
        UDP if matches!(listen.trans, KCP(_)) => {
            let lis = MaybeQuic::Other(new_kcp_lis(&listen.addr, &listen.net));
            transport::spawn_with_trans(workers, listen, remote, lis, conn)
        }
        #[cfg(feature = "udp")]
        UDP => {
            let lis = MaybeQuic::Other(new_udp_lis(&listen.addr, &listen.net));
            transport::spawn_with_trans(workers, listen, remote, lis, conn)
        }
    }
}

pub fn spawn_conn_half_with_net(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
) {
    use NetConfig::*;
    #[cfg(feature = "quic")]
    use crate::config::TransportConfig::QUIC;
    #[cfg(feature = "kcp")]
    use crate::config::TransportConfig::KCP;

    debug!("load remote network[{}]", &remote.net);

    match &remote.net {
        TCP => {
            let conn = new_plain_conn(&remote.addr, &remote.net);
            spawn_lis_half_with_net(workers, listen, remote, conn)
        }
        #[cfg(all(unix, feature = "uds"))]
        UDS => {
            let conn = new_plain_conn(&remote.addr, &remote.net);
            spawn_lis_half_with_net(workers, listen, remote, conn)
        }
        #[cfg(feature = "quic")]
        UDP if matches!(&remote.trans, QUIC(_)) => {
            let conn = new_quic_conn(
                &remote.addr,
                &remote.net,
                &remote.trans,
                &remote.tls,
            );
            spawn_lis_half_with_net(workers, listen, remote, conn)
        }
        #[cfg(feature = "kcp")]
        UDP if matches!(&remote.trans, KCP(_)) => {
            let conn = new_kcp_conn(&remote.addr, &remote.net);
            spawn_lis_half_with_net(workers, listen, remote, conn)
        }
        #[cfg(feature = "udp")]
        UDP => {
            let conn = new_udp_conn(&remote.addr, &remote.net);
            spawn_lis_half_with_net(workers, listen, remote, conn)
        }
    }
}

pub fn spawn_with_net(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
) {
    spawn_conn_half_with_net(workers, listen, remote)
}
