use std::fs::File;
use std::io::BufReader;

use rustls::{Certificate, PrivateKey};
use rustls_pemfile::Item;

use crate::error::cert::{CertError, Result};

pub fn generate_cert_key(
    common_name: &str,
) -> Result<(Vec<Certificate>, PrivateKey)> {
    let certificate =
        rcgen::generate_simple_self_signed(vec![common_name.to_string()])?;
    let cert = certificate.serialize_der()?;
    let key = certificate.serialize_private_key_der();
    // der
    let cert = Certificate(cert);
    let key = PrivateKey(key);
    Ok((vec![cert], key))
}

pub fn load_certs(path: &str) -> Result<Vec<Certificate>> {
    let items: Vec<Item>  = rustls_pemfile::read_all(&mut BufReader::new(File::open(path)?)).map_err(|_| CertError::LoadCertificate).expect("TODO: panic message");
    if !items.is_empty() {
        let mut certificates = Vec::<Certificate>::new();
        for item in items {
            match item {
                Item::X509Certificate(cert) => certificates.push(Certificate(cert)),
                _ => println!("unhandled Certificate item"),
            }
        }
        return Ok(certificates);
    }
    Err(CertError::LoadCertificate)
}

pub fn load_keys(path: &str) -> Result<Vec<PrivateKey>> {
    let items: Vec<Item>  = rustls_pemfile::read_all(&mut BufReader::new(File::open(path)?)).map_err(|_| CertError::LoadCertificate).expect("TODO: panic message");
    if !items.is_empty() {
        let mut certificates = Vec::<PrivateKey>::new();
        for item in items {
            match item {
                Item::RSAKey(key) => certificates.push(PrivateKey(key)),
                Item::PKCS8Key(key) => certificates.push(PrivateKey(key)),
                Item::ECKey(key) => certificates.push(PrivateKey(key)),
                _ => println!("unhandled PrivateKey item"),
            }
        }
        return Ok(certificates);
    }
    Err(CertError::LoadPrivateKey)
}

/*
// whoops! rustls does not support such format
// users can use openssl to convert it to pkcs8:
//
// openssl pkcs8 -topk8 -nocrypt -in x.key -out xx.pem
//
// but I have no idea how to write code to achieve this
// maybe use openssl's rust binding..?
//
// deprecated below
// legacy format
fn ec_private_keys(rd: &mut dyn io::BufRead) -> Result<Vec<PrivateKey>, ()> {
    extract(
        rd,
        "-----BEGIN EC PRIVATE KEY-----",
        "-----END EC PRIVATE KEY-----",
        &|v| PrivateKey(v),
    )
}

// borrow from
// https://docs.rs/rustls/0.19.1/src/rustls/pemfile.rs.html#73-80
fn extract<A>(
    rd: &mut dyn io::BufRead,
    start_mark: &str,
    end_mark: &str,
    f: &dyn Fn(Vec<u8>) -> A,
) -> Result<Vec<A>, ()> {
    let mut ders = Vec::new();
    let mut b64buf = String::new();
    let mut take_base64 = false;

    let mut raw_line = Vec::<u8>::new();
    loop {
        raw_line.clear();
        let len = rd.read_until(b'\n', &mut raw_line).map_err(|_| ())?;

        if len == 0 {
            return Ok(ders);
        }
        let line = String::from_utf8_lossy(&raw_line);

        if line.starts_with(start_mark) {
            take_base64 = true;
            continue;
        }

        if line.starts_with(end_mark) {
            take_base64 = false;
            let der = base64::decode(&b64buf).map_err(|_| ())?;
            ders.push(f(der));
            b64buf = String::new();
            continue;
        }

        if take_base64 {
            b64buf.push_str(line.trim());
        }
    }
}
*/