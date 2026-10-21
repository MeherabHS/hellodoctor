use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::crypto;
use crate::domain::models::PayoutMethodType;
use crate::domain::validation;
use crate::error::AppError;
use crate::repository::{wallet_repo, AppState};
use crate::services::DisbursementService;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use base64::Engine as _;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

pub async fn get_doctor_wallet(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let summary = DisbursementService::get_doctor_wallet_summary(&state, &ctx, doctor_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: summary,
    }))
}

#[derive(Debug, Deserialize)]
pub struct SetPayoutMethodRequest {
    pub method_type: PayoutMethodType,
    pub account_identifier: String,
}

#[derive(Debug, Serialize)]
pub struct PayoutMethodResponseDto {
    pub configured: bool,
    pub method_type: Option<PayoutMethodType>,
    pub display_label: Option<String>,
    pub account_identifier_last4: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

fn method_display_name(method: PayoutMethodType) -> &'static str {
    match method {
        PayoutMethodType::Mpesa => "M-Pesa",
        // Kept for backward-compatible historical records and the legacy API;
        // active Kenya onboarding uses the provider-bound destination API.
        PayoutMethodType::Bkash => "bKash",
        PayoutMethodType::BankCard => "Card",
        PayoutMethodType::BankTransfer => "Bank Transfer",
    }
}

/// The legacy setter is retained for historical API compatibility; active
/// Kenya onboarding uses provider-bound destinations and does not accept raw
/// card data.
fn validate_payout_identifier(method: PayoutMethodType, raw: &str) -> Result<String, AppError> {
    let clean = raw.trim().replace([' ', '-'], "");
    match method {
        PayoutMethodType::Mpesa | PayoutMethodType::Bkash => {
            validation::validate_phone_number(&clean)?;
        }
        PayoutMethodType::BankCard => {
            if clean.chars().count() < 12
                || clean.chars().count() > 19
                || !clean.chars().all(|c| c.is_ascii_digit())
            {
                return Err(AppError::Validation(
                    "Card number must be 12-19 digits.".into(),
                ));
            }
        }
        PayoutMethodType::BankTransfer => {
            if clean.chars().count() < 4
                || clean.chars().count() > 34
                || !clean.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(AppError::Validation(
                    "Bank account number must be 4-34 alphanumeric characters.".into(),
                ));
            }
        }
    }
    Ok(clean)
}

pub async fn get_payout_method(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let found = wallet_repo::get_payout_method(&mut *tx, doctor_id).await?;
    tx.commit().await?;

    let data = match found {
        Some(m) => PayoutMethodResponseDto {
            configured: true,
            method_type: Some(m.method_type),
            display_label: Some(m.display_label),
            account_identifier_last4: Some(m.account_identifier_last4),
            updated_at: Some(m.updated_at),
        },
        None => PayoutMethodResponseDto {
            configured: false,
            method_type: None,
            display_label: None,
            account_identifier_last4: None,
            updated_at: None,
        },
    };
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

pub async fn set_payout_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SetPayoutMethodRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    if state.config.environment != "development" {
        return Err(AppError::Forbidden(
            "Legacy payout setter is disabled; use the configured Kenya provider binding flow."
                .into(),
        ));
    }
    if matches!(payload.method_type, PayoutMethodType::BankCard) {
        return Err(AppError::Validation(
            "Use the configured Kenya provider binding flow for this payout method.".into(),
        ));
    }
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let clean = validate_payout_identifier(payload.method_type, &payload.account_identifier)?;
    let last4: String = clean
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let display_label = format!(
        "{} \u{2022}\u{2022}\u{2022}\u{2022} {}",
        method_display_name(payload.method_type),
        last4
    );
    let encrypted = crypto::encrypt_secret(&clean, &state.config.totp_encryption_key_b64)?;

    let mut tx = state.begin_authed(&ctx).await?;
    wallet_repo::upsert_payout_method(
        &mut *tx,
        doctor_id,
        payload.method_type,
        &display_label,
        &encrypted,
        &last4,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "configured": true,
            "method_type": payload.method_type,
            "display_label": display_label,
            "account_identifier_last4": last4
        }),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevPayoutBindingRequest {
    pub method_type: String,
}

/// Development fixture for the provider-bound flow. Real merchant/provider
/// authentication is intentionally deferred; production never exposes this route.
pub async fn dev_bind_payout_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<DevPayoutBindingRequest>,
) -> Result<impl IntoResponse, AppError> {
    if state.config.environment != "development" {
        return Err(AppError::Forbidden(
            "Development payout binding is disabled.".into(),
        ));
    }
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let method = payload.method_type.trim().to_uppercase();
    if !matches!(method.as_str(), "MPESA" | "BANK_CARD" | "BANK_TRANSFER") {
        return Err(AppError::Validation("Unsupported payout method.".into()));
    }
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let destination =
        wallet_repo::create_dev_payout_destination(&mut tx, doctor_id, &method, "•••• 8214")
            .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "id": destination.id, "provider_code": destination.provider_code,
            "method_type": destination.method_type, "currency": "KES",
            "masked_destination": destination.masked_destination,
            "binding_status": destination.binding_status,
            "verification_status": destination.verification_status,
            "payout_capable": destination.payout_capable, "is_primary": destination.is_primary,
            "message": "Development payout destination connected (simulated)."
        }),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindPayoutDestinationRequest {
    pub method_type: PayoutMethodType,
    pub account_identifier: String,
    /// Defaults to true for a doctor's very first destination (nothing else
    /// to be primary over); the service layer below applies that default,
    /// this field only needs to be explicit when a doctor already has at
    /// least one destination and is choosing whether the new one replaces
    /// it as primary.
    #[serde(default)]
    pub make_primary: Option<bool>,
}

/// The production-grade "configured Kenya provider binding flow" —
/// previously referenced by `set_payout_method`'s own error message but
/// never implemented anywhere (confirmed by searching the codebase before
/// writing this), and distinct from `dev_bind_payout_method` above (which
/// is dev-only and writes a server-generated fake reference). This accepts
/// a doctor's real account identifier, reachable in every environment, and
/// writes to `doctor_payout_destinations` — the table Admin's payout
/// routing and the disbursement pipeline actually read from (unlike the
/// legacy `set_payout_method` above, which writes to the separate,
/// unread `doctor_payout_methods` table). Real provider verification is not
/// yet wired up (`verification_status: AWAITING_PROVIDER_INTEGRATION`)
/// — this is the complete body the mission is waiting to attach a real
/// payment-provider driver to, not a partial stub.
pub async fn bind_payout_destination(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<BindPayoutDestinationRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let clean = validate_payout_identifier(payload.method_type, &payload.account_identifier)?;
    let last4: String = clean
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let masked_destination = format!(
        "{} \u{2022}\u{2022}\u{2022}\u{2022} {}",
        method_display_name(payload.method_type),
        last4
    );
    let encrypted_bytes = crypto::encrypt_secret(&clean, &state.config.totp_encryption_key_b64)?;
    let encrypted = base64::engine::general_purpose::STANDARD.encode(encrypted_bytes);
    let provider_code = match payload.method_type {
        PayoutMethodType::Mpesa => "MPESA",
        PayoutMethodType::Bkash => "BKASH",
        PayoutMethodType::BankCard => "CARD",
        PayoutMethodType::BankTransfer => "BANK_TRANSFER",
    };

    let mut tx = state.begin_authed(&ctx).await?;
    // Requires the platform to actually have a provider configured for this
    // method type before anything is bound — see
    // services::payment_provider_verification for why this is a hard stop,
    // not a warning.
    let verification =
        crate::services::payment_provider_verification::verify_doctor_payout_destination(
            &mut *tx,
            payload.method_type,
        )
        .await?;
    let existing = wallet_repo::list_payout_destinations(&mut *tx, doctor_id).await?;
    let make_primary = payload.make_primary.unwrap_or(existing.is_empty());
    let destination = wallet_repo::bind_payout_destination(
        &mut tx,
        doctor_id,
        provider_code,
        &format!("{:?}", payload.method_type),
        &encrypted,
        &masked_destination,
        verification.verification_status,
        verification.platform_payment_method_id,
        make_primary,
    )
    .await?;
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "id": destination.id, "provider_code": destination.provider_code,
            "method_type": destination.method_type, "currency": destination.currency,
            "masked_destination": destination.masked_destination,
            "binding_status": destination.binding_status,
            "verification_status": destination.verification_status,
            "payout_capable": destination.payout_capable, "is_primary": destination.is_primary,
        }),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPrimaryPayoutDestinationRequest {
    pub destination_id: Uuid,
}

pub async fn set_primary_payout_destination(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SetPrimaryPayoutDestinationRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let updated =
        wallet_repo::set_primary_payout_destination(&mut tx, doctor_id, payload.destination_id)
            .await?;
    if !updated {
        return Err(AppError::NotFound(
            "No active payout destination with that id belongs to this account.".into(),
        ));
    }
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": payload.destination_id, "is_primary": true }),
    }))
}

pub async fn list_payout_destinations(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let mut tx = state.begin_authed(&ctx).await?;
    let rows = wallet_repo::list_payout_destinations(&mut *tx, doctor_id).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: rows.into_iter().map(|destination| serde_json::json!({
            "id":destination.id,"method_type":destination.method_type,
            "provider_code":destination.provider_code,"currency":destination.currency,
            "masked_destination":destination.masked_destination,"binding_status":destination.binding_status,
            "verification_status":destination.verification_status,"payout_capable":destination.payout_capable,
            "is_primary":destination.is_primary,"updated_at":destination.updated_at
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementQuery {
    pub month: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn get_monthly_statement(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<StatementQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewOwnWallet)?;
    let doctor_id = ctx.doctor_profile_id(&state).await?;
    let month = query
        .month
        .unwrap_or_else(|| Utc::now().format("%Y-%m").to_string());
    let start = NaiveDate::parse_from_str(&format!("{}-01", month), "%Y-%m-%d")
        .map_err(|_| AppError::Validation("month must use YYYY-MM format.".into()))?;
    if month.len() != 7
        || !(2000..=2200).contains(&start.year())
        || start > Utc::now().date_naive().with_day(1).unwrap()
    {
        return Err(AppError::Validation(
            "Choose a current or previous month using YYYY-MM.".into(),
        ));
    }
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);
    let mut profile_tx = state.begin_authed(&ctx).await?;
    let doctor = crate::repository::doctor_repo::find_by_id(&mut *profile_tx, doctor_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    profile_tx.commit().await?;
    let end = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1).unwrap()
    };
    let mut tx = state.begin_authed(&ctx).await?;
    let period = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT status, id FROM financial_periods WHERE currency='KES' AND period_year=$1 AND period_month=$2",
    )
    .bind(start.year() as i16)
    .bind(start.month() as i16)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((status, period_id)) = period.filter(|(status, _)| status == "FINALIZED") {
        if let Some(snapshot) = sqlx::query_as::<_, (bigdecimal::BigDecimal, bigdecimal::BigDecimal, bigdecimal::BigDecimal, bigdecimal::BigDecimal, Option<String>, Option<String>, Uuid, DateTime<Utc>)>(
            "SELECT gross_eligible, platform_charge, adjustments, final_payable, provider_code, masked_destination_snapshot, id, snapshot_created_at FROM monthly_payout_statements WHERE doctor_id=$1 AND financial_period_id=$2",
        ).bind(doctor_id).bind(period_id).fetch_optional(&mut *tx).await? {
            let payout: Option<serde_json::Value> = sqlx::query_scalar(
                "SELECT jsonb_build_object('id',di.id,'status',di.status,'provider_code',di.provider_code,'masked_destination',di.masked_destination_snapshot,'payout_method_id',di.payout_destination_id,'provider_transaction_reference',di.gateway_reference,'paid_at',di.paid_at) FROM disbursement_items di WHERE di.doctor_id=$1 AND di.statement_id=$2 AND di.status <> 'CANCELLED' ORDER BY di.created_at DESC LIMIT 1",
            )
            .bind(doctor_id)
            .bind(snapshot.6)
            .fetch_optional(&mut *tx)
            .await?;
            let entries: Vec<serde_json::Value> = sqlx::query_scalar("SELECT jsonb_build_object('consultation_reference',a.appointment_number,'date',t.created_at,'gross_fee',m.gross_amount::text,'platform_charge',m.platform_charge::text,'doctor_earning',m.doctor_earning::text,'financial_status',t.payment_status) FROM monthly_statement_transactions m JOIN transactions t ON t.id=m.transaction_id JOIN appointments a ON a.id=t.appointment_id WHERE m.statement_id=$1 ORDER BY t.created_at,t.id LIMIT $2 OFFSET $3")
                .bind(snapshot.6).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM monthly_statement_transactions WHERE statement_id=$1")
                .bind(snapshot.6).fetch_one(&mut *tx).await?;
            let adjustments: Vec<serde_json::Value> = sqlx::query_scalar("SELECT jsonb_build_object('reference',id,'category',category,'amount',amount::text,'date',created_at) FROM financial_adjustments WHERE doctor_id=$1 AND currency='KES' AND period_year=$2 AND period_month=$3 ORDER BY created_at,id")
                .bind(doctor_id).bind(start.year() as i16).bind(start.month() as i16).fetch_all(&mut *tx).await?;
            let destination = payout.as_ref().map(|p| serde_json::json!({"id":p["payout_method_id"],"provider_code":p["provider_code"],"masked_destination":p["masked_destination"],"status":if p["provider_code"]=="DEV_SIMULATED" {"DEV_SIMULATED"}else{"LOCKED"}}))
                .or_else(|| snapshot.4.as_ref().map(|provider| serde_json::json!({"provider_code":provider,"masked_destination":snapshot.5,"status":if provider=="DEV_SIMULATED" {"DEV_SIMULATED"}else{"SNAPSHOTTED"}})));
            let payout_status = payout.as_ref().and_then(|p|p["status"].as_str()).unwrap_or("NOT_ALLOCATED").to_owned();
            tx.commit().await?;
            return Ok(Json(ApiResponse { success: true, data: serde_json::json!({
                "period": month, "currency":"KES", "status":status, "amount_label":"FINAL_PAYABLE",
                "statement_id":snapshot.6,"doctor_display_name":doctor.full_name,"professional_registration":doctor.license_number,
                "finalized_at":snapshot.7,"generated_at":Utc::now(),
                "gross_earnings":snapshot.0.to_string(), "platform_charge":snapshot.1.to_string(),
                "adjustments":snapshot.2.to_string(), "payable":snapshot.3.to_string(),
                "payout_status":payout_status,"payout_destination":destination,"disbursement":payout,
                "consultation_count":count,"entries":entries,"adjustment_entries":adjustments,
                "offset":offset,"next_offset":if offset+limit<count {Some(offset+limit)}else{None},
            }) }));
        }
    }
    let row = sqlx::query_as::<_, (bigdecimal::BigDecimal,bigdecimal::BigDecimal,bigdecimal::BigDecimal,i64,bigdecimal::BigDecimal)>(
        "SELECT COALESCE(SUM(t.gross_amount),0), COALESCE(SUM(t.platform_fee_amount),0), COALESCE(SUM(t.net_amount),0), COUNT(*), COALESCE((SELECT SUM(fa.amount) FROM financial_adjustments fa WHERE fa.doctor_id=$1 AND fa.currency='KES' AND fa.period_year=$4 AND fa.period_month=$5),0) FROM transactions t JOIN appointments a ON a.id=t.appointment_id WHERE a.doctor_id=$1 AND t.currency='KES' AND t.payment_status IN ('SETTLED_TO_DOCTOR','DISBURSED') AND t.created_at >= $2 AND t.created_at < $3")
        .bind(doctor_id).bind(start).bind(end).bind(start.year() as i16).bind(start.month() as i16).fetch_one(&mut *tx).await?;
    let destination = wallet_repo::get_primary_payout_destination(&mut *tx, doctor_id).await?;
    let entries: Vec<serde_json::Value> = sqlx::query_scalar("SELECT jsonb_build_object('consultation_reference',a.appointment_number,'date',t.created_at,'gross_fee',t.gross_amount::text,'platform_charge',t.platform_fee_amount::text,'doctor_earning',t.net_amount::text,'financial_status',t.payment_status) FROM transactions t JOIN appointments a ON a.id=t.appointment_id WHERE a.doctor_id=$1 AND t.currency='KES' AND t.payment_status IN ('SETTLED_TO_DOCTOR','DISBURSED') AND t.created_at >= $2 AND t.created_at < $3 ORDER BY t.created_at,t.id LIMIT $4 OFFSET $5")
        .bind(doctor_id).bind(start).bind(end).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "period": month, "currency": "KES", "status": "IN_PROGRESS",
            "statement_id":null,"doctor_display_name":doctor.full_name,"professional_registration":doctor.license_number,"generated_at":Utc::now(),
            "amount_label": "ESTIMATED_PAYOUT",
            "gross_earnings": row.0.to_string(), "platform_charge": row.1.to_string(),
            "adjustments": row.4.to_string(), "payable": (&row.2 + &row.4).to_string(), "consultation_count": row.3,
            "payout_status": "NOT_ALLOCATED",
            "entries":entries,"offset":offset,"next_offset":if offset+limit<row.3 {Some(offset+limit)}else{None},
            "payout_destination": destination.map(|d| serde_json::json!({"id":d.id,"provider_code":d.provider_code,"masked_destination":d.masked_destination,"status":d.binding_status})),
        }),
    }))
}
