//! Stage A regression coverage for migration `20261111000001_schedule_exclusivity`
//! (INV-BOOK-01/02/03 — see that migration's own header comment). Each test
//! below maps to one item in the Stage A resume brief's missing-coverage list:
//!
//!   1. Same Patient, overlapping appointments, different Doctors
//!   2. Partial interval overlap vs. adjacent non-overlapping intervals
//!   3. Same Doctor double-booking across two distinct (overlapping) slot rows
//!   4/5. Concurrent booking across genuinely independent DB transactions
//!   6. Rescheduling into a conflicting Patient appointment
//!   7. Rescheduling into a conflicting Doctor appointment
//!   8. Rejected reschedule preserves the original booking + its finances
//!   9. Expired payment-hold reclamation
//!  10. Concurrent hold-expiry workers (SKIP LOCKED partitioning)
//!  11. Payment confirmation racing hold expiration
//!  12. Doctor-slot time change that would create a conflict
//!  13/14. Migration preflight fail-closed + rollback/reapply data safety
//!  15. RLS: the new columns stay scoped by the existing Patient-ownership policy
//!
//! Per the resume brief's own test-quality requirement, the concurrency tests
//! here use real independent `PgPool` connections/transactions and assert on
//! final persisted appointment/slot state — never "some HTTP calls failed".
//!
//! Three pre-existing protections, found while writing this file, reshape
//! what "same Doctor double-booking" can even mean here:
//!   - `no_overlapping_slots` (initial schema): one real Doctor can never
//!     hold two overlapping, non-cancelled `doctor_schedule_slots` rows.
//!   - `appointment_slot_doctor` (`20261026000001_canonical_user_isolation`):
//!     a `FOREIGN KEY (slot_id, doctor_id) REFERENCES
//!     doctor_schedule_slots(id, doctor_id)` — an appointment's `doctor_id`
//!     can never diverge from its own slot's real owner.
//! Together they mean `appt_doctor_no_overlap` (Stage A) can only ever be
//! reached via a data-integrity edge case (an appointment's `doctor_id`
//! claim mismatching its slot's real owner), not through any ordinary,
//! FK-consistent booking — see items 3 and 12 below for exactly where each
//! constraint actually fires, and why.
#![allow(clippy::doc_lazy_continuation)] // numbered/"4/5."-style list markers above read fine as prose; not worth reformatting into clippy's strict list-continuation indentation.
use super::*;
use chrono::SubsecRound;
use hellodoctor_backend::error::AppError;

// ============================================================================
// Local fixtures (kept separate from api_tests.rs's own fixtures because these
// need explicit control over doctor/slot/patient identity and timing that the
// existing DOCTOR1-only, "first available slot" helpers don't expose).
// ============================================================================

/// A brand-new Patient user + profile, independent of the shared seeded
/// patient, with its own auth session. Mirrors `fresh_patient_token` but also
/// returns the `patient_profiles.id`, which direct appointment-row fixtures
/// below need and `fresh_patient_token` does not expose.
async fn new_patient(pool: &PgPool) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) VALUES($1,$2,'PATIENT'::user_role_enum,true,true,NOW(),NOW())")
        .bind(user_id)
        .bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO patient_profiles (id, user_id, full_name, date_of_birth, gender, created_at, updated_at) \
         VALUES ($1, $2, 'Schedule Exclusivity Test Patient', '1990-01-01', 'OTHER', NOW(), NOW())",
    )
    .bind(profile_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    fixture_session(pool, user_id).await;
    (profile_id, make_token(user_id, "PATIENT"))
}

/// A brand-new `doctor_profiles` row, independent of `seed::seed_dev_data`'s
/// two fixed Doctors. Several tests below need more independent Doctor
/// identities than the seed provides — e.g. to own N distinct, mutually
/// non-conflicting slots (`doctor_schedule_slots`'s own pre-existing
/// `no_overlapping_slots` exclusion constraint, from the very first
/// migration, already forbids any one Doctor from having two overlapping
/// slot rows — see the discovery note on `appt_doctor_no_overlap`'s tests
/// below). Works against any schema from the initial migration onward, so
/// it is also used by the `migrations = false` migration-safety tests.
async fn new_doctor(pool: &PgPool) -> Uuid {
    let user_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) VALUES($1,$2,'DOCTOR'::user_role_enum,true,true,NOW(),NOW())")
        .bind(user_id)
        .bind(format!("+2547{:08}", rand::thread_rng().gen_range(0..100_000_000)))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO doctor_profiles (id, user_id, full_name, license_number, license_authority, license_country, primary_specialty, experience_years, current_hospital, consultation_fee_video, consultation_fee_chat, residential_address, verified_phone, is_verified, created_at, updated_at) \
         VALUES ($1,$2,'Schedule Exclusivity Test Doctor',$3,'KMPDC','KE','General Practice',5,'Test Hospital',500.00,300.00,'Test Address','+254700000000',true,NOW(),NOW())",
    )
    .bind(profile_id)
    .bind(user_id)
    .bind(format!("LIC-{}", profile_id.to_string()[0..12].to_uppercase()))
    .execute(pool)
    .await
    .unwrap();
    profile_id
}

/// Inserts an `AVAILABLE` slot directly, bypassing `POST /doctor/slots`'s own
/// same-doctor overlap check (`doctor_handlers::create_own_slot`). That check
/// is a convenience for the normal scheduling UI; the exclusion constraints
/// under test here are the actual authority, so tests that need two
/// overlapping slot rows for one Doctor must go around it deliberately — the
/// same way out-of-band/legacy data could.
async fn raw_slot(
    pool: &PgPool,
    doctor_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
         VALUES ($1,$2,$3,$4,'AVAILABLE',clock_timestamp())",
    )
    .bind(id)
    .bind(doctor_id)
    .bind(start)
    .bind(end)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Books `slot_id` for `doctor_id` through the real HTTP booking endpoint and
/// immediately confirms it via a signed payment webhook, landing the
/// appointment in `CONFIRMED`. Unlike `book_and_confirm`/`milestone_book_chat`
/// this lets the caller pick exactly which slot (and therefore which time) is
/// booked, which every overlap scenario below depends on.
async fn book_and_confirm_at(
    app: &axum::Router,
    patient_token: &str,
    doctor_id: Uuid,
    slot_id: Uuid,
) -> axum::http::Response<Body> {
    let payload = serde_json::json!({
        "doctor_id": doctor_id,
        "slot_id": slot_id,
        "modality": "VIDEO",
        "gateway": "MPESA",
        "chief_complaint": "Schedule exclusivity regression fixture"
    });
    let idem_key = Uuid::new_v4().to_string();
    // `role_pool` (this file) gives each test's payment-worker pool a 100ms
    // idle timeout, and `begin_authed_as` (production) fails a connection
    // acquire closed after 250ms. On a host where establishing a fresh
    // Postgres connection is itself slow, the pool's one warm connection can
    // already be reaped by the time this first booking call lands, and the
    // resulting cold connect loses the 250ms race — a transient
    // "SERVICE_UNAVAILABLE" with no relation to the exclusivity invariant
    // under test. Confirmed independently: an existing, unmodified test
    // (`test_bola_patient_cannot_view_another_patients_appointment`) hits the
    // exact same 503 on this host. The idempotency middleware only caches
    // *successful* responses (`idempotency_middleware`, `!response.status
    // ().is_success()`), so retrying the identical key is safe — it can
    // never replay a stale result or double-book.
    for attempt in 0..5 {
        let res = post_idem(
            app,
            "/api/v1/appointments/book",
            Some(patient_token),
            &payload,
            &idem_key,
        )
        .await;
        if res.status() != StatusCode::SERVICE_UNAVAILABLE || attempt == 4 {
            return res;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    unreachable!()
}

async fn confirm_via_webhook(app: &axum::Router, payment_session_id: Uuid) {
    let payload = serde_json::json!({
        "payment_session_id": payment_session_id,
        "transaction_reference": format!("MPESA-TEST-{}", Uuid::new_v4()),
        "status": "SUCCESS",
        "event_timestamp": chrono::Utc::now().to_rfc3339(),
        "provider_event_id": Uuid::new_v4().to_string(),
    });
    let body_str = serde_json::to_string(&payload).unwrap();
    let sig = sign_webhook_body(&body_str, TEST_WEBHOOK_SECRET);
    let res = send_webhook_provider(app, "mpesa", &body_str, &sig).await;
    assert_eq!(res.status(), StatusCode::OK);
}

/// End-to-end booking fixture: books `slot_id` for `doctor_id` and confirms
/// it, returning the new `appointment_id`. Fails the test loudly if either
/// step is rejected, since every test using this expects the setup itself to
/// succeed (the conflict under test always comes later).
async fn book_confirmed(
    app: &axum::Router,
    patient_token: &str,
    doctor_id: Uuid,
    slot_id: Uuid,
) -> Uuid {
    let res = book_and_confirm_at(app, patient_token, doctor_id, slot_id).await;
    let status = res.status();
    let body = json_body(res).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "fixture booking failed: {body}"
    );
    let appointment_id = uid(body["data"]["appointment_id"].as_str().unwrap());
    let payment_session_id = uid(body["data"]["payment_session_id"].as_str().unwrap());
    confirm_via_webhook(app, payment_session_id).await;
    appointment_id
}

/// Directly inserts a `CONFIRMED` appointment for `(patient_id, doctor_id,
/// slot_id)`, exercising the `appointments_slot_period` trigger and the two
/// exclusion constraints exactly as a real booking would, without the HTTP/
/// payment machinery around it. Used by the low-level concurrency and
/// trigger tests, which need to isolate the constraint itself.
async fn insert_confirmed<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    patient_id: Uuid,
    doctor_id: Uuid,
    slot_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,'VIDEO','CONFIRMED',100.00,clock_timestamp(),clock_timestamp())",
    )
    .bind(id)
    .bind(format!("APT-TEST-{}", id.to_string()[0..8].to_uppercase()))
    .bind(patient_id)
    .bind(doctor_id)
    .bind(slot_id)
    .execute(executor)
    .await?;
    Ok(id)
}

fn doctor1() -> Uuid {
    uid(DOCTOR1_PROFILE_ID)
}
fn doctor2() -> Uuid {
    uid(DOCTOR2_PROFILE_ID)
}

// ============================================================================
// 1. Same Patient, overlapping appointments, different Doctors.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_same_patient_overlapping_appointment_with_different_doctor_rejected(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (_patient_id, patient_token) = new_patient(&pool).await;

    let start = Utc::now() + Duration::days(40);
    let d1_slot = raw_slot(&pool, doctor1(), start, start + Duration::minutes(30)).await;
    let d2_slot = raw_slot(
        &pool,
        doctor2(),
        start + Duration::minutes(10),
        start + Duration::minutes(40),
    )
    .await;

    let first = book_and_confirm_at(&app, &patient_token, doctor1(), d1_slot).await;
    assert_eq!(first.status(), StatusCode::CREATED);

    // Same Patient, different Doctor, overlapping interval: must be rejected
    // even though Doctor2's own schedule has no conflict at all.
    let second = book_and_confirm_at(&app, &patient_token, doctor2(), d2_slot).await;
    assert_eq!(
        second.status(),
        StatusCode::CONFLICT,
        "a Patient must not be able to hold two overlapping appointments, even with two different Doctors"
    );

    let active_for_patient: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM appointments WHERE patient_id=(SELECT patient_id FROM appointments WHERE slot_id=$1) \
         AND status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')",
    )
    .bind(d1_slot)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_for_patient, 1,
        "the rejected second booking must not persist any active appointment"
    );

    // The rejected attempt must not leave Doctor2's slot stuck locked: the
    // whole booking transaction (including try_lock_slot's UPDATE) rolls back.
    assert_eq!(schedule_slot_status(&pool, d2_slot).await, "AVAILABLE");
}

// ============================================================================
// 2. Partial interval overlap vs. adjacent non-overlapping intervals.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_partial_overlap_rejected_adjacent_interval_allowed(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (_patient_id, patient_token) = new_patient(&pool).await;

    let t0 = Utc::now() + Duration::days(41);
    let d1_slot = raw_slot(&pool, doctor1(), t0, t0 + Duration::minutes(30)).await;
    let first = book_and_confirm_at(&app, &patient_token, doctor1(), d1_slot).await;
    assert_eq!(first.status(), StatusCode::CREATED);

    // Adjacent, not overlapping: [t0,t0+30) and [t0+30,t0+60) share only a
    // boundary instant, which the half-open `[)` range treats as disjoint.
    let d2_adjacent = raw_slot(
        &pool,
        doctor2(),
        t0 + Duration::minutes(30),
        t0 + Duration::minutes(60),
    )
    .await;
    let adjacent = book_and_confirm_at(&app, &patient_token, doctor2(), d2_adjacent).await;
    assert_eq!(
        adjacent.status(),
        StatusCode::CREATED,
        "back-to-back, non-overlapping appointments for the same Patient must both be allowed"
    );

    // Partial overlap with the just-booked [t0+30,t0+60) interval.
    let d1_partial = raw_slot(
        &pool,
        doctor1(),
        t0 + Duration::minutes(45),
        t0 + Duration::minutes(75),
    )
    .await;
    let partial = book_and_confirm_at(&app, &patient_token, doctor1(), d1_partial).await;
    assert_eq!(
        partial.status(),
        StatusCode::CONFLICT,
        "a slot that partially overlaps an existing active appointment must be rejected, not just an exact duplicate"
    );

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM appointments a JOIN patient_profiles p ON p.id=a.patient_id \
         WHERE p.full_name='Schedule Exclusivity Test Patient' AND a.status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_count, 2,
        "exactly the two non-overlapping bookings, not the rejected partial-overlap one"
    );
}

// ============================================================================
// 3. Same Doctor double-booking across two distinct (overlapping) slot rows.
//
// Discovery while writing this test: `doctor_schedule_slots` already carries
// its own pre-existing `no_overlapping_slots` EXCLUDE constraint, from the
// very first migration (20261001000001) — `EXCLUDE USING gist (doctor_id
// WITH =, tstzrange(start_time,end_time) WITH &&) WHERE status<>'CANCELLED'`.
// That means two overlapping, non-cancelled SLOT rows for one real Doctor
// were already structurally impossible before Stage A, so `raw_slot` cannot
// construct the originally-planned premise (confirmed: it fails with
// `no_overlapping_slots` the instant the second overlapping slot is
// inserted, never reaching an appointment insert at all).
//
// `appt_doctor_no_overlap`'s real, narrower job is therefore defense-in-depth
// against `appointments.doctor_id` ever disagreeing with its own slot's
// `doctor_id` — exactly the data-integrity case
// `scripts/preflight_schedule_exclusivity.sql`'s check #5 watches for
// ("Informational: appointment.doctor_id differs from its slot.doctor_id").
// This test constructs that scenario directly: two slots with genuinely
// different real owners (so `no_overlapping_slots` never objects), then two
// appointments that both claim the SAME `doctor_id` regardless of which slot
// they point at.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_same_doctor_double_booking_across_distinct_slots_rejected_at_db_level(pool: PgPool) {
    let (patient_a_id, _) = new_patient(&pool).await;
    let (patient_b_id, _) = new_patient(&pool).await;
    let subject_doctor = new_doctor(&pool).await;
    let other_doctor = new_doctor(&pool).await;

    let start = Utc::now() + Duration::days(42);
    let slot_a = raw_slot(&pool, subject_doctor, start, start + Duration::minutes(30)).await;
    let slot_b = raw_slot(
        &pool,
        other_doctor,
        start + Duration::minutes(15),
        start + Duration::minutes(45),
    )
    .await;

    insert_confirmed(&pool, patient_a_id, subject_doctor, slot_a)
        .await
        .expect("first appointment on slot_a (consistent with its real owner) must succeed");

    let err = insert_confirmed(&pool, patient_b_id, subject_doctor, slot_b)
        .await
        .expect_err("a second active appointment claiming the same Doctor, overlapping that Doctor's other appointment, must be rejected even though slot_b's REAL owner is a different Doctor");
    let db_err = err.as_database_error().unwrap();
    assert_eq!(db_err.code().as_deref(), Some("23P01"));
    assert_eq!(db_err.constraint(), Some("appt_doctor_no_overlap"));

    let mapped: AppError = err.into();
    assert!(
        matches!(mapped, AppError::Conflict(_)),
        "error.rs must map this exclusion violation to a 409, not a 500"
    );

    let doctor_active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM appointments WHERE doctor_id=$1 AND status IN ('PENDING_PAYMENT','CONFIRMED','WAITING','IN_CONSULTATION')",
    )
    .bind(subject_doctor)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(doctor_active, 1);
}

// ============================================================================
// 4/5. Concurrent booking across genuinely independent DB transactions.
//
// The resume brief is explicit that the existing 50-concurrent-booking test
// (api_tests.rs, same slot_id, admission-control shedding) is NOT sufficient
// proof of the exclusivity invariant. These two tests instead open N real,
// independent `PgPool` connections/transactions and inspect final persisted
// counts/statuses — never "some requests failed". The Patient-side test races
// N independent appointment INSERTs, each against its own distinct,
// genuinely non-conflicting slot (`appt_patient_no_overlap` is the only thing
// that can decide the outcome). The Doctor-side test races N independent slot
// INSERTs instead of appointment INSERTs — see that test's own doc comment
// for why an appointments-level Doctor race is not constructible at all,
// given this schema's own pre-existing guarantees.
// ============================================================================

async fn concurrent_overlap_race(
    pool: &PgPool,
    rows: Vec<(Uuid, Uuid, Uuid)>, // (patient_id, doctor_id, slot_id) per attempt
) -> (usize, usize) {
    let mut handles = Vec::with_capacity(rows.len());
    for (patient_id, doctor_id, slot_id) in rows {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut tx = pool.begin().await.unwrap();
            let locked = doctor_repo::try_lock_slot(
                &mut tx,
                slot_id,
                DoctorProfileId::from(doctor_id),
            )
            .await
            .unwrap();
            assert!(locked, "each attempt owns its own distinct AVAILABLE slot row, so locking it must never itself fail");
            let result = insert_confirmed(&mut *tx, patient_id, doctor_id, slot_id).await;
            match result {
                Ok(_) => {
                    tx.commit().await.unwrap();
                    true
                }
                Err(e) => {
                    let code = e.as_database_error().and_then(|d| d.code().map(|c| c.to_string()));
                    // Under genuine tight concurrency (confirmed once real DB
                    // latency dropped from ~2.3s to sub-100ms — see this
                    // file's own connection-timing discovery), a losing
                    // attempt can legitimately surface as either the
                    // exclusion violation itself (23P01) or a deadlock abort
                    // (40P01) from PostgreSQL's deadlock detector catching a
                    // lock-wait cycle between the GiST index and the slot
                    // row before the exclusion check even runs. Both mean
                    // exactly the same thing for this invariant: the
                    // conflicting write never committed.
                    assert!(
                        matches!(code.as_deref(), Some("23P01") | Some("40P01")),
                        "a losing attempt must fail on the exclusion constraint or a deadlock abort, not some unrelated error (got {code:?})"
                    );
                    tx.rollback().await.unwrap();
                    false
                }
            }
        }));
    }
    let mut won = 0;
    let mut lost = 0;
    for h in handles {
        if h.await.unwrap() {
            won += 1;
        } else {
            lost += 1;
        }
    }
    (won, lost)
}

/// Third discovery while writing this file: a genuine N-way concurrent race
/// of independent transactions each *inserting a brand-new appointment* for
/// the same real Doctor's overlapping time is not constructible at all. Two
/// pre-existing protections jointly rule it out: `no_overlapping_slots`
/// means one real Doctor can hold only one non-cancelled slot in any given
/// time window, and `appointment_slot_doctor` (migration
/// `20261026000001_canonical_user_isolation`, `FOREIGN KEY (slot_id,
/// doctor_id) REFERENCES doctor_schedule_slots(id, doctor_id)`) means an
/// appointment's `doctor_id` can never diverge from its own slot's real
/// owner. Together, any two *consistent* candidate appointments for the same
/// Doctor are, by construction, for non-overlapping times — there is no
/// racing INSERT to even attempt. The double-booking test above exercises
/// `appt_doctor_no_overlap` directly via a sequential, already-existing
/// mismatched row (the one case that FK still allows: an INSERT that
/// conflicts via the exclusion index before the FK's own AFTER-row trigger
/// ever runs); that trick does not extend to a genuine N-way concurrent race
/// — the very first (no pre-existing conflict yet) attempt would simply hit
/// the FK, not the exclusion constraint.
///
/// So the actual, reachable concurrency question for "same Doctor double-
/// booking" is one layer down: can independent transactions race to create
/// the conflicting SLOT itself? This test answers that directly against
/// `no_overlapping_slots`, with real independent transactions and a final
/// persisted-row inspection — not against `appt_doctor_no_overlap`, which
/// this file's other tests confirm is correctly configured but, given the
/// schema's own guarantees, unreachable via concurrent INSERT.
#[sqlx::test(migrations = "./migrations")]
async fn test_concurrent_independent_transactions_same_doctor_slot_overlap_single_winner(
    pool: PgPool,
) {
    const ATTEMPTS: usize = 6;
    let start = Utc::now() + Duration::days(43);
    let doctor = new_doctor(&pool).await;

    let mut handles = Vec::with_capacity(ATTEMPTS);
    for _ in 0..ATTEMPTS {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut tx = pool.begin().await.unwrap();
            let result = sqlx::query(
                "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
                 VALUES ($1,$2,$3,$4,'AVAILABLE',clock_timestamp())",
            )
            .bind(Uuid::new_v4())
            .bind(doctor)
            .bind(start)
            .bind(start + Duration::minutes(30))
            .execute(&mut *tx)
            .await;
            match result {
                Ok(_) => {
                    tx.commit().await.unwrap();
                    true
                }
                Err(e) => {
                    let db_err = e.as_database_error().unwrap();
                    let code = db_err.code().map(|c| c.to_string());
                    // Under genuine tight concurrency a losing attempt can
                    // legitimately surface as either the exclusion violation
                    // itself (23P01, constraint `no_overlapping_slots`) or a
                    // deadlock abort (40P01) from PostgreSQL's deadlock
                    // detector catching a lock-wait cycle first — see the
                    // matching discovery note on `concurrent_overlap_race`.
                    // Both mean the same thing here: the conflicting slot
                    // never committed.
                    match code.as_deref() {
                        Some("23P01") => assert_eq!(db_err.constraint(), Some("no_overlapping_slots")),
                        Some("40P01") => {}
                        other => panic!("a losing attempt must fail on no_overlapping_slots or a deadlock abort, not some unrelated error (got {other:?})"),
                    }
                    tx.rollback().await.unwrap();
                    false
                }
            }
        }));
    }
    let mut won = 0;
    let mut lost = 0;
    for h in handles {
        if h.await.unwrap() {
            won += 1;
        } else {
            lost += 1;
        }
    }
    assert_eq!(won, 1, "exactly one of {ATTEMPTS} truly concurrent, independent transactions creating an overlapping slot for the same real Doctor must commit");
    assert_eq!(lost, ATTEMPTS - 1);

    let surviving_slots: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM doctor_schedule_slots WHERE doctor_id=$1 AND status<>'CANCELLED'",
    )
    .bind(doctor)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(surviving_slots, 1, "PostgreSQL itself must be the thing enforcing this, not an assumption about which transactions happened to fail");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_concurrent_independent_transactions_same_patient_overlap_single_winner(pool: PgPool) {
    const ATTEMPTS: usize = 6;
    let start = Utc::now() + Duration::days(44);
    let (patient_id, _) = new_patient(&pool).await;
    let mut rows = Vec::with_capacity(ATTEMPTS);
    let mut slots = Vec::with_capacity(ATTEMPTS);
    for _ in 0..ATTEMPTS {
        // A genuinely distinct Doctor per attempt — `doctor_schedule_slots`'s
        // pre-existing `no_overlapping_slots` constraint (see the discovery
        // note on the double-booking test above) forbids any one real Doctor
        // from owning two overlapping slots, so each attempt here needs its
        // own Doctor to even construct an overlapping slot in the first
        // place. The invariant under test (`appt_patient_no_overlap`) keys on
        // (patient_id, slot_period) regardless of doctor_id, so six different
        // Doctors genuinely exercises "same Patient, same contested time,
        // different Doctors every time".
        let doctor = new_doctor(&pool).await;
        let slot = raw_slot(&pool, doctor, start, start + Duration::minutes(30)).await;
        slots.push(slot);
        rows.push((patient_id, doctor, slot));
    }

    let (won, lost) = concurrent_overlap_race(&pool, rows).await;
    assert_eq!(won, 1, "exactly one of {ATTEMPTS} truly concurrent, independent transactions against the same Patient's overlapping time must commit");
    assert_eq!(lost, ATTEMPTS - 1);

    let active_appointments: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM appointments WHERE patient_id=$1 AND status='CONFIRMED'",
    )
    .bind(patient_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active_appointments, 1);
}

// ============================================================================
// 6/7/8. Rescheduling into a conflicting Patient or Doctor appointment; a
// rejected reschedule must preserve the original booking and its finances.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_reschedule_into_conflicting_patient_appointment_rejected_preserves_original(
    pool: PgPool,
) {
    let (app, pool) = test_app(pool).await;
    let (_patient_id, patient_token) = new_patient(&pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let t0 = Utc::now() + Duration::days(45);
    let d1_slot = raw_slot(&pool, doctor1(), t0, t0 + Duration::minutes(30)).await;
    let d2_slot = raw_slot(
        &pool,
        doctor2(),
        t0 + Duration::hours(2),
        t0 + Duration::hours(2) + Duration::minutes(30),
    )
    .await;
    let appointment1 = book_confirmed(&app, &patient_token, doctor1(), d1_slot).await;
    let appointment2 = book_confirmed(&app, &patient_token, doctor2(), d2_slot).await;

    let original: (Uuid, String) =
        sqlx::query_as("SELECT slot_id, status::text FROM appointments WHERE id=$1")
            .bind(appointment1)
            .fetch_one(&pool)
            .await
            .unwrap();
    let original_transactions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE appointment_id=$1")
            .bind(appointment1)
            .fetch_one(&pool)
            .await
            .unwrap();

    // A new slot for Doctor1 that overlaps appointment2's (Doctor2) time.
    let conflicting_target = raw_slot(
        &pool,
        doctor1(),
        t0 + Duration::hours(2) + Duration::minutes(10),
        t0 + Duration::hours(2) + Duration::minutes(40),
    )
    .await;
    let response = post(
        &app,
        &format!("/api/v1/admin/appointments/{appointment1}/reschedule"),
        Some(&admin),
        &serde_json::json!({"slot_id": conflicting_target, "reason": "Synthetic conflicting reschedule attempt"}),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::CONFLICT,
        "rescheduling into a time that overlaps the Patient's OTHER active appointment must be rejected"
    );

    let after: (Uuid, String) =
        sqlx::query_as("SELECT slot_id, status::text FROM appointments WHERE id=$1")
            .bind(appointment1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        after, original,
        "a rejected reschedule must leave the original appointment completely untouched"
    );
    assert_eq!(
        schedule_slot_status(&pool, conflicting_target).await,
        "AVAILABLE"
    );
    // "BOOKED", not "LOCKED_IN_PAYMENT": appointment1 was confirmed via the
    // real payment webhook (`book_confirmed`), which transitions the slot
    // past the payment-hold state. The rejected reschedule must not move it
    // off that state either way.
    assert_eq!(
        schedule_slot_status(&pool, d1_slot).await,
        "BOOKED".to_string()
    );

    let reschedule_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM appointment_reschedules WHERE appointment_id=$1")
            .bind(appointment1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        reschedule_rows, 0,
        "no reschedule record may be written for a rejected attempt"
    );

    let transactions_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE appointment_id=$1")
            .bind(appointment1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        transactions_after, original_transactions,
        "financial state for the untouched appointment must not change just because a reschedule was attempted and rejected"
    );

    let _ = appointment2;
}

#[sqlx::test(migrations = "./migrations")]
async fn test_reschedule_into_doctor_already_booked_slot_rejected_preserves_original(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (_patient1_id, patient1_token) = new_patient(&pool).await;
    let (_patient2_id, patient2_token) = new_patient(&pool).await;
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");

    let t0 = Utc::now() + Duration::days(46);
    let p1_slot = raw_slot(&pool, doctor1(), t0, t0 + Duration::minutes(30)).await;
    let busy_slot = raw_slot(
        &pool,
        doctor1(),
        t0 + Duration::hours(3),
        t0 + Duration::hours(3) + Duration::minutes(30),
    )
    .await;
    let appointment1 = book_confirmed(&app, &patient1_token, doctor1(), p1_slot).await;
    // A second, unrelated Patient already holds `busy_slot` with the same
    // Doctor, so it is no longer AVAILABLE by the time the reschedule is tried.
    let _appointment2 = book_confirmed(&app, &patient2_token, doctor1(), busy_slot).await;

    let original: Uuid = sqlx::query_scalar("SELECT slot_id FROM appointments WHERE id=$1")
        .bind(appointment1)
        .fetch_one(&pool)
        .await
        .unwrap();

    let response = post(
        &app,
        &format!("/api/v1/admin/appointments/{appointment1}/reschedule"),
        Some(&admin),
        &serde_json::json!({"slot_id": busy_slot, "reason": "Synthetic already-booked target slot"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let after: Uuid = sqlx::query_scalar("SELECT slot_id FROM appointments WHERE id=$1")
        .bind(appointment1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, original);
    // "BOOKED", not "LOCKED_IN_PAYMENT": appointment2 was confirmed via the
    // real payment webhook (`book_confirmed`).
    assert_eq!(
        schedule_slot_status(&pool, busy_slot).await,
        "BOOKED",
        "the already-booked target slot's status must be exactly what appointment2 holds, unaffected by the rejected reschedule"
    );
    let reschedule_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM appointment_reschedules WHERE appointment_id=$1")
            .bind(appointment1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reschedule_rows, 0);
}

// ============================================================================
// 9. Expired payment-hold reclamation.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_expired_payment_hold_reclaimed_by_new_booking(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (abandoner_id, _) = new_patient(&pool).await;
    let (_newcomer_id, newcomer_token) = new_patient(&pool).await;

    let start = Utc::now() + Duration::days(47);
    let slot = raw_slot(&pool, doctor1(), start, start + Duration::minutes(30)).await;

    // A stale, already-due hold: its explicit deadline is in the past, so the
    // sweep/reclaim predicate (`COALESCE(hold_expires_at, ...) <=
    // clock_timestamp()`) must treat it as abandoned.
    let abandoned_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, created_at, updated_at, hold_expires_at) \
         VALUES ($1,$2,$3,$4,$5,'VIDEO','PENDING_PAYMENT',100.00,clock_timestamp()-interval '1 hour',clock_timestamp()-interval '1 hour',clock_timestamp()-interval '30 minutes')",
    )
    .bind(abandoned_id)
    .bind(format!("APT-TEST-{}", abandoned_id.to_string()[0..8].to_uppercase()))
    .bind(abandoner_id)
    .bind(doctor1())
    .bind(slot)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE doctor_schedule_slots SET status='LOCKED_IN_PAYMENT' WHERE id=$1")
        .bind(slot)
        .execute(&pool)
        .await
        .unwrap();

    let response = book_and_confirm_at(&app, &newcomer_token, doctor1(), slot).await;
    let status = response.status();
    let body = json_body(response).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "a new booking must reclaim an abandoned, already-due hold on the slot it targets: {body}"
    );

    let abandoned_status: String =
        sqlx::query_scalar("SELECT status::text FROM appointments WHERE id=$1")
            .bind(abandoned_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(abandoned_status, "EXPIRED");

    let new_appointment_id = uid(body["data"]["appointment_id"].as_str().unwrap());
    let payment_session_id = uid(body["data"]["payment_session_id"].as_str().unwrap());
    confirm_via_webhook(&app, payment_session_id).await;
    let new_status: String =
        sqlx::query_scalar("SELECT status::text FROM appointments WHERE id=$1")
            .bind(new_appointment_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_status, "CONFIRMED");
    assert_eq!(schedule_slot_status(&pool, slot).await, "BOOKED");
}

// ============================================================================
// 10. Concurrent hold-expiry workers (multi-instance-safe sweep).
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_concurrent_hold_expiry_workers_partition_without_double_processing(pool: PgPool) {
    seed::seed_dev_data(&pool).await.expect("seed dev data");
    const DUE_HOLDS: usize = 8;
    const WORKERS: usize = 4;
    let start = Utc::now() + Duration::days(48);
    let mut appointment_ids = Vec::with_capacity(DUE_HOLDS);
    let mut slot_ids = Vec::with_capacity(DUE_HOLDS);
    for i in 0..DUE_HOLDS {
        let (patient_id, _) = new_patient(&pool).await;
        // Non-overlapping times/doctors: this test's subject is the sweep's
        // own SKIP LOCKED partitioning, not the exclusion constraints, so no
        // two fixtures here should ever contend with each other.
        let doctor = if i % 2 == 0 { doctor1() } else { doctor2() };
        let slot_start = start + Duration::hours(i as i64);
        let slot = raw_slot(
            &pool,
            doctor,
            slot_start,
            slot_start + Duration::minutes(30),
        )
        .await;
        let apt_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, created_at, updated_at, hold_expires_at) \
             VALUES ($1,$2,$3,$4,$5,'VIDEO','PENDING_PAYMENT',100.00,clock_timestamp()-interval '1 hour',clock_timestamp()-interval '1 hour',clock_timestamp()-interval '1 minute')",
        )
        .bind(apt_id)
        .bind(format!("APT-TEST-{}", apt_id.to_string()[0..8].to_uppercase()))
        .bind(patient_id)
        .bind(doctor)
        .bind(slot)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE doctor_schedule_slots SET status='LOCKED_IN_PAYMENT' WHERE id=$1")
            .bind(slot)
            .execute(&pool)
            .await
            .unwrap();
        appointment_ids.push(apt_id);
        slot_ids.push(slot);
    }

    // WORKERS concurrent "scheduler instances", each repeatedly draining the
    // due-hold queue with `FOR UPDATE SKIP LOCKED` until it sees nothing left.
    let mut handles = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut expired_by_this_worker = Vec::new();
            loop {
                let mut tx = pool.begin().await.unwrap();
                let batch = appointment_repo::expire_due_holds_batch(&mut tx, 900, 2)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
                if batch.is_empty() {
                    break;
                }
                expired_by_this_worker.extend(batch);
            }
            expired_by_this_worker
        }));
    }
    let mut all_expired = Vec::new();
    for h in handles {
        all_expired.extend(h.await.unwrap());
    }

    assert_eq!(
        all_expired.len(),
        DUE_HOLDS,
        "every due hold must be expired exactly once across all concurrent workers combined"
    );
    let mut unique = all_expired.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        DUE_HOLDS,
        "SKIP LOCKED must partition the work: no hold may be claimed and processed by two workers"
    );

    for apt_id in &appointment_ids {
        let status: String =
            sqlx::query_scalar("SELECT status::text FROM appointments WHERE id=$1")
                .bind(apt_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "EXPIRED");
    }
    for slot_id in &slot_ids {
        assert_eq!(schedule_slot_status(&pool, *slot_id).await, "AVAILABLE");
    }
}

// ============================================================================
// 11. Payment confirmation racing hold expiration.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_payment_confirmation_races_hold_expiration_exactly_one_outcome(pool: PgPool) {
    seed::seed_dev_data(&pool).await.expect("seed dev data");
    const TRIALS: usize = 10;
    for trial in 0..TRIALS {
        let (patient_id, _) = new_patient(&pool).await;
        let start = Utc::now() + Duration::days(49) + Duration::hours(trial as i64);
        let slot = raw_slot(&pool, doctor1(), start, start + Duration::minutes(30)).await;
        let apt_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, created_at, updated_at, hold_expires_at) \
             VALUES ($1,$2,$3,$4,$5,'VIDEO','PENDING_PAYMENT',100.00,clock_timestamp()-interval '1 hour',clock_timestamp()-interval '1 hour',clock_timestamp()-interval '1 minute')",
        )
        .bind(apt_id)
        .bind(format!("APT-TEST-{}", apt_id.to_string()[0..8].to_uppercase()))
        .bind(patient_id)
        .bind(doctor1())
        .bind(slot)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE doctor_schedule_slots SET status='LOCKED_IN_PAYMENT' WHERE id=$1")
            .bind(slot)
            .execute(&pool)
            .await
            .unwrap();

        let confirm_pool = pool.clone();
        let expire_pool = pool.clone();
        let confirm = tokio::spawn(async move {
            let mut tx = confirm_pool.begin().await.unwrap();
            let confirmed = appointment_repo::confirm_payment_if_pending(&mut *tx, apt_id)
                .await
                .unwrap();
            tx.commit().await.unwrap();
            confirmed
        });
        let expire = tokio::spawn(async move {
            let mut tx = expire_pool.begin().await.unwrap();
            let expired = appointment_repo::expire_abandoned_hold_for_slot(&mut tx, slot, 900)
                .await
                .unwrap();
            tx.commit().await.unwrap();
            !expired.is_empty()
        });
        let (confirmed, expired) = tokio::join!(confirm, expire);
        let confirmed = confirmed.unwrap();
        let expired = expired.unwrap();

        assert!(
            confirmed ^ expired,
            "exactly one of {{confirm, expire}} must win this race on trial {trial}, never both and never neither"
        );

        let final_status: String =
            sqlx::query_scalar("SELECT status::text FROM appointments WHERE id=$1")
                .bind(apt_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let slot_status = schedule_slot_status(&pool, slot).await;
        if confirmed {
            assert_eq!(final_status, "CONFIRMED", "trial {trial}");
            assert_eq!(slot_status, "LOCKED_IN_PAYMENT", "trial {trial}: a confirmed booking's slot stays locked for the consultation, it is not re-opened");
        } else {
            assert_eq!(final_status, "EXPIRED", "trial {trial}");
            assert_eq!(slot_status, "AVAILABLE", "trial {trial}");
        }
    }
}

// ============================================================================
// 12. Doctor-slot time change that would create a conflict.
//
// No HTTP endpoint currently edits a slot's start/end time (only its
// AVAILABLE/BLOCKED status) — `doctor_slots_propagate_period` exists purely
// as the migration's own forward-looking defense-in-depth for whenever one
// is added.
//
// Second discovery while writing this file (building on the
// `no_overlapping_slots` one on the double-booking test above): migration
// `20261026000001_canonical_user_isolation` already added
// `appointment_slot_doctor FOREIGN KEY (slot_id, doctor_id) REFERENCES
// doctor_schedule_slots(id, doctor_id) ... DEFERRABLE INITIALLY IMMEDIATE`.
// That FK makes it impossible, by default transaction behavior, for any
// `appointments` row to claim a `doctor_id` other than its own slot's real
// owner. Combined with `no_overlapping_slots`, this means `appt_doctor_no_
// overlap`'s cascade path can only ever be reached when slot_a and slot_b
// share the SAME real owner — but that is exactly the case
// `no_overlapping_slots` itself already rejects on the slot-time UPDATE,
// before `doctor_slots_propagate_period` ever runs (confirmed directly: an
// earlier version of this test tried two different real owners specifically
// to dodge that overlap and instead hit `appointment_slot_doctor` on setup).
// So this test, correctly, demonstrates the constraint that actually fires
// in every reachable case: `no_overlapping_slots` on the slot table itself.
// `appt_doctor_no_overlap`'s own doctor-dimension protection is confirmed
// correct (see the double-booking test, which reaches it directly via
// INSERT) but is unreachable via this slot-edit cascade specifically.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_doctor_slot_time_change_creating_conflict_rejected_by_trigger(pool: PgPool) {
    let (patient_a, _) = new_patient(&pool).await;
    let (patient_b, _) = new_patient(&pool).await;
    let doctor = new_doctor(&pool).await;
    let start = Utc::now() + Duration::days(50);
    let slot_a = raw_slot(&pool, doctor, start, start + Duration::minutes(30)).await;
    let slot_b = raw_slot(
        &pool,
        doctor,
        start + Duration::hours(1),
        start + Duration::hours(1) + Duration::minutes(30),
    )
    .await;
    insert_confirmed(&pool, patient_a, doctor, slot_a)
        .await
        .unwrap();
    insert_confirmed(&pool, patient_b, doctor, slot_b)
        .await
        .unwrap();

    let original_b: (DateTime<Utc>, DateTime<Utc>) =
        sqlx::query_as("SELECT start_time, end_time FROM doctor_schedule_slots WHERE id=$1")
            .bind(slot_b)
            .fetch_one(&pool)
            .await
            .unwrap();

    // Shift slot_b so it would now overlap slot_a's time for the same real
    // Doctor — rejected by `no_overlapping_slots` directly on this UPDATE,
    // before `doctor_slots_propagate_period`'s cascade even runs.
    let new_start = start + Duration::minutes(15);
    let new_end = start + Duration::minutes(45);
    let err = sqlx::query(
        "UPDATE doctor_schedule_slots SET start_time=$2, end_time=$3 WHERE id=$1",
    )
    .bind(slot_b)
    .bind(new_start)
    .bind(new_end)
    .execute(&pool)
    .await
    .expect_err("a Doctor's own slot-table exclusion constraint must reject a time edit that would double-book them, independent of the appointments layer");
    let db_err = err.as_database_error().unwrap();
    assert_eq!(db_err.code().as_deref(), Some("23P01"));
    assert_eq!(db_err.constraint(), Some("no_overlapping_slots"));

    let after_b: (DateTime<Utc>, DateTime<Utc>) =
        sqlx::query_as("SELECT start_time, end_time FROM doctor_schedule_slots WHERE id=$1")
            .bind(slot_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        after_b, original_b,
        "the rejected slot-time edit must not partially apply"
    );
}

// ============================================================================
// 13/14. Migration preflight fail-closed behavior, and rollback/reapply data
// safety, for 20261111000001_schedule_exclusivity itself.
// ============================================================================

const SCHEDULE_EXCLUSIVITY_VERSION: i64 = 20261111000001;

fn migrator_up_to(ceiling: i64) -> sqlx::migrate::Migrator {
    let mut m = sqlx::migrate!("./migrations");
    m.migrations = std::borrow::Cow::Owned(
        m.iter()
            .filter(|mig| mig.version <= ceiling)
            .cloned()
            .collect(),
    );
    m
}

async fn pre_schedule_exclusivity_schema(pool: &PgPool) -> i64 {
    let full = sqlx::migrate!("./migrations");
    let before = full
        .iter()
        .filter(|m| m.version < SCHEDULE_EXCLUSIVITY_VERSION)
        .map(|m| m.version)
        .max()
        .unwrap();
    migrator_up_to(before).run(pool).await.unwrap();
    before
}

// `new_doctor` (defined near `new_patient` above) also works against this
// pre-Stage-A schema — `doctor_profiles` is unchanged by migration
// 20261111000001 — so these two migration-safety tests use it directly
// rather than depending on `seed::seed_dev_data`, which targets the
// *current* schema shape, not whatever partial migration ceiling a given
// test has deliberately stopped at.

/// A bare doctor/patient profile pair plus one `doctor_schedule_slots` row,
/// built directly against the pre-migration schema (no `slot_period` column
/// exists yet at this point).
async fn legacy_slot(
    pool: &PgPool,
    doctor_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO doctor_schedule_slots (id, doctor_id, start_time, end_time, status, created_at) \
         VALUES ($1,$2,$3,$4,'AVAILABLE',clock_timestamp())",
    )
    .bind(id)
    .bind(doctor_id)
    .bind(start)
    .bind(end)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn legacy_appointment(
    pool: &PgPool,
    patient_id: Uuid,
    doctor_id: Uuid,
    slot_id: Uuid,
    status: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, modality, status, consultation_fee, created_at, updated_at) \
         VALUES ($1,$2,$3,$4,$5,'VIDEO',$6::appointment_status_enum,100.00,clock_timestamp(),clock_timestamp())",
    )
    .bind(id)
    .bind(format!("APT-LEGACY-{}", id.to_string()[0..8].to_uppercase()))
    .bind(patient_id)
    .bind(doctor_id)
    .bind(slot_id)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = false)]
async fn test_schedule_exclusivity_migration_preflight_blocks_on_existing_conflicts(pool: PgPool) {
    pre_schedule_exclusivity_schema(&pool).await;
    let (patient_a, _) = new_patient(&pool).await;
    let doctor_a = new_doctor(&pool).await;
    let doctor_b = new_doctor(&pool).await;
    let start = Utc::now() + Duration::days(60);
    // Two ACTIVE, overlapping appointments for the same Patient, both already
    // on disk before the migration ever runs — the pre-migration schema has
    // no exclusion constraint to have stopped this from happening.
    let slot_a = legacy_slot(&pool, doctor_a, start, start + Duration::minutes(30)).await;
    let slot_b = legacy_slot(
        &pool,
        doctor_b,
        start + Duration::minutes(10),
        start + Duration::minutes(40),
    )
    .await;
    legacy_appointment(&pool, patient_a, doctor_a, slot_a, "CONFIRMED").await;
    legacy_appointment(&pool, patient_a, doctor_b, slot_b, "CONFIRMED").await;

    let result = migrator_up_to(SCHEDULE_EXCLUSIVITY_VERSION)
        .run(&pool)
        .await;
    assert!(
        result.is_err(),
        "the migration's own preflight DO block must refuse to run over pre-existing overlapping active appointments"
    );

    let has_slot_period: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_name='appointments' AND column_name='slot_period')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !has_slot_period,
        "a failed preflight must leave the schema completely unchanged (the migration runs inside a transaction) — no partial column add"
    );

    // No appointment or slot was auto-cancelled or deleted to force the
    // migration through — the preflight's own documented guarantee.
    let still_confirmed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM appointments WHERE patient_id=$1 AND status='CONFIRMED'",
    )
    .bind(patient_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(still_confirmed, 2);
}

#[sqlx::test(migrations = false)]
async fn test_schedule_exclusivity_migration_rollback_and_reapply_preserves_data_and_enforcement(
    pool: PgPool,
) {
    let before_version = pre_schedule_exclusivity_schema(&pool).await;
    let (patient_a, _) = new_patient(&pool).await;
    let (patient_b, _) = new_patient(&pool).await;
    let doctor_a_id = new_doctor(&pool).await;
    let doctor_b_id = new_doctor(&pool).await;
    // Truncated to microseconds: `TIMESTAMPTZ` only stores that much
    // precision, so comparing a value read back from Postgres against an
    // untruncated `Utc::now()` (which can carry sub-microsecond digits on
    // this platform) would spuriously fail below on the trailing digits
    // alone, not on any real data-preservation defect.
    let start = (Utc::now() + Duration::days(61)).trunc_subsecs(6);
    let slot_a = legacy_slot(&pool, doctor_a_id, start, start + Duration::minutes(30)).await;
    let slot_b = legacy_slot(
        &pool,
        doctor_b_id,
        start + Duration::hours(2),
        start + Duration::hours(2) + Duration::minutes(30),
    )
    .await;
    let apt_a = legacy_appointment(&pool, patient_a, doctor_a_id, slot_a, "CONFIRMED").await;
    let apt_b = legacy_appointment(&pool, patient_b, doctor_b_id, slot_b, "COMPLETED").await;

    let batch = migrator_up_to(SCHEDULE_EXCLUSIVITY_VERSION);
    batch.run(&pool).await.unwrap();

    // Forward: columns backfilled correctly from the slots they denormalise.
    let (period_a, doctor_a): (sqlx::postgres::types::PgRange<DateTime<Utc>>, Uuid) =
        sqlx::query_as("SELECT slot_period, doctor_id FROM appointments WHERE id=$1")
            .bind(apt_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(doctor_a, doctor_a_id);
    match (period_a.start, period_a.end) {
        (std::ops::Bound::Included(s), std::ops::Bound::Excluded(e)) => {
            assert_eq!(s, start);
            assert_eq!(e, start + Duration::minutes(30));
        }
        other => panic!("expected a half-open [start,end) range, got {other:?}"),
    }

    // A genuinely new overlapping booking is now rejected by the freshly
    // applied constraint. `conflicting_slot` belongs to a THIRD, different
    // real Doctor (reusing doctor_a_id here would hit the pre-existing
    // `no_overlapping_slots` constraint at slot-creation time, before this
    // migration's own `appt_patient_no_overlap` ever got a chance to run —
    // see the discovery note atop this file) — patient_a already holds
    // apt_a, so booking them again with a different Doctor at an overlapping
    // time is exactly `appt_patient_no_overlap`'s job.
    let doctor_c_id = new_doctor(&pool).await;
    let conflicting_slot = legacy_slot(
        &pool,
        doctor_c_id,
        start + Duration::minutes(10),
        start + Duration::minutes(40),
    )
    .await;
    insert_confirmed(&pool, patient_a, doctor_c_id, conflicting_slot)
        .await
        .expect_err("the constraint must be live immediately after migrating forward");

    // Undo: only this migration's own objects are removed; appointment rows
    // (including the unrelated COMPLETED one) are untouched.
    batch.undo(&pool, before_version).await.unwrap();
    let has_slot_period: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_name='appointments' AND column_name='slot_period')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!has_slot_period);
    let rows_after_undo: Vec<(Uuid, String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, status::text, patient_id, doctor_id FROM appointments WHERE id IN ($1,$2) ORDER BY id",
    )
    .bind(apt_a)
    .bind(apt_b)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows_after_undo.len(),
        2,
        "down-migration must not touch/delete any appointment row"
    );
    let mut expected = vec![
        (apt_a, "CONFIRMED".to_string(), patient_a, doctor_a_id),
        (apt_b, "COMPLETED".to_string(), patient_b, doctor_b_id),
    ];
    expected.sort_by_key(|r| r.0);
    assert_eq!(rows_after_undo, expected);

    // Redo: real enforcement comes back, not just the columns.
    batch.run(&pool).await.unwrap();
    insert_confirmed(&pool, patient_a, doctor_c_id, conflicting_slot)
        .await
        .expect_err("re-applying the migration must re-enforce the exclusion constraint, proving the round trip is genuinely repeatable");
}

// ============================================================================
// 15. RLS: the new columns stay scoped by the existing Patient-ownership
// policy — adding slot_period/hold_expires_at must not widen what a Patient
// can see of someone else's appointment.
// ============================================================================
#[sqlx::test(migrations = "./migrations")]
async fn test_schedule_exclusivity_columns_respect_existing_patient_rls(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let (_owner_id, owner_token) = new_patient(&pool).await;
    let (other_patient_id, _other_token) = new_patient(&pool).await;
    let other_user: Uuid = sqlx::query_scalar("SELECT user_id FROM patient_profiles WHERE id=$1")
        .bind(other_patient_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let start = Utc::now() + Duration::days(51);
    let slot = raw_slot(&pool, doctor1(), start, start + Duration::minutes(30)).await;
    let appointment_id = book_confirmed(&app, &owner_token, doctor1(), slot).await;

    let other_ctx = isolation_ctx(UserId::from(other_user), UserRole::Patient);
    let mut tx = state.begin_authed(&other_ctx).await.unwrap();
    #[allow(clippy::type_complexity)]
    let visible: Option<(
        sqlx::postgres::types::PgRange<DateTime<Utc>>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as("SELECT slot_period, hold_expires_at FROM appointments WHERE id=$1")
        .bind(appointment_id)
        .fetch_optional(&mut *tx)
        .await
        .unwrap();
    assert!(
        visible.is_none(),
        "a different Patient must not be able to read even the new slot_period/hold_expires_at columns of someone else's appointment — row-level security covers the whole row, not just the pre-existing columns"
    );
}
