use crate::crypto;
use crate::domain::models::*;
use crate::repository::{doctor_repo, patient_repo, session_repo, user_repo, wallet_repo};
use bigdecimal::BigDecimal;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Demo credentials, intentionally fixed so documentation/tests can reference them.
/// Only ever called when `ENVIRONMENT=development`.
pub const SEED_ADMIN_EMAIL: &str = "admin@hellodoctor.health";
pub const SEED_ADMIN_PASSWORD: &str = "AdminPass123!";
pub const SEED_ADMIN_TOTP_SECRET_BASE32: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
pub const SEED_DOCTOR1_LICENSE: &str = "BMDC #45821";
pub const SEED_DOCTOR1_PASSWORD: &str = "Password123!";
pub const SEED_PATIENT_PHONE: &str = "+8801712345678";

pub const ADMIN_USER_ID: &str = "a0000001-0000-0000-0000-000000000001";
pub const PATIENT_USER_ID: &str = "b0000001-0000-0000-0000-000000000001";
pub const PATIENT_PROFILE_ID: &str = "ba000001-0000-0000-0000-000000000001";
pub const DOCTOR1_USER_ID: &str = "10000001-0000-0000-0000-000000000001";
pub const DOCTOR1_PROFILE_ID: &str = "da000001-0000-0000-0000-000000000001";
pub const DOCTOR2_USER_ID: &str = "10000002-0000-0000-0000-000000000002";
pub const DOCTOR2_PROFILE_ID: &str = "da000002-0000-0000-0000-000000000002";

fn uid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap()
}

/// Idempotent: safe to call on every startup. Seeds the fixed demo accounts the
/// handlers' hardcoded "current actor" placeholders (patient/doctor/admin) resolve to,
/// plus a second on-duty doctor and open schedule slots.
pub async fn seed_dev_data(pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    // ---- Admin ----
    if user_repo::find_by_id(pool, uid(ADMIN_USER_ID)).await?.is_none() {
        let password_hash = crypto::hash_password(SEED_ADMIN_PASSWORD).expect("seed admin password hash");
        let admin_user = User {
            id: uid(ADMIN_USER_ID),
            phone_number: "+8801700000000".into(),
            email: Some(SEED_ADMIN_EMAIL.into()),
            password_hash: Some(password_hash),
            role: UserRole::PlatformAdmin,
            preferred_language: "en".into(),
            is_active: true,
            is_verified: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        user_repo::insert_user(pool, &admin_user).await?;

        let encrypted_secret = crypto::encrypt_totp_secret(
            SEED_ADMIN_TOTP_SECRET_BASE32,
            &totp_key_for_seed(),
        )
        .expect("seed TOTP secret encryption");

        session_repo::upsert_mfa_credential(
            pool,
            &MfaCredential {
                id: Uuid::new_v4(),
                user_id: uid(ADMIN_USER_ID),
                method: MfaMethod::Totp,
                totp_secret_encrypted: encrypted_secret,
                recovery_codes_hash: None,
                is_enabled: true,
                enabled_at: Some(now),
                last_verified_at: None,
                revoked_at: None,
                created_at: now,
            },
        )
        .await?;
    }

    // ---- Demo patient (the fixed id every patient-facing handler currently assumes) ----
    if user_repo::find_by_id(pool, uid(PATIENT_USER_ID)).await?.is_none() {
        let patient_user = User {
            id: uid(PATIENT_USER_ID),
            phone_number: SEED_PATIENT_PHONE.into(),
            email: None,
            password_hash: None,
            role: UserRole::Patient,
            preferred_language: "en".into(),
            is_active: true,
            is_verified: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        user_repo::insert_user(pool, &patient_user).await?;

        patient_repo::insert(
            pool,
            &PatientProfile {
                id: uid(PATIENT_PROFILE_ID),
                user_id: uid(PATIENT_USER_ID),
                full_name: "Patient User".into(),
                date_of_birth: chrono::NaiveDate::from_ymd_opt(1995, 1, 1).unwrap(),
                gender: Gender::Other,
                blood_group: None,
                weight_kg: None,
                emergency_contact_phone: None,
                emergency_contact_relation: None,
                created_at: now,
                updated_at: now,
            },
        )
        .await?;
    }

    // ---- Doctor 1 (matches prototype: Dr. Sabrina Akter) ----
    if doctor_repo::find_by_id(pool, uid(DOCTOR1_PROFILE_ID)).await?.is_none() {
        let password_hash = crypto::hash_password(SEED_DOCTOR1_PASSWORD).expect("seed doctor password hash");
        let doc1_user = User {
            id: uid(DOCTOR1_USER_ID),
            phone_number: "+880 1711-884920".into(),
            email: None,
            password_hash: Some(password_hash),
            role: UserRole::Doctor,
            preferred_language: "en".into(),
            is_active: true,
            is_verified: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        user_repo::insert_user(pool, &doc1_user).await?;

        doctor_repo::insert(
            pool,
            &DoctorProfile {
                id: uid(DOCTOR1_PROFILE_ID),
                user_id: uid(DOCTOR1_USER_ID),
                full_name: "Dr. Sabrina Akter".into(),
                license_number: SEED_DOCTOR1_LICENSE.into(),
                license_authority: "BMDC".into(),
                license_country: "BD".into(),
                primary_specialty: "Internal Medicine".into(),
                experience_years: 12,
                current_hospital: "Dhaka Medical College Hospital".into(),
                qualifications: vec!["MBBS".into(), "FCPS".into(), "MD".into()],
                consultation_fee_video: BigDecimal::from(800),
                consultation_fee_chat: BigDecimal::from(500),
                residential_address: "Dhanmondi, Dhaka (House 42, Road 7A)".into(),
                verified_phone: "+880 1711-884920".into(),
                is_on_duty: true,
                is_verified: true,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            },
        )
        .await?;
        wallet_repo::insert_default(pool, uid(DOCTOR1_PROFILE_ID)).await?;

        for i in 1..=5 {
            let start = now + Duration::hours(i);
            let end = start + Duration::minutes(15);
            doctor_repo::insert_slot(
                pool,
                &ScheduleSlot {
                    id: Uuid::new_v4(),
                    doctor_id: uid(DOCTOR1_PROFILE_ID),
                    start_time: start,
                    end_time: end,
                    status: SlotStatus::Available,
                    created_at: now,
                },
            )
            .await?;
        }
    }

    // ---- Doctor 2 (matches prototype: Dr. Anika Rahman) ----
    if doctor_repo::find_by_id(pool, uid(DOCTOR2_PROFILE_ID)).await?.is_none() {
        let doc2_user = User {
            id: uid(DOCTOR2_USER_ID),
            phone_number: "+880 1712-445566".into(),
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
        user_repo::insert_user(pool, &doc2_user).await?;

        doctor_repo::insert(
            pool,
            &DoctorProfile {
                id: uid(DOCTOR2_PROFILE_ID),
                user_id: uid(DOCTOR2_USER_ID),
                full_name: "Dr. Anika Rahman".into(),
                license_number: "BMDC #52891".into(),
                license_authority: "BMDC".into(),
                license_country: "BD".into(),
                primary_specialty: "Cardiology".into(),
                experience_years: 9,
                current_hospital: "National Institute of Cardiovascular Diseases".into(),
                qualifications: vec!["MBBS".into(), "MD (Cardiology)".into()],
                consultation_fee_video: BigDecimal::from(1000),
                consultation_fee_chat: BigDecimal::from(600),
                residential_address: "Gulshan-2, Dhaka".into(),
                verified_phone: "+880 1712-445566".into(),
                is_on_duty: true,
                is_verified: true,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            },
        )
        .await?;
        wallet_repo::insert_default(pool, uid(DOCTOR2_PROFILE_ID)).await?;
    }

    Ok(())
}

/// The TOTP-secret-at-rest encryption key must come from config in production; seeding
/// runs before a request-scoped `AppState` exists, so it re-reads the same env var directly.
fn totp_key_for_seed() -> String {
    std::env::var("TOTP_ENCRYPTION_KEY_B64").unwrap_or_else(|_| "zjw5FesMAwveYp3JKXqYJhbZ4O1s2yHgTAp9xgvfqq0=".into())
}
