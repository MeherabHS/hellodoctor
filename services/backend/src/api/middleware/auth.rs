use crate::auth_context::AuthContext;
use crate::crypto;
use crate::domain::models::UserRole;
use crate::error::AppError;
use crate::repository::AppState;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

/// Verifies the bearer JWT exactly once per request and inserts the resulting
/// `AuthContext` into the request's extensions. Applied as a `route_layer` on
/// the protected router only (see `router.rs`) — this is what makes a route
/// "authenticated unless deliberately placed on the public router" rather than
/// "authenticated if the programmer remembered to declare `AuthContext`".
/// `AuthContext`'s own `FromRequestParts` impl then just reads back what this
/// middleware already verified, rather than re-parsing the token.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Body is not Sync: retain only the small request metadata across awaits.
    let proof_headers = request.headers().clone();
    let proof_method = request.method().clone();
    let proof_uri = request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map(|original| original.0.clone())
        .unwrap_or_else(|| request.uri().clone());
    let authentication = match authenticate(&state, &request) {
        Ok((ctx, family)) => {
            let cost = crate::admission::classify(&proof_method, proof_uri.path());
            let limit = match cost {
                crate::admission::Cost::Consultation => 240,
                crate::admission::Cost::Upload => 20,
                crate::admission::Cost::Payment => 30,
                crate::admission::Cost::Expensive => 60,
                _ => 120,
            };
            if let Err(error) = crate::rate_limit::check_and_increment(
                &state.redis,
                &format!("ratelimit:authenticated:{}:{}", ctx.user_id, cost.label()),
                limit,
                60,
            )
            .await
            {
                let status = if matches!(error, AppError::RateLimitExceeded(_)) {
                    axum::http::StatusCode::TOO_MANY_REQUESTS
                } else {
                    axum::http::StatusCode::SERVICE_UNAVAILABLE
                };
                return crate::admission::reject(status, cost, "account_budget");
            }
            check_active_session(
                &state,
                ctx,
                family,
                &proof_headers,
                &proof_method,
                &proof_uri,
            )
            .await
        }
        Err(error) => Err(error),
    };
    match authentication {
        Ok(ctx) => {
            let request_id = ctx.request_id.clone();
            request.extensions_mut().insert(ctx);
            let mut response = next.run(request).await;
            if let Some(id) = request_id.and_then(|id| id.parse().ok()) {
                response.headers_mut().insert("x-request-id", id);
            }
            response
        }
        Err(e) => e.into_response(),
    }
}

fn authenticate(state: &AppState, request: &Request) -> Result<(AuthContext, Uuid), AppError> {
    let header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header.".into()))?;

    let token = header.strip_prefix("Bearer ").ok_or_else(|| {
        AppError::Unauthorized("Authorization header must be a Bearer token.".into())
    })?;

    let claims = crypto::verify_access_token(token, &state.config.jwt_ed25519_public_key_b64)?;

    let user_id = Uuid::parse_str(&claims.user_id)
        .map_err(|_| AppError::Unauthorized("Malformed user identifier in access token.".into()))?;

    let role = UserRole::parse(&claims.role)
        .ok_or_else(|| AppError::Unauthorized("Unrecognized role in access token.".into()))?;

    let family = Uuid::parse_str(&claims.sid)
        .map_err(|_| AppError::Unauthorized("Malformed session identifier.".into()))?;
    if claims.sub != claims.user_id {
        return Err(AppError::Unauthorized("Invalid token subject.".into()));
    }
    Ok((
        AuthContext {
            user_id: crate::domain::ids::UserId::from(user_id),
            role,
            permissions: claims.permissions,
            session_id: None,
            request_id: None,
        },
        family,
    ))
}

async fn check_active_session(
    state: &AppState,
    mut ctx: AuthContext,
    family: Uuid,
    headers: &axum::http::HeaderMap,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
) -> Result<AuthContext, AppError> {
    if !state.config.is_development_or_test()
        && crate::seed::is_fixture_user(ctx.user_id.into_inner())
    {
        return Err(AppError::Unauthorized(
            "Development fixtures are unavailable.".into(),
        ));
    }
    // No positive cache: logout/reuse revocation is authoritative at the next
    // authentication check. Rotation preserves the family and its active leaf.
    let active: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT s.id,u.role::text,k.key_thumbprint FROM auth_sessions s JOIN users u ON u.id=s.user_id \
         LEFT JOIN auth_session_device_keys k ON k.token_family_id=s.token_family_id \
         WHERE s.token_family_id=$1 AND s.user_id=$2 AND s.revoked_at IS NULL \
         AND s.rotated_at IS NULL AND s.expires_at>NOW() AND u.is_active \
         AND u.deleted_at IS NULL LIMIT 1",
    )
    .bind(family)
    .bind(ctx.user_id)
    .fetch_optional(&state.auth_db)
    .await
    .map_err(|error| {
        AppError::service_unavailable(format!("Session validation unavailable: {error}"))
    })?;
    let (session, actual_role, device_key) = active.ok_or_else(|| {
        AppError::Unauthorized("This session is no longer available. Please sign in again.".into())
    })?;
    if let Some(thumbprint) = device_key {
        let token = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::Unauthorized("Session proof is required.".into()))?;
        crate::services::auth_service::device_proof::verify(
            state,
            headers,
            method,
            uri,
            token,
            Some(&thumbprint),
        )
        .await?;
    }
    ctx.session_id = Some(session);
    ctx.request_id = Some(crate::api::middleware::request_context::request_id());
    if actual_role != ctx.role.as_str() {
        let mut actor = ctx.clone();
        actor.role = UserRole::parse(&actual_role)
            .ok_or_else(|| AppError::Unauthorized("Account role unavailable.".into()))?;
        crate::services::AuditService::log(
            state,
            &actor,
            crate::services::AuditLogEntry::new(
                Some(actor.user_id),
                actor.role,
                "AUTH_ROLE_MISMATCH",
                "AUTH_SESSION",
                Some(session),
                "DENIED",
            )
            .with_reason(Some("TOKEN_ROLE_MISMATCH".into())),
        )
        .await?;
        return Err(AppError::Unauthorized(
            "This session is no longer available. Please sign in again.".into(),
        ));
    }
    ctx.request_id = Some(crate::api::middleware::request_context::request_id());
    Ok(ctx)
}
