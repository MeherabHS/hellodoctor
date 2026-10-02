use crate::repository::{idempotency_repo, AppState};
use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Duration;
use sha2::{Digest, Sha256};

const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
const IDEMPOTENCY_TTL_HOURS: i64 = 24;

/// Replays a previously-stored response for a request carrying a repeated
/// `Idempotency-Key` on the same route with the same body, and otherwise
/// records the freshly-generated response for future replay.
pub async fn idempotency_middleware(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(idempotency_key) = request
        .headers()
        .get("Idempotency-Key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
    else {
        return next.run(request).await;
    };

    let method = request.method().clone();
    let route = request.uri().path().to_string();
    let (parts, body) = request.into_parts();

    let body_bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large.").into_response(),
    };

    let key_hash = format!("{:x}", Sha256::digest(format!("{}:{}:{}", method, route, idempotency_key)));
    let request_hash = format!("{:x}", Sha256::digest(&body_bytes));

    match idempotency_repo::find(&state.db, &route, &key_hash).await {
        Ok(Some(record)) => {
            if record.request_hash == request_hash {
                let status = StatusCode::from_u16(record.response_status as u16).unwrap_or(StatusCode::OK);
                return (status, record.response_body.unwrap_or_default()).into_response();
            }
            return (
                StatusCode::CONFLICT,
                "Idempotency-Key was already used with a different request body.",
            )
                .into_response();
        }
        Ok(None) => {}
        Err(_) => {
            // Fail open on idempotency-store errors rather than blocking the request.
        }
    }

    let request = Request::from_parts(parts, Body::from(body_bytes));
    let response = next.run(request).await;

    let (resp_parts, resp_body) = response.into_parts();
    let resp_bytes = match to_bytes(resp_body, MAX_BODY_BYTES).await {
        Ok(b) => b,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if resp_parts.status.is_success() {
        let _ = idempotency_repo::insert(
            &state.db,
            &key_hash,
            &route,
            &request_hash,
            resp_parts.status.as_u16() as i16,
            &String::from_utf8_lossy(&resp_bytes),
            chrono::Utc::now() + Duration::hours(IDEMPOTENCY_TTL_HOURS),
        )
        .await;
    }

    Response::from_parts(resp_parts, Body::from(resp_bytes))
}
