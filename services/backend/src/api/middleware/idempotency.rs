use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

pub async fn idempotency_middleware(request: Request, next: Next) -> Response {
    // If Idempotency-Key header is present, track in state
    let _idempotency_key = request
        .headers()
        .get("Idempotency-Key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    next.run(request).await
}
