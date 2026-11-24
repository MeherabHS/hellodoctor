use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::repository::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct BookingPolicy {
    /// The single effective server-controlled policy every client must
    /// honor — never decided client-side. `false` only when
    /// `FINANCIAL_MODULE_MODE=deferred`; `true` (today's existing behavior)
    /// otherwise. This is a global policy, not per-doctor: an individual
    /// doctor's configured consultation fee still decides which modalities
    /// they offer, exactly as before, regardless of this flag.
    pub payment_required: bool,
}

#[derive(Debug, Serialize)]
pub struct AppVersionPolicy {
    /// Oldest Care (doctor) app build still supported; 0 = no minimum.
    pub doctor_app_min_build: i64,
}

/// `GET /config/app-version` — the Care app asks the doctor to update when
/// its build is older than this (`DOCTOR_APP_MIN_BUILD`).
pub async fn app_version(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, crate::error::AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    Ok(Json(ApiResponse {
        success: true,
        data: AppVersionPolicy {
            doctor_app_min_build: state.config.doctor_app_min_build.max(0),
        },
    }))
}

/// `GET /config/booking-policy` — the one place every client (Patient,
/// Doctor Flutter, Doctor Web, Admin Web) checks before deciding whether to
/// show payment UI, require a payment method at booking, or show Earnings &
/// Wallet as active. Authenticated like every other endpoint in this API;
/// "public" here means "any signed-in role may read it", not
/// "unauthenticated" — there is no secret in this response.
pub async fn booking_policy(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, crate::error::AppError> {
    abac::authorize(&ctx, Action::AnyAuthenticated)?;
    Ok(Json(ApiResponse {
        success: true,
        data: BookingPolicy {
            payment_required: !state.config.financial_module_deferred(),
        },
    }))
}
