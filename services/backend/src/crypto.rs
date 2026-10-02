use crate::error::AppError;
use aes_gcm::aead::{Aead, KeyInit, OsRng as AesOsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use jsonwebtoken::{encode, EncodingKey, Header};
use rand::RngCore;
use serde::{Deserialize, Serialize};

// ---------------- Password hashing (Argon2id) ----------------

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

// ---------------- Short-lived numeric code hashing (OTP / pending MFA challenge) ----------------

pub fn generate_numeric_code(digits: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..digits)
        .map(|_| char::from_digit(rng.gen_range(0..10), 10).unwrap())
        .collect()
}

pub fn hash_code(code: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(code.as_bytes()))
}

pub fn verify_code(code: &str, hash: &str) -> bool {
    hash_code(code) == hash
}

// ---------------- TOTP secret encryption at rest (AES-256-GCM) ----------------

fn totp_key(key_b64: &str) -> Result<Key<Aes256Gcm>, AppError> {
    let bytes = STANDARD
        .decode(key_b64)
        .map_err(|_| AppError::Internal("Invalid TOTP encryption key configuration.".into()))?;
    if bytes.len() != 32 {
        return Err(AppError::Internal("TOTP encryption key must be 32 bytes.".into()));
    }
    Ok(*Key::<Aes256Gcm>::from_slice(&bytes))
}

/// Returns `nonce || ciphertext`, suitable for storing directly in `totp_secret_encrypted BYTEA`.
pub fn encrypt_totp_secret(plaintext_secret: &str, key_b64: &str) -> Result<Vec<u8>, AppError> {
    let key = totp_key(key_b64)?;
    let cipher = Aes256Gcm::new(&key);
    let mut nonce_bytes = [0u8; 12];
    AesOsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext_secret.as_bytes())
        .map_err(|e| AppError::Internal(format!("TOTP secret encryption failed: {e}")))?;
    let mut out = nonce_bytes.to_vec();
    out.extend(ciphertext);
    Ok(out)
}

pub fn decrypt_totp_secret(encrypted: &[u8], key_b64: &str) -> Result<String, AppError> {
    if encrypted.len() < 12 {
        return Err(AppError::Internal("Corrupt encrypted TOTP secret.".into()));
    }
    let key = totp_key(key_b64)?;
    let cipher = Aes256Gcm::new(&key);
    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AppError::Internal(format!("TOTP secret decryption failed: {e}")))?;
    String::from_utf8(plaintext).map_err(|e| AppError::Internal(format!("TOTP secret corrupt: {e}")))
}

// ---------------- TOTP (admin second factor) ----------------

pub fn generate_totp_secret_base32() -> String {
    totp_rs::Secret::generate_secret().to_encoded().to_string()
}

pub fn build_totp(secret_base32: &str) -> Result<totp_rs::TOTP, AppError> {
    let bytes = totp_rs::Secret::Encoded(secret_base32.to_string())
        .to_bytes()
        .map_err(|e| AppError::Internal(format!("Invalid TOTP secret: {e:?}")))?;
    totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 1, 30, bytes)
        .map_err(|e| AppError::Internal(format!("Failed to build TOTP: {e}")))
}

pub fn verify_totp(secret_base32: &str, code: &str) -> Result<bool, AppError> {
    let totp = build_totp(secret_base32)?;
    totp.check_current(code)
        .map_err(|e| AppError::Internal(format!("TOTP time error: {e}")))
}

// ---------------- Ed25519 JWT access tokens ----------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub user_id: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}

const JWT_ISSUER: &str = "hellodoctor-backend";
const JWT_AUDIENCE: &str = "hellodoctor-mobile";
const ACCESS_TOKEN_TTL_SECONDS: i64 = 15 * 60;

pub fn default_permissions_for_role(role: &str) -> Vec<String> {
    match role {
        "PATIENT" => vec!["patient:self".into()],
        "DOCTOR" => vec!["doctor:self".into(), "consultation:conduct".into()],
        "PLATFORM_ADMIN" => vec!["admin:*".into()],
        "FINANCE_ADMIN" => vec!["admin:finance".into()],
        "CLINICAL_ADMIN" => vec!["admin:clinical".into()],
        "COMPLIANCE" => vec!["admin:compliance".into()],
        "SECURITY_ADMIN" => vec!["admin:security".into()],
        "SUPPORT" => vec!["admin:support".into()],
        _ => vec![],
    }
}

pub fn create_access_token(
    user_id: uuid::Uuid,
    role: &str,
    private_key_der_b64: &str,
) -> Result<String, AppError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        user_id: user_id.to_string(),
        role: role.to_string(),
        permissions: default_permissions_for_role(role),
        iss: JWT_ISSUER.to_string(),
        aud: JWT_AUDIENCE.to_string(),
        exp: now + ACCESS_TOKEN_TTL_SECONDS as usize,
        iat: now,
    };

    let der = STANDARD
        .decode(private_key_der_b64)
        .map_err(|_| AppError::Internal("Invalid JWT signing key configuration.".into()))?;
    let key = EncodingKey::from_ed_der(&der);
    let header = Header::new(jsonwebtoken::Algorithm::EdDSA);

    encode(&header, &claims, &key).map_err(|e| AppError::Internal(format!("JWT signing failed: {e}")))
}
