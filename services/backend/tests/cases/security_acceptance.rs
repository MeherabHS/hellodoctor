//! Runtime security acceptance using the parent suite's disposable capability
//! pools and real Redis. No provider calls or production authentication bypass.
use super::*;
use axum::http::{HeaderMap, Method, Uri};
use hellodoctor_backend::crypto;
use hellodoctor_backend::error::AppError;
use hellodoctor_backend::repository::session_repo;
use hellodoctor_backend::services::auth_service::{device_proof, AuthService};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

#[sqlx::test(migrations = "./migrations")]
async fn every_capability_role_is_denied_direct_economic_journal_mutation(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM financial_economic_effects")
        .fetch_one(&pool)
        .await
        .unwrap();
    for role_pool in [
        &state.db,
        &state.auth_db,
        &state.payment_db,
        &state.system_db,
        &state.finance_db,
    ] {
        for statement in [
            "INSERT INTO financial_economic_effects DEFAULT VALUES",
            "UPDATE financial_economic_effects SET effect_type=effect_type",
            "DELETE FROM financial_economic_effects",
        ] {
            let error = sqlx::query(statement).execute(role_pool).await.unwrap_err();
            assert_eq!(
                error.as_database_error().and_then(|e| e.code()).as_deref(),
                Some("42501"),
                "application capability roles must not write the economic journal directly"
            );
        }
    }
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM financial_economic_effects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    let forced: bool = sqlx::query_scalar("SELECT relrowsecurity AND relforcerowsecurity FROM pg_class WHERE oid='financial_economic_effects'::regclass")
        .fetch_one(&pool).await.unwrap();
    assert!(forced);
}

const SYNTHETIC_TOTP_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const SYNTHETIC_OTP: &str = "123456";
const SYNTHETIC_WEB_AUTH_ASSERTION_SECRET: &str =
    "test-web-auth-assertion-secret-dev-only-not-real";
// Public jsonwebtoken ECDSA fixture, never an installation/production key.
const SYNTHETIC_EC_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWTFfCGljY6aw3Hrt\nkHmPRiazukxPLb6ilpRAewjW8nihRANCAATDskChT+Altkm9X7MI69T3IUmrQU0L\n950IxEzvw/x5BMEINRMrXLBJhqzO9Bm+d6JbqA21YQmd1Kt4RzLJR1W+\n-----END PRIVATE KEY-----";
const KEY_X: &str = "w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ";
const KEY_Y: &str = "wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4";

struct EnvVarOverride {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarOverride {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarOverride {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn synthetic_totp(step: i64) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(b"12345678901234567890").unwrap();
    mac.update(&(step as u64).to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[31] & 15);
    let truncated =
        u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", truncated % 1_000_000)
}

async fn strict_admin_state(pool: PgPool) -> (PgPool, AppState) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    let mut config = (*state.config).clone();
    config.environment = "test".into();
    config.dev_auth_otp_enabled = false;
    state.config = Arc::new(config);
    (pool, state)
}

async fn enroll_synthetic_totp(pool: &PgPool, state: &AppState, algorithm: &str) {
    sqlx::query("DELETE FROM mfa_credentials WHERE user_id=$1")
        .bind(uid(ADMIN_USER_ID))
        .execute(pool)
        .await
        .unwrap();
    let encrypted =
        crypto::encrypt_secret(SYNTHETIC_TOTP_SECRET, &state.config.totp_encryption_key_b64)
            .unwrap();
    sqlx::query("INSERT INTO mfa_credentials(id,user_id,method,totp_secret_encrypted,is_enabled,totp_algorithm) VALUES($1,$2,'TOTP',$3,true,$4)")
        .bind(Uuid::new_v4()).bind(uid(ADMIN_USER_ID)).bind(encrypted).bind(algorithm)
        .execute(pool).await.unwrap();
}

async fn admin_challenge(pool: &PgPool) -> String {
    let token = Uuid::new_v4().to_string();
    session_repo::insert_pending_challenge(
        pool,
        &token,
        UserId::from(uid(ADMIN_USER_ID)),
        &crypto::hash_code(SYNTHETIC_OTP),
        Utc::now() + Duration::minutes(5),
    )
    .await
    .unwrap();
    token
}

async fn admin_session_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM auth_sessions WHERE user_id=$1")
        .bind(uid(ADMIN_USER_ID))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_totp_mandatory_algorithm_window_and_account_fail_closed(pool: PgPool) {
    let (pool, state) = strict_admin_state(pool).await;
    enroll_synthetic_totp(&pool, &state, "SHA256").await;
    let before = admin_session_count(&pool).await;
    let now = Utc::now().timestamp() / 30;
    // TOTP is mandatory even in the approved test environment when dev OTP is
    // disabled. Every rejected factor must leave session issuance unchanged.
    for submitted in [
        None,
        Some("abcdef".to_owned()),
        Some(synthetic_totp(now - 3)),
        Some(synthetic_totp(now + 3)),
    ] {
        reset_owned_redis(&state.redis).await;
        let challenge = admin_challenge(&pool).await;
        assert!(matches!(
            AuthService::admin_verify_otp_with_totp(
                &state,
                "192.0.2.10",
                &challenge,
                SYNTHETIC_OTP,
                submitted.as_deref()
            )
            .await,
            Err(AppError::Unauthorized(_))
        ));
        assert_eq!(admin_session_count(&pool).await, before);
        let used: Option<i64> =
            sqlx::query_scalar("SELECT last_totp_step FROM mfa_credentials WHERE user_id=$1")
                .bind(uid(ADMIN_USER_ID))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            used, None,
            "invalid factors must not consume a valid TOTP step"
        );
    }
    for condition in ["wrong-email-otp", "SHA1", "unenrolled", "disabled"] {
        reset_owned_redis(&state.redis).await;
        enroll_synthetic_totp(
            &pool,
            &state,
            if condition == "SHA1" {
                "SHA1"
            } else {
                "SHA256"
            },
        )
        .await;
        if condition == "unenrolled" {
            sqlx::query("UPDATE mfa_credentials SET is_enabled=false WHERE user_id=$1")
                .bind(uid(ADMIN_USER_ID))
                .execute(&pool)
                .await
                .unwrap();
        }
        if condition == "disabled" {
            sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
                .bind(uid(ADMIN_USER_ID))
                .execute(&pool)
                .await
                .unwrap();
        }
        let challenge = admin_challenge(&pool).await;
        let code = synthetic_totp(Utc::now().timestamp() / 30);
        let otp = if condition == "wrong-email-otp" {
            "654321"
        } else {
            SYNTHETIC_OTP
        };
        assert!(
            matches!(
                AuthService::admin_verify_otp_with_totp(
                    &state,
                    "192.0.2.11",
                    &challenge,
                    otp,
                    Some(&code)
                )
                .await,
                Err(AppError::Unauthorized(_))
            ),
            "condition {condition} must fail closed"
        );
        assert_eq!(admin_session_count(&pool).await, before);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_totp_concurrent_step_issues_one_session_and_replay_is_denied(pool: PgPool) {
    let (pool, state) = strict_admin_state(pool).await;
    enroll_synthetic_totp(&pool, &state, "SHA256").await;
    let before = admin_session_count(&pool).await;
    let step = Utc::now().timestamp() / 30;
    let code = synthetic_totp(step);
    let a = admin_challenge(&pool).await;
    let b = admin_challenge(&pool).await;
    let (first, second) = tokio::join!(
        AuthService::admin_verify_otp_with_totp(
            &state,
            "192.0.2.20",
            &a,
            SYNTHETIC_OTP,
            Some(&code)
        ),
        AuthService::admin_verify_otp_with_totp(
            &state,
            "192.0.2.21",
            &b,
            SYNTHETIC_OTP,
            Some(&code)
        )
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(
        matches!(first, Err(AppError::Unauthorized(_)))
            || matches!(second, Err(AppError::Unauthorized(_)))
    );
    assert_eq!(admin_session_count(&pool).await, before + 1);
    let accepted: Option<i64> =
        sqlx::query_scalar("SELECT last_totp_step FROM mfa_credentials WHERE user_id=$1")
            .bind(uid(ADMIN_USER_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(accepted, Some(step));
    let challenges: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pending_mfa_challenges WHERE token IN ($1,$2)")
            .bind(&a)
            .bind(&b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(challenges, 1, "only the winning challenge is consumed");
    let replay = admin_challenge(&pool).await;
    assert!(matches!(
        AuthService::admin_verify_otp_with_totp(
            &state,
            "192.0.2.22",
            &replay,
            SYNTHETIC_OTP,
            Some(&code)
        )
        .await,
        Err(AppError::Unauthorized(_))
    ));
    assert_eq!(admin_session_count(&pool).await, before + 1);
}

fn thumbprint() -> String {
    device_proof::token_binding(&format!(
        "{{\"crv\":\"P-256\",\"kty\":\"EC\",\"x\":\"{KEY_X}\",\"y\":\"{KEY_Y}\"}}"
    ))
}

fn signed_proof(method: &str, path: &str, binding: &str, jti: Uuid) -> HeaderMap {
    let mut header = Header::new(Algorithm::ES256);
    header.typ = Some("hd-device+jwt".into());
    header.jwk = Some(
        serde_json::from_value(serde_json::json!({"kty":"EC","crv":"P-256","x":KEY_X,"y":KEY_Y}))
            .unwrap(),
    );
    let claims = serde_json::json!({"htm":method,"htu":path,"iat":Utc::now().timestamp(),"jti":jti,
        "binding":device_proof::token_binding(binding)});
    let encoded = encode(
        &header,
        &claims,
        &EncodingKey::from_ec_pem(SYNTHETIC_EC_KEY.as_bytes()).unwrap(),
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-device-proof", encoded.parse().unwrap());
    headers
}

async fn bound_family(pool: &PgPool, owner: Uuid, key: &str) -> (Uuid, String) {
    let family = Uuid::new_v4();
    let raw = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_family_id,refresh_token_hash,created_at,last_used_at,expires_at) VALUES($1,$2,$3,$4,now(),now(),now()+interval '1 day')")
        .bind(Uuid::new_v4()).bind(owner).bind(family).bind(crypto::hash_code(&raw)).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO auth_session_device_keys(token_family_id,key_thumbprint) VALUES($1,$2)",
    )
    .bind(family)
    .bind(key)
    .execute(pool)
    .await
    .unwrap();
    (family, raw)
}

async fn family_state(pool: &PgPool, family: Uuid) -> (i64, i64, i64) {
    sqlx::query_as("SELECT COUNT(*),COUNT(*) FILTER(WHERE revoked_at IS NOT NULL),COUNT(*) FILTER(WHERE revoked_at IS NULL AND rotated_at IS NULL) FROM auth_sessions WHERE token_family_id=$1")
        .bind(family).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn device_proof_real_redis_concurrent_replay_reserves_once(pool: PgPool) {
    let (_, _, state) = test_app_and_state(pool).await;
    let jti = Uuid::new_v4();
    let proof = signed_proof("GET", "/api/v1/patients/me", "synthetic-access", jti);
    let uri: Uri = "/api/v1/patients/me".parse().unwrap();
    let expected = thumbprint();
    let method = Method::GET;
    let jobs = (0..16).map(|_| {
        device_proof::verify(
            &state,
            &proof,
            &method,
            &uri,
            "synthetic-access",
            Some(&expected),
        )
    });
    let results = futures_util::future::join_all(jobs).await;
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(AppError::Unauthorized(_))))
            .count(),
        15
    );
    let mut connection = state.redis.get().await.unwrap();
    let key = format!("device-proof:{expected}:{jti}");
    let value: String = deadpool_redis::redis::cmd("GET")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert_eq!(value, "1");
    let ttl: i64 = deadpool_redis::redis::cmd("TTL")
        .arg(key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!((1..=120).contains(&ttl));
}

#[sqlx::test(migrations = "./migrations")]
async fn bound_refresh_stolen_credential_cannot_revoke_or_cross_family(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;
    let (family, raw) = bound_family(&pool, uid(PATIENT_USER_ID), &thumbprint()).await;
    let other_key = device_proof::token_binding("synthetic-other-installation-public-key");
    let (other_family, other) = bound_family(&pool, uid(DOCTOR1_USER_ID), &other_key).await;
    assert!(matches!(
        AuthService::refresh_session_with_proof(&state, &raw, None).await,
        Err(AppError::Unauthorized(_))
    ));
    let uri: Uri = "/api/v1/auth/refresh".parse().unwrap();
    let cross_token = signed_proof("POST", uri.path(), &other, Uuid::new_v4());
    assert!(matches!(
        AuthService::refresh_session_with_proof(
            &state,
            &raw,
            Some((&cross_token, &Method::POST, &uri))
        )
        .await,
        Err(AppError::Unauthorized(_))
    ));
    assert!(
        matches!(
            AuthService::refresh_session_with_proof(
                &state,
                &other,
                Some((&cross_token, &Method::POST, &uri))
            )
            .await,
            Err(AppError::Unauthorized(_))
        ),
        "a proof from another installation cannot authorize this family"
    );
    assert_eq!(family_state(&pool, family).await, (1, 0, 1));
    assert_eq!(family_state(&pool, other_family).await, (1, 0, 1));
    let valid = signed_proof("POST", uri.path(), &raw, Uuid::new_v4());
    let (_, successor) =
        AuthService::refresh_session_with_proof(&state, &raw, Some((&valid, &Method::POST, &uri)))
            .await
            .unwrap();
    assert_eq!(family_state(&pool, family).await, (2, 0, 1));
    assert!(matches!(
        AuthService::refresh_session_with_proof(&state, &raw, None).await,
        Err(AppError::Unauthorized(_))
    ));
    assert_eq!(
        family_state(&pool, family).await,
        (2, 0, 1),
        "stolen consumed credential must not trigger family revocation without its key"
    );
    let reuse = signed_proof("POST", uri.path(), &raw, Uuid::new_v4());
    assert!(matches!(
        AuthService::refresh_session_with_proof(&state, &raw, Some((&reuse, &Method::POST, &uri)))
            .await,
        Err(AppError::Unauthorized(_))
    ));
    assert_eq!(family_state(&pool, family).await, (2, 2, 0));
    let denied = signed_proof("POST", uri.path(), &successor, Uuid::new_v4());
    assert!(matches!(
        AuthService::refresh_session_with_proof(
            &state,
            &successor,
            Some((&denied, &Method::POST, &uri))
        )
        .await,
        Err(AppError::Unauthorized(_))
    ));
    assert_eq!(family_state(&pool, other_family).await, (1, 0, 1));
}

#[sqlx::test(migrations = "./migrations")]
async fn bound_refresh_concurrent_valid_proofs_preserve_reuse_revocation(pool: PgPool) {
    let (_, pool, state) = test_app_and_state(pool).await;
    let (family, raw) = bound_family(&pool, uid(PATIENT_USER_ID), &thumbprint()).await;
    let uri: Uri = "/api/v1/auth/refresh".parse().unwrap();
    let first = signed_proof("POST", uri.path(), &raw, Uuid::new_v4());
    let second = signed_proof("POST", uri.path(), &raw, Uuid::new_v4());
    let (a, b) = tokio::join!(
        AuthService::refresh_session_with_proof(&state, &raw, Some((&first, &Method::POST, &uri))),
        AuthService::refresh_session_with_proof(&state, &raw, Some((&second, &Method::POST, &uri)))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(
        matches!(a, Err(AppError::Unauthorized(_))) || matches!(b, Err(AppError::Unauthorized(_)))
    );
    assert_eq!(
        family_state(&pool, family).await,
        (2, 2, 0),
        "a valid-key ancestor reuse must revoke the racing successor, not leave a zombie family"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn device_proof_dependency_loss_denies_before_refresh_mutation(pool: PgPool) {
    use axum::response::IntoResponse;
    let (_, pool, mut state) = test_app_and_state(pool).await;
    let (family, raw) = bound_family(&pool, uid(PATIENT_USER_ID), &thumbprint()).await;
    // Deny this client's connection at the fixture-owned Redis, without
    // interrupting other tests or probing an unowned service/port.
    let denied_url = test_redis_url().replacen(
        "redis://",
        "redis://:invalid-synthetic-proof-store-password@",
        1,
    );
    state.redis = deadpool_redis::Config::from_url(denied_url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let uri: Uri = "/api/v1/auth/refresh".parse().unwrap();
    let proof = signed_proof("POST", uri.path(), &raw, Uuid::new_v4());
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        AuthService::refresh_session_with_proof(&state, &raw, Some((&proof, &Method::POST, &uri))),
    )
    .await
    .expect("proof store failure must be bounded");
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("Redis failure must never establish refresh authority"),
    };
    assert_eq!(
        error.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(family_state(&pool, family).await, (1, 0, 1));
}

async fn send_security_request(
    app: &axum::Router,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    extra_headers: &HeaderMap,
    body: &str,
) -> axum::http::Response<Body> {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = bearer {
        builder = builder.header("Authorization", format!("Bearer {token}"));
    }
    for (name, value) in extra_headers {
        builder = builder.header(name, value);
    }
    if !body.is_empty() {
        builder = builder.header("Content-Type", "application/json");
    }
    let mut request = builder.body(Body::from(body.to_owned())).unwrap();
    request.extensions_mut().insert(random_peer());
    app.clone().oneshot(request).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn device_proof_http_enforces_original_path_replay_and_logout(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (family, raw) = bound_family(&pool, uid(PATIENT_USER_ID), &thumbprint()).await;
    let access = crypto::create_access_token(
        uid(PATIENT_USER_ID),
        "PATIENT",
        family,
        &state.config.jwt_ed25519_private_key_b64,
    )
    .unwrap();
    let path = "/api/v1/patients/me";
    assert_eq!(
        send_security_request(&app, "GET", path, Some(&access), &HeaderMap::new(), "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let wrong = signed_proof("GET", path, "another-access-token", Uuid::new_v4());
    assert_eq!(
        send_security_request(&app, "GET", path, Some(&access), &wrong, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let proof = signed_proof("GET", path, &access, Uuid::new_v4());
    let response = send_security_request(&app, "GET", path, Some(&access), &proof, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(
        body["data"]["user_id"], PATIENT_USER_ID,
        "proof authorizes only the persisted Patient identity"
    );
    assert_eq!(
        send_security_request(&app, "GET", path, Some(&access), &proof, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let logout = "/api/v1/auth/logout";
    let payload = serde_json::json!({"refresh_token":raw}).to_string();
    assert_eq!(
        send_security_request(&app, "POST", logout, None, &HeaderMap::new(), &payload)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(family_state(&pool, family).await, (1, 0, 1));
    let logout_proof = signed_proof("POST", logout, &raw, Uuid::new_v4());
    assert_eq!(
        send_security_request(&app, "POST", logout, None, &logout_proof, &payload)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(family_state(&pool, family).await, (1, 1, 0));
    let fresh = signed_proof("GET", path, &access, Uuid::new_v4());
    assert_eq!(
        send_security_request(&app, "GET", path, Some(&access), &fresh, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // A different approved legacy unbound family remains independent.
    let legacy = make_token(uid(PATIENT_USER_ID), "PATIENT");
    assert_eq!(
        get(&app, path, Some(&legacy)).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_web_hmac_http_exact_body_and_real_redis_nonce_replay(pool: PgPool) {
    use hellodoctor_backend::api::middleware::web_auth::WebAuthGate;
    let (_, pool, mut state) = test_app_and_state(pool).await;
    // This integration test must exercise the deployed gate with the same
    // isolated synthetic secret it uses to sign the proof. Production startup
    // still requires the real environment-managed secret; the override exists
    // only for this single-threaded test and restores the caller's environment
    // when the test exits.
    let _secret_override = EnvVarOverride::set(
        "WEB_AUTH_ASSERTION_SECRET",
        SYNTHETIC_WEB_AUTH_ASSERTION_SECRET,
    );
    let secret = SYNTHETIC_WEB_AUTH_ASSERTION_SECRET;
    assert!(secret.len() >= 32 && !secret.chars().any(char::is_whitespace));
    state.web_auth = Arc::new(WebAuthGate::new(true));
    assert!(state.web_auth.configured());
    let app = create_router(state.clone());
    let path = "/api/v1/auth/admin/login";
    let body = serde_json::json!({"email":"absent-synthetic-admin@example.invalid","password":"invalid-synthetic-password"}).to_string();
    let nonce = Uuid::new_v4().simple().to_string();
    let timestamp = Utc::now().timestamp();
    let message = format!(
        "v1\n{timestamp}\n{nonce}\nPOST\n{path}\n{}",
        hex::encode(Sha256::digest(body.as_bytes()))
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(message.as_bytes());
    let proof = format!(
        "v1.{timestamp}.{nonce}.{}",
        hex::encode(mac.finalize().into_bytes())
    );
    let mut headers = HeaderMap::new();
    headers.insert("x-hellodoctor-web-proof", proof.parse().unwrap());
    let mut forged = HeaderMap::new();
    forged.insert(
        "x-hellodoctor-web-proof",
        format!("v1.{timestamp}.{nonce}.{}", "00".repeat(32))
            .parse()
            .unwrap(),
    );
    let before = admin_session_count(&pool).await;
    assert_eq!(
        send_security_request(&app, "POST", path, None, &HeaderMap::new(), &body)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send_security_request(&app, "POST", path, None, &forged, &body)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let modified_body = body.replace("invalid-synthetic-password", "modified-synthetic-password");
    assert_eq!(
        send_security_request(&app, "POST", path, None, &headers, &modified_body)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let key = format!("auth:web-proof:{nonce}");
    let mut connection = state.redis.get().await.unwrap();
    let claimed: bool = deadpool_redis::redis::cmd("EXISTS")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!(!claimed, "forged assertions must not consume a valid nonce");
    assert_eq!(send_security_request(&app, "POST", path, None, &headers, &body).await.status(), StatusCode::UNAUTHORIZED,
        "valid BFF proof must reach actual credential rejection, never authenticate an absent account");
    assert_eq!(
        send_security_request(&app, "POST", path, None, &headers, &body)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let claimed: String = deadpool_redis::redis::cmd("GET")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert_eq!(claimed, "1");
    let ttl: i64 = deadpool_redis::redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!((1..=90).contains(&ttl));
    assert_eq!(admin_session_count(&pool).await, before);
}
