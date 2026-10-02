use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use crate::services::DisbursementService;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub action: Option<String>, // "TOGGLE", "CREATE", "DELETE"
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
pub struct AdjudicateBmdcRequest {
    pub note: Option<String>,
}

/// GET /api/v1/admin/overview
pub async fn get_admin_overview(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let total_doctors = docs.len();
    let on_duty_doctors = docs.values().filter(|d| d.is_on_duty).count();
    let pending_bmdc = docs.values().filter(|d| !d.is_verified).count();

    let sessions = state.consultation_sessions.read();
    let active_consultations = sessions
        .values()
        .filter(|s| s.status == SessionStatus::Active)
        .count();

    let txs = state.transactions.read();
    let mut gross_volume = BigDecimal::from(0);
    let mut platform_fees = BigDecimal::from(0);
    for tx in txs.values() {
        gross_volume += &tx.gross_amount;
        platform_fees += &tx.platform_fee_amount;
    }

    let wallets = state.doctor_wallets.read();
    let mut pending_disbursement = BigDecimal::from(0);
    for w in wallets.values() {
        pending_disbursement += &w.pending_disbursement;
    }

    let grievances = state.grievance_reports.read();
    let pending_grievances = grievances
        .values()
        .filter(|g| g.status == GrievanceStatus::PendingReview)
        .count();

    let patients = state.patient_profiles.read();
    let total_patients = patients.len();

    let audit = state.audit_events.read();
    let unresolved_traces = audit
        .iter()
        .filter(|a| a.result == "FAILURE" || a.result == "ERROR")
        .count();

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "total_doctors": total_doctors,
            "on_duty_doctors": on_duty_doctors,
            "pending_bmdc": pending_bmdc,
            "active_consultations": active_consultations,
            "total_patients": total_patients,
            "gross_volume": gross_volume.to_string(),
            "platform_fees": platform_fees.to_string(),
            "pending_disbursement": pending_disbursement.to_string(),
            "pending_grievances": pending_grievances,
            "unresolved_traces": unresolved_traces,
            "system_health": if unresolved_traces > 5 { "DEGRADED" } else { "OPTIMAL" },
        }),
    }))
}

/// GET /api/v1/admin/doctors
pub async fn list_admin_doctors(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let wallets = state.doctor_wallets.read();
    let appts = state.appointments.read();

    let mut list = Vec::new();
    for doc in docs.values() {
        let wallet = wallets.get(&doc.id).cloned().unwrap_or(DoctorWallet {
            doctor_id: doc.id,
            lifetime_gross: BigDecimal::from(0),
            lifetime_fee_withheld: BigDecimal::from(0),
            lifetime_net: BigDecimal::from(0),
            pending_disbursement: BigDecimal::from(0),
            last_disbursement_at: None,
            last_disbursement_amount: BigDecimal::from(0),
            updated_at: Utc::now(),
        });

        let consult_count = appts
            .values()
            .filter(|a| a.doctor_id == doc.id && a.status == AppointmentStatus::Completed)
            .count();

        list.push(serde_json::json!({
            "id": doc.id,
            "user_id": doc.user_id,
            "full_name": doc.full_name,
            "license_number": doc.license_number,
            "license_authority": doc.license_authority,
            "primary_specialty": doc.primary_specialty,
            "experience_years": doc.experience_years,
            "current_hospital": doc.current_hospital,
            "residential_address": doc.residential_address,
            "verified_phone": doc.verified_phone,
            "qualifications": doc.qualifications,
            "consultation_fee_video": doc.consultation_fee_video.to_string(),
            "consultation_fee_chat": doc.consultation_fee_chat.to_string(),
            "is_on_duty": doc.is_on_duty,
            "is_verified": doc.is_verified,
            "consultations_count": consult_count,
            "lifetime_gross": wallet.lifetime_gross.to_string(),
            "lifetime_fee_withheld": wallet.lifetime_fee_withheld.to_string(),
            "lifetime_net": wallet.lifetime_net.to_string(),
            "pending_disbursement": wallet.pending_disbursement.to_string(),
            "last_disbursement_at": wallet.last_disbursement_at,
            "last_disbursement_amount": wallet.last_disbursement_amount.to_string(),
        }));
    }

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// POST /api/v1/admin/doctors
pub async fn create_doctor(
    State(state): State<AppState>,
    Json(payload): Json<CreateDoctorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::new_v4();
    let doc_id = Uuid::new_v4();
    let now = Utc::now();

    let user = User {
        id: user_id,
        phone_number: payload.verified_phone.clone(),
        email: None,
        password_hash: None,
        role: UserRole::Doctor,
        preferred_language: "en".into(),
        is_active: true,
        is_verified: true,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    state.users.write().insert(user_id, user);

    let doc = DoctorProfile {
        id: doc_id,
        user_id,
        full_name: payload.full_name,
        license_number: payload.license_number,
        license_authority: "BMDC".into(),
        license_country: "BD".into(),
        primary_specialty: payload.primary_specialty,
        experience_years: payload.experience_years,
        current_hospital: payload.current_hospital,
        qualifications: payload.qualifications.unwrap_or_else(|| vec!["MBBS".into()]),
        consultation_fee_video: payload.consultation_fee_video,
        consultation_fee_chat: payload.consultation_fee_chat,
        residential_address: payload
            .residential_address
            .unwrap_or_else(|| "Dhaka, Bangladesh".into()),
        verified_phone: payload.verified_phone,
        is_on_duty: true,
        is_verified: true,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    state.doctor_profiles.write().insert(doc_id, doc.clone());

    state.doctor_wallets.write().insert(
        doc_id,
        DoctorWallet {
            doctor_id: doc_id,
            lifetime_gross: BigDecimal::from(0),
            lifetime_fee_withheld: BigDecimal::from(0),
            lifetime_net: BigDecimal::from(0),
            pending_disbursement: BigDecimal::from(0),
            last_disbursement_at: None,
            last_disbursement_amount: BigDecimal::from(0),
            updated_at: now,
        },
    );

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: doc,
        }),
    ))
}

/// GET /api/v1/admin/doctors/:id/dossier
pub async fn get_doctor_dossier(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let doc = docs.get(&id).cloned().ok_or_else(|| {
        AppError::NotFound("Doctor profile not found.".into())
    })?;

    let wallets = state.doctor_wallets.read();
    let wallet = wallets.get(&id).cloned().unwrap_or(DoctorWallet {
        doctor_id: id,
        lifetime_gross: BigDecimal::from(0),
        lifetime_fee_withheld: BigDecimal::from(0),
        lifetime_net: BigDecimal::from(0),
        pending_disbursement: BigDecimal::from(0),
        last_disbursement_at: None,
        last_disbursement_amount: BigDecimal::from(0),
        updated_at: Utc::now(),
    });

    let appts = state.appointments.read();
    let patients = state.patient_profiles.read();
    let doctor_appts: Vec<_> = appts
        .values()
        .filter(|a| a.doctor_id == id)
        .map(|a| {
            let patient_name = patients
                .get(&a.patient_id)
                .map(|p| p.full_name.clone())
                .unwrap_or_else(|| "Patient".into());
            serde_json::json!({
                "appointment_id": a.id,
                "appointment_number": a.appointment_number,
                "patient_name": patient_name,
                "modality": a.modality,
                "status": a.status,
                "consultation_fee": a.consultation_fee.to_string(),
                "created_at": a.created_at,
                "clinical_outcome": a.clinical_outcome,
            })
        })
        .collect();

    let disb_items = state.disbursement_items.read();
    let doctor_disbs = disb_items.get(&id).cloned().unwrap_or_default();

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
            "is_verified": doc.is_verified,
            "wallet": {
                "lifetime_gross": wallet.lifetime_gross.to_string(),
                "lifetime_fee_withheld": wallet.lifetime_fee_withheld.to_string(),
                "lifetime_net": wallet.lifetime_net.to_string(),
                "pending_disbursement": wallet.pending_disbursement.to_string(),
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
    Json(payload): Json<InitiateDisbursementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let admin_id = Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap();
    let batch = DisbursementService::initiate_monthly_disbursement(
        &state,
        admin_id,
        payload.period_start,
        payload.period_end,
    )?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: batch,
        }),
    ))
}

/// GET /api/v1/admin/finance/transactions
pub async fn list_admin_transactions(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let txs = state.transactions.read();
    let appts = state.appointments.read();
    let patients = state.patient_profiles.read();
    let doctors = state.doctor_profiles.read();

    let mut list = Vec::new();
    for tx in txs.values() {
        let appt = appts.get(&tx.appointment_id);
        let patient_name = appt
            .and_then(|a| patients.get(&a.patient_id))
            .map(|p| p.full_name.clone())
            .unwrap_or_else(|| "Patient".into());
        let doctor_name = appt
            .and_then(|a| doctors.get(&a.doctor_id))
            .map(|d| d.full_name.clone())
            .unwrap_or_else(|| "Physician".into());

        list.push(serde_json::json!({
            "id": tx.id,
            "transaction_number": tx.transaction_number,
            "appointment_id": tx.appointment_id,
            "payment_session_id": tx.payment_session_id,
            "patient_name": patient_name,
            "doctor_name": doctor_name,
            "gateway": tx.gateway,
            "gateway_reference": tx.gateway_reference,
            "gross_amount": tx.gross_amount.to_string(),
            "platform_fee_amount": tx.platform_fee_amount.to_string(),
            "net_amount": tx.net_amount.to_string(),
            "payment_status": tx.payment_status,
            "created_at": tx.created_at,
            "settled_at": tx.settled_at,
        }));
    }

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// GET /api/v1/admin/finance/summary
pub async fn get_finance_summary(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let txs = state.transactions.read();
    let mut total_gross = BigDecimal::from(0);
    let mut total_platform_fee = BigDecimal::from(0);
    let mut total_net = BigDecimal::from(0);

    let mut bkash_vol = BigDecimal::from(0);
    let mut nagad_vol = BigDecimal::from(0);
    let mut card_vol = BigDecimal::from(0);

    for tx in txs.values() {
        total_gross += &tx.gross_amount;
        total_platform_fee += &tx.platform_fee_amount;
        total_net += &tx.net_amount;

        match tx.gateway {
            Gateway::Bkash => bkash_vol += &tx.gross_amount,
            Gateway::Nagad => nagad_vol += &tx.gross_amount,
            Gateway::Card => card_vol += &tx.gross_amount,
            _ => {}
        }
    }

    let wallets = state.doctor_wallets.read();
    let mut pending_disbursement = BigDecimal::from(0);
    for w in wallets.values() {
        pending_disbursement += &w.pending_disbursement;
    }

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "total_gross": total_gross.to_string(),
            "total_platform_fee": total_platform_fee.to_string(),
            "total_net": total_net.to_string(),
            "pending_disbursement": pending_disbursement.to_string(),
            "gateway_volume": {
                "bkash": bkash_vol.to_string(),
                "nagad": nagad_vol.to_string(),
                "card": card_vol.to_string(),
            }
        }),
    }))
}

/// GET /api/v1/admin/command/telemetry
pub async fn get_command_telemetry(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let active_specialists = docs.values().filter(|d| d.is_on_duty).count();

    let sessions = state.consultation_sessions.read();
    let telemetries = state.consultation_telemetries.read();
    let appts = state.appointments.read();
    let patients = state.patient_profiles.read();

    let mut active_streams = Vec::new();
    for session in sessions.values() {
        if session.status == SessionStatus::Active || session.status == SessionStatus::Initialized {
            let appt = appts.get(&session.appointment_id);
            let doc_name = appt
                .and_then(|a| docs.get(&a.doctor_id))
                .map(|d| d.full_name.clone())
                .unwrap_or_else(|| "Doctor".into());
            let patient_name = appt
                .and_then(|a| patients.get(&a.patient_id))
                .map(|p| p.full_name.clone())
                .unwrap_or_else(|| "Patient".into());
            let telemetry = telemetries.values().find(|t| t.session_id == session.id);

            active_streams.push(serde_json::json!({
                "session_id": session.id,
                "appointment_id": session.appointment_id,
                "channel_name": session.agora_channel_name,
                "doctor_name": doc_name,
                "patient_name": patient_name,
                "started_at": session.started_at,
                "packet_loss_percent": telemetry.and_then(|t| t.packet_loss_percent.as_ref().map(|p| p.to_string())).unwrap_or_else(|| "0.0".into()),
                "rtt_ms": telemetry.and_then(|t| t.round_trip_time_ms).unwrap_or(24),
                "connection_state": telemetry.map(|t| t.connection_state.as_str()).unwrap_or("Connected"),
            }));
        }
    }

    let completed_today = appts
        .values()
        .filter(|a| a.status == AppointmentStatus::Completed)
        .count();

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "active_specialists": active_specialists,
            "completed_today": completed_today,
            "active_sessions_count": active_streams.len(),
            "active_streams": active_streams,
        }),
    }))
}

/// GET /api/v1/admin/slots
pub async fn list_admin_slots(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let slots = state.schedule_slots.read();
    let docs = state.doctor_profiles.read();

    let mut list = Vec::new();
    for slot in slots.values() {
        let doctor_name = docs
            .get(&slot.doctor_id)
            .map(|d| d.full_name.clone())
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

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// POST /api/v1/admin/slots
pub async fn create_or_toggle_slot(
    State(state): State<AppState>,
    Json(payload): Json<SlotToggleRequest>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(slot_id) = payload.slot_id {
        let mut slots = state.schedule_slots.write();
        let slot = slots.get_mut(&slot_id).ok_or_else(|| {
            AppError::NotFound("Slot not found.".into())
        })?;

        if slot.status == SlotStatus::Available {
            slot.status = SlotStatus::Cancelled;
        } else if slot.status == SlotStatus::Cancelled {
            slot.status = SlotStatus::Available;
        }

        return Ok(Json(ApiResponse {
            success: true,
            data: serde_json::json!({
                "slot_id": slot.id,
                "status": slot.status,
            }),
        }));
    }

    if let (Some(doctor_id), Some(start), Some(end)) = (payload.doctor_id, payload.start_time, payload.end_time) {
        let slot_id = Uuid::new_v4();
        let slot = ScheduleSlot {
            id: slot_id,
            doctor_id,
            start_time: start,
            end_time: end,
            status: SlotStatus::Available,
            created_at: Utc::now(),
        };
        state.schedule_slots.write().insert(slot_id, slot.clone());

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
) -> Result<impl IntoResponse, AppError> {
    let patients = state.patient_profiles.read();
    let appts = state.appointments.read();

    let mut list = Vec::new();
    for p in patients.values() {
        let patient_appts: Vec<_> = appts
            .values()
            .filter(|a| a.patient_id == p.id)
            .collect();
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
            "last_consult_at": last_consult,
            "created_at": p.created_at,
        }));
    }

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}

/// GET /api/v1/admin/patients/:id/dossier
pub async fn get_patient_dossier(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let patients = state.patient_profiles.read();
    let patient = patients.get(&id).cloned().ok_or_else(|| {
        AppError::NotFound("Patient profile not found.".into())
    })?;

    let appts = state.appointments.read();
    let doctors = state.doctor_profiles.read();
    let encounters: Vec<_> = appts
        .values()
        .filter(|a| a.patient_id == id)
        .map(|a| {
            let doc_name = doctors
                .get(&a.doctor_id)
                .map(|d| d.full_name.clone())
                .unwrap_or_else(|| "Physician".into());
            serde_json::json!({
                "appointment_id": a.id,
                "appointment_number": a.appointment_number,
                "doctor_name": doc_name,
                "modality": a.modality,
                "status": a.status,
                "fee": a.consultation_fee.to_string(),
                "created_at": a.created_at,
                "clinical_outcome": a.clinical_outcome,
            })
        })
        .collect();

    let intake_docs = state.intake_documents.read();
    let vault_records: Vec<_> = intake_docs
        .values()
        .flat_map(|docs| docs.iter())
        .filter(|d| d.patient_id == id)
        .cloned()
        .collect();

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "patient": patient,
            "encounters": encounters,
            "vault_records": vault_records,
        }),
    }))
}

/// GET /api/v1/admin/bmdc/queue
pub async fn get_bmdc_queue(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let docs = state.doctor_profiles.read();
    let queue: Vec<_> = docs
        .values()
        .filter(|d| !d.is_verified)
        .cloned()
        .collect();

    Ok(Json(ApiResponse {
        success: true,
        data: queue,
    }))
}

/// POST /api/v1/admin/bmdc/:id/verify
pub async fn verify_bmdc_doctor(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateBmdcRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut docs = state.doctor_profiles.write();
    let doc = docs.get_mut(&id).ok_or_else(|| {
        AppError::NotFound("Doctor profile not found in BMDC queue.".into())
    })?;

    doc.is_verified = true;
    doc.updated_at = Utc::now();

    let mut audit = state.audit_events.write();
    audit.push(AuditEvent {
        id: Uuid::new_v4(),
        actor_id: Some(Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap()),
        actor_role: UserRole::PlatformAdmin,
        action: "BMDC_VERIFY".into(),
        resource_type: "DOCTOR_PROFILE".into(),
        resource_id: Some(id),
        session_id: None,
        request_id: None,
        ip_hash: None,
        result: "SUCCESS".into(),
        reason: payload.note,
        created_at: Utc::now(),
    });

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "doctor_id": id,
            "is_verified": true,
            "status": "APPROVED",
        }),
    }))
}

/// POST /api/v1/admin/bmdc/:id/reject
pub async fn reject_bmdc_doctor(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdjudicateBmdcRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut docs = state.doctor_profiles.write();
    let doc = docs.get_mut(&id).ok_or_else(|| {
        AppError::NotFound("Doctor profile not found in BMDC queue.".into())
    })?;

    doc.is_verified = false;
    doc.is_on_duty = false;
    doc.updated_at = Utc::now();

    let mut audit = state.audit_events.write();
    audit.push(AuditEvent {
        id: Uuid::new_v4(),
        actor_id: Some(Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap()),
        actor_role: UserRole::PlatformAdmin,
        action: "BMDC_REJECT".into(),
        resource_type: "DOCTOR_PROFILE".into(),
        resource_id: Some(id),
        session_id: None,
        request_id: None,
        ip_hash: None,
        result: "REJECTED".into(),
        reason: payload.note,
        created_at: Utc::now(),
    });

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "doctor_id": id,
            "is_verified": false,
            "status": "REJECTED",
        }),
    }))
}

/// GET /api/v1/admin/compliance/alerts
pub async fn get_compliance_alerts(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let prescriptions = state.prescriptions.read();
    let doctors = state.doctor_profiles.read();
    let patients = state.patient_profiles.read();

    let mut alerts = Vec::new();
    for rx in prescriptions.values() {
        if let Some(notes) = &rx.doctor_notes {
            let lower = notes.to_lowercase();
            if lower.contains("morphine") || lower.contains("pethidine") || lower.contains("schedule h") {
                let doc_name = doctors
                    .get(&rx.doctor_id)
                    .map(|d| d.full_name.clone())
                    .unwrap_or_else(|| "Doctor".into());
                let patient_name = patients
                    .get(&rx.patient_id)
                    .map(|p| p.full_name.clone())
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

    Ok(Json(ApiResponse {
        success: true,
        data: alerts,
    }))
}

/// GET /api/v1/admin/logs
pub async fn list_admin_logs(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let audit = state.audit_events.read();
    let mut logs: Vec<_> = audit.iter().cloned().collect();
    logs.reverse();

    Ok(Json(ApiResponse {
        success: true,
        data: logs,
    }))
}

/// POST /api/v1/admin/logs/simulate
pub async fn simulate_admin_log(
    State(state): State<AppState>,
    Json(payload): Json<SimulateLogRequest>,
) -> Result<impl IntoResponse, AppError> {
    let event = AuditEvent {
        id: Uuid::new_v4(),
        actor_id: Some(Uuid::parse_str("a0000001-0000-0000-0000-000000000001").unwrap()),
        actor_role: UserRole::PlatformAdmin,
        action: payload.action,
        resource_type: payload.subsystem,
        resource_id: Some(Uuid::new_v4()),
        session_id: None,
        request_id: Some(format!("sim-trace-{}", &Uuid::new_v4().to_string()[0..8])),
        ip_hash: None,
        result: payload.severity.to_uppercase(),
        reason: Some(payload.error_summary),
        created_at: Utc::now(),
    };

    state.audit_events.write().push(event.clone());

    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: event,
        }),
    ))
}

/// GET /api/v1/admin/grievances
pub async fn list_admin_grievances(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let grievances = state.grievance_reports.read();
    let patients = state.patient_profiles.read();
    let appts = state.appointments.read();
    let docs = state.doctor_profiles.read();
    let telemetries = state.consultation_telemetries.read();

    let mut list = Vec::new();
    for g in grievances.values() {
        let patient_name = patients
            .get(&g.patient_id)
            .map(|p| p.full_name.clone())
            .unwrap_or_else(|| "Patient".into());

        let appt = appts.get(&g.consultation_id);
        let doctor_name = appt
            .and_then(|a| docs.get(&a.doctor_id))
            .map(|d| d.full_name.clone())
            .unwrap_or_else(|| "Doctor".into());

        let telemetry = telemetries
            .values()
            .find(|t| t.session_id == g.consultation_id);

        list.push(serde_json::json!({
            "id": g.id,
            "grievance_number": g.grievance_number,
            "patient_id": g.patient_id,
            "patient_name": patient_name,
            "consultation_id": g.consultation_id,
            "doctor_name": doctor_name,
            "target_type": g.target_type,
            "category": g.category,
            "claim_summary": g.claim_summary,
            "status": g.status,
            "created_at": g.created_at,
            "telemetry": telemetry.map(|t| serde_json::json!({
                "duration_seconds": t.call_duration_seconds,
                "packet_loss_percent": t.packet_loss_percent.as_ref().map(|p| p.to_string()).unwrap_or_else(|| "0.0".into()),
                "rtt_ms": t.round_trip_time_ms,
                "connection_state": t.connection_state,
                "premature_end": t.premature_end,
                "prescription_issued": t.prescription_issued,
            })),
        }));
    }

    Ok(Json(ApiResponse {
        success: true,
        data: list,
    }))
}
