from pathlib import Path
root=Path(__file__).resolve().parents[3]
p=root/'services/backend/tests/api_tests.rs';s=p.read_text(encoding='utf-8')
start=s.index('async fn test_complete_consultation_without_prescription(')
begin=s.index('    let doctor_id =',start);end=s.index('    let no_rx_payload =',begin)
s=s[:begin]+'''    let patient_token = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (apt_id, _) = book_and_confirm(&app, &patient_token).await;
    let doctor_id = uid(DOCTOR1_PROFILE_ID);

'''+s[end:]
pos=s.index('\n}\n',start)
s=s[:pos]+'''
    let completed = appointment_repo::find_by_id(&pool, apt_id).await.unwrap().unwrap();
    assert_eq!(completed.status, AppointmentStatus::Completed);
    assert_eq!(completed.clinical_outcome, Some(ClinicalOutcome::CompletedNoRx));
    assert!(completed.completed_at.is_some());
    let session = consultation_repo::find_by_appointment(&pool, apt_id).await.unwrap().unwrap();
    assert_eq!(session.status, SessionStatus::Completed);
    assert!(session.ended_at.is_some());
    let before: (bigdecimal::BigDecimal, bigdecimal::BigDecimal) = sqlx::query_as(
        "SELECT lifetime_net,pending_disbursement FROM doctor_wallets WHERE doctor_id=$1"
    ).bind(doctor_id).fetch_one(&pool).await.unwrap();
    assert_eq!(before.0, bigdecimal::BigDecimal::from(640));
    let retry = post(&app, &format!("/api/v1/consultations/{}/complete-no-rx", apt_id), Some(&doctor1_token), &no_rx_payload).await;
    assert_eq!(retry.status(), StatusCode::OK);
    let after: (bigdecimal::BigDecimal, bigdecimal::BigDecimal) = sqlx::query_as(
        "SELECT lifetime_net,pending_disbursement FROM doctor_wallets WHERE doctor_id=$1"
    ).bind(doctor_id).fetch_one(&pool).await.unwrap();
    assert_eq!(before, after, "no-Rx retry must never credit earnings twice");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prescriptions WHERE appointment_id=$1").bind(apt_id).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
'''+s[pos:]
p.write_text(s,encoding='utf-8')
