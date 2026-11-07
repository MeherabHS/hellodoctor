//! Email transport boundary (Phase 0: OTP only, via Azure Communication
//! Services). Mirrors `notification_transport`'s shape (`is_configured`, an
//! outcome enum distinguishing a retryable failure from a permanent one, a
//! Noop implementation for development/test/recovery-mode) so the two
//! provider boundaries stay structurally consistent and easy to review
//! together, even though nothing shares code between them (different
//! providers, different wire protocols, different credential types).

use crate::config::AppConfig;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailOutcome {
    SentToProvider { provider_message_id: Option<String> },
    Retryable { reason: String },
    Permanent { reason: String },
    NoProviderConfigured,
}

/// Borrowed content for one send attempt. Holds no state and performs no
/// I/O, so it can be constructed in a test with no transport at all.
pub struct EmailMessage<'a> {
    pub recipient: &'a str,
    pub subject: &'a str,
    pub plain_text_body: &'a str,
    /// `None` for every message this codebase actually sends today (OTP is
    /// plain text only, preserving the pre-Phase-0 behavior exactly).
    /// Populated once a future phase wires `services::email_templates`'
    /// HTML bodies to a real trigger.
    pub html_body: Option<&'a str>,
}

#[async_trait]
pub trait EmailTransport: Send + Sync {
    fn is_configured(&self) -> bool;

    async fn send_email(&self, message: &EmailMessage<'_>) -> EmailOutcome;
}

pub struct NoopEmailTransport;

#[async_trait]
impl EmailTransport for NoopEmailTransport {
    fn is_configured(&self) -> bool {
        false
    }

    async fn send_email(&self, _message: &EmailMessage<'_>) -> EmailOutcome {
        EmailOutcome::NoProviderConfigured
    }
}

/// Azure Communication Services Email via its REST data-plane endpoint,
/// authenticated with HMAC-SHA256-signed access-key requests — the only
/// authentication mode this transport implements. See
/// `config::validate_azure_email_configuration`'s doc comment for exactly
/// why `AZURE_EMAIL_AUTH_MODE=managed_identity` is rejected at startup
/// instead of attempted here: config validation guarantees this struct is
/// only ever constructed when the auth mode is `access_key`.
/// Per-attempt network timeout. Deliberately shorter than a generic
/// "give the provider plenty of time" value: `send_otp_email_tracked`
/// (`email_delivery.rs`) retries a `Retryable` outcome inline, within the
/// same login/registration HTTP request a user is actively waiting on, so
/// this value directly multiplies into worst-case login latency
/// (`MAX_INLINE_ATTEMPTS * ACS_REQUEST_TIMEOUT + inter-attempt backoff`).
/// Real Azure Communication Services latency is sub-second in the normal
/// case; 8 seconds is already generous slack before concluding a request is
/// genuinely hung rather than merely slow, and keeps the two-attempt
/// worst case (~16.5s) well under the previous, untuned 15s-per-attempt
/// value's ~30.5s worst case.
const ACS_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

#[derive(Clone)]
pub struct AcsEmailTransport {
    endpoint: String,
    host: String,
    signing_key: Vec<u8>,
    sender: String,
    client: reqwest::Client,
}

impl AcsEmailTransport {
    pub fn from_config(config: &AppConfig) -> Result<Option<Self>, String> {
        let (Some(endpoint), Some(access_key), Some(sender)) = (
            config.azure_email_endpoint.as_deref(),
            config.azure_email_access_key.as_deref(),
            config.azure_email_sender.as_deref(),
        ) else {
            return Ok(None);
        };
        let endpoint = endpoint.trim_end_matches('/').to_string();
        let uri = endpoint
            .parse::<reqwest::Url>()
            .map_err(|_| "Invalid Azure email endpoint.".to_string())?;
        if uri.scheme() != "https"
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.port().is_some()
        {
            return Err(
                "Azure email endpoint must use HTTPS with no embedded credentials or custom port."
                    .to_string(),
            );
        }
        let host = uri
            .host_str()
            .ok_or_else(|| "Azure email endpoint has no host.".to_string())?
            .to_string();
        let signing_key = STANDARD
            .decode(access_key)
            .map_err(|_| "Azure email access key is not valid base64.".to_string())?;
        let client = reqwest::Client::builder()
            .timeout(ACS_REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Could not initialize email delivery.".to_string())?;
        Ok(Some(Self {
            endpoint,
            host,
            signing_key,
            sender: sender.to_string(),
            client,
        }))
    }
}

/// The exact JSON body Azure Communication Services' `/emails:send` REST
/// endpoint receives — a pure function with no network/credential
/// dependency, specifically so a templated message's `html_body` (used by
/// `services::email_templates`, never populated by the OTP path) can be
/// proven to reach the real wire payload without ever making a network
/// call, mirroring `notification_transport::build_fcm_message`'s rationale.
/// This is the concrete proof that a Phase 1A template is not merely a
/// string builder: it is the exact content this transport would hand to
/// Azure the moment `AZURE_EMAIL_*` is configured in a real environment —
/// see `tests::acs_request_body_carries_a_full_templated_html_email_end_to_end`.
fn build_request_body(sender: &str, message: &EmailMessage<'_>) -> serde_json::Value {
    let mut content = serde_json::json!({
        "subject": message.subject,
        "plainText": message.plain_text_body,
    });
    if let Some(html) = message.html_body {
        content["html"] = serde_json::Value::String(html.to_string());
    }
    serde_json::json!({
        "senderAddress": sender,
        "content": content,
        "recipients": {"to": [{"address": message.recipient}]}
    })
}

#[async_trait]
impl EmailTransport for AcsEmailTransport {
    fn is_configured(&self) -> bool {
        true
    }

    async fn send_email(&self, message: &EmailMessage<'_>) -> EmailOutcome {
        let url = format!("{}/emails:send?api-version=2023-03-31", self.endpoint);
        let Ok(uri) = url.parse::<reqwest::Url>() else {
            return EmailOutcome::Permanent {
                reason: "Could not build Azure email request URL.".into(),
            };
        };
        let path_query = uri.path().to_string()
            + uri
                .query()
                .map(|q| format!("?{q}"))
                .as_deref()
                .unwrap_or("");
        let body = build_request_body(&self.sender, message);
        let Ok(body_bytes) = serde_json::to_vec(&body) else {
            return EmailOutcome::Permanent {
                reason: "Could not encode Azure email request.".into(),
            };
        };
        let content_hash = STANDARD.encode(Sha256::digest(&body_bytes));
        let date = chrono::Utc::now()
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string();
        let string_to_sign = format!("POST\n{path_query}\n{date};{};{content_hash}", self.host);
        let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(&self.signing_key) else {
            return EmailOutcome::Permanent {
                reason: "Azure email signing key is invalid.".into(),
            };
        };
        mac.update(string_to_sign.as_bytes());
        let signature = STANDARD.encode(mac.finalize().into_bytes());
        let authorization = format!(
            "HMAC-SHA256 SignedHeaders=x-ms-date;host;x-ms-content-sha256&Signature={signature}"
        );
        let response = match self
            .client
            .post(url)
            .header("x-ms-date", &date)
            .header("x-ms-content-sha256", &content_hash)
            .header("host", &self.host)
            .header("Authorization", authorization)
            .header("Content-Type", "application/json")
            .body(body_bytes)
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return EmailOutcome::Retryable {
                    reason: "Azure email transport is unavailable".into(),
                }
            }
        };
        let status = response.status();
        if status.is_success() {
            // The /emails:send response does not expose a stable, documented
            // field this implementation has confirmed for a
            // provider-assigned message id; left `None` rather than
            // guessing a JSON shape that could silently never match.
            return EmailOutcome::SentToProvider {
                provider_message_id: None,
            };
        }
        tracing::warn!(status = %status, "Azure email delivery rejected");
        if status == reqwest::StatusCode::BAD_REQUEST {
            return EmailOutcome::Permanent {
                reason: format!("Azure email rejected the request (HTTP {status})"),
            };
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return EmailOutcome::Permanent {
                reason: format!("Azure email authorization failed (HTTP {status})"),
            };
        }
        EmailOutcome::Retryable {
            reason: format!("Azure email returned retryable HTTP {status}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_transport_reports_unconfigured_and_never_sends() {
        let transport = NoopEmailTransport;
        assert!(!transport.is_configured());
        let message = EmailMessage {
            recipient: "patient@example.invalid",
            subject: "test",
            plain_text_body: "body",
            html_body: None,
        };
        assert_eq!(
            transport.send_email(&message).await,
            EmailOutcome::NoProviderConfigured
        );
    }

    /// Answers "will this Phase 1A template actually reach Azure once
    /// configured?" with a real assertion on the exact wire payload, not a
    /// structural assumption: builds a full rendered template (with a real
    /// HTML body, something the pre-Phase-1A OTP path never exercised) and
    /// confirms the JSON this transport would send to
    /// `/emails:send` carries both `plainText` and the complete `html`
    /// content, byte for byte.
    #[test]
    fn acs_request_body_carries_a_full_templated_html_email_end_to_end() {
        let rendered = crate::services::email_templates::render_patient_welcome(
            "Amani",
            &crate::services::email_templates::EmailAssets::none(),
            &crate::services::email_templates::EmailLinks::none(),
        );
        let message = rendered.as_email_message("patient@example.invalid");
        let body = build_request_body("no-reply@hellodoctor.asia", &message);

        assert_eq!(body["senderAddress"], "no-reply@hellodoctor.asia");
        assert_eq!(
            body["recipients"]["to"][0]["address"],
            "patient@example.invalid"
        );
        assert_eq!(body["content"]["subject"], rendered.subject);
        assert_eq!(body["content"]["plainText"], rendered.text_body);
        assert_eq!(
            body["content"]["html"].as_str(),
            Some(rendered.html_body.as_str()),
            "the full rendered HTML template must reach the real Azure request body unmodified"
        );
        // Sanity: this is a genuine, non-trivial HTML document, not an
        // accidental empty string slipping through.
        assert!(body["content"]["html"]
            .as_str()
            .unwrap()
            .contains("Welcome to"));
    }

    /// Confirms the *real* transport (not `NoopEmailTransport`) is what
    /// gets constructed the moment Azure configuration is present — using a
    /// synthetic, well-formed (never real) endpoint/key/sender, mirroring
    /// `config.rs`'s own synthetic-fixture convention. This is the other
    /// half of "sync to backend": proving configuration alone, with zero
    /// additional glue code, is enough to make `AcsEmailTransport` ready to
    /// send a template.
    #[test]
    fn configuring_azure_email_yields_the_real_transport_ready_to_send_templates() {
        // Reuses config.rs's own synthetic fixture (already shaped as a
        // fully-configured deployment, azure_email_* included) rather than
        // duplicating a 50+ field AppConfig literal here — only the access
        // key is overridden, since the shared fixture's value is a
        // human-readable marker string, not valid base64.
        let mut config = crate::config::remediation_e3_tests::synthetic_config();
        config.azure_email_access_key = Some(STANDARD.encode([7u8; 32]));

        let transport = AcsEmailTransport::from_config(&config)
            .expect("synthetic config must be accepted")
            .expect("a fully-configured endpoint/key/sender must yield Some(transport)");
        assert!(transport.is_configured());

        // The same conversion the backend would use for a real send.
        let rendered = crate::services::email_templates::render_doctor_approved(
            "Njoroge",
            &crate::services::email_templates::EmailAssets::none(),
            &crate::services::email_templates::EmailLinks::none(),
        );
        let message = rendered.as_email_message("doctor@example.invalid");
        let body = build_request_body(&transport.sender, &message);
        assert_eq!(
            body["content"]["html"].as_str(),
            Some(rendered.html_body.as_str())
        );
    }

    /// Real network I/O (a genuine TCP+TLS handshake against a socket that
    /// accepts the connection and then never responds), not a scripted
    /// stand-in — this is the closest a real Azure outage/hang can be
    /// reproduced on a development machine with no live ACS credentials.
    /// Confirms two things a scripted transport cannot: (1) the configured
    /// `reqwest::Client` timeout actually fires against a connection that is
    /// silently stuck, rather than hanging forever, and (2) the resulting
    /// outcome is classified `Retryable` (the `Err(_)` branch in
    /// `send_email`), matching what `email_delivery::send_otp_email_tracked`
    /// expects in order to apply its own bounded inline-retry-then-fail
    /// behavior. Uses its own short timeout (not `ACS_REQUEST_TIMEOUT`) only
    /// so this test runs quickly; the mechanism under test is identical.
    #[tokio::test]
    async fn unresponsive_endpoint_times_out_and_is_classified_retryable() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind an ephemeral loopback port");
        let port = listener.local_addr().unwrap().port();
        // Accept the connection and then do nothing with it at all —
        // never writes a TLS ServerHello, never closes the socket. The
        // client's TLS handshake is left waiting on a read that will never
        // complete, which is exactly what a hung provider looks like from
        // this code's point of view.
        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                let _held_open = stream;
                std::future::pending::<()>().await;
            }
        });

        let test_timeout = std::time::Duration::from_millis(400);
        let transport = AcsEmailTransport {
            endpoint: format!("https://127.0.0.1:{port}"),
            host: "127.0.0.1".to_string(),
            signing_key: vec![0u8; 32],
            sender: "no-reply@example.invalid".to_string(),
            client: reqwest::Client::builder()
                .timeout(test_timeout)
                .build()
                .unwrap(),
        };
        let message = EmailMessage {
            recipient: "patient@example.invalid",
            subject: "test",
            plain_text_body: "body",
            html_body: None,
        };

        let started = std::time::Instant::now();
        let outcome = transport.send_email(&message).await;
        let elapsed = started.elapsed();

        assert!(
            matches!(outcome, EmailOutcome::Retryable { .. }),
            "a hung connection must be classified Retryable, got {outcome:?}"
        );
        assert!(
            elapsed >= test_timeout,
            "must not return before the configured timeout elapses: {elapsed:?}"
        );
        assert!(
            elapsed < test_timeout * 3,
            "must not hang indefinitely past the configured timeout: {elapsed:?}"
        );
    }
}
