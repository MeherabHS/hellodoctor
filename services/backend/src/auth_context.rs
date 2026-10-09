use crate::domain::ids::{DoctorProfileId, PatientProfileId, UserId};
use crate::domain::models::UserRole;
use crate::error::AppError;
use crate::repository::AppState;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use uuid::Uuid;

/// The authenticated identity of the caller. The actual JWT verification
/// happens exactly once per request, in `api::middleware::auth::auth_middleware`,
/// which runs as a `route_layer` on the protected half of the router (see
/// `router.rs`) and inserts the verified `AuthContext` into the request's
/// extensions; this extractor just reads it back out. That split is what makes
/// authentication default-deny at the router level — a route is authenticated
/// because it lives under the protected router, not because a handler
/// happened to declare `ctx: AuthContext` (the previous, per-handler-only
/// design: a handler forgetting the parameter silently made a route public).
/// A handler still declares `ctx: AuthContext` to *use* the identity for ABAC,
/// but omitting it no longer has any security consequence either way.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: UserId,
    pub role: UserRole,
    pub permissions: Vec<String>,
    pub session_id: Option<Uuid>,
    pub request_id: Option<String>,
}

impl AuthContext {
    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm || p == "admin:*")
    }

    /// Resolves the calling patient's own `patient_profiles.id`. Goes through
    /// `begin_authed` (not the raw pool) since `patient_profiles` is RLS-protected —
    /// an unscoped query would see no rows at all.
    pub async fn patient_profile_id(&self, state: &AppState) -> Result<PatientProfileId, AppError> {
        if self.role != UserRole::Patient {
            return Err(AppError::Forbidden("Patient identity required.".into()));
        }
        let mut tx = state.begin_authed(self).await?;
        let profile = crate::repository::patient_repo::find_by_user_id(&mut *tx, self.user_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound("No patient profile is associated with this account.".into())
            })?;
        tx.commit().await?;
        Ok(profile.id)
    }

    /// Resolves the canonical private Doctor profile under transaction-local RLS.
    pub async fn doctor_profile_id(&self, state: &AppState) -> Result<DoctorProfileId, AppError> {
        if self.role != UserRole::Doctor {
            return Err(AppError::Forbidden("Doctor identity required.".into()));
        }
        let mut tx = state.begin_authed(self).await?;
        let profile = crate::repository::doctor_repo::find_by_user_id(&mut *tx, self.user_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound("No doctor profile is associated with this account.".into())
            })?;
        tx.commit().await?;
        Ok(profile.id)
    }

    pub async fn verified_doctor_profile_id(
        &self,
        state: &AppState,
    ) -> Result<DoctorProfileId, AppError> {
        let mut tx = state.begin_authed(self).await?;
        let profile = crate::repository::doctor_repo::find_by_user_id(&mut *tx, self.user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
        if self.role != UserRole::Doctor || !profile.is_professionally_verified() {
            return Err(AppError::Forbidden(
                "Professional verification is required for clinical access.".into(),
            ));
        }
        tx.commit().await?;
        Ok(profile.id)
    }

    /// Pseudo-identity for trusted, non-user-initiated backend work that still needs to
    /// operate under RLS — namely verified payment gateway webhooks. `PlatformAdmin` is
    /// in every policy's admin allow-list, so this reads/writes as the system rather than
    /// as any particular patient/doctor. There is no real user behind it (`Uuid::nil()`),
    /// so it must never be used for anything a real end-user triggered.
    pub fn system() -> Self {
        AuthContext {
            user_id: UserId::from(Uuid::nil()),
            role: UserRole::PlatformAdmin,
            permissions: vec!["admin:*".into()],
            session_id: None,
            request_id: None,
        }
    }
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    /// Reads back the `AuthContext` that `auth_middleware` already verified
    /// and inserted into the request's extensions — does not re-parse the
    /// JWT. If this ever fails with "missing", it means a handler using this
    /// extractor was reachable without going through the protected router's
    /// `auth_middleware` layer, which is a routing bug, not an expected
    /// runtime condition (hence `Internal`, not `Unauthorized`).
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthContext>().cloned().ok_or_else(|| {
            AppError::internal(
                "AuthContext missing from request extensions — this route is reachable without the auth middleware layer.".into(),
            )
        })
    }
}
