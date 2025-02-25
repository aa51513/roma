use std::fmt::{Display, Formatter};
use std::time::SystemTime;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TLSConfig {
    None,
    #[cfg(feature = "tls")]
    Client(TLSClientConfig),
    #[cfg(feature = "tls")]
    Server(TLSServerConfig),
}

impl Default for TLSConfig {
    fn default() -> Self { Self::None }
}

impl Display for TLSConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use TLSConfig::*;
        match self {
            None => write!(f, "none"),
            #[cfg(feature = "tls")]
            Client(_) => write!(f, "rustls"),
            #[cfg(feature = "tls")]
            Server(_) => write!(f, "rustls"),
        }
    }
}

#[cfg(feature = "tls")]
pub use enable_tls::*;
#[cfg(feature = "tls")]
pub mod enable_tls {
    use super::*;
    use std::fs;
    use std::io::{BufReader, Read};
    use std::sync::Arc;

    use webpki::DnsNameRef;
    use rustls::{ClientConfig, ServerConfig};
    use rustls::internal::msgs::enums::ProtocolVersion;

    use crate::utils::{self, must, CommonAddr, NOT_A_DNS_NAME};
    use crate::transport::tls;
    use crate::transport::{AsyncConnect, AsyncAccept};

    // default values
    fn def_true() -> bool { true }

    fn def_roots_str() -> String { "firefox".to_string() }

    // TLS Client
    #[derive(Debug, Serialize, Deserialize)]
    pub struct TLSClientConfig {
        pub skip_verify: bool,

        #[serde(default = "def_true")]
        pub enable_sni: bool,

        #[serde(default)]
        pub enable_early_data: bool,

        #[serde(default)]
        pub sni: String,

        #[serde(default)]
        pub alpns: Vec<String>,

        // tlsv1.2, tlsv1.3
        #[serde(default)]
        pub versions: Vec<String>,

        // native, firefox, or provide a file
        #[serde(default = "def_roots_str")]
        pub roots: String,
    }

    struct ClientSkipVerify;

    impl rustls::client::ServerCertVerifier for ClientSkipVerify {
        fn verify_server_cert(
            &self,
            _: &rustls::Certificate,
            _: &[rustls::Certificate],
            _: &rustls::ServerName,
            _: &mut dyn Iterator<Item = &[u8]>,
            _: &[u8],
            _: SystemTime,
        ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::ServerCertVerified::assertion())
        }
    }

    impl TLSClientConfig {
        pub fn to_tls(&self) -> ClientConfig { make_client_config(self) }

        pub fn set_sni(
            &self,
            tlsc: &mut ClientConfig,
            addr: &CommonAddr,
        ) -> String {
            if !self.sni.is_empty() {
                return self.sni.clone();
            };
            let sni = addr.to_dns_name();
            if !sni.is_empty() {
                return sni;
            };
            tlsc.enable_sni = false;
            String::from(NOT_A_DNS_NAME)
        }

        pub fn apply_to_conn<C: AsyncConnect>(
            &self,
            conn: C,
        ) -> impl AsyncConnect {
            let mut tlsc = make_client_config(self);
            let sni = self.set_sni(&mut tlsc, conn.addr());
            let sni = DnsNameRef::try_from_ascii_str(&sni).unwrap().to_owned();
            tls::Connector::new(conn, sni, tlsc)
        }
    }

    fn make_client_config(config: &TLSClientConfig) -> ClientConfig {
        let client_config_builder = rustls::ClientConfig::builder()
            .with_safe_defaults();

        let mut root_store = rustls::RootCertStore::empty();
        // configure the validator
        match config.roots.as_str() {
            "native" => root_store.add_server_trust_anchors(
                webpki_roots::TLS_SERVER_ROOTS
                    .0
                    .iter()
                    .map(|ta| {
                        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                            ta.subject,
                            ta.spki,
                            ta.name_constraints,
                        )
                    }),
            ),

            "firefox" => root_store.add_server_trust_anchors(
                webpki_roots::TLS_SERVER_ROOTS
                    .0
                    .iter()
                    .map(|ta| {
                        rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                            ta.subject,
                            ta.spki,
                            ta.name_constraints,
                        )
                    })
            ),

            file_path => {
                let self_certs = rustls_pemfile::certs(&mut BufReader::new(must!(
                        fs::File::open(file_path),
                        "open {}",
                        file_path
                    ))).expect("read self_certs failed");
                 root_store.add_parsable_certificates(&self_certs);
            }
        };
        // cert set end
        let client_config_builder = client_config_builder.with_root_certificates(root_store);

        let mut client_config =   client_config_builder.with_no_client_auth();
        client_config.enable_sni = config.enable_sni;
        client_config.enable_early_data = config.enable_early_data;
        // if not specified, use the constructor's default value
        if !config.alpns.is_empty() {
            client_config.alpn_protocols =
                config.alpns.iter().map(|x| x.as_bytes().to_vec()).collect();
        };
        // the same as alpns
        if !config.versions.is_empty() {
            let _ = config.versions.iter()
                .map(|x| match x.as_str() {
                    "tlsv1.2" => client_config.supports_version(ProtocolVersion::TLSv1_2),
                    "tlsv1.3" => client_config.supports_version(ProtocolVersion::TLSv1_3),
                    _ => panic!("unknown ssl version"),
                });
        };
        // skip verify
        if config.skip_verify {
            client_config.dangerous()
                .set_certificate_verifier(Arc::new(ClientSkipVerify));
            return client_config;
        };


        client_config
    }

    // TLS Server
    #[derive(Debug, Serialize, Deserialize)]
    pub struct TLSServerConfig {
        pub cert: String,

        pub key: String,

        #[serde(default)]
        pub alpns: Vec<String>,

        #[serde(default)]
        pub versions: Vec<String>,

        #[serde(default)]
        pub ocsp: String,
    }

    use crate::utils::MaybeQuic;

    impl TLSServerConfig {
        // pub fn to_tls(&self) -> ServerConfig { make_server_config(self) }

        pub fn apply_to_lis<L: AsyncAccept>(&self, lis: L) -> impl AsyncAccept {
            let config = make_server_config(self);
            tls::Acceptor::new(lis, config)
        }

        pub fn apply_to_lis_ext<L: AsyncAccept>(
            &self,
            lis: MaybeQuic<L>,
        ) -> MaybeQuic<impl AsyncAccept> {
            match lis {
                #[cfg(feature = "quic")]
                MaybeQuic::Quic(x) => MaybeQuic::Quic(x),
                MaybeQuic::Other(x) => MaybeQuic::Other(self.apply_to_lis(x)),
            }
        }
    }

    fn make_server_config(config: &TLSServerConfig) -> ServerConfig {
        let server_config_builder = rustls::ServerConfig::builder()
            .with_safe_defaults().with_no_client_auth();

        let (certs, key) = if config.cert == config.key {
            must!(utils::generate_cert_key(&config.cert))
        } else {
            let certs =
                must!(utils::load_certs(&config.cert), "load {}", &config.cert);
            let mut keys =
                must!(utils::load_keys(&config.key), "load {}", &config.key);
            (certs, keys.remove(0))
        };
        let mut server_config;

        let mut ocsp = vec![0u8];
        if !config.ocsp.is_empty() {
            ocsp.reserve(utils::OCSP_BUF_SIZE);
            let mut r = BufReader::new(must!(
                fs::File::open(&config.ocsp),
                "open {}",
                &config.ocsp
            ));
            must!(r.read_to_end(&mut ocsp), "load {}", &config.ocsp);
            server_config = server_config_builder.with_single_cert_with_ocsp_and_sct(certs, key, ocsp, Vec::new()).expect("bad server certs");
        }else{
            server_config = server_config_builder.with_single_cert(certs,key).expect("bad server certs");
        }

        // if not specified, use the constructor's default value
        if !config.alpns.is_empty() {
            server_config.alpn_protocols =
                config.alpns.iter().map(|x| x.as_bytes().to_vec()).collect();
        };
        // the same as alpns
        if !config.versions.is_empty() {
            let _ = config.versions.iter()
                .map(|x| match x.as_str() {
                    "tlsv1.2" => server_config.supports_version(ProtocolVersion::TLSv1_2),
                    "tlsv1.3" => server_config.supports_version(ProtocolVersion::TLSv1_3),
                    _ => panic!("unknown ssl version"),
                });
        };

        server_config
    }
}