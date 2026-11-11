//! M8-G-20 narrow investigation: `list_admin_doctors`, `list_admin_transactions`,
//! `list_admin_grievances` — the three handlers `CHECKPOINT.md:1724` flagged
//! as suspected of sharing the whole-table-fetch pattern already fixed
//! elsewhere (`get_command_telemetry`). Deliberately NOT part of the regular
//! suite (`#[ignore]`d) — this seeds >=10,000 rows and prints EXPLAIN
//! ANALYZE plans plus wall-clock timings, it does not assert pass/fail.
//! Run explicitly:
//!   cargo test --test api_tests admin_query_g20_investigation:: -- --ignored --exact --test-threads=1 --nocapture
//!
//! See docs/MILESTONE_8_G20_ADMIN_QUERY_VERIFICATION.md for the conclusions
//! drawn from this evidence.
use super::*;
use std::time::Instant;

/// Seeds exactly 10,000 non-overlapping appointments (and their slots,
/// transactions, and consultation sessions) for the shared seeded
/// DOCTOR1/PATIENT pair. Sequential 15-minute slots starting from a fixed
/// synthetic epoch are used specifically so `no_overlapping_slots`/
/// `appt_patient_no_overlap`/`appt_doctor_no_overlap` are never at risk of
/// firing — every interval is disjoint from every other by construction, so
/// a single doctor/patient pair can legitimately hold all 10,000 rows.
async fn seed_10k_appointments_and_transactions(pool: &PgPool) {
    let doctor = uid(DOCTOR1_PROFILE_ID);
    let patient = uid(PATIENT_PROFILE_ID);

    let slot_ids: Vec<Uuid> = sqlx::query_scalar(
        "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
         SELECT gen_random_uuid(), $1, \
                '2020-01-01T00:00:00Z'::timestamptz + (g * interval '15 minutes'), \
                '2020-01-01T00:00:00Z'::timestamptz + (g * interval '15 minutes') + interval '14 minutes', \
                'BOOKED', clock_timestamp() \
         FROM generate_series(0, 9999) AS g \
         RETURNING id",
    )
    .bind(doctor)
    .fetch_all(pool)
    .await
    .unwrap();

    let appointment_ids: Vec<Uuid> = sqlx::query_scalar(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, created_at, updated_at) \
         SELECT gen_random_uuid(), 'G20-'||row_number() OVER (), $2, $3, s, \
                (CASE WHEN (row_number() OVER ()) % 2 = 0 THEN 'VIDEO' ELSE 'CHAT' END)::modality_enum, \
                'COMPLETED'::appointment_status_enum, 100.00, clock_timestamp(), clock_timestamp() \
         FROM UNNEST($1::uuid[]) AS s \
         RETURNING id",
    )
    .bind(&slot_ids)
    .bind(patient)
    .bind(doctor)
    .fetch_all(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO transactions (id, transaction_number, appointment_id, payment_session_id, gateway, gross_amount, platform_fee_amount, net_amount, payment_status, created_at, settled_at) \
         SELECT gen_random_uuid(), 'G20TX-'||row_number() OVER (), a, gen_random_uuid(), 'MPESA'::gateway_enum, \
                100.00, 20.00, 80.00, 'SETTLED_TO_DOCTOR'::payment_status_enum, \
                clock_timestamp() - (row_number() OVER () % 400) * interval '1 day', clock_timestamp() \
         FROM UNNEST($1::uuid[]) AS a",
    )
    .bind(&appointment_ids)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO consultation_sessions (id, appointment_id, agora_channel_name, started_at, ended_at, status, created_at) \
         SELECT gen_random_uuid(), a, 'g20_'||a::text, clock_timestamp(), clock_timestamp(), \
                'COMPLETED'::session_status_enum, clock_timestamp() \
         FROM UNNEST($1::uuid[]) AS a",
    )
    .bind(&appointment_ids)
    .execute(pool)
    .await
    .unwrap();
}

/// A smaller, realistic grievance-table population (2,000 rows) referencing
/// a subset of the 10,000 consultation sessions just seeded — grievances are
/// not expected to approach the same order of magnitude as appointments in
/// any realistic near-term deployment, but 2,000 is still well beyond what a
/// naive in-memory loop should be exercised against casually.
async fn seed_2k_grievances(pool: &PgPool) {
    let patient = uid(PATIENT_PROFILE_ID);
    sqlx::query(
        "INSERT INTO grievance_reports (id, grievance_number, patient_id, consultation_id, target_type, category, claim_summary, status, created_at) \
         SELECT gen_random_uuid(), 'G20GR-'||row_number() OVER (), $1, cs.id, \
                'DOCTOR'::grievance_target_enum, 'Quality of care', 'Synthetic G-20 investigation grievance row.', \
                'PENDING_REVIEW'::grievance_status_enum, clock_timestamp() \
         FROM consultation_sessions cs \
         WHERE cs.agora_channel_name LIKE 'g20_%' \
         ORDER BY cs.id LIMIT 2000",
    )
    .bind(patient)
    .execute(pool)
    .await
    .unwrap();
}

async fn explain_analyze(pool: &PgPool, label: &str, sql: &str) {
    let plan: Vec<String> =
        sqlx::query_scalar(&format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT) {sql}"))
            .fetch_all(pool)
            .await
            .unwrap();
    eprintln!("\n=== EXPLAIN ANALYZE: {label} ===");
    for line in plan {
        eprintln!("{line}");
    }
}

#[sqlx::test(migrations = "./migrations")]
#[ignore]
async fn investigate_admin_doctor_transaction_grievance_queries(pool: PgPool) {
    seed::seed_dev_data(&pool).await.expect("seed dev data");
    let seed_start = Instant::now();
    seed_10k_appointments_and_transactions(&pool).await;
    seed_2k_grievances(&pool).await;
    eprintln!("\n=== Fixture seeded in {:?} ===", seed_start.elapsed());

    let (appt_count, tx_count, grievance_count): (i64, i64, i64) = (
        sqlx::query_scalar("SELECT COUNT(*) FROM appointments")
            .fetch_one(&pool)
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT COUNT(*) FROM grievance_reports")
            .fetch_one(&pool)
            .await
            .unwrap(),
    );
    eprintln!(
        "Fixture sizes: appointments={appt_count} transactions={tx_count} grievances={grievance_count}"
    );

    // --- 1. EXPLAIN ANALYZE on the actual whole-table SELECT statements ----
    explain_analyze(
        &pool,
        "appointment_repo::list_all underlying SELECT",
        "SELECT id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, clinical_outcome, completed_at, created_at, updated_at, hold_expires_at FROM appointments",
    )
    .await;
    explain_analyze(
        &pool,
        "transaction_repo::list_all underlying SELECT",
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id FROM transactions ORDER BY created_at DESC",
    )
    .await;
    explain_analyze(
        &pool,
        "list_admin_transactions' in-memory month filter, pushed into SQL instead (proposed correction, evaluated here before deciding to apply it)",
        "SELECT id, transaction_number, appointment_id, payment_session_id, gateway, gateway_reference, gross_amount, platform_fee_amount, net_amount, payment_status, reconciliation_required, created_at, settled_at, currency, disbursement_item_id, payment_method_id FROM transactions WHERE created_at >= '2026-06-01' AND created_at < '2026-07-01' ORDER BY created_at DESC",
    )
    .await;
    explain_analyze(
        &pool,
        "list_admin_doctors' appointment-count aggregation, current whole-table-fetch-then-filter-in-Rust shape has no SQL equivalent to EXPLAIN directly -- shown instead: the proposed single GROUP BY replacement",
        "SELECT doctor_id, COUNT(*) FILTER (WHERE status='COMPLETED') AS completed, COUNT(*) FILTER (WHERE modality='VIDEO') AS video, COUNT(*) FILTER (WHERE modality='CHAT') AS chat FROM appointments GROUP BY doctor_id",
    )
    .await;

    // --- 2. Wall-clock timing of the actual repo functions as currently written ---
    let t = Instant::now();
    let appts = hellodoctor_backend::repository::appointment_repo::list_all(&pool)
        .await
        .unwrap();
    eprintln!(
        "appointment_repo::list_all -> {} rows in {:?}",
        appts.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let txs = hellodoctor_backend::repository::transaction_repo::list_all(&pool)
        .await
        .unwrap();
    eprintln!(
        "transaction_repo::list_all -> {} rows in {:?}",
        txs.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let docs = hellodoctor_backend::repository::doctor_repo::list(&pool)
        .await
        .unwrap();
    eprintln!(
        "doctor_repo::list -> {} rows in {:?}",
        docs.len(),
        t.elapsed()
    );

    let t = Instant::now();
    let patients = hellodoctor_backend::repository::patient_repo::list(&pool)
        .await
        .unwrap();
    eprintln!(
        "patient_repo::list -> {} rows in {:?}",
        patients.len(),
        t.elapsed()
    );

    // --- 3. End-to-end timing of each real HTTP handler, through the actual
    //        router, exactly as a browser request would hit it ------------
    let (app, _pool, _state) = test_app_and_state(pool.clone()).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let t = Instant::now();
    let res = get(&app, "/api/v1/admin/doctors", Some(&admin)).await;
    let doctors_elapsed = t.elapsed();
    assert_eq!(res.status(), StatusCode::OK);
    eprintln!("GET /admin/doctors end-to-end: {doctors_elapsed:?}");

    let t = Instant::now();
    let res = get(&app, "/api/v1/admin/finance/transactions", Some(&admin)).await;
    let tx_unfiltered_elapsed = t.elapsed();
    assert_eq!(res.status(), StatusCode::OK);
    eprintln!(
        "GET /admin/finance/transactions (no month filter) end-to-end: {tx_unfiltered_elapsed:?}"
    );

    let t = Instant::now();
    let res = get(
        &app,
        "/api/v1/admin/finance/transactions?month=2026-06",
        Some(&admin),
    )
    .await;
    let tx_filtered_elapsed = t.elapsed();
    assert_eq!(res.status(), StatusCode::OK);
    eprintln!("GET /admin/finance/transactions?month=2026-06 end-to-end: {tx_filtered_elapsed:?}");

    let t = Instant::now();
    let res = get(&app, "/api/v1/admin/grievances", Some(&admin)).await;
    let grievances_elapsed = t.elapsed();
    assert_eq!(res.status(), StatusCode::OK);
    eprintln!("GET /admin/grievances end-to-end: {grievances_elapsed:?}");

    eprintln!("\n=== SUMMARY (fixture: {appt_count} appointments, {tx_count} transactions, {grievance_count} grievances) ===");
    eprintln!("GET /admin/doctors:                              {doctors_elapsed:?}");
    eprintln!("GET /admin/finance/transactions (unfiltered):     {tx_unfiltered_elapsed:?}");
    eprintln!("GET /admin/finance/transactions?month=2026-06:    {tx_filtered_elapsed:?}");
    eprintln!("GET /admin/grievances:                            {grievances_elapsed:?}");
}
