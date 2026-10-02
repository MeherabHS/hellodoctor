use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct PrescriptionService;

impl PrescriptionService {
    /// Patient multi-prescription upload intake (Max 5 photos per appointment)
    pub fn upload_patient_intake_documents(
        state: &AppState,
        appointment_id: Uuid,
        patient_id: Uuid,
        files: Vec<(String, Vec<u8>)>, // (mime_type, file_bytes)
    ) -> Result<Vec<PrescriptionIntakeDocument>, AppError> {
        let mut docs_map = state.intake_documents.write();
        let existing = docs_map.entry(appointment_id).or_default();

        if existing.len() + files.len() > 5 {
            return Err(AppError::UnprocessableEntity(
                "Maximum 5 prescription photos allowed per appointment.".into(),
            ));
        }

        let mut uploaded = Vec::new();
        for (mime, bytes) in files {
            // Size limit check (10 MB = 10485760 bytes)
            if bytes.len() > 10_485_760 {
                return Err(AppError::PayloadTooLarge(
                    "Uploaded file exceeds maximum limit of 10 MB.".into(),
                ));
            }

            // Magic bytes verification
            if !Self::verify_magic_bytes(&bytes, &mime) {
                return Err(AppError::UnprocessableEntity(
                    "Invalid file content. Must be a valid JPEG, PNG, or PDF.".into(),
                ));
            }

            let page_number = (existing.len() + 1) as i16;
            let hash = format!("{:x}", Sha256::digest(&bytes));
            let object_key = format!("intake/{}/{}_{}.bin", appointment_id, Uuid::now_v7(), page_number);

            let doc = PrescriptionIntakeDocument {
                id: Uuid::new_v4(),
                appointment_id,
                patient_id,
                page_number,
                object_bucket: "helodoc-medical-intake".into(),
                object_key,
                content_hash: Some(hash),
                file_size_bytes: bytes.len() as i64,
                mime_type: mime,
                created_at: Utc::now(),
            };

            existing.push(doc.clone());
            uploaded.push(doc);
        }

        Ok(uploaded)
    }

    /// Doctor handwritten prescription photo upload
    pub fn upload_doctor_prescription_photo(
        state: &AppState,
        appointment_id: Uuid,
        doctor_id: Uuid,
        photo_bytes: Vec<u8>,
        doctor_notes: Option<String>,
    ) -> Result<Prescription, AppError> {
        if photo_bytes.len() > 10_485_760 {
            return Err(AppError::PayloadTooLarge(
                "Prescription photo exceeds 10 MB maximum limit.".into(),
            ));
        }

        let apts = state.appointments.read();
        let apt = apts.get(&appointment_id).ok_or_else(|| {
            AppError::NotFound("Appointment not found.".into())
        })?;

        if apt.doctor_id != doctor_id {
            return Err(AppError::Forbidden("Caller is not the assigned physician for this appointment.".into()));
        }

        let hash = format!("{:x}", Sha256::digest(&photo_bytes));
        let rx_id = Uuid::new_v4();
        let rx_number = format!("RX-{}-{}", Utc::now().format("%Y%m%d"), &rx_id.to_string()[0..4].to_uppercase());
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

        state.prescriptions.write().insert(appointment_id, prescription.clone());
        Ok(prescription)
    }

    /// Doctor explicitly signs that no prescription is required (follow-ups/lifestyle advice)
    pub fn complete_without_prescription(
        state: &AppState,
        appointment_id: Uuid,
        doctor_id: Uuid,
        reason: String,
    ) -> Result<Prescription, AppError> {
        let apts = state.appointments.read();
        let apt = apts.get(&appointment_id).ok_or_else(|| {
            AppError::NotFound("Appointment not found.".into())
        })?;

        if apt.doctor_id != doctor_id {
            return Err(AppError::Forbidden("Caller is not the assigned physician for this appointment.".into()));
        }

        let rx_id = Uuid::new_v4();
        let rx_number = format!("NO-RX-{}-{}", Utc::now().format("%Y%m%d"), &rx_id.to_string()[0..4].to_uppercase());

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

        state.prescriptions.write().insert(appointment_id, prescription.clone());
        Ok(prescription)
    }

    fn verify_magic_bytes(bytes: &[u8], _mime: &str) -> bool {
        if bytes.len() < 4 {
            return false;
        }
        // JPEG: FF D8 FF
        let is_jpeg = bytes.starts_with(&[0xFF, 0xD8, 0xFF]);
        // PNG: 89 50 4E 47
        let is_png = bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]);
        // PDF: 25 50 44 46 (%PDF)
        let is_pdf = bytes.starts_with(&[0x25, 0x50, 0x44, 0x46]);

        is_jpeg || is_png || is_pdf
    }
}
