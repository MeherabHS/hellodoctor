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
    let authentication = match authenticate(&state, &request) {
        Ok((ctx, family)) => check_active_session(&state, ctx, family).await,
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
    let active: Option<Uuid> = sqlx::query_scalar(
        "SELECT s.id FROM auth_sessions s JOIN users u ON u.id=s.user_id \
         WHERE s.token_family_id=$1 AND s.user_id=$2 AND s.revoked_at IS NULL \
         AND s.rotated_at IS NULL AND s.expires_at>NOW() AND u.is_active \
         AND u.deleted_at IS NULL AND u.role::text=$3 LIMIT 1",
    )
    .bind(family)
    .bind(ctx.user_id)
    .bind(ctx.role.as_str())
    .fetch_optional(&state.auth_db)
    .await
    .map_err(|error| {
        AppError::service_unavailable(format!("Session validation unavailable: {error}"))
    })?;
    if active.is_none() {
        return Err(AppError::Unauthorized(
            "This session is no longer available. Please sign in again.".into(),
        ));
    }
    ctx.session_id = active;
    ctx.request_id = Some(Uuid::new_v4().to_string());
    Ok(ctx)
}
