//! Slack incoming-webhook notifier. The webhook URL is a secret
//! (`chat_webhook` in the auth profile); the project config only names the
//! provider. `notify` POSTs a plain-text message; `verify` does not post (it
//! would spam the channel) — it only confirms a webhook is configured.

use super::ChatNotify;
use crate::http::Http;

pub struct Slack {
    webhook: String,
    http: Http,
}

impl Slack {
    pub fn new(webhook: &str, insecure: bool) -> Self {
        Slack { webhook: webhook.to_string(), http: Http::new(insecure) }
    }
}

/// Slack incoming-webhook JSON payload for a plain-text message. Uses serde_json
/// so quotes/newlines in `message` are escaped correctly.
pub fn payload(message: &str) -> String {
    serde_json::json!({ "text": message }).to_string()
}

impl ChatNotify for Slack {
    fn notify(&self, message: &str) -> Result<String, String> {
        if self.webhook.is_empty() {
            return Err("chat_webhook is empty in the auth profile".into());
        }
        let body = self.http.post_json(&self.webhook, &[], &payload(message))?;
        if body.trim() == "ok" {
            Ok("sent".to_string())
        } else {
            Ok(body)
        }
    }

    fn verify(&self) -> Result<String, String> {
        if self.webhook.is_empty() {
            return Err("chat_webhook is empty in the auth profile".into());
        }
        Ok("Slack webhook configured".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::payload;

    #[test]
    fn payload_escapes_special_chars() {
        assert_eq!(payload("hello"), r#"{"text":"hello"}"#);
        // quotes and newlines must be JSON-escaped
        assert_eq!(payload("a\"b\nc"), r#"{"text":"a\"b\nc"}"#);
    }
}
