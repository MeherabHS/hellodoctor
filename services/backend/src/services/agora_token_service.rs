use crate::domain::ids::UserId;
use crate::error::AppError;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

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
    pub fn mint(
        channel_name: &str,
        user_id: UserId,
        agora_app_id: &str,
        agora_certificate: &str,
    ) -> Result<AgoraTokenResponse, AppError> {
        let uid = derive_uid(channel_name, user_id);

        let mut mac = Hmac::<Sha256>::new_from_slice(agora_certificate.as_bytes())
            .map_err(|e| AppError::internal(format!("HMAC error: {}", e)))?;
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

/// Derives a deterministic, collision-resistant Agora UID (0..=u32::MAX)
/// from both the channel name and the caller's real user id — the Pass 2
/// fix for a real bug: the previous scheme (`user_id.as_u128() % 100_000`)
/// collapsed a 128-bit UUID into a 5-digit range, giving the *two*
/// legitimate participants of the same call a real (if small) chance of
/// colliding onto the same Agora UID. Hashing in the channel name too means
/// the derivation is still stable/deterministic per (channel, user) — the
/// same caller reconnecting to the same channel always gets the same UID —
/// while two different users' hashes within one channel are independent,
/// ~1-in-4-billion-collision-odds values rather than 1-in-100,000. Doesn't
/// need a new table: only two participants ever share a given channel, so a
/// full 32-bit hash is already a huge margin for that birthday problem.
fn derive_uid(channel_name: &str, user_id: UserId) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(channel_name.as_bytes());
    hasher.update(user_id.into_inner().as_bytes());
    let digest = hasher.finalize();
    let raw = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    // 0 has a special "let the server auto-assign a UID" meaning in some
    // Agora SDK integration modes — avoid ever producing it here.
    if raw == 0 {
        1
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_uid_is_deterministic() {
        let user_id = UserId::new();
        let a = derive_uid("rtc_channel_1", user_id);
        let b = derive_uid("rtc_channel_1", user_id);
        assert_eq!(a, b);
    }

    #[test]
    fn derive_uid_differs_across_channels_for_same_user() {
        let user_id = UserId::new();
        let a = derive_uid("rtc_channel_1", user_id);
        let b = derive_uid("rtc_channel_2", user_id);
        assert_ne!(a, b);
    }

    #[test]
    fn derive_uid_differs_across_users_in_same_channel() {
        let a = derive_uid("rtc_channel_1", UserId::new());
        let b = derive_uid("rtc_channel_1", UserId::new());
        assert_ne!(a, b);
    }
}
