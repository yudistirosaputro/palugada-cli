//! Thin synchronous HTTP helper over `ureq`: Bearer/header auth, with an
//! optional `--insecure` mode for self-signed corporate TLS certs.

use std::sync::Arc;
use std::time::Duration;

pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    pub fn new(insecure: bool) -> Self {
        let agent = if insecure {
            let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerify))
            .with_no_client_auth();

            ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(90))
                .timeout_read(Duration::from_secs(60))
                .timeout_write(Duration::from_secs(30))
                .tls_config(Arc::new(tls))
                .build()
        } else {
            ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(90))
                .timeout_read(Duration::from_secs(60))
                .timeout_write(Duration::from_secs(30))
                .build()
        };
        Http { agent }
    }

    /// GET a URL with arbitrary headers and decode the JSON body into `T`.
    pub fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        headers: &[(&str, String)],
    ) -> Result<T, String> {
        let mut req = self.agent.get(url).set("Accept", "application/json");
        for (k, v) in headers {
            req = req.set(k, v);
        }
        let resp = req.call().map_err(|e| describe_ureq(url, e))?;
        resp.into_json::<T>()
            .map_err(|e| format!("failed to decode JSON from {url}: {e}"))
    }

    /// GET a URL and return the raw response body as a string.
    pub fn get_text(&self, url: &str, headers: &[(&str, String)]) -> Result<String, String> {
        let mut req = self.agent.get(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        let resp = req.call().map_err(|e| describe_ureq(url, e))?;
        resp.into_string()
            .map_err(|e| format!("failed to read body from {url}: {e}"))
    }
}

/// Turn a ureq error into a readable message, surfacing URL + HTTP status.
fn describe_ureq(url: &str, e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let snippet: String = body.chars().take(300).collect();
            format!("GET {url} -> HTTP {code}: {snippet}")
        }
        ureq::Error::Transport(t) => format!("GET {url}: transport error: {t}"),
    }
}

/// Percent-encode a single URL path segment (RFC 3986 unreserved kept).
pub fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_segment_percent_encodes_reserved_chars() {
        assert_eq!(encode_segment("PROJ-123"), "PROJ-123");
        assert_eq!(encode_segment("a b/c?d"), "a%20b%2Fc%3Fd");
        assert_eq!(encode_segment("naïve"), "na%C3%AFve");
    }
}

/// TLS verifier that accepts any certificate — only used with `--insecure`.
/// (Standard rustls 0.23 dangerous-verifier pattern.)
#[derive(Debug)]
struct NoVerify;

impl rustls::client::danger::ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}
