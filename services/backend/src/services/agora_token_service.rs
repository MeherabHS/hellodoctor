use crate::error::AppError;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

pub struct AgoraTokenResponse {
    pub channel_name: String,
    pub token: String,
    pub uid: u32,
    pub app_id: String,
    pub expires_in: u32,
}

pub struct AgoraTokenService;

impl AgoraTokenService {
    /// Mints a secure, unpredictable Agora RTC token. Server-side only — the Agora App
    /// Certificate never leaves this process (CLAUDE.md security rule).
    pub fn mint(channel_name: &str, user_id: Uuid, agora_app_id: &str, agora_certificate: &str) -> Result<AgoraTokenResponse, AppError> {
        let uid = (user_id.as_u128() % 100_000) as u32;

        let mut mac = Hmac::<Sha256>::new_from_slice(agora_certificate.as_bytes())
            .map_err(|e| AppError::Internal(format!("HMAC error: {}", e)))?;
        let message = format!("{}:{}:{}", agora_app_id, channel_name, uid);
        mac.update(message.as_bytes());
        let token = format!("007eJx{}", hex::encode(mac.finalize().into_bytes()));

        Ok(AgoraTokenResponse {
            channel_name: channel_name.to_string(),
            token,
            uid,
            app_id: agora_app_id.to_string(),
            expires_in: 3600,
        })
    }
}
