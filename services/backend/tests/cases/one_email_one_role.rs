//! One email belongs to one account and so to one role (product-owner
//! instruction 2026-11-15): the same address can't be a Patient and a
//! Doctor, or a Peer Navigator and either, in any letter case, including
//! when a patient's emailed-code sign-up finishes after another role took
//! the address (migration `20261222000011_users_email_one_account`).

use super::*;

fn doctor_payload(email: &str, phone: &str, license: &str) -> Value {
    serde_json::json!({
        "full_name": "Dr. Synthetic Unique",
        "license_number": license,
        "phone_number": phone,
        "email": email,
        "password": "Str0ng!Passw0rd9",
        "primary_specialty": "General Practice",
        "experience_years": 3,
        "current_hospital": "Fictional Clinic",
        "qualifications": ["MBChB"],
        "residential_address": "Nairobi",
        "accepted_documents": [
            {"type": "DOCTOR_TERMS", "version": "1.0-afyamkononi"},
            {"type": "PRIVACY_NOTICE", "version": "1.0-afyamkononi"},
            {"type": "PROFESSIONAL_VERIFICATION_DECLARATION", "version": "1.0-afyamkononi"},
            {"type": "TELEMEDICINE_PRACTITIONER_POLICY", "version": "1.0-afyamkononi"}
        ]
    })
}

async fn peer_payload(pool: &PgPool, email: &str, phone: &str) -> Value {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT document_type::text, version FROM legal_documents WHERE is_current \
         AND document_type::text IN ('PEER_TERMS','PEER_PRIVACY_NOTICE','PEER_CONDUCT_SAFEGUARDING')",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    serde_json::json!({
        "full_name": "Synthetic Peer",
        "email": email,
        "phone_number": phone,
        "password": "Peer-Navigator-2026!",
        "county": "Nairobi",
        "accepted_documents": rows.into_iter()
            .map(|(t, v)| serde_json::json!({"type": t, "version": v}))
            .collect::<Vec<_>>()
    })
}

async fn accounts_with(pool: &PgPool, email: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM users WHERE lower(email) = lower($1)")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn finish_patient_signup(app: &axum::Router, pool: &PgPool, started: &Value) -> StatusCode {
    post(
        app,
        "/api/v1/auth/patient/verify-otp",
        None,
        &serde_json::json!({
            "challenge_token": started["data"]["challenge_token"],
            "otp_code": started["data"]["debug_otp_code"],
            "accepted_documents": current_patient_acceptances(pool).await
        }),
    )
    .await
    .status()
}

#[sqlx::test(migrations = "./migrations")]
async fn one_email_is_one_account_across_patient_doctor_and_peer(pool: PgPool) {
    let (app, pool) = test_app(pool).await;

    // A Doctor's email can't become a Patient or a Peer, in any case.
    let doctor_email = "one.role.doctor@example.test";
    let created = post(
        &app,
        "/api/v1/auth/doctor/register",
        None,
        &doctor_payload(doctor_email, "0719999101", "SYNTH-KE-1101"),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let patient =
        patient_email_registration(&app, "One.Role.Doctor@Example.test", "+254719999102").await;
    assert_eq!(patient.status(), StatusCode::CONFLICT);
    let peer = post(
        &app,
        "/api/v1/auth/peer/register",
        None,
        &peer_payload(&pool, "ONE.ROLE.DOCTOR@EXAMPLE.TEST", "0719999103").await,
    )
    .await;
    assert_eq!(peer.status(), StatusCode::CONFLICT);
    assert_eq!(accounts_with(&pool, doctor_email).await, 1);

    // A Peer's email can't become a Doctor or a Patient.
    let peer_email = "one.role.peer@example.test";
    let peer = post(
        &app,
        "/api/v1/auth/peer/register",
        None,
        &peer_payload(&pool, peer_email, "0719999104").await,
    )
    .await;
    assert_eq!(peer.status(), StatusCode::CREATED);
    let doctor = post(
        &app,
        "/api/v1/auth/doctor/register",
        None,
        &doctor_payload("One.Role.Peer@example.test", "0719999105", "SYNTH-KE-1105"),
    )
    .await;
    assert_eq!(doctor.status(), StatusCode::CONFLICT);
    let patient = patient_email_registration(&app, peer_email, "+254719999106").await;
    assert_eq!(patient.status(), StatusCode::CONFLICT);
    assert_eq!(accounts_with(&pool, peer_email).await, 1);

    // A Patient's email (once sign-up completes) can't become a Doctor or Peer.
    let patient_email = "one.role.patient@example.test";
    let started =
        json_body(patient_email_registration(&app, patient_email, "+254719999107").await).await;
    assert_eq!(
        finish_patient_signup(&app, &pool, &started).await,
        StatusCode::OK
    );
    let doctor = post(
        &app,
        "/api/v1/auth/doctor/register",
        None,
        &doctor_payload(patient_email, "0719999108", "SYNTH-KE-1108"),
    )
    .await;
    assert_eq!(doctor.status(), StatusCode::CONFLICT);
    let peer = post(
        &app,
        "/api/v1/auth/peer/register",
        None,
        &peer_payload(&pool, patient_email, "0719999109").await,
    )
    .await;
    assert_eq!(peer.status(), StatusCode::CONFLICT);
    assert_eq!(accounts_with(&pool, patient_email).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn patient_code_finishing_after_another_role_took_the_email_is_refused(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let email = "race.one.role@example.test";
    // The patient starts sign-up; no account exists until the code is entered.
    let started = json_body(patient_email_registration(&app, email, "+254719999201").await).await;
    // Meanwhile a Doctor registers with the same address.
    let doctor = post(
        &app,
        "/api/v1/auth/doctor/register",
        None,
        &doctor_payload(email, "0719999202", "SYNTH-KE-1202"),
    )
    .await;
    assert_eq!(doctor.status(), StatusCode::CREATED);
    // Finishing the patient sign-up is a clean conflict, not a server error.
    assert_eq!(
        finish_patient_signup(&app, &pool, &started).await,
        StatusCode::CONFLICT
    );
    let roles: Vec<String> =
        sqlx::query_scalar("SELECT role::text FROM users WHERE lower(email) = $1")
            .bind(email)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(roles, ["DOCTOR"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_database_refuses_the_same_email_in_another_case(pool: PgPool) {
    let sql = "INSERT INTO users (id, email, phone_number, role, preferred_language, is_active, is_verified) \
               VALUES ($1, $2, $3, 'PATIENT', 'en', true, true)";
    sqlx::query(sql)
        .bind(Uuid::new_v4())
        .bind("case.check@example.test")
        .bind("+254719999301")
        .execute(&pool)
        .await
        .unwrap();
    let duplicate = sqlx::query(sql)
        .bind(Uuid::new_v4())
        .bind("Case.Check@Example.Test")
        .bind("+254719999302")
        .execute(&pool)
        .await
        .expect_err("a different-case copy of an email must be refused");
    assert_eq!(
        duplicate
            .as_database_error()
            .and_then(|e| e.code())
            .as_deref(),
        Some("23505")
    );
}
