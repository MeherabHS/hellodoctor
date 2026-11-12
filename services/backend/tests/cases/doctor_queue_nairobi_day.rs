//! Regression tests for the Africa/Nairobi calendar-day boundary of the doctor
//! appointment views (`GET /doctor/appointments`), evaluated at FIXED instants so
//! they are deterministic whatever the wall clock is when the suite runs.
//!
//! Contract under test (docs/DOCTOR_QUEUE_AND_HISTORY_VERIFICATION.md):
//! * `view=upcoming` — IN_CONSULTATION work, then *today's* (Nairobi calendar day)
//!   remaining actionable consultations. Tomorrow's are intentionally absent: the
//!   Doctor Web Queue is labelled "Current and remaining today".
//! * `view=schedule` — CONFIRMED/WAITING on a later Nairobi calendar day.
//! * `view=history`  — terminal/elapsed encounters.
//! * `view=all`      — everything (the master Schedule data source).
//!
//! These exist because two older tests failed in CI at 23:06–23:23 EAT: their
//! seeded slot (`now + 1h`) had crossed into the next Nairobi day, so it was —
//! correctly — absent from the default `upcoming` view. That was a test defect,
//! not a product defect; production query semantics are unchanged.
use super::*;
use chrono::{DateTime, TimeZone, Utc};
use sqlx::Row;

// Fixed date far from any seeded slot (which sit at real-now + hours), so the
// slot-overlap exclusion constraint can never collide with the fixtures.
// Nairobi is UTC+3 with no DST: 2026-03-10 23:40:00 EAT == 20:40:00Z.
fn utc(h: u32, m: u32, s: u32, day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, day, h, m, s).unwrap()
}

struct Fixture {
    /// 23:40:00–23:59:59 EAT on the 10th — just BEFORE Nairobi midnight.
    before_midnight: Uuid,
    /// 00:00:00–00:10:00 EAT on the 11th — starts exactly AT Nairobi midnight.
    at_midnight: Uuid,
    /// 00:30–00:40 EAT on the 11th — the FOLLOWING calendar day.
    next_day: Uuid,
}

async fn fixture(pool: &PgPool, app: &axum::Router) -> Fixture {
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let (base, _) = book_and_confirm(app, &patient).await;
    let template = appointment_repo::find_by_id(pool, base)
        .await
        .unwrap()
        .unwrap();
    // Take the template booking out of the picture; only the three fixtures matter.
    sqlx::query("UPDATE appointments SET status='CANCELLED' WHERE id=$1")
        .bind(base)
        .execute(pool)
        .await
        .unwrap();
    let make = |start: DateTime<Utc>, end: DateTime<Utc>| {
        let template = template.clone();
        let pool = pool.clone();
        async move {
            let slot: Uuid = sqlx::query_scalar(
                "INSERT INTO doctor_schedule_slots(doctor_id,start_time,end_time,status) \
                 VALUES($1,$2,$3,'BOOKED') RETURNING id",
            )
            .bind(uid(DOCTOR1_PROFILE_ID))
            .bind(start)
            .bind(end)
            .fetch_one(&pool)
            .await
            .unwrap();
            let mut a = template;
            a.id = Uuid::new_v4();
            a.appointment_number = format!("NQ-{}", a.id);
            a.status = AppointmentStatus::Confirmed;
            a.modality = Modality::Video;
            a.slot_id = slot;
            appointment_repo::insert(&pool, &a).await.unwrap();
            a.id
        }
    };
    Fixture {
        before_midnight: make(utc(20, 40, 0, 10), utc(20, 59, 59, 10)).await,
        at_midnight: make(utc(21, 0, 0, 10), utc(21, 10, 0, 10)).await,
        next_day: make(utc(21, 30, 0, 10), utc(21, 40, 0, 10)).await,
    }
}

/// The ids of OUR fixtures returned by the real query for `view` at `as_of`.
async fn view(pool: &PgPool, f: &Fixture, view: &str, as_of: DateTime<Utc>) -> Vec<Uuid> {
    let ours = [f.before_midnight, f.at_midnight, f.next_day];
    let rows = doctor_repo::list_appointments_as_of(
        pool,
        DoctorProfileId::from(uid(DOCTOR1_PROFILE_ID)),
        hellodoctor_backend::domain::ids::UserId::from(uid(DOCTOR1_USER_ID)),
        None,
        view.into(),
        100,
        0,
        as_of,
    )
    .await
    .unwrap();
    let mut ids: Vec<Uuid> = rows
        .iter()
        .map(|r| r.get::<Uuid, _>("id"))
        .filter(|id| ours.contains(id))
        .collect();
    ids.sort();
    ids
}

fn sorted(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort();
    ids
}

#[sqlx::test(migrations = "./migrations")]
async fn test_upcoming_excludes_next_nairobi_day_but_schedule_and_all_include_it(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let f = fixture(&pool, &app).await;

    // 23:30 EAT on the 10th: the doctor is looking at the dashboard.
    let as_of = utc(20, 30, 0, 10);
    // The slot just before midnight is today's remaining work…
    assert_eq!(
        view(&pool, &f, "upcoming", as_of).await,
        vec![f.before_midnight]
    );
    // …a booking for 00:00 and 00:30 on the following calendar day is NOT in
    // "Current and remaining today" (documented), but it is not lost:
    assert_eq!(
        view(&pool, &f, "schedule", as_of).await,
        sorted(vec![f.at_midnight, f.next_day])
    );
    assert_eq!(
        view(&pool, &f, "all", as_of).await,
        sorted(vec![f.before_midnight, f.at_midnight, f.next_day])
    );
    assert!(view(&pool, &f, "history", as_of).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn test_a_slot_one_hour_ahead_at_23_06_eat_is_tomorrows_and_leaves_the_queue(pool: PgPool) {
    // The exact CI failure scenario: suite running at 23:06 EAT, seeded slot at now+1h.
    let (app, pool) = test_app(pool).await;
    let f = fixture(&pool, &app).await;
    let as_of = utc(20, 6, 0, 10); // 23:06 EAT; +1h = 00:06 EAT on the 11th
    let upcoming = view(&pool, &f, "upcoming", as_of).await;
    assert!(!upcoming.contains(&f.at_midnight) && !upcoming.contains(&f.next_day));
    let schedule = view(&pool, &f, "schedule", as_of).await;
    assert!(schedule.contains(&f.at_midnight) && schedule.contains(&f.next_day));
}

#[sqlx::test(migrations = "./migrations")]
async fn test_midnight_rolls_the_day_over_exactly_at_00_00_00_eat(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let f = fixture(&pool, &app).await;

    // One second before midnight (23:59:58.5 EAT): still the 10th.
    let before = utc(20, 59, 58, 10) + chrono::Duration::milliseconds(500);
    assert_eq!(
        view(&pool, &f, "upcoming", before).await,
        vec![f.before_midnight]
    );
    assert_eq!(
        view(&pool, &f, "schedule", before).await,
        sorted(vec![f.at_midnight, f.next_day])
    );

    // Exactly 00:00:00 EAT on the 11th: the day has rolled. The 23:40 slot is
    // yesterday's (and over); the 00:00 and 00:30 slots are now today's work.
    let at = utc(21, 0, 0, 10);
    assert_eq!(
        view(&pool, &f, "upcoming", at).await,
        sorted(vec![f.at_midnight, f.next_day])
    );
    assert!(view(&pool, &f, "schedule", at).await.is_empty());
    assert_eq!(
        view(&pool, &f, "history", at).await,
        vec![f.before_midnight]
    );

    // 00:20 EAT: the 00:00 slot has ended and drops to history; 00:30 remains.
    let later = utc(21, 20, 0, 10);
    assert_eq!(view(&pool, &f, "upcoming", later).await, vec![f.next_day]);
    assert_eq!(
        view(&pool, &f, "history", later).await,
        sorted(vec![f.before_midnight, f.at_midnight])
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_the_http_default_view_is_todays_queue_and_all_is_complete(pool: PgPool) {
    // Real clock, real HTTP: a booking exactly one day ahead is always "tomorrow"
    // (robust at any wall-clock time) — absent from the default queue, present in
    // `schedule` and `all`.
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (apt, _) = book_and_confirm(&app, &patient).await;
    let slot: Uuid = sqlx::query_scalar("SELECT slot_id FROM appointments WHERE id = $1")
        .bind(apt)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE doctor_schedule_slots SET start_time = now() + interval '1 day', \
         end_time = now() + interval '1 day 10 minutes' WHERE id = $1",
    )
    .bind(slot)
    .execute(&pool)
    .await
    .unwrap();
    let ids = |body: &serde_json::Value| -> Vec<String> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect()
    };
    let mine = apt.to_string();
    for (uri, expected) in [
        ("/api/v1/doctor/appointments?limit=100", false),
        ("/api/v1/doctor/appointments?view=upcoming&limit=100", false),
        ("/api/v1/doctor/appointments?view=schedule&limit=100", true),
        ("/api/v1/doctor/appointments?view=all&limit=100", true),
    ] {
        let body = json_body(get(&app, uri, Some(&doctor)).await).await;
        assert_eq!(ids(&body).contains(&mine), expected, "{uri}");
    }
}
