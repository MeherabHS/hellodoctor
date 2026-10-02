use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{appointment_repo, prescription_repo, AppState};
use crate::services::upload_security_service::UploadSecurityService;
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct PrescriptionService;

impl PrescriptionService {
    /// Patient multi-prescription upload intake (Max 5 photos per appointment)
    pub async fn upload_patient_intake_documents(
        state: &AppState,
        appointment_id: Uuid,
        patient_id: Uuid,
        files: Vec<(String, Vec<u8>)>, // (mime_type, file_bytes)
    ) -> Result<Vec<PrescriptionIntakeDocument>, AppError> {
        let existing_count = prescription_repo::count_intake_documents(&state.db, appointment_id).await?;

        if existing_count as usize + files.len() > 5 {
            return Err(AppError::UnprocessableEntity(
                "Maximum 5 prescription photos allowed per appointment.".into(),
            ));
        }

        let mut uploaded = Vec::new();
        let mut next_page = existing_count as i16;

        for (mime, bytes) in files {
            let sanitized = UploadSecurityService::validate_and_sanitize(&bytes, &mime)?;
            next_page += 1;

            let hash = format!("{:x}", Sha256::digest(&sanitized));
            let object_key = format!("intake/{}/{}_{}.bin", appointment_id, Uuid::now_v7(), next_page);

            let doc = PrescriptionIntakeDocument {
                id: Uuid::new_v4(),
                appointment_id,
                patient_id,
                page_number: next_page,
                object_bucket: "helodoc-medical-intake".into(),
                object_key,
                content_hash: Some(hash),
                file_size_bytes: sanitized.len() as i64,
                mime_type: mime,
                created_at: Utc::now(),
            };

            prescription_repo::insert_intake_document(&state.db, &doc).await?;
            uploaded.push(doc);
        }

        Ok(uploaded)
    }

    /// Doctor handwritten prescription photo upload
    pub async fn upload_doctor_prescription_photo(
        state: &AppState,
        appointment_id: Uuid,
        doctor_id: Uuid,
        photo_bytes: Vec<u8>,
        doctor_notes: Option<String>,
    ) -> Result<Prescription, AppError> {
        let apt = appointment_repo::find_by_id(&state.db, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

        if apt.doctor_id != doctor_id {
            return Err(AppError::Forbidden("Caller is not the assigned physician for this appointment.".into()));
        }

        let sanitized = UploadSecurityService::validate_and_sanitize(&photo_bytes, "image/jpeg")?;

        let hash = format!("{:x}", Sha256::digest(&sanitized));
        let rx_id = Uuid::new_v4();
        let rx_number = format!("RX-{}-{}", Utc::now().format("%Y%m%d"), rx_id.to_string()[0..4].to_uppercase());
        let object_key = format!("rx/{}/{}.jpg", appointment_id, Uuid::now_v7());

        let prescription = Prescription {
            id: rx_id,
            appointment_id,
            doctor_id,
            patient_id: apt.patient_id,
            rx_number,
            integrity_verification_hash: Some(hash),
            rx_image_object_bucket: Some("helodoc-prescriptions".into()),
            rx_image_object_key: Some(object_key),
            doctor_notes,
            is_no_rx_required: false,
            no_rx_reason: None,
            created_at: Utc::now(),
        };

        prescription_repo::insert_prescription(&state.db, &prescription).await?;
        Ok(prescription)
    }

    /// Doctor explicitly signs that no prescription is required (follow-ups/lifestyle advice)
    pub async fn complete_without_prescription(
        state: &AppState,
        appointment_id: Uuid,
        doctor_id: Uuid,
        reason: String,
    ) -> Result<Prescription, AppError> {
        let apt = appointment_repo::find_by_id(&state.db, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

        if apt.doctor_id != doctor_id {
            return Err(AppError::Forbidden("Caller is not the assigned physician for this appointment.".into()));
        }

        let rx_id = Uuid::new_v4();
        let rx_number = format!("NO-RX-{}-{}", Utc::now().format("%Y%m%d"), rx_id.to_string()[0..4].to_uppercase());

        let prescription = Prescription {
            id: rx_id,
            appointment_id,
            doctor_id,
            patient_id: apt.patient_id,
            rx_number,
            integrity_verification_hash: None,
            rx_image_object_bucket: None,
            rx_image_object_key: None,
            doctor_notes: None,
            is_no_rx_required: true,
            no_rx_reason: Some(reason),
            created_at: Utc::now(),
        };

        prescription_repo::insert_prescription(&state.db, &prescription).await?;
        Ok(prescription)
    }
}
