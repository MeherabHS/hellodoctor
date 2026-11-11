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
    /// `users.role` (`"DOCTOR"`/`"PATIENT"`/an admin variant) for the
    /// notification's recipient — resolved by the caller
    /// (`notification_scheduler::send_with_lease`) from the already-known
    /// `user_id`, never client-supplied. Required because the Doctor and
    /// Patient Flutter apps register different Android notification channel
    /// sets (see `android_channel_id` below); the same `notification_type`
    /// can map to a different real channel depending which app receives it.
    pub recipient_role: String,
}

/// M8-G-13: maps a backend `notification_type` to the exact Android
/// notification channel ID the recipient's app will already have created,
/// mirroring — and kept in sync by this module's own tests with —
/// `apps/mobile/lib/core/notifications/local_notification_service.dart`
/// (Doctor) and
/// `apps/mobile_patient/lib/core/notifications/local_notification_service.dart`
/// (Patient). Falls back to `"GENERAL"` for any type not explicitly listed
/// or any unrecognized role, exactly like both Dart `forType()` functions do
/// — a new notification kind or an unexpected role must never fail delivery,
/// it should just land somewhere reasonable (both apps' channel sets include
/// a `GENERAL` channel) until this map is extended to match.
pub fn android_channel_id(notification_type: &str, recipient_role: &str) -> &'static str {
    match recipient_role {
        "DOCTOR" => match notification_type {
            "CONSULTATION_REMINDER_15M" => "CONSULTATION_REMINDERS",
            "DAILY_CONSULTATION_SUMMARY" => "DAILY_SCHEDULE",
            "NEW_CHAT_MESSAGE" => "MESSAGES",
            "REGISTRATION_VERIFIED" | "REGISTRATION_REJECTED" | "ARTICLE_STATUS_CHANGED" => {
                "ACCOUNT"
            }
            "EARNING_CREDITED" | "PAYOUT_PAID" => "FINANCIAL",
            _ => "GENERAL",
        },
        // Patient and any other/unrecognized role share the Patient app's
        // channel set — "GENERAL" always exists there too, so an
        // unanticipated role never fails to resolve to a real channel.
        _ => match notification_type {
            "CONSULTATION_REMINDER_15M" => "CONSULTATION_REMINDERS",
            "NEW_CHAT_MESSAGE" => "MESSAGES",
            "BOOKING_HOLD_CREATED" | "BOOKING_HOLD_ONE_MINUTE" | "BOOKING_HOLD_EXPIRED" => {
                "APPOINTMENTS"
            }
            _ => "GENERAL",
        },
    }
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

/// Builds the exact FCM HTTP v1 request body, with no network or credential
/// dependency — the whole reason this is a free function, not inlined in
/// `send_push`, is so it can be unit-tested deterministically (M8-G-13)
/// without any live Firebase configuration. `android.notification.channel_id`
/// is the fix: previously omitted entirely, which left a backgrounded/
/// terminated app falling back to the Android manifest's
/// `default_notification_channel_id` (the low-importance `GENERAL` channel)
/// for every notification type regardless of its real importance.
fn build_fcm_message(
    push_token: &str,
    title: &str,
    body: &str,
    options: &PushOptions,
) -> serde_json::Value {
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
    let channel_id = android_channel_id(&options.notification_type, &options.recipient_role);
    let mut android = serde_json::json!({
        "ttl": format!("{}s", ttl),
        "priority": "high",
        "notification": {"channel_id": channel_id}
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
    message
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
        let message = build_fcm_message(push_token, title, body, options);
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

#[cfg(test)]
mod m8_g13_channel_routing_tests {
    use super::*;

    fn options(notification_type: &str, recipient_role: &str) -> PushOptions {
        PushOptions {
            notification_id: "11111111-1111-1111-1111-111111111111".into(),
            notification_type: notification_type.into(),
            related_entity_type: None,
            related_entity_id: None,
            collapse_key: None,
            ttl_seconds: Some(900),
            recipient_role: recipient_role.into(),
        }
    }

    /// Exact parity with
    /// `apps/mobile/lib/core/notifications/local_notification_service.dart`'s
    /// `NotificationChannels.forType` (Doctor app). If either side changes,
    /// this test must be updated in the same change — that is the whole
    /// point of pinning it here rather than leaving the two in sync by
    /// convention alone.
    #[test]
    fn doctor_channel_mapping_matches_dart_source() {
        let cases = [
            ("CONSULTATION_REMINDER_15M", "CONSULTATION_REMINDERS"),
            ("DAILY_CONSULTATION_SUMMARY", "DAILY_SCHEDULE"),
            ("NEW_CHAT_MESSAGE", "MESSAGES"),
            ("REGISTRATION_VERIFIED", "ACCOUNT"),
            ("REGISTRATION_REJECTED", "ACCOUNT"),
            ("ARTICLE_STATUS_CHANGED", "ACCOUNT"),
            ("EARNING_CREDITED", "FINANCIAL"),
            ("PAYOUT_PAID", "FINANCIAL"),
            ("SOME_UNMAPPED_FUTURE_TYPE", "GENERAL"),
        ];
        for (notification_type, expected_channel) in cases {
            assert_eq!(
                android_channel_id(notification_type, "DOCTOR"),
                expected_channel,
                "DOCTOR/{notification_type}"
            );
        }
    }

    /// Exact parity with
    /// `apps/mobile_patient/lib/core/notifications/local_notification_service.dart`'s
    /// `NotificationChannels.forType` (Patient app) — a deliberately
    /// *different* channel set from the Doctor app (no DAILY_SCHEDULE/
    /// ACCOUNT/FINANCIAL channels exist there).
    #[test]
    fn patient_channel_mapping_matches_dart_source() {
        let cases = [
            ("CONSULTATION_REMINDER_15M", "CONSULTATION_REMINDERS"),
            ("NEW_CHAT_MESSAGE", "MESSAGES"),
            ("BOOKING_HOLD_CREATED", "APPOINTMENTS"),
            ("BOOKING_HOLD_ONE_MINUTE", "APPOINTMENTS"),
            ("BOOKING_HOLD_EXPIRED", "APPOINTMENTS"),
            // Doctor-only types must never resolve to a Doctor-only channel
            // ID when the recipient is a patient — a Doctor-targeted type
            // should never actually be sent to a patient, but if it somehow
            // were, it must still land on a channel the Patient app actually
            // created, not silently reference a non-existent one.
            ("DAILY_CONSULTATION_SUMMARY", "GENERAL"),
            ("EARNING_CREDITED", "GENERAL"),
            ("SOME_UNMAPPED_FUTURE_TYPE", "GENERAL"),
        ];
        for (notification_type, expected_channel) in cases {
            assert_eq!(
                android_channel_id(notification_type, "PATIENT"),
                expected_channel,
                "PATIENT/{notification_type}"
            );
        }
    }

    /// An unrecognized/unexpected role must still resolve to a real channel
    /// (`GENERAL`, present in both apps) rather than panicking or producing
    /// an empty string — defensive, since `recipient_role` ultimately comes
    /// from a database read, not a closed Rust enum.
    #[test]
    fn unknown_role_falls_back_to_general_channel_set() {
        assert_eq!(
            android_channel_id("CONSULTATION_REMINDER_15M", "PLATFORM_ADMIN"),
            "CONSULTATION_REMINDERS"
        );
        assert_eq!(
            android_channel_id("SOME_UNMAPPED_TYPE", "PLATFORM_ADMIN"),
            "GENERAL"
        );
    }

    /// Synthetic FCM HTTP v1 payload construction — no network, no Firebase
    /// credentials. Confirms `android.notification.channel_id` is actually
    /// present in the built request body (the real defect: it was entirely
    /// absent before this change) and matches the channel-mapping function.
    #[test]
    fn built_fcm_message_carries_the_mapped_channel_id() {
        let message = build_fcm_message(
            "fake-push-token",
            "Consultation starting soon",
            "Your consultation starts in 15 minutes.",
            &options("CONSULTATION_REMINDER_15M", "DOCTOR"),
        );
        assert_eq!(
            message["message"]["android"]["notification"]["channel_id"],
            "CONSULTATION_REMINDERS"
        );
        assert_eq!(message["message"]["token"], "fake-push-token");
        assert_eq!(
            message["message"]["notification"]["title"],
            "Consultation starting soon"
        );
        // No clinical content should ever appear in `data` beyond ids/types.
        let data = message["message"]["data"].as_object().unwrap();
        assert!(data.contains_key("notification_id"));
        assert!(data.contains_key("notification_type"));
        assert!(!data.contains_key("title"));
        assert!(!data.contains_key("body"));
    }

    #[test]
    fn built_fcm_message_falls_back_to_general_for_unmapped_type_and_role() {
        let message = build_fcm_message(
            "fake-push-token",
            "Something happened",
            "Details",
            &options("SOME_BRAND_NEW_TYPE", "PATIENT"),
        );
        assert_eq!(
            message["message"]["android"]["notification"]["channel_id"],
            "GENERAL"
        );
    }

    /// Every real `notification_type` this backend can actually dispatch
    /// (the authoritative `notifications_type_check` CHECK constraint list,
    /// `migrations/20261124000001_milestone4_5_engagement_and_holidays.up.sql`)
    /// must resolve to *some* real channel for both roles — i.e. the mapping
    /// function can never be called with a type that silently breaks. This
    /// doesn't assert *which* channel (most intentionally fall through to
    /// GENERAL), only that the function is total and panic-free.
    #[test]
    fn every_known_notification_type_resolves_without_panicking() {
        let all_types = [
            "REGISTRATION_SUBMITTED",
            "REGISTRATION_VERIFIED",
            "REGISTRATION_REJECTED",
            "PROFESSIONAL_VERIFICATION_REVOKED",
            "ACCOUNT_SUSPENDED",
            "ACCOUNT_REACTIVATED",
            "ACCOUNT_UPDATED",
            "BOOKING_HOLD_CREATED",
            "BOOKING_HOLD_ONE_MINUTE",
            "BOOKING_HOLD_EXPIRED",
            "BOOKING_CONFIRMED",
            "BOOKING_POSTPONED",
            "BOOKING_RESCHEDULED",
            "BOOKING_CANCELLED",
            "PAYMENT_HOLD_UPDATED",
            "CONSULTATION_REMINDER_15M",
            "CONSULTATION_READY",
            "CONSULTATION_REJOIN",
            "CONSULTATION_INTERRUPTED",
            "CONSULTATION_NEEDS_REVIEW",
            "CHAT_MESSAGE",
            "NEW_CHAT_MESSAGE",
            "DOCTOR_REPLY",
            "PRESCRIPTION_AVAILABLE",
            "MEDICAL_RECORD_UPDATED",
            "EARNING_CREDITED",
            "PAYOUT_PAID",
            "DAILY_CONSULTATION_SUMMARY",
            "PATIENT_GREETING",
            "HOLIDAY_GREETING",
            "BIRTHDAY_GREETING",
            "WELCOME_GREETING",
            "HEALTH_EDUCATION",
            "SERVICE_ANNOUNCEMENT",
            "MAINTENANCE",
            "ADMIN_CAMPAIGN",
            "ARTICLE_STATUS_CHANGED",
            "REFUND_APPROVED",
            "REFUND_PROCESSING",
            "REFUND_COMPLETED",
            "CONSULTATION_COMPLETED",
            "CONSULTATION_RECOVERY_RESOLVED",
            "DOCTOR_GREETING",
            "FIRST_CONSULTATION_MILESTONE",
            "NEW_ARTICLE_PUBLISHED",
        ];
        for t in all_types {
            for role in ["DOCTOR", "PATIENT"] {
                let channel = android_channel_id(t, role);
                assert!(
                    !channel.is_empty(),
                    "{role}/{t} resolved to an empty channel id"
                );
            }
        }
    }
}
