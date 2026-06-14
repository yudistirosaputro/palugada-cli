//! Thin synchronous HTTP helper over `ureq`: arbitrary header auth, with an
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
        let resp = req.call().map_err(|e| describe_ureq("GET", url, e))?;
        resp.into_json::<T>()
            .map_err(|e| format!("failed to decode JSON from {url}: {e}"))
    }

    /// GET a URL and return the raw response body as a string.
    pub fn get_text(&self, url: &str, headers: &[(&str, String)]) -> Result<String, String> {
        let mut req = self.agent.get(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        let resp = req.call().map_err(|e| describe_ureq("GET", url, e))?;
        resp.into_string()
            .map_err(|e| format!("failed to read body from {url}: {e}"))
    }

    /// POST a JSON string body with arbitrary headers; return the raw response
    /// body (some APIs return JSON, some — like Slack webhooks — return "ok").
    pub fn post_json(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &str,
    ) -> Result<String, String> {
        let mut req = self.agent.post(url).set("Content-Type", "application/json");
        for (k, v) in headers {
            req = req.set(k, v);
        }
        let resp = req.send_string(body).map_err(|e| describe_ureq("POST", url, e))?;
        resp.into_string()
            .map_err(|e| format!("failed to read body from {url}: {e}"))
    }
}

/// Turn a ureq error into a readable message, surfacing method + URL + status.
fn describe_ureq(method: &str, url: &str, e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let snippet: String = body.chars().take(300).collect();
            format!("{method} {url} -> HTTP {code}: {snippet}")
        }
        ureq::Error::Transport(t) => format!("{method} {url}: transport error: {t}"),
    }
}

/// Percent-decode a URL path segment (inverse of `encode_segment`). Invalid
/// `%XX` sequences are left as-is. `+` is kept literal (it means space only in
/// query strings, not path segments).
pub fn decode_segment(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
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

    #[test]
    fn decode_segment_inverts_encode() {
        for s in ["PROJ-123", "my project", "a b/c?d", "naïve", "a&b=c"] {
            assert_eq!(decode_segment(&encode_segment(s)), s, "round-trip failed for {s:?}");
        }
        assert_eq!(decode_segment("my%20project"), "my project");
        // malformed sequences are left as-is
        assert_eq!(decode_segment("100%done"), "100%done");
        assert_eq!(decode_segment("ends%2"), "ends%2");
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
