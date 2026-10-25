use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::crypto;
use crate::domain::ids::{DoctorProfileId, PatientProfileId, UserId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{
    audit_repo, consultation_repo, doctor_repo, grievance_repo, patient_repo, payment_methods_repo,
    prescription_repo, system_error_repo, transaction_repo, wallet_repo, AppState, DbRole,
};
use crate::services::{AuditLogEntry, AuditService, ConsultationService, DisbursementService};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use base64::Engine as _;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityTraceRequest {
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
}

/// IDs-only, scoped internal investigation. The database primitive authorizes
/// independently and records access atomically with retrieval.
pub async fn trace_identity(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(body): Json<IdentityTraceRequest>,
) -> Result<impl IntoResponse, AppError> {
    if let Err(error) = abac::authorize(&ctx, Action::InvestigateIdentity) {
        AuditService::log(
            &state,
            &ctx,
            AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                "COMPLIANCE_TRACE_DENIED",
                "IDENTITY_TRACE",
                Some(body.target_id),
                "DENIED",
            ),
        )
        .await?;
        return Err(error);
    }
    if !["USER", "PATIENT", "DOCTOR", "APPOINTMENT", "CONSULTATION"]
        .contains(&body.target_type.as_str())
        || !(8..=300).contains(&body.reason.trim().chars().count())
    {
        return Err(AppError::Validation(
            "Use an internal UUID target and an investigation reason of 8–300 characters.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let data: serde_json::Value = sqlx::query_scalar("SELECT compliance_trace($1,$2,$3)")
        .bind(body.target_type)
        .bind(body.target_id)
        .bind(body.reason.trim())
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PayoutRoutingQuery {
    pub month: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PayoutRoutingRow {
    doctor_id: DoctorProfileId,
    doctor: String,
    license: String,
    gross: BigDecimal,
    fee: BigDecimal,
    adjustments: BigDecimal,
    net: BigDecimal,
    method: Option<String>,
    destination: Option<String>,
    provider_code: Option<String>,
    readiness: String,
    method_id: Option<Uuid>,
    statement_id: Option<Uuid>,
    item_id: Option<Uuid>,
    batch_id: Option<Uuid>,
    statement_status: String,
    disbursement_status: String,
}

/// Finance read model: each doctor's payable amount is paired with the
/// server-resolved primary Kenya destination. No destination is client-supplied.
pub async fn list_payout_routing(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<PayoutRoutingQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewFinanceSummary)?;
    let month = query
        .month
        .unwrap_or_else(|| Utc::now().format("%Y-%m").to_string());
    let start = NaiveDate::parse_from_str(&format!("{}-01", month), "%Y-%m-%d")
        .map_err(|_| AppError::Validation("month must use YYYY-MM format.".into()))?;
    let end = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1).unwrap()
    };
    let mut tx = state.begin_authed_as(&ctx, DbRole::FinanceWorker).await?;
    let rows = sqlx::query_as::<_, PayoutRoutingRow>(
        "SELECT d.id doctor_id, d.full_name doctor, d.license_number license,
         COALESCE(s.gross_eligible,e.gross,0) gross, COALESCE(s.platform_charge,e.fee,0) fee,
         COALESCE(s.adjustments,e.adjustments,0) adjustments, COALESCE(s.final_payable,e.net+e.adjustments,0) net,
         CASE WHEN di.id IS NOT NULL THEN locked.method_type ELSE p.method_type END method,
         CASE WHEN di.id IS NOT NULL THEN di.masked_destination_snapshot ELSE p.masked_destination END destination,
         CASE WHEN di.id IS NOT NULL THEN di.provider_code ELSE p.provider_code END provider_code,
         CASE WHEN di.provider_code='DEV_SIMULATED' OR p.provider_code='DEV_SIMULATED' THEN 'DEV_SIMULATED'
              WHEN di.id IS NOT NULL THEN 'ASSIGNED'
              WHEN p.id IS NULL THEN 'PAYOUT_SETUP_REQUIRED' ELSE p.verification_status END readiness,
         CASE WHEN di.id IS NOT NULL THEN di.payout_destination_id ELSE p.id END method_id,
         s.id statement_id, di.id item_id, di.batch_id,
         COALESCE(fp.status,'OPEN') statement_status, COALESCE(di.status,'NOT_ALLOCATED') disbursement_status
         FROM doctor_profiles d LEFT JOIN LATERAL (
           SELECT COALESCE(SUM(t.gross_amount),0) gross, COALESCE(SUM(t.platform_fee_amount),0) fee,
             COALESCE(SUM(t.net_amount),0) net, COALESCE((SELECT SUM(a.amount) FROM financial_adjustments a
               WHERE a.doctor_id=d.id AND a.currency='KES' AND a.period_year=$3 AND a.period_month=$4),0) adjustments
           FROM appointments ap JOIN transactions t ON t.appointment_id=ap.id WHERE ap.doctor_id=d.id
             AND t.currency='KES' AND t.payment_status IN ('SETTLED_TO_DOCTOR','DISBURSED')
             AND t.created_at >= $1::date AND t.created_at < $2::date
         ) e ON TRUE LEFT JOIN financial_periods fp ON fp.currency='KES' AND fp.period_year=$3 AND fp.period_month=$4
         LEFT JOIN monthly_payout_statements s ON s.doctor_id=d.id AND s.financial_period_id=fp.id
         LEFT JOIN LATERAL (SELECT id, method_type, masked_destination, provider_code, verification_status
           FROM doctor_payout_destinations WHERE doctor_id=d.id AND currency='KES' AND is_primary=TRUE
             AND payout_capable=TRUE AND binding_status='ACTIVE' LIMIT 1) p ON TRUE
         LEFT JOIN disbursement_items di ON di.statement_id=s.id AND di.status <> 'CANCELLED'
         LEFT JOIN doctor_payout_destinations locked ON locked.id=di.payout_destination_id ORDER BY d.full_name")
        .bind(start).bind(end).bind(start.year() as i16).bind(start.month() as i16).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let data = rows.into_iter().map(|r| serde_json::json!({
        "doctor_id": r.doctor_id, "doctor": r.doctor, "license": r.license,
        "statement_period": month, "currency": "KES", "gross_eligible_earnings": r.gross.to_string(),
        "platform_charge": r.fee.to_string(), "adjustments": r.adjustments.to_string(), "final_payable": r.net.to_string(),
        "payout_method_id": r.method_id, "payout_method": r.method, "masked_destination": r.destination,
        "provider_code": r.provider_code, "payout_readiness": r.readiness,
        "amount_label": if r.statement_status == "FINALIZED" {"FINAL_PAYABLE"} else {"ESTIMATED_PAYOUT"},
        "statement_id": r.statement_id, "disbursement_item_id": r.item_id, "batch_id": r.batch_id,
        "statement_status": r.statement_status, "disbursement_status": r.disbursement_status,
    })).collect::<Vec<_>>();
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct FinalizePeriodRequest {
    pub year: i32,
    pub month: u32,
}

/// Closes one KES accounting period and snapshots each Doctor's statement.
/// Calendar age alone never finalizes a period.
pub async fn finalize_financial_period(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<FinalizePeriodRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::InitiateDisbursement)?;
    if !(1..=12).contains(&payload.month) || !(2000..=2200).contains(&payload.year) {
        return Err(AppError::Validation("Invalid financial period.".into()));
    }
    let requested_period = (payload.year, payload.month);
    let current_period = {
        let now = Utc::now();
        (now.year(), now.month())
    };
    if requested_period > current_period {
        return Err(AppError::Validation(
            "A future financial period cannot be finalized.".into(),
        ));
    }
    let mut tx = state.begin_authed_as(&ctx, DbRole::FinanceWorker).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(61012, $1)")
        .bind(payload.year * 100 + payload.month as i32)
        .execute(&mut *tx)
        .await?;
    if let Some(existing_status) = sqlx::query_scalar::<_, String>(
        "SELECT status FROM financial_periods WHERE currency='KES' AND period_year=$1 AND period_month=$2 FOR UPDATE",
    )
    .bind(payload.year as i16)
    .bind(payload.month as i16)
    .fetch_optional(&mut *tx)
    .await?
    {
        if existing_status == "FINALIZED" {
            return Err(AppError::Conflict(
                "This financial period is already finalized and immutable.".into(),
            ));
        }
    }
    let period_id: Uuid = sqlx::query_scalar(
        "INSERT INTO financial_periods (currency, period_year, period_month, status, finalized_at, finalized_by, updated_at) VALUES ('KES',$1,$2,'FINALIZED',NOW(),$3,NOW()) ON CONFLICT (currency,period_year,period_month) DO UPDATE SET status='FINALIZED', finalized_at=NOW(), finalized_by=$3, updated_at=NOW() RETURNING id",
    ).bind(payload.year as i16).bind(payload.month as i16).bind(ctx.user_id).fetch_one(&mut *tx).await?;
    let start = NaiveDate::from_ymd_opt(payload.year, payload.month, 1).unwrap();
    let end = if payload.month == 12 {
        NaiveDate::from_ymd_opt(payload.year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(payload.year, payload.month + 1, 1).unwrap()
    };
    let pending_payments: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE currency='KES' AND created_at >= $1::date AND created_at < $2::date AND payment_status IN ('INITIATED','PAYMENT_HELD'))")
        .bind(start).bind(end).fetch_one(&mut *tx).await?;
    if pending_payments {
        return Err(AppError::Conflict("This period still has unconfirmed payments or unsettled consultations. Resolve them before accounting close.".into()));
    }
    sqlx::query(
        "INSERT INTO monthly_payout_statements (doctor_id, financial_period_id, currency, gross_eligible, platform_charge, adjustments, final_payable, status, payout_destination_id, provider_code, provider_reference_snapshot, masked_destination_snapshot) SELECT d.id, $4, 'KES', COALESCE(SUM(t.gross_amount),0), COALESCE(SUM(t.platform_fee_amount),0), COALESCE((SELECT SUM(a.amount) FROM financial_adjustments a WHERE a.doctor_id=d.id AND a.currency='KES' AND a.period_year=$1 AND a.period_month=$2),0), COALESCE(SUM(t.net_amount),0) + COALESCE((SELECT SUM(a.amount) FROM financial_adjustments a WHERE a.doctor_id=d.id AND a.currency='KES' AND a.period_year=$1 AND a.period_month=$2),0), 'FINALIZED', p.id, p.provider_code, p.provider_reference, p.masked_destination FROM doctor_profiles d LEFT JOIN appointments ap ON ap.doctor_id=d.id LEFT JOIN transactions t ON t.appointment_id=ap.id AND t.currency='KES' AND t.payment_status IN ('SETTLED_TO_DOCTOR','DISBURSED') AND t.created_at >= $3::date AND t.created_at < $5::date LEFT JOIN LATERAL (SELECT id, provider_code, provider_reference, masked_destination FROM doctor_payout_destinations WHERE doctor_id=d.id AND currency='KES' AND is_primary=TRUE AND payout_capable=TRUE AND binding_status='ACTIVE' LIMIT 1) p ON TRUE GROUP BY d.id, p.id, p.provider_code, p.provider_reference, p.masked_destination",
    ).bind(payload.year as i16).bind(payload.month as i16).bind(start).bind(period_id).bind(end).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO monthly_statement_transactions (statement_id, transaction_id, gross_amount, platform_charge, doctor_earning) SELECT s.id, t.id, t.gross_amount, t.platform_fee_amount, t.net_amount FROM monthly_payout_statements s JOIN appointments ap ON ap.doctor_id=s.doctor_id JOIN transactions t ON t.appointment_id=ap.id WHERE s.financial_period_id=$1 AND t.currency='KES' AND t.payment_status IN ('SETTLED_TO_DOCTOR','DISBURSED') AND t.created_at >= $2::date AND t.created_at < $3::date",
    ).bind(period_id).bind(start).bind(end).execute(&mut *tx).await?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "FINANCIAL_PERIOD_FINALIZED",
            "FINANCIAL_PERIOD",
            Some(period_id),
            "SUCCESS",
        )
        .with_after(
            serde_json::json!({"currency":"KES","year":payload.year,"month":payload.month}),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({"currency":"KES","year":payload.year,"month":payload.month,"status":"FINALIZED","period_id":period_id}),
    }))
}

/// Matches `wallet_repo::get_or_default`'s fallback shape for a doctor with no
/// wallet row yet, for use when building an in-memory lookup map instead of
/// querying per-doctor (see `wallet_repo::list_all` call sites below).
fn default_wallet(doctor_id: DoctorProfileId) -> DoctorWallet {
    DoctorWallet {
        doctor_id,
        lifetime_gross: BigDecimal::from(0),
        lifetime_fee_withheld: BigDecimal::from(0),
        lifetime_net: BigDecimal::from(0),
        pending_disbursement: BigDecimal::from(0),
        last_disbursement_at: None,
        last_disbursement_amount: BigDecimal::from(0),
        updated_at: Utc::now(),
    }
}

#[derive(Debug, Deserialize)]
pub struct InitiateDisbursementRequest {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct CreateDoctorRequest {
    pub email: String,
    pub full_name: String,
    pub license_number: String,
    pub primary_specialty: String,
    pub experience_years: i16,
    pub current_hospital: String,
    pub verified_phone: String,
    pub consultation_fee_video: BigDecimal,
    pub consultation_fee_chat: BigDecimal,
    pub residential_address: Option<String>,
    pub qualifications: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SlotToggleRequest {
    pub slot_id: Option<Uuid>,
    pub doctor_id: Option<Uuid>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub action: Option<String>, // "BLOCK", "UNBLOCK", "TOGGLE", "CREATE"
}

#[derive(Debug, Deserialize)]
pub struct SimulateLogRequest {
    pub action: String,
    pub subsystem: String,
    pub severity: String,
    pub error_summary: String,
    pub stack_trace: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfessionalReviewRequest {
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeDoctorRequest {
    pub reason: String,
}

/// POST /api/v1/admin/doctors/:id/revoke
///
/// h22 remediation: the only code path that can ever move a VERIFIED Doctor
/// out of VERIFIED. Mirrors verify/reject's shape and audit discipline; the
/// DB trigger (guard_professional_verification) independently re-enforces
/// VERIFIED->REVOKED as the only allowed transition here, so this handler
/// cannot be used to reach any other target status even if misused.
pub async fn revoke_doctor_professional_registration(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<RevokeDoctorRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageProfessionalVerification)?;
    let id = DoctorProfileId::from(id);
    let reason =
        crate::domain::validation::validate_text_field(&payload.reason, "Revocation reason", 1000)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let doc = doctor_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    if !doctor_repo::revoke_professional_registration(&mut *tx, id, ctx.user_id, &reason).await? {
        return Err(AppError::Conflict(
            "Only a currently verified Doctor can be revoked.".into(),
        ));
    }
    crate::services::NotificationService::notify_professional_verification_revoked(
        &mut tx,
        doc.user_id,
        id.into_inner(),
    )
    .await?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "DOCTOR_PROFESSIONAL_REVOKE",
            "DOCTOR_PROFILE",
            Some(id.into_inner()),
            "SUCCESS",
        )
        .with_reason(Some(reason))
        .with_before(serde_json::json!({"professional_verification_status": doc.professional_verification_status}))
        .with_after(serde_json::json!({"professional_verification_status": ProfessionalVerificationStatus::Revoked})),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({"doctor_id": id, "professional_verification_status": ProfessionalVerificationStatus::Revoked, "is_verified": false}),
    }))
}

/// GET /api/v1/admin/overview
pub async fn get_admin_overview(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewAdminOverview)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let docs = doctor_repo::list_public(&mut *tx).await?;
    let total_doctors = docs.len();
    let on_duty_doctors = docs.iter().filter(|d| d.is_on_duty).count();
    let pending_professional_verification = docs
        .iter()
        .filter(|d| d.professional_verification_status == ProfessionalVerificationStatus::Pending)
        .count();

    let active_consultations = consultation_repo::list_active(&mut *tx).await?.len();

    let txs = transaction_repo::list_all(&mut *tx).await?;
    let mut gross_volume = BigDecimal::from(0);
    let mut platform_fees = BigDecimal::from(0);
    for tx_row in &txs {
        gross_volume += &tx_row.gross_amount;
        platform_fees += &tx_row.platform_fee_amount;
    }

    // "Unpaid" is derived from finalized statements / in-flight disbursement
    // items, not `doctor_wallets.pending_disbursement` (that column only ever
    // grows — see wallet_repo::list_true_unpaid_totals and
    // docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md GAP-C01).
    let unpaid_by_doctor = wallet_repo::list_true_unpaid_totals(&mut *tx).await?;
    let mut pending_disbursement = BigDecimal::from(0);
    for amount in unpaid_by_doctor.values() {
        pending_disbursement += amount;
    }

    let grievances = grievance_repo::list_all(&mut *tx).await?;
    let pending_grievances = grievances
        .iter()
        .filter(|g| g.status == GrievanceStatus::PendingReview)
        .count();

    let total_patients = patient_repo::list(&mut *tx).await?.len();
    let unresolved_traces = audit_repo::count_unresolved(&mut *tx).await?;
    let active_users = crate::repository::user_repo::count_active(&mut *tx).await?;
    let (video_count,chat_count,video_value,chat_value):(i64,i64,BigDecimal,BigDecimal)=sqlx::query_as("SELECT COUNT(*) FILTER(WHERE a.modality='VIDEO'),COUNT(*) FILTER(WHERE a.modality='CHAT'),COALESCE(SUM(a.consultation_fee) FILTER(WHERE a.modality='VIDEO'),0),COALESCE(SUM(a.consultation_fee) FILTER(WHERE a.modality='CHAT'),0) FROM appointments a JOIN doctor_profiles d ON d.id=a.doctor_id WHERE d.deleted_at IS NULL").fetch_one(&mut *tx).await?;
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "modality_split":{"videoCount":video_count,"chatCount":chat_count,"videoRevenue":video_value.to_string(),"chatRevenue":chat_value.to_string()},
            "total_doctors": total_doctors,
            "on_duty_doctors": on_duty_doctors,
            "pending_professional_verification": pending_professional_verification,
            "active_consultations": active_consultations,
            "total_patients": total_patients,
            "gross_volume": gross_volume.to_string(),
            "platform_fees": platform_fees.to_string(),
            "pending_disbursement": pending_disbursement.to_string(),
            "pending_grievances": pending_grievances,
            "unresolved_traces": unresolved_traces,
            "system_health": if unresolved_traces > 5 { "DEGRADED" } else { "OPTIMAL" },
            "active_users": active_users,
            "max_active_users": state.config.max_active_users,
            "capacity_used_percent": (active_users as f64 / state.config.max_active_users as f64 * 100.0).round(),
        }),
    }))
}

/// GET /api/v1/admin/doctors
pub async fn list_admin_doctors(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageDoctorCredentialing)?;

    let mut tx = state.begin_authed(&ctx).await?;

    let docs = doctor_repo::list(&mut *tx).await?;
    let appts = crate::repository::appointment_repo::list_all(&mut *tx).await?;
    // One query for every doctor's wallet instead of one query per doctor —
    // the N+1 that was here before.
    let wallet_by_doctor: HashMap<DoctorProfileId, DoctorWallet> = wallet_repo::list_all(&mut *tx)
        .await?
        .into_iter()
        .map(|w| (w.doctor_id, w))
        .collect();
    // See GAP-C01 note in `get_admin_overview` above: `pending_disbursement`
    // below is sourced from the real statement/disbursement-item aggregate,
    // not the wallet column of the same name.
    let unpaid_by_doctor = wallet_repo::list_true_unpaid_totals(&mut *tx).await?;

    let mut list = Vec::new();
    for doc in docs {
        let wallet = wallet_by_doctor
            .get(&doc.id)
            .cloned()
            .unwrap_or_else(|| default_wallet(doc.id));
        let unpaid = unpaid_by_doctor
            .get(&doc.id)
            .cloned()
            .unwrap_or_else(|| BigDecimal::from(0));
        let consult_count = appts
            .iter()
            .filter(|a| a.doctor_id == doc.id && a.status == AppointmentStatus::Completed)
            .count();

        list.push(serde_json::json!({
            "id": doc.id,
            "user_id": doc.user_id,
            "full_name": doc.full_name,
            "license_number": doc.license_number,
            "license_authority": doc.license_authority,
            "license_country": doc.license_country,
            "primary_specialty": doc.primary_specialty,
            "experience_years": doc.experience_years,
            "current_hospital": doc.current_hospital,
            "residential_address": doc.residential_address,
            "verified_phone": doc.verified_phone,
            "qualifications": doc.qualifications,
            "consultation_fee_video": doc.consultation_fee_video.to_string(),
            "consultation_fee_chat": doc.consultation_fee_chat.to_string(),
            "is_on_duty": doc.is_on_duty,
            "is_verified": doc.is_professionally_verified(),
            "professional_verification_status": doc.professional_verification_status,
            "consultations_count": consult_count,
            "video_count":appts.iter().filter(|a|a.doctor_id==doc.id&&a.modality==Modality::Video).count(),
            "chat_count":appts.iter().filter(|a|a.doctor_id==doc.id&&a.modality==Modality::Chat).count(),
            "lifetime_gross": wallet.lifetime_gross.to_string(),
            "lifetime_fee_withheld": wallet.lifetime_fee_withheld.to_string(),
            "lifetime_net": wallet.lifetime_net.to_string(),
            "pending_disbursement": unpaid.to_string(),
            "last_disbursement_at": wallet.last_disbursement_at,
            "last_disbursement_amount": wallet.last_disbursement_amount.to_string(),
        }));
    }
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// POST /api/v1/admin/doctors
pub async fn create_doctor(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<CreateDoctorRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageDoctorCredentialing)?;
    if payload.current_hospital.trim().is_empty()
        || payload.current_hospital.chars().count() > 200
        || payload.qualifications.as_ref().is_some_and(|entries| {
            entries.is_empty()
                || entries.len() > 10
                || entries
                    .iter()
                    .any(|v| v.trim().is_empty() || v.chars().count() > 150)
        })
    {
        return Err(AppError::Validation("Hospital must contain 1-200 characters; qualifications must contain 1-10 entries of 1-150 characters.".into()));
    }
    // `doctor_register` (self-service, auth_handlers.rs) already enforces
    // these same limits on the same conceptual fields — this admin-created
    // path previously accepted full_name/primary_specialty/
    // residential_address with zero length validation at all. Matching the
    // self-service path's own numbers rather than inventing different ones,
    // so the two doctor-creation paths stay consistent with each other.
    if payload.full_name.trim().is_empty() || payload.full_name.chars().count() > 120 {
        return Err(AppError::Validation(
            "Full name must contain 1-120 characters.".into(),
        ));
    }
    if payload.primary_specialty.trim().is_empty() || payload.primary_specialty.chars().count() > 80
    {
        return Err(AppError::Validation(
            "Primary specialty must contain 1-80 characters.".into(),
        ));
    }
    if let Some(ref address) = payload.residential_address {
        if address.trim().is_empty() || address.chars().count() > 500 {
            return Err(AppError::Validation(
                "Residential address must contain 1-500 characters.".into(),
            ));
        }
    }
    crate::domain::validation::validate_email(&payload.email)?;
    let phone = crate::domain::validation::normalize_kenya_phone_number(&payload.verified_phone)?;

    let active_count = crate::repository::user_repo::count_active(&state.auth_db).await?;
    if active_count as u32 >= state.config.max_active_users {
        return Err(AppError::CapacityLimitReached(
            "AfyaMkononi is at its current controlled pilot capacity. Raise MAX_ACTIVE_USERS only after an evidence-gated capacity review.".into(),
        ));
    }

    let user_id = UserId::new();
    let doc_id = DoctorProfileId::new();
    let now = Utc::now();

    // Admin-created doctors get a random temporary password (Argon2id-hashed at rest);
    // returned once here so the admin can relay it for the doctor's first login.
    let temporary_password = Uuid::new_v4().to_string();
    let password_hash = crate::crypto::hash_password_async(temporary_password.clone()).await?;

    let user = User {
        id: user_id,
        phone_number: phone.clone(),
        email: Some(payload.email.trim().to_lowercase()),
        password_hash: Some(password_hash),
        role: UserRole::Doctor,
        preferred_language: "en".into(),
        is_active: true,
        is_verified: true,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    // `users` is auth-worker-exclusive (see migrations/20261003000001_rls_hardening)
    // even when the row is created by an admin action rather than self-registration.
    let mut auth_tx = state.begin_authed_as(&ctx, DbRole::AuthWorker).await?;
    crate::repository::user_repo::insert_user(&mut *auth_tx, &user).await?;

    let doc = DoctorProfile {
        id: doc_id,
        user_id,
        full_name: payload.full_name,
        license_number: payload.license_number,
        license_authority: "KMPDC".into(),
        license_country: "KE".into(),
        primary_specialty: payload.primary_specialty,
        experience_years: payload.experience_years,
        current_hospital: payload.current_hospital,
        qualifications: payload
            .qualifications
            .unwrap_or_else(|| vec!["MBBS".into()]),
        consultation_fee_video: payload.consultation_fee_video,
        consultation_fee_chat: payload.consultation_fee_chat,
        residential_address: payload
            .residential_address
            .unwrap_or_else(|| "Kenya".into()),
        verified_phone: phone,
        is_on_duty: false,
        is_verified: false,
        professional_verification_status: ProfessionalVerificationStatus::Pending,
        professional_verification_reviewed_at: None,
        professional_verification_reviewed_by: None,
        professional_verification_note: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    let mut tx = auth_tx;
    doctor_repo::insert(&mut *tx, &doc).await?;
    wallet_repo::insert_default(&mut *tx, doc_id).await?;

    // Same transaction as the business mutation above, not a separate
    // post-commit write (Pass 2 fix, docs/CHECKPOINT.md §10).
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "DOCTOR_PROFILE_CREATED",
            "DOCTOR_PROFILE",
            Some(doc_id.into_inner()),
            "SUCCESS",
        )
        // Never the temporary_password — that's a secret, not an audit fact.
        .with_after(serde_json::json!({
            "full_name": doc.full_name,
            "license_number": doc.license_number,
            "primary_specialty": doc.primary_specialty,
            "current_hospital": doc.current_hospital,
            "is_verified": doc.is_professionally_verified(),
            "professional_verification_status": doc.professional_verification_status,
            "is_on_duty": doc.is_on_duty,
        })),
    )
    .await?;

    tx.commit().await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: serde_json::json!({
                "doctor": doc,
                "temporary_password": temporary_password,
            }),
        }),
    ))
}

/// GET /api/v1/admin/doctors/:id/dossier
pub async fn get_doctor_dossier(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewDoctorDossier)?;
    let id = DoctorProfileId::from(id);
    let mut tx = state.begin_authed(&ctx).await?;

    let doc = doctor_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;

    let wallet = wallet_repo::get_or_default(&mut *tx, id).await?;
    // See GAP-C01 (wallet_repo::list_true_unpaid_totals doc comment): the
    // dossier's "unpaid" figure must reflect real statement/disbursement-item
    // state, not the ever-growing `doctor_wallets.pending_disbursement` column.
    let true_unpaid = wallet_repo::list_true_unpaid_totals(&mut *tx)
        .await?
        .get(&id)
        .cloned()
        .unwrap_or_else(|| BigDecimal::from(0));
    let appts = crate::repository::appointment_repo::list_all(&mut *tx).await?;

    let mut doctor_appts = Vec::new();
    for a in appts.iter().filter(|a| a.doctor_id == id) {
        let patient_name = patient_repo::find_by_id(&mut *tx, a.patient_id)
            .await?
            .map(|p| p.full_name)
            .unwrap_or_else(|| "Patient".into());

        doctor_appts.push(serde_json::json!({
            "appointment_id": a.id,
            "appointment_number": a.appointment_number,
            "patient_name": patient_name,
            "modality": a.modality,
            "status": a.status,
            "consultation_fee": a.consultation_fee.to_string(),
            "created_at": a.created_at,
            "clinical_outcome": a.clinical_outcome,
        }));
    }

    let doctor_disbs = wallet_repo::list_items_for_doctor(&mut *tx, id).await?;
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "id": doc.id,
            "full_name": doc.full_name,
            "license_number": doc.license_number,
            "license_authority": doc.license_authority,
            "license_country": doc.license_country,
            "primary_specialty": doc.primary_specialty,
            "experience_years": doc.experience_years,
            "current_hospital": doc.current_hospital,
            "residential_address": doc.residential_address,
            "verified_phone": doc.verified_phone,
            "qualifications": doc.qualifications,
            "consultation_fee_video": doc.consultation_fee_video.to_string(),
            "consultation_fee_chat": doc.consultation_fee_chat.to_string(),
            "is_on_duty": doc.is_on_duty,
            "is_verified": doc.is_professionally_verified(),
            "professional_verification_status": doc.professional_verification_status,
            "wallet": {
                "lifetime_gross": wallet.lifetime_gross.to_string(),
                "lifetime_fee_withheld": wallet.lifetime_fee_withheld.to_string(),
                "lifetime_net": wallet.lifetime_net.to_string(),
                "pending_disbursement": true_unpaid.to_string(),
                "last_disbursement_at": wallet.last_disbursement_at,
                "last_disbursement_amount": wallet.last_disbursement_amount.to_string(),
            },
            "encounters": doctor_appts,
            "disbursements": doctor_disbs,
            "disciplinary_warnings": []
        }),
    }))
}

/// POST /api/v1/admin/disbursements/initiate
pub async fn initiate_disbursement(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<InitiateDisbursementRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::InitiateDisbursement)?;
    // Audit write now happens inside the service, in the same FinanceWorker
    // transaction as the batch/item rows (Pass 2 fix, docs/CHECKPOINT.md §10)
    // — not a separate call here.
    let batch = DisbursementService::initiate_monthly_disbursement(
        &state,
        &ctx,
        ctx.user_id,
        payload.period_start,
        payload.period_end,
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: batch,
        }),
    ))
}

pub async fn approve_disbursement_item(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::InitiateDisbursement)?;
    let mut tx = state.begin_authed_as(&ctx, DbRole::FinanceWorker).await?;
    let batch_id: Uuid = sqlx::query_scalar("SELECT b.id FROM disbursement_batches b JOIN disbursement_items i ON i.batch_id=b.id WHERE i.id=$1 FOR UPDATE OF b")
        .bind(item_id).fetch_optional(&mut *tx).await?
        .ok_or_else(|| AppError::NotFound("Disbursement item not found.".into()))?;
    let approved =
        wallet_repo::approve_disbursement_item(&mut *tx, item_id, ctx.user_id.into_inner()).await?;
    if !approved {
        return Err(AppError::Conflict(
            "Disbursement item is not pending approval.".into(),
        ));
    }
    sqlx::query("UPDATE disbursement_batches SET status=CASE WHEN EXISTS(SELECT 1 FROM disbursement_items WHERE batch_id=$1 AND status <> 'APPROVED') THEN 'PARTIALLY_APPROVED' ELSE 'APPROVED' END WHERE id=$1")
        .bind(batch_id).execute(&mut *tx).await?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "DISBURSEMENT_APPROVED",
            "DISBURSEMENT_ITEM",
            Some(item_id),
            "SUCCESS",
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({"item_id":item_id,"status":"APPROVED","money_moved":false}),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmDisbursementPaidRequest {
    /// Finance's out-of-band confirmation reference from the real provider
    /// statement (e.g. an Mpesa/bank transfer transaction id) — never
    /// server-fabricated.
    pub gateway_reference: String,
}

/// POST /api/v1/admin/disbursements/:item_id/confirm-paid
///
/// Finance's admin-operated confirmation that a disbursement item was
/// genuinely paid by the provider. Only transitions an `APPROVED`/
/// `PROCESSING` item with a real (non-`DEV_SIMULATED`) payout destination to
/// `PAID`, and fires the `PAYOUT_PAID` notification in the same transaction —
/// see `DisbursementService::confirm_disbursement_item_paid` for the full
/// idempotency/invariant rationale.
pub async fn confirm_disbursement_item_paid(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<ConfirmDisbursementPaidRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::InitiateDisbursement)?;
    let gateway_reference = payload.gateway_reference.trim();
    if gateway_reference.is_empty() {
        return Err(AppError::Validation(
            "gateway_reference is required.".into(),
        ));
    }
    let item = DisbursementService::confirm_disbursement_item_paid(
        &state,
        &ctx,
        ctx.user_id,
        item_id,
        gateway_reference,
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "item_id": item.id,
            "status": item.status,
            "paid_at": item.paid_at,
            "gateway_reference": item.gateway_reference,
        }),
    }))
}

#[derive(Debug, Deserialize)]
pub struct FinanceMonthQuery {
    pub month: Option<String>,
}

/// Parses an optional `?month=YYYY-MM` into a half-open `[start, end)` date
/// range. `None` means "no filter" (all-time), matching this endpoint's
/// pre-existing default; an invalid format is a 422, not a silent ignore.
fn parse_optional_month(
    month: &Option<String>,
) -> Result<Option<(NaiveDate, NaiveDate)>, AppError> {
    let Some(month) = month else { return Ok(None) };
    let start = NaiveDate::parse_from_str(&format!("{}-01", month), "%Y-%m-%d")
        .map_err(|_| AppError::Validation("month must use YYYY-MM format.".into()))?;
    let end = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1).unwrap()
    };
    Ok(Some((start, end)))
}

/// GET /api/v1/admin/finance/transactions
pub async fn list_admin_transactions(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<FinanceMonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewTransactions)?;
    let range = parse_optional_month(&query.month)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let mut txs = transaction_repo::list_all(&mut *tx).await?;
    if let Some((start, end)) = range {
        txs.retain(|t| t.created_at.date_naive() >= start && t.created_at.date_naive() < end);
    }
    // Four whole-table fetches total instead of two per-row queries per
    // transaction (the N+1 that was here before: find_by_id on appointments,
    // then patient_repo/doctor_repo find_by_id for each one).
    let appt_by_id: HashMap<Uuid, Appointment> =
        crate::repository::appointment_repo::list_all(&mut *tx)
            .await?
            .into_iter()
            .map(|a| (a.id, a))
            .collect();
    let patient_name_by_id: HashMap<PatientProfileId, String> = patient_repo::list(&mut *tx)
        .await?
        .into_iter()
        .map(|p| (p.id, p.full_name))
        .collect();
    let doctor_name_by_id: HashMap<DoctorProfileId, String> = doctor_repo::list(&mut *tx)
        .await?
        .into_iter()
        .map(|d| (d.id, d.full_name))
        .collect();

    let mut list = Vec::new();
    for t in &txs {
        let appt = appt_by_id.get(&t.appointment_id);
        let patient_name = appt
            .and_then(|a| patient_name_by_id.get(&a.patient_id))
            .cloned()
            .unwrap_or_else(|| "Patient".into());
        let doctor_name = appt
            .and_then(|a| doctor_name_by_id.get(&a.doctor_id))
            .cloned()
            .unwrap_or_else(|| "Physician".into());

        list.push(serde_json::json!({
            "id": t.id,
            "transaction_number": t.transaction_number,
            "appointment_id": t.appointment_id,
            "payment_session_id": t.payment_session_id,
            "patient_name": patient_name,
            "doctor_name": doctor_name,
            "gateway": t.gateway,
            "gateway_reference": t.gateway_reference,
            "gross_amount": t.gross_amount.to_string(),
            "platform_fee_amount": t.platform_fee_amount.to_string(),
            "net_amount": t.net_amount.to_string(),
            "payment_status": t.payment_status,
            "currency": t.currency,
            "created_at": t.created_at,
            "settled_at": t.settled_at,
        }));
    }
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// GET /api/v1/admin/finance/summary
pub async fn get_finance_summary(
    State(state): State<AppState>,
    ctx: AuthContext,
    Query(query): Query<FinanceMonthQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewFinanceSummary)?;
    let range = parse_optional_month(&query.month)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let mut txs = transaction_repo::list_all(&mut *tx).await?;
    if let Some((start, end)) = range {
        txs.retain(|t| t.created_at.date_naive() >= start && t.created_at.date_naive() < end);
    }
    let mut total_gross = BigDecimal::from(0);
    let mut total_platform_fee = BigDecimal::from(0);
    let mut total_net = BigDecimal::from(0);

    // This deployment is Kenya/KES-only. `card_vol`/`mpesa_vol` are the only
    // live gateways; BKASH/NAGAD transactions can still exist as historical
    // (pre-cutover) rows — `transactions.gateway` keeps them readable here in
    // `total_gross`/`total_platform_fee`/`total_net` — but they are
    // deliberately excluded from `gateway_volume`, which is presented as this
    // deployment's live gateway breakdown and must not show a Bangladesh
    // bucket on a Kenya Finance dashboard. See
    // docs/audit/DOCTOR_BACKEND_CONTRACT_AUDIT.md §6 /
    // docs/audit/DOCTOR_BUSINESS_LOGIC_GAP_AUDIT.md GAP-C05.
    let mut card_vol = BigDecimal::from(0);
    let mut mpesa_vol = BigDecimal::from(0);

    for t in &txs {
        total_gross += &t.gross_amount;
        total_platform_fee += &t.platform_fee_amount;
        total_net += &t.net_amount;

        match t.gateway {
            Gateway::Card => card_vol += &t.gross_amount,
            Gateway::Mpesa => mpesa_vol += &t.gross_amount,
            Gateway::BankTransfer => card_vol += &t.gross_amount,
            Gateway::Bkash | Gateway::Nagad => {}
        }
    }

    // See GAP-C01 (wallet_repo::list_true_unpaid_totals doc comment).
    let unpaid_by_doctor = wallet_repo::list_true_unpaid_totals(&mut *tx).await?;
    let mut pending_disbursement = BigDecimal::from(0);
    for amount in unpaid_by_doctor.values() {
        pending_disbursement += amount;
    }
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "total_gross": total_gross.to_string(),
            "total_platform_fee": total_platform_fee.to_string(),
            "total_net": total_net.to_string(),
            "pending_disbursement": pending_disbursement.to_string(),
            "gateway_volume": {
                "card": card_vol.to_string(),
                "mpesa": mpesa_vol.to_string(),
            }
        }),
    }))
}

/// GET /api/v1/admin/command/telemetry
pub async fn get_command_telemetry(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewCommandTelemetry)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let docs = doctor_repo::list_public(&mut *tx).await?;
    let active_specialists = docs.iter().filter(|d| d.is_on_duty).count();
    let doc_name_by_id: HashMap<DoctorProfileId, String> =
        docs.iter().map(|d| (d.id, d.full_name.clone())).collect();

    let sessions = consultation_repo::list_monitoring(&mut *tx).await?;
    // Scoped to just the (typically small) set of appointments/patients/
    // telemetry rows behind *active* sessions, not a whole-table fetch —
    // this is an "active now" dashboard, and its cost must scale with how
    // much is active, not with total appointment history. A whole-table
    // version of this was measured adding seconds of latency per call under
    // a production-scale seeded dataset (docs/CONSTRAINED_CAPACITY_TEST.md);
    // it was "fine" only because the dev dataset it was written against was
    // tiny enough to hide the problem.
    let session_appointment_ids: Vec<Uuid> = sessions.iter().map(|s| s.appointment_id).collect();
    let session_ids: Vec<Uuid> = sessions.iter().map(|s| s.id).collect();
    let appts =
        crate::repository::appointment_repo::find_by_ids(&mut *tx, &session_appointment_ids)
            .await?;
    let appt_by_id: HashMap<Uuid, Appointment> = appts.iter().cloned().map(|a| (a.id, a)).collect();
    let patient_ids: Vec<PatientProfileId> = appts.iter().map(|a| a.patient_id).collect();
    let patient_name_by_id: HashMap<PatientProfileId, String> =
        patient_repo::find_by_ids(&mut *tx, &patient_ids)
            .await?
            .into_iter()
            .map(|p| (p.id, p.full_name))
            .collect();
    let telemetry_by_session: HashMap<Uuid, ConsultationTelemetry> =
        consultation_repo::list_telemetry_by_session_ids(&mut *tx, &session_ids)
            .await?
            .into_iter()
            .map(|t| (t.session_id, t))
            .collect();
    let payment_by_appointment: HashMap<Uuid, String> = if session_appointment_ids.is_empty() {
        HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT DISTINCT ON (appointment_id) appointment_id, payment_status::text
             FROM transactions WHERE appointment_id = ANY($1)
             ORDER BY appointment_id, (payment_status = 'FAILED') ASC, created_at DESC, id DESC",
        )
        .bind(&session_appointment_ids)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect()
    };

    let mut active_streams = Vec::new();
    for session in &sessions {
        let appt = appt_by_id.get(&session.appointment_id);
        let doc_name = appt
            .and_then(|a| doc_name_by_id.get(&a.doctor_id))
            .cloned()
            .unwrap_or_else(|| "Doctor".into());
        let patient_name = appt
            .and_then(|a| patient_name_by_id.get(&a.patient_id))
            .cloned()
            .unwrap_or_else(|| "Patient".into());
        let telemetry = telemetry_by_session.get(&session.id);
        let needs_review = session.recovery_state == "NEEDS_REVIEW"
            || matches!(session.status, SessionStatus::PrematureTermination);

        active_streams.push(serde_json::json!({
            "session_id": session.id,
            "appointment_id": session.appointment_id,
            "channel_name": session.agora_channel_name,
            "doctor_name": doc_name,
            "patient_name": patient_name,
            "started_at": session.started_at,
            "packet_loss_percent": telemetry.as_ref().and_then(|t| t.packet_loss_percent.as_ref().map(|p| p.to_string())),
            "rtt_ms": telemetry.as_ref().and_then(|t| t.round_trip_time_ms),
            "connection_state": telemetry.as_ref().map(|t| t.connection_state.clone()).unwrap_or_else(|| "Unmeasured".into()),
            "session_status": session.status,
            "recovery_state": session.recovery_state,
            "empty_room_deadline": session.empty_room_deadline,
            "interruption_time": session.ended_at,
            "interruption_reason": if needs_review { Some("EMPTY_ROOM_GRACE_EXPIRED") } else { None::<&str> },
            "patient_present": session.patient_present,
            "doctor_present": session.doctor_present,
            "patient_presence_sequence": session.patient_presence_sequence,
            "doctor_presence_sequence": session.doctor_presence_sequence,
            "needs_review": needs_review,
            "payment_status": appt.and_then(|a| payment_by_appointment.get(&a.id)).cloned(),
            "timeline_url": format!("/admin/consultations/{}/timeline", session.id),
            "provider_presence_summary": {
                "source": "AGORA_PROVIDER_CALLBACK",
                "patient_present": session.patient_present,
                "doctor_present": session.doctor_present,
                "patient_sequence": session.patient_presence_sequence,
                "doctor_sequence": session.doctor_presence_sequence,
            },
        }));
    }

    let completed_today = appts
        .iter()
        .filter(|a| a.status == AppointmentStatus::Completed)
        .count();
    let active_count = active_streams
        .iter()
        .filter(|stream| stream["needs_review"] != true)
        .count();
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "active_specialists": active_specialists,
            "completed_today": completed_today,
            "active_sessions_count": active_count,
            "needs_review_count": active_streams.iter().filter(|s| s["needs_review"] == true).count(),
            "active_streams": active_streams,
        }),
    }))
}

/// GET /api/v1/admin/slots
pub async fn list_admin_slots(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageSlots)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let slots = doctor_repo::list_all_slots(&mut *tx).await?;

    let mut list = Vec::new();
    for slot in &slots {
        let doctor_name = doctor_repo::find_by_id(&mut *tx, slot.doctor_id)
            .await?
            .map(|d| d.full_name)
            .unwrap_or_else(|| "Doctor".into());

        list.push(serde_json::json!({
            "id": slot.id,
            "doctor_id": slot.doctor_id,
            "doctor_name": doctor_name,
            "start_time": slot.start_time,
            "end_time": slot.end_time,
            "status": slot.status,
            "created_at": slot.created_at,
        }));
    }
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// POST /api/v1/admin/slots
pub async fn create_or_toggle_slot(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SlotToggleRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageSlots)?;
    let mut tx = state.begin_authed(&ctx).await?;

    if let Some(slot_id) = payload.slot_id {
        let slot = doctor_repo::find_slot(&mut *tx, slot_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Slot not found.".into()))?;

        let blocked = match payload.action.as_deref() {
            Some("BLOCK") => true,
            Some("UNBLOCK") => false,
            None | Some("TOGGLE") => match slot.status {
                SlotStatus::Available => true,
                SlotStatus::Blocked => false,
                _ => {
                    return Err(AppError::Conflict(
                        "Only available or blocked slots can change availability.".into(),
                    ))
                }
            },
            _ => return Err(AppError::Validation("Unsupported slot action.".into())),
        };
        let old_status = slot.status;
        let new_status = if blocked {
            SlotStatus::Blocked
        } else {
            SlotStatus::Available
        };
        if !doctor_repo::set_own_slot_blocked(&mut *tx, slot_id, slot.doctor_id, blocked).await? {
            return Err(AppError::Conflict(
                "Slot availability changed. Refresh the schedule before retrying.".into(),
            ));
        }

        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                "SLOT_STATUS_TOGGLED",
                "SCHEDULE_SLOT",
                Some(slot_id),
                "SUCCESS",
            )
            .with_before(serde_json::json!({ "status": old_status }))
            .with_after(serde_json::json!({ "status": new_status })),
        )
        .await?;

        tx.commit().await?;

        return Ok(Json(ApiResponse {
            success: true,
            data: serde_json::json!({ "slot_id": slot_id, "status": new_status }),
        }));
    }

    if let (Some(doctor_id), Some(start), Some(end)) =
        (payload.doctor_id, payload.start_time, payload.end_time)
    {
        crate::domain::validation::validate_slot_times(start, end)?;

        let slot = ScheduleSlot {
            id: Uuid::new_v4(),
            doctor_id: DoctorProfileId::from(doctor_id),
            start_time: start,
            end_time: end,
            status: SlotStatus::Available,
            created_at: Utc::now(),
        };
        doctor_repo::insert_slot(&mut *tx, &slot).await?;

        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                "SLOT_CREATED",
                "SCHEDULE_SLOT",
                Some(slot.id),
                "SUCCESS",
            )
            .with_after(serde_json::json!({
                "doctor_id": slot.doctor_id,
                "start_time": slot.start_time,
                "end_time": slot.end_time,
                "status": slot.status,
            })),
        )
        .await?;

        tx.commit().await?;

        return Ok(Json(ApiResponse {
            success: true,
            data: serde_json::json!(slot),
        }));
    }

    Err(AppError::Validation("Invalid slot parameters.".into()))
}

/// GET /api/v1/admin/patients
pub async fn list_admin_patients(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewPatientDirectory)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let patients = patient_repo::list(&mut *tx).await?;
    let appts = crate::repository::appointment_repo::list_all(&mut *tx).await?;

    let mut list = Vec::new();
    for p in &patients {
        let patient_appts: Vec<_> = appts.iter().filter(|a| a.patient_id == p.id).collect();
        let last_consult = patient_appts.iter().map(|a| a.created_at).max();

        list.push(serde_json::json!({
            "id": p.id,
            "user_id": p.user_id,
            "full_name": p.full_name,
            "date_of_birth": p.date_of_birth,
            "gender": p.gender,
            "blood_group": p.blood_group,
            "weight_kg": p.weight_kg.as_ref().map(|w| w.to_string()),
            "emergency_contact_phone": p.emergency_contact_phone,
            "consultations_count": patient_appts.len(),
            "video_count":patient_appts.iter().filter(|a|a.modality==Modality::Video).count(),
            "chat_count":patient_appts.iter().filter(|a|a.modality==Modality::Chat).count(),
            "consultation_fee_total":patient_appts.iter().fold(BigDecimal::from(0),|sum,a|sum+&a.consultation_fee).to_string(),
            "last_consult_at": last_consult,
            "created_at": p.created_at,
        }));
    }
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// GET /api/v1/admin/patients/:id/dossier
pub async fn get_patient_dossier(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewPatientDossier)?;
    let id = PatientProfileId::from(id);
    let mut tx = state.begin_authed(&ctx).await?;

    let patient = patient_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Patient profile not found.".into()))?;

    let appts = crate::repository::appointment_repo::list_all(&mut *tx).await?;
    let mut encounters = Vec::new();
    for a in appts.iter().filter(|a| a.patient_id == id) {
        let doc_name = doctor_repo::find_by_id(&mut *tx, a.doctor_id)
            .await?
            .map(|d| d.full_name)
            .unwrap_or_else(|| "Physician".into());

        encounters.push(serde_json::json!({
            "appointment_id": a.id,
            "appointment_number": a.appointment_number,
            "doctor_name": doc_name,
            "modality": a.modality,
            "status": a.status,
            "fee": a.consultation_fee.to_string(),
            "created_at": a.created_at,
            "clinical_outcome": a.clinical_outcome,
        }));
    }

    let vault_records = prescription_repo::list_intake_documents_for_patient(&mut *tx, id).await?;
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "patient": patient,
            "encounters": encounters,
            "vault_records": vault_records,
        }),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataRightsExecutionRequest {
    pub reason: String,
}

fn validated_data_rights_reason(reason: &str) -> Result<&str, AppError> {
    let trimmed = reason.trim();
    if !(8..=300).contains(&trimmed.chars().count()) {
        return Err(AppError::Validation(
            "Provide an erasure justification of 8–300 characters.".into(),
        ));
    }
    Ok(trimmed)
}

/// POST /api/v1/admin/patients/:id/data-rights/erase — Data Protection Act
/// 2019 (Kenya) §40 erasure. See `data_rights_service` for the hard-delete
/// vs. anonymize decision and why it is Postgres's FK constraints, not this
/// handler, that make that call.
pub async fn execute_patient_data_rights(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(body): Json<DataRightsExecutionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let reason = validated_data_rights_reason(&body.reason)?;
    let outcome = crate::services::data_rights_service::execute_patient_erasure(
        &state,
        &ctx,
        PatientProfileId::from(id),
        reason,
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "outcome": outcome.outcome }),
    }))
}

/// POST /api/v1/admin/doctors/:id/data-rights/erase — doctor-side mirror of
/// `execute_patient_data_rights`.
pub async fn execute_doctor_data_rights(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(body): Json<DataRightsExecutionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let reason = validated_data_rights_reason(&body.reason)?;
    let outcome = crate::services::data_rights_service::execute_doctor_erasure(
        &state,
        &ctx,
        DoctorProfileId::from(id),
        reason,
    )
    .await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "outcome": outcome.outcome }),
    }))
}

/// GET /api/v1/admin/doctors/pending-verification
pub async fn list_pending_doctor_verifications(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManageProfessionalVerification)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let queue = doctor_repo::list_pending_verifications(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: queue,
    }))
}

/// POST /api/v1/admin/doctors/:id/verify
pub async fn verify_doctor_professional_registration(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ProfessionalReviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    review_doctor_professional_registration(
        state,
        ctx,
        DoctorProfileId::from(id),
        payload,
        ProfessionalVerificationStatus::Verified,
    )
    .await
}

/// POST /api/v1/admin/doctors/:id/reject
pub async fn reject_doctor_professional_registration(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ProfessionalReviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    review_doctor_professional_registration(
        state,
        ctx,
        DoctorProfileId::from(id),
        payload,
        ProfessionalVerificationStatus::Rejected,
    )
    .await
}

async fn review_doctor_professional_registration(
    state: AppState,
    ctx: AuthContext,
    id: DoctorProfileId,
    payload: ProfessionalReviewRequest,
    status: ProfessionalVerificationStatus,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    abac::authorize(&ctx, Action::ManageProfessionalVerification)?;
    let note = payload
        .note
        .map(|n| crate::domain::validation::validate_text_field(&n, "Review note", 1000))
        .transpose()?;
    let mut tx = state.begin_authed(&ctx).await?;
    let doc = doctor_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    if !doctor_repo::review_professional_registration(
        &mut *tx,
        id,
        status,
        ctx.user_id,
        note.as_deref(),
    )
    .await?
    {
        return Err(AppError::Conflict(
            "Only pending professional registrations can be reviewed.".into(),
        ));
    }
    let verified = status == ProfessionalVerificationStatus::Verified;
    if verified {
        crate::services::NotificationService::notify_registration_verified(
            &mut tx,
            doc.user_id,
            id.into_inner(),
        )
        .await?;
        crate::services::NotificationService::notify_doctor_welcome_greeting(
            &mut tx,
            doc.user_id,
            id.into_inner(),
        )
        .await?;
    } else {
        crate::services::NotificationService::notify_registration_rejected(
            &mut tx,
            doc.user_id,
            id.into_inner(),
        )
        .await?;
    }
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(Some(ctx.user_id), ctx.role,
            if verified { "DOCTOR_PROFESSIONAL_VERIFY" } else { "DOCTOR_PROFESSIONAL_REJECT" },
            "DOCTOR_PROFILE", Some(id.into_inner()), if verified { "SUCCESS" } else { "REJECTED" })
            .with_reason(note)
            .with_before(serde_json::json!({"professional_verification_status": doc.professional_verification_status}))
            .with_after(serde_json::json!({"professional_verification_status": status})),
    ).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({"doctor_id": id, "professional_verification_status": status, "is_verified": verified, "status": status}),
    }))
}

/// GET /api/v1/admin/compliance/alerts
pub async fn get_compliance_alerts(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewComplianceAlerts)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let prescriptions = prescription_repo::list_all_prescriptions(&mut *tx).await?;

    let mut alerts = Vec::new();
    for rx in &prescriptions {
        if let Some(notes) = &rx.doctor_notes {
            let lower = notes.to_lowercase();
            if lower.contains("morphine")
                || lower.contains("pethidine")
                || lower.contains("schedule h")
            {
                let doc_name = doctor_repo::find_by_id(&mut *tx, rx.doctor_id)
                    .await?
                    .map(|d| d.full_name)
                    .unwrap_or_else(|| "Doctor".into());
                let patient_name = patient_repo::find_by_id(&mut *tx, rx.patient_id)
                    .await?
                    .map(|p| p.full_name)
                    .unwrap_or_else(|| "Patient".into());

                alerts.push(serde_json::json!({
                    "id": rx.id,
                    "rx_number": rx.rx_number,
                    "doctor_name": doc_name,
                    "patient_name": patient_name,
                    "category": "Schedule H / Narcotics Dispensation",
                    "flag_reason": "Contains controlled substance keywords requiring DGDA counter-signature",
                    "doctor_notes": notes,
                    "created_at": rx.created_at,
                    "status": "FLAGGED",
                }));
            }
        }
    }
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: alerts,
    }))
}

/// GET /api/v1/admin/logs
pub async fn list_admin_logs(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewLogs)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let logs = audit_repo::list_recent(&mut *tx, 500).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: logs,
    }))
}

/// POST /api/v1/admin/logs/simulate
pub async fn simulate_admin_log(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SimulateLogRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::SimulateLog)?;
    if !state.config.simulations_allowed() {
        return Err(AppError::Forbidden(
            "Simulation is unavailable in this environment.".into(),
        ));
    }

    let action = crate::domain::validation::validate_text_field(&payload.action, "Action", 150)?;
    let subsystem =
        crate::domain::validation::validate_text_field(&payload.subsystem, "Subsystem", 150)?;
    let severity =
        crate::domain::validation::validate_text_field(&payload.severity, "Severity", 150)?;
    let error_summary = crate::domain::validation::validate_text_field(
        &payload.error_summary,
        "Error summary",
        1000,
    )?;
    let event = AuditService::log(
        &state,
        &ctx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            action,
            subsystem,
            Some(Uuid::new_v4()),
            severity.to_uppercase(),
        )
        .with_reason(Some(error_summary)),
    )
    .await?;

    let _ = payload.stack_trace;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: event,
        }),
    ))
}

/// GET /api/v1/admin/system-errors — the Operations/System Errors surface
/// (distinct from `/admin/logs`'s `audit_events`-backed Logs screen). Genuine
/// backend technical failures only — see `system_error.rs` module docs and
/// `docs/CHECKPOINT.md` §8 for why these are never merged with grievances.
#[derive(Debug, Deserialize)]
pub struct ListSystemErrorsQuery {
    pub severity: Option<String>,
    pub status: Option<String>,
    pub subsystem: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn list_system_errors(
    State(state): State<AppState>,
    ctx: AuthContext,
    axum::extract::Query(q): axum::extract::Query<ListSystemErrorsQuery>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewSystemErrors)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let page_size = q.page_size.unwrap_or(50).clamp(1, 200);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * page_size;

    let filter = system_error_repo::SystemErrorFilter {
        severity: q.severity,
        status: q.status,
        subsystem: q.subsystem,
    };
    let items = system_error_repo::list(&mut *tx, &filter, page_size, offset).await?;
    let total = system_error_repo::count(&mut *tx, &filter).await?;
    let open_count = system_error_repo::count_open(&mut *tx).await?;
    let open_critical_count = system_error_repo::count_open_critical(&mut *tx).await?;
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "items": items,
            "page": page,
            "page_size": page_size,
            "total": total,
            "open_count": open_count,
            "open_critical_count": open_critical_count,
        }),
    }))
}

/// GET /api/v1/admin/system-errors/:id
pub async fn get_system_error(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewSystemErrors)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let item = system_error_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("System error log not found.".into()))?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: item,
    }))
}

/// POST /api/v1/admin/system-errors/:id/acknowledge
pub async fn acknowledge_system_error(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::AcknowledgeSystemError)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let item = system_error_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("System error log not found.".into()))?;
    system_error_repo::acknowledge(&mut *tx, id, ctx.user_id).await?;

    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "SYSTEM_ERROR_ACKNOWLEDGED",
            "SYSTEM_ERROR_LOG",
            Some(id),
            "SUCCESS",
        )
        .with_before(serde_json::json!({ "status": item.status }))
        .with_after(serde_json::json!({ "status": "ACKNOWLEDGED" })),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": id, "status": "ACKNOWLEDGED" }),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ResolveSystemErrorRequest {
    pub resolution_note: Option<String>,
}

/// POST /api/v1/admin/system-errors/:id/resolve
pub async fn resolve_system_error(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<ResolveSystemErrorRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ResolveSystemError)?;
    let payload = ResolveSystemErrorRequest {
        resolution_note: payload
            .resolution_note
            .map(|n| crate::domain::validation::validate_text_field(&n, "Resolution note", 1000))
            .transpose()?,
    };
    let mut tx = state.begin_authed(&ctx).await?;
    let item = system_error_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("System error log not found.".into()))?;
    system_error_repo::resolve(
        &mut *tx,
        id,
        ctx.user_id,
        payload.resolution_note.as_deref(),
    )
    .await?;

    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "SYSTEM_ERROR_RESOLVED",
            "SYSTEM_ERROR_LOG",
            Some(id),
            "SUCCESS",
        )
        .with_before(serde_json::json!({ "status": item.status }))
        .with_after(
            serde_json::json!({ "status": "RESOLVED", "resolution_note": payload.resolution_note }),
        ),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": id, "status": "RESOLVED" }),
    }))
}

#[derive(Debug, Deserialize)]
pub struct SimulateSystemErrorRequest {
    pub scenario: String,
    pub severity: String,
    pub custom_note: Option<String>,
    pub impacted_user_id: Option<Uuid>,
}

/// Fixed, named failure presets mirroring `prototype/index.html`'s
/// "Simulate App Failure" scenario dropdown — kept as `&'static str` tuples
/// (not admin-supplied free text) because `system_error::ErrorEvent`'s
/// `subsystem`/`component`/`error_code` fields are deliberately `&'static
/// str`: a stable, finite vocabulary is what makes the fingerprint/dedup
/// mechanism in `system_error.rs` meaningful in the first place.
fn system_error_scenario_preset(
    scenario: &str,
) -> Option<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)> {
    match scenario {
        "mpesa_timeout" => Some((
            "PAYMENT_GATEWAY",
            "MpesaProviderCallback",
            "GATEWAY_TIMEOUT",
            "Upstream M-Pesa provider did not respond within the timeout threshold during STK push confirmation.",
            "services/backend/src/api/v1/webhook_handlers.rs:1 (simulated — no real webhook call occurred)",
        )),
        "webrtc_ice" => Some((
            "AGORA_RTC",
            "AgoraIceNegotiation",
            "ICE_NEGOTIATION_FAILED",
            "Agora WebRTC ICE candidate relay failed during channel join.",
            "services/backend/src/services/agora_token_service.rs:1 (simulated — no real RTC session occurred)",
        )),
        "clinical_triage" => Some((
            "CLINICAL_RX_ENGINE",
            "PrescriptionTriageValidator",
            "SCHEDULE_VIOLATION",
            "Prescription triage flagged a controlled-substance keyword requiring counter-signature.",
            "services/backend/src/api/v1/prescription_handlers.rs:1 (simulated — no real prescription was touched)",
        )),
        "offline_sync" => Some((
            "SYNC_OFFLINE_CACHE",
            "MutationSyncResolver",
            "SYNC_CONFLICT",
            "Offline mutation queue produced a conflicting write on reconnect.",
            "services/backend/src/services/document_storage_service.rs:1 (simulated — no real document was touched)",
        )),
        "kyc_hub_down" => Some((
            "PROFESSIONAL_KYC_HUB",
            "CredentialRegistryClient",
            "REGISTRY_UNAVAILABLE",
            "Professional registry verification service returned a maintenance response.",
            "services/backend/src/api/v1/admin_handlers.rs:1 (simulated — no real registry call occurred)",
        )),
        "sms_otp" => Some((
            "AUTH_OTP_GATEWAY",
            "SmsOtpDispatcher",
            "TELCO_RATE_LIMIT",
            "SMS OTP delivery was throttled by the telecom carrier gateway.",
            "services/backend/src/services/auth_service.rs:1 (simulated — no real OTP was sent)",
        )),
        _ => None,
    }
}

/// POST /api/v1/admin/system-errors/simulate — dev/staging only (per
/// `docs/ERROR_HANDLING.md` §3: "Failure simulation is available only in
/// development and staging environments. It is stripped from production
/// builds."). Writes a real row into `system_error_logs` via the same
/// `system_error::report_sync` path a genuine backend failure would use —
/// deliberately *not* the `/admin/logs/simulate` endpoint above, which
/// writes to the separate `audit_events` trail. Gated by the same
/// `Action::SimulateLog` ABAC rule as that endpoint (SecurityAdmin |
/// PlatformAdmin).
pub async fn simulate_system_error(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SimulateSystemErrorRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::SimulateLog)?;

    if !state.config.simulations_allowed() {
        return Err(AppError::Forbidden(
            "Failure simulation is disabled in production.".into(),
        ));
    }

    let (subsystem, component, error_code, default_message, stack_trace) =
        system_error_scenario_preset(&payload.scenario).ok_or_else(|| {
            AppError::Validation(format!(
                "Unknown simulation scenario '{}'.",
                payload.scenario
            ))
        })?;

    let severity = match payload.severity.to_uppercase().as_str() {
        "CRITICAL" => ErrorSeverity::Critical,
        "ERROR" => ErrorSeverity::Error,
        "WARNING" => ErrorSeverity::Warning,
        other => {
            return Err(AppError::Validation(format!(
                "Unknown severity '{other}'. Expected CRITICAL, ERROR, or WARNING."
            )))
        }
    };

    let custom_note = payload
        .custom_note
        .map(|n| crate::domain::validation::validate_text_field(&n, "Custom note", 300))
        .transpose()?;
    let message = match custom_note.as_deref() {
        Some(note) if !note.is_empty() => format!("{default_message} [{note}]"),
        _ => default_message.to_string(),
    };

    let request_id = format!("SIM-{}", Uuid::new_v4());
    let mut event = crate::system_error::ErrorEvent::new(
        severity, subsystem, component, error_code, message, request_id,
    )
    .with_stack_trace(stack_trace);
    if let Some(user_id) = payload.impacted_user_id {
        event = event.with_user(user_id);
    }

    let id = crate::system_error::report_sync(&state.system_db, event)
        .await
        .map_err(AppError::Database)?;

    let mut tx = state.begin_authed(&ctx).await?;
    let item = system_error_repo::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| {
            AppError::internal("Simulated system error row vanished after insert.".into())
        })?;
    tx.commit().await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: item,
        }),
    ))
}

/// GET /api/v1/admin/grievances
pub async fn list_admin_grievances(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewGrievanceQueue)?;
    let mut tx = state.begin_authed(&ctx).await?;

    let grievances = grievance_repo::list_all(&mut *tx).await?;
    // Batch related evidence; never perform an evidence query per grievance.
    let patient_name_by_id: HashMap<PatientProfileId, String> = patient_repo::list(&mut *tx)
        .await?
        .into_iter()
        .map(|p| (p.id, p.full_name))
        .collect();
    let session_by_id: HashMap<Uuid, ConsultationSession> = consultation_repo::list_all(&mut *tx)
        .await?
        .into_iter()
        .map(|s| (s.id, s))
        .collect();
    let appt_by_id: HashMap<Uuid, Appointment> =
        crate::repository::appointment_repo::list_all(&mut *tx)
            .await?
            .into_iter()
            .map(|a| (a.id, a))
            .collect();
    let doctor_by_id: HashMap<DoctorProfileId, DoctorProfile> = doctor_repo::list(&mut *tx)
        .await?
        .into_iter()
        .map(|d| (d.id, d))
        .collect();
    let telemetry_by_session: HashMap<Uuid, ConsultationTelemetry> =
        consultation_repo::list_all_telemetry(&mut *tx)
            .await?
            .into_iter()
            .map(|t| (t.session_id, t))
            .collect();

    let payments: HashMap<Uuid, String> =
        sqlx::query_as("SELECT DISTINCT ON (appointment_id) appointment_id,payment_status::text FROM transactions ORDER BY appointment_id,(payment_status<>'FAILED') DESC,created_at DESC,id DESC")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();
    let decisions: HashMap<Uuid,serde_json::Value> = sqlx::query_as("SELECT DISTINCT ON(grievance_id) grievance_id,jsonb_build_object('action',action_type,'date',adjudicated_at,'admin',adjudicated_by_admin_id,'notes',audit_notes) FROM grievance_adjudications ORDER BY grievance_id,adjudicated_at DESC,id DESC").fetch_all(&mut *tx).await?.into_iter().collect();
    let mut list = Vec::new();
    for g in &grievances {
        let patient_name = patient_name_by_id
            .get(&g.patient_id)
            .cloned()
            .unwrap_or_else(|| "Patient".into());

        let session = session_by_id.get(&g.consultation_id);
        let doctor = session
            .and_then(|s| appt_by_id.get(&s.appointment_id))
            .and_then(|a| doctor_by_id.get(&a.doctor_id));

        let telemetry = telemetry_by_session.get(&g.consultation_id);
        let appointment = session.and_then(|s| appt_by_id.get(&s.appointment_id));

        list.push(serde_json::json!({
            "id": g.id,
            "grievance_number": g.grievance_number,
            "patient_id": g.patient_id,
            "patient_name": patient_name,
            "consultation_id": g.consultation_id,
            "doctor_name": doctor.map(|d|&d.full_name),
            "doctor_license": doctor.map(|d|&d.license_number),
            "doctor_specialty": doctor.map(|d|&d.primary_specialty),
            "doctor_hospital": doctor.map(|d|&d.current_hospital),
            "payment_status": appointment.and_then(|a|payments.get(&a.id)),
            "adjudication": decisions.get(&g.id),
            "doctor_id": appointment.map(|a|a.doctor_id),
            "appointment_id": appointment.map(|a|a.id),
            "modality": appointment.map(|a|a.modality),
            "consultation_fee": appointment.map(|a|&a.consultation_fee),
            "target_type": g.target_type,
            "category": g.category,
            "claim_summary": g.claim_summary,
            "status": g.status,
            "created_at": g.created_at,
            "telemetry": telemetry.map(|t| serde_json::json!({
                "duration_seconds": t.call_duration_seconds,
                "packet_loss_percent": t.packet_loss_percent.as_ref().map(|p| p.to_string()),
                "rtt_ms": t.round_trip_time_ms,
                "connection_state": t.connection_state,
                "premature_end": t.premature_end,
                "prescription_issued": t.prescription_issued,
            })),
        }));
    }
    tx.commit().await?;

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// GET /api/v1/admin/consultations/:session_id/timeline — detailed call
/// event timeline + reconnect reconstruction + correlated grievances/system
/// errors for one consultation session. See
/// `ConsultationService::get_session_timeline`.
pub async fn get_consultation_timeline(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(session_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewConsultationTimeline)?;
    let data = ConsultationService::get_session_timeline(&state, &ctx, session_id).await?;
    Ok(Json(ApiResponse {
        success: true,
        data,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsultationRecoveryResolutionRequest {
    pub outcome: String,
    pub reason: String,
}

/// POST /api/v1/admin/consultations/:session_id/recovery-resolution
///
/// This is an adjudication record, not a completion/refund operation. The
/// database function locks the session at the same boundary as the B4 expiry
/// worker and leaves appointment, session and payment state untouched.
pub async fn resolve_consultation_recovery(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ConsultationRecoveryResolutionRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ViewConsultationTimeline)?;
    let reason = crate::domain::validation::validate_text_field(&body.reason, "Reason", 1000)?;
    if reason.chars().count() < 8 {
        return Err(AppError::Validation(
            "A resolution reason of at least 8 characters is required.".into(),
        ));
    }
    if !matches!(
        body.outcome.as_str(),
        "RESOLVED_NO_SETTLEMENT" | "PENDING_FINANCE_ACTION"
    ) {
        return Err(AppError::Validation(
            "Unsupported recovery resolution outcome.".into(),
        ));
    }
    let mut tx = state.begin_authed(&ctx).await?;
    let result = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT resolution_id, result FROM admin_resolve_consultation_recovery($1,$2,$3,$4)",
    )
    .bind(session_id)
    .bind(&body.outcome)
    .bind(&reason)
    .bind(ctx.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        let code = error
            .as_database_error()
            .and_then(|database| database.code())
            .map(|value| value.to_string());
        match code.as_deref() {
            Some("42501") => AppError::Forbidden("Admin role required.".into()),
            Some("P0002") => AppError::NotFound("Consultation session not found.".into()),
            Some("40001") => {
                AppError::Conflict("A different resolution is already recorded.".into())
            }
            Some("55000") => AppError::Conflict("The consultation is not awaiting review.".into()),
            _ => AppError::Database(error),
        }
    })?;
    if result.1 == "RESOLVED" {
        let resolved_appointment_id: Uuid =
            sqlx::query_scalar("SELECT appointment_id FROM consultation_sessions WHERE id=$1")
                .bind(session_id)
                .fetch_one(&mut *tx)
                .await?;
        crate::services::NotificationService::notify_consultation_recovery_resolved(
            &mut tx,
            resolved_appointment_id,
            session_id,
            &body.outcome,
        )
        .await?;
    }
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            "CONSULTATION_RECOVERY_RESOLVED",
            "CONSULTATION_SESSION",
            Some(session_id),
            "SUCCESS",
        )
        .with_reason(Some(reason))
        .with_after(serde_json::json!({
            "outcome": body.outcome,
            "resolution_id": result.0,
            "financial_state_unchanged": true,
        })),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({"resolution_id": result.0, "result": result.1}),
    }))
}

// ---------------------------------------------------------------------
// The platform's own merchant/collection configuration — "the body
// waiting for the heart" (the real payment-gateway API key/driver).
// Distinct from patient_payment_methods (how a patient pays) and
// doctor_payout_destinations (where a doctor is paid): this is the
// platform's own identity with its payment provider(s). More than one may
// be active at once (e.g. M-Pesa and a card aggregator together); see
// migrations/20261108000002_platform_payment_methods.up.sql.
// ---------------------------------------------------------------------

fn platform_payment_method_json(m: &PlatformPaymentMethod) -> serde_json::Value {
    serde_json::json!({
        "id": m.id, "provider_code": m.provider_code, "label": m.label,
        "currency": m.currency, "masked_display": m.masked_display,
        "is_primary": m.is_primary, "is_active": m.is_active,
        "created_by": m.created_by, "created_at": m.created_at, "updated_at": m.updated_at,
    })
}

pub async fn list_platform_payment_methods(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePlatformPaymentMethod)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let methods = payment_methods_repo::list_platform_payment_methods(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: methods
            .iter()
            .map(platform_payment_method_json)
            .collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavePlatformPaymentMethodRequest {
    /// E.g. "MPESA_PAYBILL", "MPESA_TILL", "CARD_AGGREGATOR" — not a closed
    /// Rust enum on purpose ("admin may set mpesa or any kind of payment
    /// method"): the real provider roster isn't fixed yet, and a new
    /// provider code needs no migration to add, only a new real driver
    /// behind it later.
    pub provider_code: String,
    pub label: String,
    /// Admin-authored, not derived — this is what every list/detail view
    /// shows, and the backend never tries to guess a presentable mask out
    /// of an arbitrary credentials shape it can't assume the structure of.
    pub masked_display: String,
    /// Whatever fields the eventual real driver for this provider_code
    /// needs (paybill number, consumer key/secret, shortcode, etc.) —
    /// encrypted as one JSON blob before storage, the same AES-256-GCM
    /// encryption every other credential in this codebase already uses.
    /// Never returned to any client in decrypted form.
    pub credentials: serde_json::Value,
    #[serde(default)]
    pub make_primary: Option<bool>,
}

pub async fn save_platform_payment_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(payload): Json<SavePlatformPaymentMethodRequest>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePlatformPaymentMethod)?;
    let provider_code = payload.provider_code.trim().to_uppercase();
    if provider_code.is_empty() || provider_code.len() > 40 {
        return Err(AppError::Validation(
            "provider_code must be 1-40 characters.".into(),
        ));
    }
    if payload.label.trim().is_empty() || payload.label.chars().count() > 80 {
        return Err(AppError::Validation(
            "label must be 1-80 characters.".into(),
        ));
    }
    let credentials_text = serde_json::to_string(&payload.credentials)
        .map_err(|e| AppError::Validation(format!("credentials must be valid JSON: {e}")))?;
    let encrypted_bytes =
        crypto::encrypt_secret(&credentials_text, &state.config.totp_encryption_key_b64)?;
    let credentials_encrypted = base64::engine::general_purpose::STANDARD.encode(encrypted_bytes);

    let mut tx = state.begin_authed(&ctx).await?;
    let existing = payment_methods_repo::list_platform_payment_methods(&mut *tx).await?;
    let has_existing_for_provider = existing
        .iter()
        .any(|m| m.provider_code == provider_code && m.is_active);
    let make_primary = payload.make_primary.unwrap_or(!has_existing_for_provider);
    let method = payment_methods_repo::insert_platform_payment_method(
        &mut tx,
        &provider_code,
        payload.label.trim(),
        &payload.masked_display,
        &credentials_encrypted,
        ctx.user_id,
        make_primary,
    )
    .await?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: platform_payment_method_json(&method),
        }),
    ))
}

pub async fn set_primary_platform_payment_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePlatformPaymentMethod)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let method = payment_methods_repo::find_platform_payment_method(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("No platform payment method with that id.".into()))?;
    let updated = payment_methods_repo::set_primary_platform_payment_method(
        &mut tx,
        id,
        &method.provider_code,
    )
    .await?;
    if !updated {
        return Err(AppError::Conflict(
            "That payment method is not active and cannot be made primary.".into(),
        ));
    }
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": id, "is_primary": true }),
    }))
}

pub async fn deactivate_platform_payment_method(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    abac::authorize(&ctx, Action::ManagePlatformPaymentMethod)?;
    let mut tx = state.begin_authed(&ctx).await?;
    let removed = payment_methods_repo::deactivate_platform_payment_method(&mut *tx, id).await?;
    if !removed {
        return Err(AppError::NotFound(
            "No active platform payment method with that id.".into(),
        ));
    }
    tx.commit().await?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": id, "is_active": false }),
    }))
}
