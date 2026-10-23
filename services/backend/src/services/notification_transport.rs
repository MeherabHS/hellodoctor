//! Server-side push transport boundary. FCM HTTP v1 is optional in
//! development/test, but production startup requires a complete service
//! account configuration. Credentials never enter notification rows, payloads
//! or logs.

use crate::config::AppConfig;
use async_trait::async_trait;
use base64::Engine as _;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct PushOptions {
    pub notification_id: String,
    pub notification_type: String,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<String>,
    pub collapse_key: Option<String>,
    pub ttl_seconds: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushOutcome {
    SentToProvider { provider_message_id: Option<String> },
    Retryable { reason: String },
    Permanent { reason: String },
    NoProviderConfigured,
}

#[async_trait]
pub trait NotificationTransport: Send + Sync {
    fn is_configured(&self) -> bool;

    async fn send_push(
        &self,
        push_token: &str,
        title: &str,
        body: &str,
        options: &PushOptions,
    ) -> PushOutcome;
}

pub struct NoopTransport;

#[async_trait]
impl NotificationTransport for NoopTransport {
    fn is_configured(&self) -> bool {
        false
    }

    async fn send_push(
        &self,
        _push_token: &str,
        _title: &str,
        _body: &str,
        _options: &PushOptions,
    ) -> PushOutcome {
        PushOutcome::NoProviderConfigured
    }
}

#[derive(Debug, Serialize)]
struct OAuthClaims<'a> {
    iss: &'a str,
    scope: &'static str,
    aud: &'static str,
    iat: i64,
    exp: i64,
}

#[derive(Debug, Deserialize)]
struct OAuthResponse {
    access_token: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct FcmResponse {
    name: Option<String>,
}

type CachedAccessToken = Arc<Mutex<Option<(String, chrono::DateTime<Utc>)>>>;

#[derive(Clone)]
pub struct FcmTransport {
    project_id: String,
    client_email: String,
    signing_key: EncodingKey,
    client: Client,
    access_token: CachedAccessToken,
}

impl FcmTransport {
    pub fn from_config(config: &AppConfig) -> Result<Option<Self>, String> {
        let (Some(project_id), Some(client_email), Some(private_key_b64)) = (
            config.fcm_project_id.as_deref(),
            config.fcm_client_email.as_deref(),
            config.fcm_private_key_b64.as_deref(),
        ) else {
            return Ok(None);
        };
        let pem = base64::engine::general_purpose::STANDARD
            .decode(private_key_b64)
            .map_err(|_| "FCM_PRIVATE_KEY_B64 is not valid base64".to_string())?;
        let signing_key = EncodingKey::from_rsa_pem(&pem)
            .map_err(|_| "FCM service-account private key is not a valid RSA PEM".to_string())?;
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|_| "FCM HTTP client could not be initialized".to_string())?;
        Ok(Some(Self {
            project_id: project_id.to_owned(),
            client_email: client_email.to_owned(),
            signing_key,
            client,
            access_token: Arc::new(Mutex::new(None)),
        }))
    }

    async fn invalidate_access_token(&self) {
        *self.access_token.lock().await = None;
    }

    async fn access_token(&self) -> Result<String, String> {
        {
            let cached = self.access_token.lock().await;
            if let Some((token, expires_at)) = cached.as_ref() {
                if *expires_at > Utc::now() + Duration::seconds(60) {
                    return Ok(token.clone());
                }
            }
        }
        let now = Utc::now();
        let assertion = encode(
            &Header::new(Algorithm::RS256),
            &OAuthClaims {
                iss: &self.client_email,
                scope: "https://www.googleapis.com/auth/firebase.messaging",
                aud: "https://oauth2.googleapis.com/token",
                iat: now.timestamp(),
                exp: (now + Duration::minutes(55)).timestamp(),
            },
            &self.signing_key,
        )
        .map_err(|_| "FCM OAuth assertion could not be created".to_string())?;
        let response = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", assertion.as_str()),
            ])
            .send()
            .await
            .map_err(|_| "FCM OAuth endpoint is unavailable".to_string())?;
        if !response.status().is_success() {
            return Err(format!(
                "FCM OAuth endpoint returned HTTP {}",
                response.status()
            ));
        }
        let token: OAuthResponse = response
            .json()
            .await
            .map_err(|_| "FCM OAuth response was invalid".to_string())?;
        let expires_at = now + Duration::seconds(token.expires_in.max(120));
        *self.access_token.lock().await = Some((token.access_token.clone(), expires_at));
        Ok(token.access_token)
    }
}

#[async_trait]
impl NotificationTransport for FcmTransport {
    fn is_configured(&self) -> bool {
        true
    }

    async fn send_push(
        &self,
        push_token: &str,
        title: &str,
        body: &str,
        options: &PushOptions,
    ) -> PushOutcome {
        let token = match self.access_token().await {
            Ok(token) => token,
            Err(reason) => return PushOutcome::Retryable { reason },
        };
        let ttl = options.ttl_seconds.unwrap_or(3600).clamp(0, 2_419_200);
        let mut data = serde_json::Map::new();
        data.insert(
            "notification_id".into(),
            options.notification_id.clone().into(),
        );
        data.insert(
            "notification_type".into(),
            options.notification_type.clone().into(),
        );
        if let Some(value) = &options.related_entity_type {
            data.insert("related_entity_type".into(), value.clone().into());
        }
        if let Some(value) = &options.related_entity_id {
            data.insert("related_entity_id".into(), value.clone().into());
        }
        let mut android = serde_json::json!({
            "ttl": format!("{}s", ttl),
            "priority": "high"
        });
        if let Some(key) = options.collapse_key.as_ref() {
            android["collapse_key"] = serde_json::Value::String(key.clone());
        }
        let mut message = serde_json::json!({
            "message": {
                "token": push_token,
                "notification": {"title": title, "body": body},
                "data": data,
                "android": android,
                "apns": {"headers": {"apns-expiration": (Utc::now().timestamp() + i64::from(ttl)).to_string()}, "payload": {"aps": {"sound": "default"}}}
            }
        });
        if let Some(key) = options.collapse_key.as_ref() {
            message["message"]["fcm_options"] = serde_json::json!({"analytics_label": key});
        }
        let response = match self
            .client
            .post(format!(
                "https://fcm.googleapis.com/v1/projects/{}/messages:send",
                self.project_id
            ))
            .bearer_auth(token)
            .json(&message)
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return PushOutcome::Retryable {
                    reason: "FCM transport is unavailable".into(),
                }
            }
        };
        let status = response.status();
        if status.is_success() {
            let parsed = response.json::<FcmResponse>().await.ok();
            return PushOutcome::SentToProvider {
                provider_message_id: parsed.and_then(|value| value.name),
            };
        }
        let error_payload = response.json::<serde_json::Value>().await.ok();
        let provider_code = error_payload
            .as_ref()
            .and_then(|payload| payload.get("error"))
            .and_then(|error| error.get("status"))
            .and_then(serde_json::Value::as_str);
        let detail_code = error_payload
            .as_ref()
            .and_then(|payload| payload.get("error"))
            .and_then(|error| error.get("details"))
            .and_then(serde_json::Value::as_array)
            .and_then(|details| {
                details
                    .iter()
                    .find_map(|detail| detail.get("errorCode").and_then(serde_json::Value::as_str))
            });
        if status == reqwest::StatusCode::NOT_FOUND
            || provider_code == Some("UNREGISTERED")
            || detail_code == Some("UNREGISTERED")
        {
            return PushOutcome::Permanent {
                reason: format!("FCM rejected the registration token (HTTP {})", status),
            };
        }
        // INVALID_ARGUMENT can describe a malformed message as well as a
        // token problem. Keep it retryable unless FCM explicitly identifies
        // the registration as UNREGISTERED; disabling every token on a
        // transient payload/configuration error would strand valid devices.
        if status == reqwest::StatusCode::BAD_REQUEST {
            return PushOutcome::Retryable {
                reason: format!("FCM rejected the request (HTTP {})", status),
            };
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            self.invalidate_access_token().await;
            return PushOutcome::Retryable {
                reason: format!("FCM authorization failed (HTTP {})", status),
            };
        }
        PushOutcome::Retryable {
            reason: format!("FCM returned retryable HTTP {}", status),
        }
    }
}
