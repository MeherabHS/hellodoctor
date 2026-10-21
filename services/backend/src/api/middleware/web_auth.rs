//! A signed, single-use assertion from the trusted Admin BFF. The BFF verifies
//! Turnstile; accepting an unsigned client "passed" flag would bypass it.
use crate::{
    admission::{self, Cost},
    repository::AppState,
};
use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub struct WebAuthGate {
    secret: Option<String>,
    required: bool,
}
impl WebAuthGate {
    pub fn new(deployed: bool) -> Self {
        let secret = std::env::var("WEB_AUTH_ASSERTION_SECRET")
            .ok()
            .filter(|s| s.len() >= 32 && !s.chars().any(char::is_whitespace));
        Self {
            secret,
            required: deployed,
        }
    }
    pub fn configured(&self) -> bool {
        !self.required || self.secret.is_some()
    }
    fn verify<'a>(&self, proof: &'a str, body: &[u8], now: i64) -> Option<&'a str> {
        let mut parts = proof.split('.');
        if parts.next()? != "v1" {
            return None;
        }
        let ts = parts.next()?;
        let nonce = parts.next()?;
        let mac = parts.next()?;
        if parts.next().is_some()
            || nonce.len() != 32
            || !nonce.bytes().all(|c| c.is_ascii_hexdigit())
            || mac.len() != 64
        {
            return None;
        }
        let timestamp: i64 = ts.parse().ok()?;
        if now.abs_diff(timestamp) > 30 {
            return None;
        }
        let body_hash = hex::encode(Sha256::digest(body));
        let message = format!("v1\n{ts}\n{nonce}\nPOST\n/api/v1/auth/admin/login\n{body_hash}");
        let mut verifier = Hmac::<Sha256>::new_from_slice(self.secret.as_ref()?.as_bytes()).ok()?;
        verifier.update(message.as_bytes());
        verifier.verify_slice(&hex::decode(mac).ok()?).ok()?;
        Some(nonce)
    }
}

pub async fn middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    if request.method() != axum::http::Method::POST
        || request.uri().path() != "/api/v1/auth/admin/login"
    {
        return next.run(request).await;
    }
    if !state.web_auth.configured() {
        return admission::reject(
            StatusCode::SERVICE_UNAVAILABLE,
            Cost::Auth,
            "web_challenge_unavailable",
        );
    }
    let proof = request
        .headers()
        .get("x-hellodoctor-web-proof")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if !state.web_auth.required && proof.is_none() {
        return next.run(request).await;
    }
    let Some(proof) = proof else {
        return admission::reject(StatusCode::FORBIDDEN, Cost::Auth, "web_challenge");
    };
    let body = std::mem::replace(request.body_mut(), Body::empty());
    let bytes = match tokio::time::timeout(Duration::from_secs(2), to_bytes(body, 16 * 1024)).await
    {
        Ok(Ok(bytes)) => bytes,
        _ => return admission::reject(StatusCode::BAD_REQUEST, Cost::Auth, "auth_body"),
    };
    let Some(nonce) = state
        .web_auth
        .verify(&proof, &bytes, chrono::Utc::now().timestamp())
    else {
        return admission::reject(StatusCode::FORBIDDEN, Cost::Auth, "web_challenge");
    };
    let claim = async {
        let mut conn = state.redis.get().await.map_err(|_| ())?;
        deadpool_redis::redis::cmd("SET")
            .arg(format!("auth:web-proof:{nonce}"))
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(90)
            .query_async::<Option<String>>(&mut conn)
            .await
            .map_err(|_| ())
    };
    match tokio::time::timeout(Duration::from_millis(250), claim).await {
        Ok(Ok(Some(_))) => {}
        Ok(Ok(None)) => {
            return admission::reject(StatusCode::FORBIDDEN, Cost::Auth, "web_challenge_replay")
        }
        _ => {
            return admission::reject(
                StatusCode::SERVICE_UNAVAILABLE,
                Cost::Auth,
                "limiter_unavailable",
            )
        }
    }
    *request.body_mut() = Body::from(bytes);
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assertion_cannot_be_retargeted_or_changed() {
        let gate = WebAuthGate {
            secret: Some("test-secret-only-012345678901234567890123".into()),
            required: true,
        };
        let body = br#"{"email":"synthetic@example.invalid"}"#;
        let nonce = "00112233445566778899aabbccddeeff";
        let message = format!(
            "v1\n123\n{nonce}\nPOST\n/api/v1/auth/admin/login\n{}",
            hex::encode(Sha256::digest(body))
        );
        let mut mac =
            Hmac::<Sha256>::new_from_slice(gate.secret.as_ref().unwrap().as_bytes()).unwrap();
        mac.update(message.as_bytes());
        let proof = format!(
            "v1.123.{nonce}.{}",
            hex::encode(mac.finalize().into_bytes())
        );
        assert_eq!(gate.verify(&proof, body, 123), Some(nonce));
        assert!(gate.verify(&proof, b"modified", 123).is_none());
        assert!(gate.verify(&proof, body, 154).is_none());
    }
}
