//! HelloDoctor path-bound proof of possession (DPoP-inspired, not RFC 9449).
//! P-256 private keys stay in Android Keystore. No positive auth cache is added.
use crate::{error::AppError, repository::AppState};
use axum::http::{HeaderMap, Method, Uri};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Deserialize)]
struct PublicKey {
    kty: String,
    crv: String,
    x: String,
    y: String,
}
#[derive(Deserialize)]
struct Proof {
    htm: String,
    htu: String,
    iat: i64,
    jti: String,
    binding: String,
}

fn denied() -> AppError {
    metrics::counter!("device_proof_denied_total").increment(1);
    AppError::Unauthorized("Session proof is invalid. Please sign in again.".into())
}

pub fn token_binding(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

/// Verify all cryptographic/request claims before reserving a bounded replay key.
/// `binding` is SHA-256 of access token, refresh token or OTP challenge, encoded
/// base64url. The JWK thumbprint uses RFC 7638 canonical member ordering.
pub async fn verify(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    binding: &str,
    expected_thumbprint: Option<&str>,
) -> Result<String, AppError> {
    let (thumbprint, jti) = validate(
        headers,
        method,
        uri,
        binding,
        expected_thumbprint,
        chrono::Utc::now().timestamp(),
    )?;
    let replay_claim = async {
        let mut conn = state.redis.get().await.map_err(|_| {
            AppError::service_unavailable("Session proof store unavailable.".into())
        })?;
        let accepted: Option<String> = deadpool_redis::redis::cmd("SET")
            .arg(format!("device-proof:{thumbprint}:{jti}"))
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(120)
            .query_async(&mut conn)
            .await
            .map_err(|_| {
                AppError::service_unavailable("Session proof store unavailable.".into())
            })?;
        Ok::<_, AppError>(accepted)
    };
    let accepted = tokio::time::timeout(std::time::Duration::from_millis(250), replay_claim)
        .await
        .map_err(|_| AppError::service_unavailable("Session proof store unavailable.".into()))??;
    if accepted.is_none() {
        tracing::warn!(event = "DEVICE_PROOF_REPLAY", "Session proof denied");
        return Err(denied());
    }
    Ok(thumbprint)
}

fn validate(
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    binding: &str,
    expected_thumbprint: Option<&str>,
    now: i64,
) -> Result<(String, String), AppError> {
    let raw = headers
        .get("x-device-proof")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(denied)?;
    if raw.len() > 4096 {
        return Err(denied());
    }
    let header = decode_header(raw).map_err(|_| denied())?;
    if header.alg != Algorithm::ES256 || header.typ.as_deref() != Some("hd-device+jwt") {
        return Err(denied());
    }
    let jwk = header.jwk.ok_or_else(denied)?;
    let key: PublicKey = serde_json::from_value(serde_json::to_value(jwk).map_err(|_| denied())?)
        .map_err(|_| denied())?;
    if key.kty != "EC" || key.crv != "P-256" || key.x.len() != 43 || key.y.len() != 43 {
        return Err(denied());
    }
    let canonical = format!(
        "{{\"crv\":\"P-256\",\"kty\":\"EC\",\"x\":\"{}\",\"y\":\"{}\"}}",
        key.x, key.y
    );
    let thumbprint = token_binding(&canonical);
    if expected_thumbprint.is_some_and(|expected| expected != thumbprint) {
        return Err(denied());
    }
    let decoding_key = DecodingKey::from_ec_components(&key.x, &key.y).map_err(|_| denied())?;
    let mut validation = Validation::new(Algorithm::ES256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    validation.validate_aud = false;
    let proof = decode::<Proof>(raw, &decoding_key, &validation)
        .map_err(|_| denied())?
        .claims;
    if proof.htm != method.as_str()
        || proof.htu != uri.path()
        || proof.iat < now - 60
        || proof.iat > now + 5
        || proof.binding != token_binding(binding)
        || Uuid::parse_str(&proof.jti).is_err()
    {
        return Err(denied());
    }
    Ok((thumbprint, proof.jti))
}

pub async fn family_thumbprint(state: &AppState, family: Uuid) -> Result<Option<String>, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT key_thumbprint FROM auth_session_device_keys WHERE token_family_id=$1",
    )
    .bind(family)
    .fetch_optional(&state.auth_db)
    .await?)
}

pub async fn establishment_proof(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    challenge: &str,
) -> Result<Option<String>, AppError> {
    if headers.contains_key("x-device-proof") {
        Ok(Some(
            verify(state, headers, method, uri, challenge, None).await?,
        ))
    } else if headers
        .get("x-device-proof-required")
        .and_then(|v| v.to_str().ok())
        == Some("1")
    {
        Err(denied())
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    // Public synthetic fixture from jsonwebtoken 9.3.1's ECDSA tests. This is
    // deliberately not a device or production private key.
    const TEST_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWTFfCGljY6aw3Hrt\nkHmPRiazukxPLb6ilpRAewjW8nihRANCAATDskChT+Altkm9X7MI69T3IUmrQU0L\n950IxEzvw/x5BMEINRMrXLBJhqzO9Bm+d6JbqA21YQmd1Kt4RzLJR1W+\n-----END PRIVATE KEY-----";

    fn signed(now: i64) -> HeaderMap {
        let mut header = Header::new(Algorithm::ES256);
        header.typ = Some("hd-device+jwt".into());
        header.jwk = Some(
            serde_json::from_value(serde_json::json!({
                "kty":"EC", "crv":"P-256",
                "x":"w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ",
                "y":"wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4"
            }))
            .unwrap(),
        );
        let proof = serde_json::json!({"htm":"GET", "htu":"/api/v1/patient/me",
            "iat":now, "jti":Uuid::new_v4().to_string(), "binding":token_binding("synthetic-access")});
        let token = encode(
            &header,
            &proof,
            &EncodingKey::from_ec_pem(TEST_KEY.as_bytes()).unwrap(),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-device-proof", token.parse().unwrap());
        headers
    }

    #[test]
    fn stolen_token_needs_signature_key_request_and_freshness() {
        let now = 1_800_000_000;
        let uri: Uri = "/api/v1/patient/me".parse().unwrap();
        let headers = signed(now);
        let (key, _) =
            validate(&headers, &Method::GET, &uri, "synthetic-access", None, now).unwrap();
        assert!(validate(
            &headers,
            &Method::GET,
            &uri,
            "synthetic-access",
            Some(&key),
            now
        )
        .is_ok());
        assert!(validate(
            &HeaderMap::new(),
            &Method::GET,
            &uri,
            "synthetic-access",
            Some(&key),
            now
        )
        .is_err());
        assert!(validate(
            &headers,
            &Method::GET,
            &uri,
            "synthetic-access",
            Some("different-key"),
            now
        )
        .is_err());
        assert!(validate(
            &headers,
            &Method::POST,
            &uri,
            "synthetic-access",
            Some(&key),
            now
        )
        .is_err());
        assert!(validate(
            &headers,
            &Method::GET,
            &"/api/v1/admin/users".parse().unwrap(),
            "synthetic-access",
            Some(&key),
            now
        )
        .is_err());
        assert!(validate(&headers, &Method::GET, &uri, "other-token", Some(&key), now).is_err());
        assert!(validate(
            &headers,
            &Method::GET,
            &uri,
            "synthetic-access",
            Some(&key),
            now + 61
        )
        .is_err());
        assert!(validate(
            &headers,
            &Method::GET,
            &uri,
            "synthetic-access",
            Some(&key),
            now - 6
        )
        .is_err());
        let original = headers.get("x-device-proof").unwrap().to_str().unwrap();
        let pieces: Vec<_> = original.split('.').collect();
        let mut claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(pieces[1]).unwrap()).unwrap();
        claims["iat"] = serde_json::json!(now + 1);
        let forged = format!(
            "{}.{}.{}",
            pieces[0],
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap()),
            pieces[2]
        );
        let mut tampered = HeaderMap::new();
        tampered.insert("x-device-proof", forged.parse().unwrap());
        assert!(validate(
            &tampered,
            &Method::GET,
            &uri,
            "synthetic-access",
            Some(&key),
            now
        )
        .is_err());
    }
}
