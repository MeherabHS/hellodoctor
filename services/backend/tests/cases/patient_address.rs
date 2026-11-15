//! Patients give a residential address at registration (product-owner
//! instruction 2026-11-14; migration `20261221000004`). It is required,
//! shown only on the patient's own profile, editable there, and cleared by
//! account erasure.

use super::*;

#[sqlx::test(migrations = "./migrations")]
async fn patient_registration_requires_and_stores_residential_address(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let base = serde_json::json!({
        "full_name": "Address Patient",
        "date_of_birth": "1990-05-01",
        "gender": "FEMALE",
        "email": "address.patient@example.test",
        "phone_number": "+254700009971",
        "password": "Synthetic-Pass-2026!"
    });
    for bad in [
        serde_json::Value::Null,
        serde_json::json!(""),
        serde_json::json!("  ab "),
    ] {
        let mut payload = base.clone();
        if !bad.is_null() {
            payload["residential_address"] = bad.clone();
        }
        let response = post(&app, "/api/v1/auth/patient/register", None, &payload).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "address {bad} must be rejected"
        );
    }

    let started = json_body(
        patient_email_registration(&app, "address.patient@example.test", "+254700009971").await,
    )
    .await;
    let created = post(
        &app,
        "/api/v1/auth/patient/verify-otp",
        None,
        &serde_json::json!({
            "challenge_token": started["data"]["challenge_token"],
            "otp_code": started["data"]["debug_otp_code"],
            "accepted_documents": current_patient_acceptances(&pool).await
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let token = json_body(created).await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let me = json_body(get(&app, "/api/v1/patients/me", Some(&token)).await).await;
    assert_eq!(me["data"]["residential_address"], "Kilimani, Nairobi");

    let updated = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&token),
        Some(&serde_json::json!({"residential_address": "  Westlands, Nairobi  "})),
        None,
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let me = json_body(get(&app, "/api/v1/patients/me", Some(&token)).await).await;
    assert_eq!(me["data"]["residential_address"], "Westlands, Nairobi");

    let too_short = send(
        &app,
        "PATCH",
        "/api/v1/patients/me",
        Some(&token),
        Some(&serde_json::json!({"residential_address": "x"})),
        None,
    )
    .await;
    assert_eq!(too_short.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
