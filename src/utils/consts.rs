use lazy_static::lazy_static;
use rustls::RootCertStore;

pub const VERSION: &str = "0.2.1";
pub const NAV_VERSION: &str = "0.1.0";

pub const BUF_SIZE: usize = 0x4000;
pub const OCSP_BUF_SIZE: usize = 0x400;

pub const NOT_A_DNS_NAME: &str = "localhost";

#[cfg(target_os = "linux")]
pub const PIPE_BUF_SIZE: usize = 0x10000;

lazy_static! {
    pub static ref NATIVE_CERTS: RootCertStore =
        rustls_native_certs::load_native_certs().unwrap();
}
