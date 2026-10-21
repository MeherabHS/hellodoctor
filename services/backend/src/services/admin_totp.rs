//! RFC 6238 HMAC-SHA256 TOTP. Existing SHA1 enrollments are never reinterpreted.
use crate::{crypto, error::AppError, repository::AppState};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn decode_base32(secret: &str) -> Result<Vec<u8>, AppError> {
    let mut bits = 0u32;
    let mut count = 0u32;
    let mut result = Vec::new();
    if secret.len() > 128 {
        return Err(AppError::Unauthorized(
            "Verification is unavailable.".into(),
        ));
    }
    for byte in secret.trim_end_matches('=').bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => {
                return Err(AppError::Unauthorized(
                    "Verification is unavailable.".into(),
                ))
            }
        };
        bits = (bits << 5) | u32::from(value);
        count += 5;
        if count >= 8 {
            count -= 8;
            result.push((bits >> count) as u8);
        }
    }
    if result.len() < 20 || count > 0 && (bits & ((1 << count) - 1)) != 0 {
        return Err(AppError::Unauthorized(
            "Verification is unavailable.".into(),
        ));
    }
    Ok(result)
}

fn code(secret: &[u8], step: u64, digits: u32) -> u32 {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&step.to_be_bytes());
    let bytes = mac.finalize().into_bytes();
    let offset = (bytes[bytes.len() - 1] & 15) as usize;
    let value = u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("SHA256 truncation"),
    ) & 0x7fff_ffff;
    value % 10u32.pow(digits)
}

pub async fn verify(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: crate::domain::ids::UserId,
    submitted: Option<&str>,
) -> Result<(), AppError> {
    if state.config.environment == "development" && state.config.dev_auth_otp_enabled {
        return Ok(());
    }
    let denied =
        || AppError::Unauthorized("We couldn't complete sign-in. Please try again.".into());
    let value = submitted
        .filter(|v| v.len() == 6 && v.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(denied)?;
    let credential: Option<(uuid::Uuid, Vec<u8>, Option<i64>, String)> = sqlx::query_as(
        "SELECT id,totp_secret_encrypted,last_totp_step,totp_algorithm FROM mfa_credentials \
         WHERE user_id=$1 AND method='TOTP' AND is_enabled AND revoked_at IS NULL FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (id, encrypted, last_step, algorithm) = credential.ok_or_else(denied)?;
    if algorithm != "SHA256" {
        return Err(denied());
    }
    let secret = crypto::decrypt_secret(&encrypted, &state.config.totp_encryption_key_b64)?;
    let secret = decode_base32(&secret)?;
    let now_step = chrono::Utc::now().timestamp() / 30;
    let accepted = [now_step - 1, now_step, now_step + 1]
        .into_iter()
        .filter(|step| last_step.is_none_or(|last| *step > last))
        .find(|step| code(&secret, *step as u64, 6) == value)
        .ok_or_else(denied)?;
    sqlx::query("UPDATE mfa_credentials SET last_totp_step=$2,last_verified_at=now() WHERE id=$1")
        .bind(id)
        .bind(accepted)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc6238_sha256_vectors() {
        let secret = b"12345678901234567890123456789012";
        for (time, expected) in [
            (59, 46119246),
            (1111111109, 68084774),
            (1111111111, 67062674),
            (1234567890, 91819424),
            (2000000000, 90698825),
            (20000000000, 77737706),
        ] {
            assert_eq!(code(secret, time / 30, 8), expected);
        }
    }
}
