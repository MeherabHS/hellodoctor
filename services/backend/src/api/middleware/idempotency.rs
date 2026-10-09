use crate::auth_context::AuthContext;
use crate::domain::models::{AppointmentStatus, UserRole};
use crate::error::AppError;
use crate::repository::{appointment_repo, idempotency_repo, AppState};
use axum::{
    body::{to_bytes, Body},
    extract::{FromRequestParts, Request, State},
    http::{header::CONTENT_TYPE, request::Parts, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Duration;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
const IDEMPOTENCY_TTL_HOURS: i64 = 24;

/// Replay is opt-in at the handler, after ABAC and identity resolution. The
/// middleware never returns a cached response or skips downstream authorization.
#[derive(Clone, Default)]
pub struct Idempotency {
    request: Option<RequestIdentity>,
}

#[derive(Clone)]
struct RequestIdentity {
    route: String,
    key_hash: String,
    request_hash: String,
}

#[derive(Clone)]
struct ReplayAppointment(Uuid);

#[async_trait::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Idempotency {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.extensions.get::<Self>().cloned().unwrap_or_default())
    }
}

impl Idempotency {
    /// Call only after the handler's ABAC check. Re-evaluate current resource
    /// visibility/ownership under RLS, including the doctor's clinical window.
    pub async fn replay(
        &self,
        state: &AppState,
        ctx: &AuthContext,
        expected_appointment: Option<Uuid>,
    ) -> Result<Option<Response>, AppError> {
        let Some(request) = &self.request else {
            return Ok(None);
        };
        let mut tx = state.begin_authed(ctx).await?;
        let Some(record) =
            idempotency_repo::find(&mut *tx, &request.route, &request.key_hash, ctx.user_id)
                .await?
        else {
            tx.commit().await?;
            return Ok(None);
        };
        let appointment_id = record.response_body_ref.ok_or_else(|| {
            AppError::internal("Idempotency record has no authorization resource.".into())
        })?;
        if expected_appointment.is_some_and(|id| id != appointment_id) {
            return Err(AppError::internal(
                "Idempotency authorization resource mismatch.".into(),
            ));
        }
        let appointment = appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
        match ctx.role {
            UserRole::Patient => {
                let patient =
                    crate::repository::patient_repo::find_by_user_id(&mut *tx, ctx.user_id)
                        .await?
                        .ok_or_else(|| AppError::NotFound("Patient profile not found.".into()))?;
                if appointment.patient_id != patient.id {
                    return Err(AppError::NotFound("Appointment not found.".into()));
                }
                if expected_appointment.is_some()
                    && appointment.status == AppointmentStatus::Cancelled
                {
                    return Err(AppError::UnprocessableEntity(
                        "Select files for an active appointment.".into(),
                    ));
                }
            }
            UserRole::Doctor => {
                let allowed: bool =
                    sqlx::query_scalar("SELECT doctor_history_context_allowed($1,$2)")
                        .bind(appointment_id)
                        .bind(ctx.user_id)
                        .fetch_one(&mut *tx)
                        .await?;
                if !allowed {
                    return Err(AppError::Forbidden(
                        "Prescription access requires an active consultation authorization window."
                            .into(),
                    ));
                }
            }
            _ => {
                return Err(AppError::Forbidden(
                    "This role cannot replay this response.".into(),
                ))
            }
        }
        tx.commit().await?;
        if record.request_hash != request.request_hash {
            return Err(AppError::Conflict(
                "Idempotency-Key was already used with a different request body.".into(),
            ));
        }
        let status = StatusCode::from_u16(record.response_status as u16)
            .map_err(|_| AppError::internal("Invalid cached response status.".into()))?;
        let body = record
            .response_body
            .ok_or_else(|| AppError::internal("Cached response body is missing.".into()))?;
        Ok(Some(
            (status, [(CONTENT_TYPE, "application/json")], body).into_response(),
        ))
    }

    /// Only explicitly authorized booking/upload handlers mark their successful
    /// responses as replayable. Reads, credentials, and other mutations never do.
    pub fn finish(&self, appointment_id: Uuid, mut response: Response) -> Response {
        if self.request.is_some() {
            response
                .extensions_mut()
                .insert(ReplayAppointment(appointment_id));
        }
        response
    }
}

fn scoped_key(ctx: &AuthContext, method: &Method, uri: &str, key: &str) -> String {
    let mut permissions = ctx.permissions.clone();
    permissions.sort();
    permissions.dedup();
    // Structured encoding prevents delimiter ambiguity. Versioning isolates all
    // legacy anonymous entries. Full path/query semantics are part of the key.
    let identity = serde_json::to_vec(&(
        "authenticated-idempotency-v2",
        ctx.user_id,
        ctx.role.as_str(),
        permissions,
        method.as_str(),
        uri,
        key,
    ))
    .expect("idempotency identity is serializable");
    format!("{:x}", Sha256::digest(identity))
}

pub async fn idempotency_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        return next.run(request).await;
    }
    let Some(key) = request
        .headers()
        .get("Idempotency-Key")
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned)
    else {
        return next.run(request).await;
    };
    let Some(ctx) = request.extensions().get::<AuthContext>().cloned() else {
        return AppError::Unauthorized("Authentication is required before idempotency.".into())
            .into_response();
    };
    let identity = RequestIdentity {
        route: request.uri().path().to_string(),
        key_hash: scoped_key(
            &ctx,
            request.method(),
            request.uri().path_and_query().map_or("", |p| p.as_str()),
            &key,
        ),
        request_hash: String::new(),
    };
    let (mut parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large.").into_response()
        }
    };
    let identity = RequestIdentity {
        request_hash: format!("{:x}", Sha256::digest(&body_bytes)),
        ..identity
    };
    parts.extensions.insert(Idempotency {
        request: Some(identity.clone()),
    });
    let response = next
        .run(Request::from_parts(parts, Body::from(body_bytes)))
        .await;
    let Some(resource) = response.extensions().get::<ReplayAppointment>().cloned() else {
        return response;
    };
    if !response.status().is_success() {
        return response;
    }
    let (response_parts, response_body) = response.into_parts();
    let response_bytes = match to_bytes(response_body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let mut tx = match state.begin_authed(&ctx).await {
        Ok(tx) => tx,
        Err(error) => return error.into_response(),
    };
    let stored = idempotency_repo::insert(
        &mut *tx,
        &identity.key_hash,
        ctx.user_id,
        resource.0,
        &identity.route,
        &identity.request_hash,
        response_parts.status.as_u16() as i16,
        &String::from_utf8_lossy(&response_bytes),
        chrono::Utc::now() + Duration::hours(IDEMPOTENCY_TTL_HOURS),
    )
    .await;
    if let Err(error) = stored {
        return AppError::from(error).into_response();
    }
    if let Err(error) = tx.commit().await {
        return AppError::from(error).into_response();
    }
    Response::from_parts(response_parts, Body::from(response_bytes))
}
