use crate::error::AppError;
use aes_gcm::aead::{Aead, KeyInit, OsRng as AesOsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

/// The exact message for an access token that is genuine but past its
/// 15-minute lifetime. Clients refresh and retry on it, so the incident log
/// skips it (`api/middleware/incident_reporting.rs`); a forged, tampered or
/// malformed token keeps the "Invalid or expired access token" message and
/// is still recorded.
pub const EXPIRED_ACCESS_TOKEN: &str = "Access token expired.";
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

struct PasswordWorkGuard;
impl PasswordWorkGuard {
    fn start() -> Self {
        metrics::gauge!("argon2_active_workers").increment(1.0);
        metrics::counter!("argon2_operations_total").increment(1);
        Self
    }
}
impl Drop for PasswordWorkGuard {
    fn drop(&mut self) {
        metrics::gauge!("argon2_active_workers").decrement(1.0);
    }
}

/// Runs Argon2 on a bounded blocking pool so password work cannot starve the
/// async request runtime under registration/login load.
pub async fn hash_password_async(password: String) -> Result<String, AppError> {
    let permit = password_semaphore().try_acquire().map_err(|_| {
        AppError::CapacityLimitReached("Sign-in is temporarily busy. Please retry shortly.".into())
    })?;
    tokio::task::spawn_blocking(move || {
        let _work = PasswordWorkGuard::start();
        let result = hash_password(&password);
        drop(permit);
        result
    })
    .await
    .map_err(|_| AppError::internal("Password hashing worker failed.".into()))?
}

pub async fn verify_password_async(password: String, hash: String) -> Result<bool, AppError> {
    let permit = password_semaphore().try_acquire().map_err(|_| {
        AppError::CapacityLimitReached("Sign-in is temporarily busy. Please retry shortly.".into())
    })?;
    tokio::task::spawn_blocking(move || {
        let _work = PasswordWorkGuard::start();
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

static DUMMY_PASSWORD_HASH: OnceLock<String> = OnceLock::new();

/// A real Argon2id hash of a fixed, non-secret placeholder password, computed
/// once per process (not per request, and not a real user's hash).
fn dummy_password_hash() -> &'static str {
    DUMMY_PASSWORD_HASH.get_or_init(|| {
        hash_password("hellodoctor-constant-time-login-padding-placeholder")
            .expect("dummy password hashing must succeed")
    })
}

/// Runs a real Argon2id verification against the precomputed dummy hash, at
/// the same cost and on the same bounded worker pool as a genuine
/// `verify_password_async` call. Call this on every login fast-exit path
/// (account not found, wrong role, inactive, no password hash set) so those
/// paths cost comparable CPU time to a real wrong-password check instead of
/// returning near-instantly — removing the obvious
/// "Argon2id-ran vs. did-not-run" branch that an account-enumeration timing
/// oracle would otherwise exploit. This does not guarantee perfect
/// network-observed constant time, only that the deterministic backend-side
/// cost gap is closed.
pub async fn verify_password_dummy_async() -> Result<(), AppError> {
    let permit = password_semaphore().try_acquire().map_err(|_| {
        AppError::CapacityLimitReached("Sign-in is temporarily busy. Please retry shortly.".into())
    })?;
    tokio::task::spawn_blocking(move || {
        let _work = PasswordWorkGuard::start();
        // First-use dummy initialization is itself Argon2 work. Keep it inside
        // the admitted blocking worker, including under concurrent cold login.
        let _ = verify_password("not-the-real-password", dummy_password_hash());
        drop(permit);
    })
    .await
    .map_err(|_| AppError::internal("Password verification worker failed.".into()))?;
    Ok(())
}

#[cfg(test)]
mod password_capacity_tests {
    use super::*;

    // These tests deliberately saturate the process-wide admission semaphore.
    // Serialize only this test module, without changing production admission.
    static PASSWORD_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn all_password_paths_fail_fast_without_worker_queue() {
        let _serial = PASSWORD_TEST_LOCK.lock().await;
        let _busy = password_semaphore().try_acquire_many(4).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_millis(100), async {
            assert!(matches!(
                hash_password_async("synthetic".into()).await,
                Err(AppError::CapacityLimitReached(_))
            ));
            assert!(matches!(
                verify_password_async("synthetic".into(), "not-a-hash".into()).await,
                Err(AppError::CapacityLimitReached(_))
            ));
            assert!(matches!(
                verify_password_dummy_async().await,
                Err(AppError::CapacityLimitReached(_))
            ));
        })
        .await;
        assert!(
            result.is_ok(),
            "excess password work must not queue on permits"
        );
    }

    fn assert_cancelled_caller_keeps_worker_permit<F, T>(work: F)
    where
        F: std::future::Future<Output = Result<T, AppError>> + Send + 'static,
        T: Send + 'static,
    {
        let _serial = PASSWORD_TEST_LOCK.blocking_lock();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async move {
            // A drop guard releases the controlled worker even if an assertion
            // fails, so shutting down this private runtime cannot hang.
            struct ReleaseWorker(Option<std::sync::mpsc::Sender<()>>);
            impl Drop for ReleaseWorker {
                fn drop(&mut self) {
                    if let Some(sender) = self.0.take() {
                        let _ = sender.send(());
                    }
                }
            }

            let deadline = std::time::Duration::from_secs(30);
            let held = password_semaphore().try_acquire_many(3).unwrap();
            let (release, wait) = std::sync::mpsc::channel();
            let mut release = ReleaseWorker(Some(release));
            let (started, started_rx) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                wait.recv().unwrap();
            });
            tokio::time::timeout(deadline, started_rx)
                .await
                .expect("controlled blocking worker must start")
                .unwrap();

            // Signal only after polling the actual production future. It has
            // acquired the fourth permit and queued its blocking closure while
            // the sole blocking thread remains under our control.
            let (polled, polled_rx) = tokio::sync::oneshot::channel();
            let caller = tokio::spawn(async move {
                let mut work = Box::pin(work);
                let mut polled = Some(polled);
                std::future::poll_fn(|context| {
                    let result = std::future::Future::poll(work.as_mut(), context);
                    if let Some(sender) = polled.take() {
                        let _ = sender.send(result.is_pending());
                    }
                    result
                })
                .await
            });
            assert!(tokio::time::timeout(deadline, polled_rx)
                .await
                .expect("password future must be polled")
                .unwrap());
            assert_eq!(password_semaphore().available_permits(), 0);

            caller.abort();
            assert!(matches!(caller.await, Err(error) if error.is_cancelled()));
            assert_eq!(
                password_semaphore().available_permits(),
                0,
                "cancelled request must not release its queued blocking work permit"
            );
            assert!(matches!(
                hash_password_async("synthetic excess request".into()).await,
                Err(AppError::CapacityLimitReached(_))
            ));

            // Wait on the permit-release event itself, after allowing the real
            // blocking computation to finish. No scheduler sleeps are needed.
            release.0.take().unwrap().send(()).unwrap();
            tokio::time::timeout(deadline, blocker)
                .await
                .expect("controlled worker must finish")
                .unwrap();
            let recovered = tokio::time::timeout(deadline, password_semaphore().acquire())
                .await
                .expect("actual password work must complete after caller cancellation")
                .unwrap();
            assert_eq!(password_semaphore().available_permits(), 0);
            drop(recovered);
            assert_eq!(password_semaphore().available_permits(), 1);
            drop(held);
            assert_eq!(password_semaphore().available_permits(), 4);
        });
    }

    #[test]
    fn cancelled_hash_request_keeps_permit_until_blocking_work_finishes() {
        assert_cancelled_caller_keeps_worker_permit(hash_password_async("synthetic".into()));
    }

    #[test]
    fn cancelled_verify_request_keeps_permit_until_blocking_work_finishes() {
        let hash = hash_password("synthetic").unwrap();
        assert_cancelled_caller_keeps_worker_permit(verify_password_async(
            "synthetic".into(),
            hash,
        ));
    }

    #[test]
    fn cancelled_dummy_request_keeps_permit_until_blocking_work_finishes() {
        assert_cancelled_caller_keeps_worker_permit(verify_password_dummy_async());
    }
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

/// Remediation E4 (Phase 1 finding #4 / Phase 5): byte-wise constant-time
/// equality for comparing digests (OTP/MFA code hashes, HMACs) so an
/// attacker measuring response latency across many attempts cannot use
/// early-exit `==` comparison to infer how many leading bytes matched.
/// Moved here from `services/dr_erasure_journal.rs` (which already had its
/// own private copy for HMAC verification) so both call sites share one
/// reviewed implementation instead of two independently-written ones.
pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes()
        .iter()
        .zip(b.as_bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub fn verify_code(code: &str, hash: &str) -> bool {
    constant_time_eq(&hash_code(code), hash)
}

// ---------------- Generic at-rest encryption (AES-256-GCM) ----------------
// Shared by Admin TOTP enrollment and small financial identifiers such as
// Doctor payout accounts. TOTP_ENCRYPTION_KEY_B64 remains the established
// configuration name for this at-rest encryption key.

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
        "ADMIN" => vec!["admin:*".into()],
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
    let mut header = Header::new(jsonwebtoken::Algorithm::EdDSA);
    // `kid` names the signing key so verification can pick the right one
    // while an old key is still accepted during a rotation.
    header.kid = Some(key_id_for_private_key(&der)?);

    encode(&header, &claims, &key)
        .map_err(|e| AppError::internal(format!("JWT signing failed: {e}")))
}

/// Stable, public identifier of an Ed25519 verification key: the first 16
/// hex characters of SHA-256 over the raw 32-byte public key. It reveals
/// nothing beyond the public key itself.
fn key_id_for_raw_public_key(raw: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(raw);
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// The `kid` of a configured public key (base64 DER, 32 raw or 44 SPKI bytes).
pub fn jwt_key_id(public_key_der_b64: &str) -> Result<String, AppError> {
    let der = STANDARD
        .decode(public_key_der_b64)
        .map_err(|_| AppError::internal("Invalid JWT verification key configuration.".into()))?;
    Ok(key_id_for_raw_public_key(raw_public_key(&der)?))
}

/// Derives the `kid` from a PKCS#8 Ed25519 private key. In both PKCS#8 v1
/// (48 bytes) and v2 the 32-byte seed sits at bytes 16..48 behind an
/// OCTET STRING header `04 20`.
fn key_id_for_private_key(der: &[u8]) -> Result<String, AppError> {
    if der.len() < 48 || der[14] != 0x04 || der[15] != 0x20 {
        return Err(AppError::internal(
            "JWT signing key is not a PKCS#8 Ed25519 key.".into(),
        ));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&der[16..48]);
    let public = ed25519_dalek::SigningKey::from_bytes(&seed)
        .verifying_key()
        .to_bytes();
    Ok(key_id_for_raw_public_key(&public))
}

/// Verifies signature, expiry, issuer and audience on an incoming bearer token.
/// This is the function that makes authentication actually enforced at request time —
/// previously tokens were issued but never checked on the way back in.
pub fn verify_access_token(token: &str, public_key_der_b64: &str) -> Result<Claims, AppError> {
    verify_access_token_with_keys(token, public_key_der_b64, &[])
}

/// Key rotation: tokens name their signing key in `kid`. A token is checked
/// against the current key or one of `previous_public_keys_der_b64` (old keys
/// kept for at most one access-token lifetime after a rotation), chosen by
/// `kid`; an unknown `kid` is refused. A token without `kid` (issued before
/// this change) is checked against the current key only.
pub fn verify_access_token_with_keys(
    token: &str,
    public_key_der_b64: &str,
    previous_public_keys_der_b64: &[String],
) -> Result<Claims, AppError> {
    let header = jsonwebtoken::decode_header(token)
        .map_err(|e| AppError::Unauthorized(format!("Invalid or expired access token: {e}")))?;
    let key_b64 = match header.kid.as_deref() {
        None => public_key_der_b64,
        Some(kid) => std::iter::once(public_key_der_b64)
            .chain(previous_public_keys_der_b64.iter().map(String::as_str))
            .find(|candidate| jwt_key_id(candidate).is_ok_and(|id| id == kid))
            .ok_or_else(|| {
                AppError::Unauthorized(
                    "Invalid or expired access token: unknown signing key.".into(),
                )
            })?,
    };
    verify_with_single_key(token, key_b64)
}

fn raw_public_key(der: &[u8]) -> Result<&[u8], AppError> {
    match der.len() {
        32 => Ok(der),
        44 => Ok(&der[12..]),
        other => Err(AppError::internal(format!(
            "JWT verification key has unexpected length {other} bytes (expected 32 raw or 44 SPKI-DER)."
        ))),
    }
}

fn verify_with_single_key(token: &str, public_key_der_b64: &str) -> Result<Claims, AppError> {
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
    let key = DecodingKey::from_ed_der(raw_public_key(&der)?);

    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_issuer(&[JWT_ISSUER]);
    validation.set_audience(&[JWT_AUDIENCE]);
    validation.validate_exp = true;

    decode::<Claims>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            // Signature, issuer and audience were already checked before
            // expiry, so this is a genuine token that simply aged out.
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                AppError::Unauthorized(EXPIRED_ACCESS_TOKEN.into())
            }
            _ => AppError::Unauthorized(format!("Invalid or expired access token: {e}")),
        })
}

#[cfg(test)]
mod jwt_key_rotation_tests {
    use super::*;

    /// A fresh Ed25519 pair in the production shapes: PKCS#8 v1 private key
    /// and 44-byte SPKI public key, both base64.
    fn keypair(seed: u8) -> (String, String) {
        let seed = [seed; 32];
        let mut private = hex::decode("302e020100300506032b657004220420").unwrap();
        private.extend_from_slice(&seed);
        let mut public = hex::decode("302a300506032b6570032100").unwrap();
        public.extend_from_slice(
            &ed25519_dalek::SigningKey::from_bytes(&seed)
                .verifying_key()
                .to_bytes(),
        );
        (STANDARD.encode(private), STANDARD.encode(public))
    }

    fn token(private: &str) -> String {
        let id = uuid::Uuid::new_v4();
        create_access_token(id, "ADMIN", id, private).unwrap()
    }

    #[test]
    fn tokens_name_their_signing_key() {
        let (private, public) = keypair(1);
        let header = jsonwebtoken::decode_header(&token(&private)).unwrap();
        assert_eq!(header.kid.unwrap(), jwt_key_id(&public).unwrap());
        assert_eq!(jwt_key_id(&public).unwrap().len(), 16);
    }

    #[test]
    fn a_rotated_out_key_is_accepted_only_while_listed_as_previous() {
        let (old_private, old_public) = keypair(1);
        let (_, new_public) = keypair(2);
        let old_token = token(&old_private);
        assert!(verify_access_token_with_keys(
            &old_token,
            &new_public,
            std::slice::from_ref(&old_public)
        )
        .is_ok());
        assert!(matches!(
            verify_access_token_with_keys(&old_token, &new_public, &[]),
            Err(AppError::Unauthorized(_))
        ));
    }

    #[test]
    fn a_token_from_an_unknown_key_is_refused() {
        let (stranger_private, _) = keypair(9);
        let (_, public) = keypair(1);
        assert!(matches!(
            verify_access_token(&token(&stranger_private), &public),
            Err(AppError::Unauthorized(_))
        ));
    }

    /// Only a genuine token past its lifetime gets the "expired" message
    /// that the incident log skips. A forged token that is also expired must
    /// still read as invalid, so it stays on the incident log.
    #[test]
    fn only_genuine_expired_tokens_are_reported_as_expired() {
        let (private, public) = keypair(1);
        let (forger_private, _) = keypair(7);
        let past = chrono::Utc::now().timestamp() as usize - 600;
        let expired_claims = Claims {
            sub: "u".into(),
            user_id: uuid::Uuid::new_v4().to_string(),
            sid: uuid::Uuid::new_v4().to_string(),
            role: "PATIENT".into(),
            permissions: vec![],
            iss: JWT_ISSUER.into(),
            aud: JWT_AUDIENCE.into(),
            exp: past,
            iat: past - 900,
        };
        let sign = |private: &str| {
            let key = EncodingKey::from_ed_der(&STANDARD.decode(private).unwrap());
            encode(&Header::new(Algorithm::EdDSA), &expired_claims, &key).unwrap()
        };
        match verify_access_token(&sign(&private), &public) {
            Err(AppError::Unauthorized(message)) => assert_eq!(message, EXPIRED_ACCESS_TOKEN),
            other => panic!("expected the expired message, got {other:?}"),
        }
        match verify_access_token(&sign(&forger_private), &public) {
            Err(AppError::Unauthorized(message)) => {
                assert_ne!(message, EXPIRED_ACCESS_TOKEN);
                assert!(
                    message.starts_with("Invalid or expired access token"),
                    "{message}"
                );
            }
            other => panic!("a forged token must be refused as invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_legacy_token_without_kid_is_checked_against_the_current_key_only() {
        let (private, public) = keypair(1);
        let (_, other_public) = keypair(2);
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: "u".into(),
            user_id: uuid::Uuid::new_v4().to_string(),
            sid: uuid::Uuid::new_v4().to_string(),
            role: "ADMIN".into(),
            permissions: vec![],
            iss: JWT_ISSUER.into(),
            aud: JWT_AUDIENCE.into(),
            exp: now + 60,
            iat: now,
        };
        let key = EncodingKey::from_ed_der(&STANDARD.decode(&private).unwrap());
        let legacy = encode(&Header::new(Algorithm::EdDSA), &claims, &key).unwrap();
        assert!(verify_access_token(&legacy, &public).is_ok());
        assert!(verify_access_token_with_keys(
            &legacy,
            &other_public,
            std::slice::from_ref(&public)
        )
        .is_err());
    }
}

#[cfg(test)]
mod remediation_e4_constant_time_tests {
    use super::*;

    #[test]
    fn constant_time_eq_accepts_identical_strings() {
        assert!(constant_time_eq("abc123", "abc123"));
    }

    #[test]
    fn constant_time_eq_rejects_different_values_same_length() {
        assert!(!constant_time_eq("abc123", "abc124"));
    }

    #[test]
    fn constant_time_eq_rejects_different_lengths() {
        assert!(!constant_time_eq("short", "muchlonger"));
    }

    #[test]
    fn constant_time_eq_rejects_empty_vs_nonempty() {
        assert!(!constant_time_eq("", "x"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn verify_code_uses_constant_time_comparison_and_still_verifies_correctly() {
        let hash = hash_code("913627");
        assert!(verify_code("913627", &hash));
        assert!(!verify_code("000000", &hash));
        assert!(!verify_code("9136270", &hash));
    }
}
