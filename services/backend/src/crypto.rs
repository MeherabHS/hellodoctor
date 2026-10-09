use crate::error::AppError;
use aes_gcm::aead::{Aead, KeyInit, OsRng as AesOsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::Semaphore;

// ---------------- Password hashing (Argon2id) ----------------

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::internal(format!("Password hashing failed: {e}")))
}

fn password_semaphore() -> &'static Semaphore {
    static SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();
    SEMAPHORE.get_or_init(|| Semaphore::new(4))
}

/// Runs Argon2 on a bounded blocking pool so password work cannot starve the
/// async request runtime under registration/login load.
pub async fn hash_password_async(password: String) -> Result<String, AppError> {
    let permit = password_semaphore()
        .acquire()
        .await
        .map_err(|_| AppError::internal("Password hashing capacity unavailable.".into()))?;
    tokio::task::spawn_blocking(move || {
        let result = hash_password(&password);
        drop(permit);
        result
    })
    .await
    .map_err(|_| AppError::internal("Password hashing worker failed.".into()))?
}

pub async fn verify_password_async(password: String, hash: String) -> Result<bool, AppError> {
    let permit = password_semaphore()
        .acquire()
        .await
        .map_err(|_| AppError::internal("Password verification capacity unavailable.".into()))?;
    tokio::task::spawn_blocking(move || {
        let result = verify_password(&password, &hash);
        drop(permit);
        Ok(result)
    })
    .await
    .map_err(|_| AppError::internal("Password verification worker failed.".into()))?
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
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

// ---------------- Generic at-rest encryption (AES-256-GCM) ----------------
// Originally written for TOTP secrets; the scheme and key are not actually
// TOTP-specific, so it's reused as-is (not reimplemented) for any other
// small secret that needs to be encrypted at rest, e.g. a doctor's payout
// account identifier. `encrypt_totp_secret`/`decrypt_totp_secret` below are
// kept as named wrappers so existing call sites and the TOTP_ENCRYPTION_KEY_B64
// config name don't need to change.

fn aes_key_from_b64(key_b64: &str) -> Result<Key<Aes256Gcm>, AppError> {
    let bytes = STANDARD
        .decode(key_b64)
        .map_err(|_| AppError::internal("Invalid at-rest encryption key configuration.".into()))?;
    if bytes.len() != 32 {
        return Err(AppError::internal(
            "At-rest encryption key must be 32 bytes.".into(),
        ));
    }
    Ok(*Key::<Aes256Gcm>::from_slice(&bytes))
}

/// Returns `nonce || ciphertext`, suitable for storing directly in a `BYTEA` column.
pub fn encrypt_secret(plaintext: &str, key_b64: &str) -> Result<Vec<u8>, AppError> {
    encrypt_bytes(plaintext.as_bytes(), key_b64)
}

pub fn encrypt_bytes(plaintext: &[u8], key_b64: &str) -> Result<Vec<u8>, AppError> {
    let key = aes_key_from_b64(key_b64)?;
    let cipher = Aes256Gcm::new(&key);
    let mut nonce_bytes = [0u8; 12];
    AesOsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AppError::internal(format!("Secret encryption failed: {e}")))?;
    let mut out = nonce_bytes.to_vec();
    out.extend(ciphertext);
    Ok(out)
}

pub fn decrypt_secret(encrypted: &[u8], key_b64: &str) -> Result<String, AppError> {
    let plaintext = decrypt_bytes(encrypted, key_b64)?;
    String::from_utf8(plaintext).map_err(|e| AppError::internal(format!("Secret corrupt: {e}")))
}

pub fn decrypt_bytes(encrypted: &[u8], key_b64: &str) -> Result<Vec<u8>, AppError> {
    if encrypted.len() < 12 {
        return Err(AppError::internal("Corrupt encrypted secret.".into()));
    }
    let key = aes_key_from_b64(key_b64)?;
    let cipher = Aes256Gcm::new(&key);
    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AppError::internal(format!("Secret decryption failed: {e}")))?;
    Ok(plaintext)
}

pub fn encrypt_totp_secret(plaintext_secret: &str, key_b64: &str) -> Result<Vec<u8>, AppError> {
    encrypt_secret(plaintext_secret, key_b64)
}

pub fn decrypt_totp_secret(encrypted: &[u8], key_b64: &str) -> Result<String, AppError> {
    decrypt_secret(encrypted, key_b64)
}

// ---------------- TOTP (admin second factor) ----------------

pub fn generate_totp_secret_base32() -> String {
    totp_rs::Secret::generate_secret().to_encoded().to_string()
}

pub fn build_totp(secret_base32: &str) -> Result<totp_rs::TOTP, AppError> {
    let bytes = totp_rs::Secret::Encoded(secret_base32.to_string())
        .to_bytes()
        .map_err(|e| AppError::internal(format!("Invalid TOTP secret: {e:?}")))?;
    totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 1, 30, bytes)
        .map_err(|e| AppError::internal(format!("Failed to build TOTP: {e}")))
}

pub fn verify_totp(secret_base32: &str, code: &str) -> Result<bool, AppError> {
    let totp = build_totp(secret_base32)?;
    totp.check_current(code)
        .map_err(|e| AppError::internal(format!("TOTP time error: {e}")))
}

// ---------------- Payment webhook HMAC-SHA256 signature verification ----------------

/// Verifies `signature_hex` against HMAC-SHA256(secret, raw_body), in constant
/// time (via `hmac::Mac::verify_slice`, not a manual `==` comparison which
/// would leak timing information about how many leading bytes matched).
/// `raw_body` must be the exact, unparsed request bytes — verifying against a
/// re-serialized JSON value would accept a payload the provider never signed.
pub fn verify_webhook_signature(raw_body: &[u8], signature_hex: &str, secret: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let Ok(sig_bytes) = hex::decode(signature_hex.trim()) else {
        return false;
    };
    let Ok(mut mac) = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(raw_body);
    mac.verify_slice(&sig_bytes).is_ok()
}

// ---------------- Ed25519 JWT access tokens ----------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub user_id: String,
    /// Stable refresh-token family. Session-less legacy JWTs must refresh or log in again.
    pub sid: String,
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
    token_family_id: uuid::Uuid,
    private_key_der_b64: &str,
) -> Result<String, AppError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        user_id: user_id.to_string(),
        sid: token_family_id.to_string(),
        role: role.to_string(),
        permissions: default_permissions_for_role(role),
        iss: JWT_ISSUER.to_string(),
        aud: JWT_AUDIENCE.to_string(),
        exp: now + ACCESS_TOKEN_TTL_SECONDS as usize,
        iat: now,
    };

    let der = STANDARD
        .decode(private_key_der_b64)
        .map_err(|_| AppError::internal("Invalid JWT signing key configuration.".into()))?;
    let key = EncodingKey::from_ed_der(&der);
    let header = Header::new(jsonwebtoken::Algorithm::EdDSA);

    encode(&header, &claims, &key)
        .map_err(|e| AppError::internal(format!("JWT signing failed: {e}")))
}

/// Verifies signature, expiry, issuer and audience on an incoming bearer token.
/// This is the function that makes authentication actually enforced at request time —
/// previously tokens were issued but never checked on the way back in.
pub fn verify_access_token(token: &str, public_key_der_b64: &str) -> Result<Claims, AppError> {
    let der = STANDARD
        .decode(public_key_der_b64)
        .map_err(|_| AppError::internal("Invalid JWT verification key configuration.".into()))?;

    // `jsonwebtoken::DecodingKey::from_ed_der` is misleadingly named: despite
    // "der" in the name, it does no ASN.1 parsing at all — it stores the bytes
    // verbatim and hands them straight to ring's `UnparsedPublicKey`, which for
    // Ed25519 requires exactly the 32-byte raw public key, not a DER structure.
    // `openssl pkey ... -pubout` (per .env.example) produces a 44-byte SPKI-DER
    // key (a fixed 12-byte header — SEQUENCE/AlgorithmIdentifier for OID
    // 1.3.101.112 — followed by the 32-byte raw key), which must be unwrapped
    // here or every signature verification fails with `InvalidSignature` even
    // though signing (which genuinely does PKCS8 parsing for the private key)
    // works fine — confirmed by an isolated sign/verify roundtrip during
    // implementation, not a hypothetical.
    let raw_pubkey: &[u8] = match der.len() {
        32 => &der,
        44 => &der[12..],
        other => {
            return Err(AppError::internal(format!(
                "JWT verification key has unexpected length {other} bytes (expected 32 raw or 44 SPKI-DER)."
            )))
        }
    };
    let key = DecodingKey::from_ed_der(raw_pubkey);

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_issuer(&[JWT_ISSUER]);
    validation.set_audience(&[JWT_AUDIENCE]);
    validation.validate_exp = true;

    decode::<Claims>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| AppError::Unauthorized(format!("Invalid or expired access token: {e}")))
}
