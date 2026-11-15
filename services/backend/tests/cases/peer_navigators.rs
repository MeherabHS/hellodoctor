//! Peer Navigator, Phase 1 (docs/peer-navigator/PEER_NAVIGATOR_PLAN.md;
//! migrations `20261222000001_peer_role`, `20261222000002_peer_navigators`):
//! sign-up and sign-in, Admin approval, 10-minute slots, the approved-peer
//! list patients see, and role isolation.

use super::*;

const PEER_EMAIL: &str = "neema.peer@example.test";
const PEER_PASSWORD: &str = "Peer-Navigator-2026!";

async fn register_peer(
    app: &axum::Router,
    docs: &Value,
    email: &str,
    phone: &str,
) -> axum::http::Response<Body> {
    post(
        app,
        "/api/v1/auth/peer/register",
        None,
        &serde_json::json!({
            "full_name": "Neema Kamau",
            "email": email,
            "phone_number": phone,
            "password": PEER_PASSWORD,
            "county": "Nairobi",
            "organization": "Kenyatta Youth Centre",
            "accepted_documents": docs
        }),
    )
    .await
}

async fn peer_login(app: &axum::Router) -> String {
    let started = post(
        app,
        "/api/v1/auth/peer/login",
        None,
        &serde_json::json!({"email": PEER_EMAIL, "password": PEER_PASSWORD}),
    )
    .await;
    assert_eq!(started.status(), StatusCode::OK);
    let started = json_body(started).await;
    let verified = post(
        app,
        "/api/v1/auth/peer/verify-otp",
        None,
        &serde_json::json!({
            "session_token": started["data"]["session_token"],
            "otp_code": started["data"]["debug_otp_code"]
        }),
    )
    .await;
    assert_eq!(verified.status(), StatusCode::OK);
    let body = json_body(verified).await;
    assert_eq!(body["data"]["user"]["role"], "PEER");
    body["data"]["access_token"].as_str().unwrap().to_string()
}

/// The current Peer documents, as sign-up must accept them.
async fn peer_docs(pool: &PgPool) -> Value {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT document_type::text, version FROM legal_documents WHERE is_current          AND document_type::text IN ('PEER_TERMS','PEER_PRIVACY_NOTICE','PEER_CONDUCT_SAFEGUARDING')",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3, "the three Peer documents are seeded");
    Value::Array(
        rows.into_iter()
            .map(|(t, v)| serde_json::json!({"type": t, "version": v}))
            .collect(),
    )
}

fn in_minutes(minutes: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::minutes(minutes)).to_rfc3339()
}

#[sqlx::test(migrations = "./migrations")]
async fn peer_signs_up_waits_for_approval_then_opens_slots_patients_can_see(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    // Sign-up is refused until all three Peer documents are accepted.
    let docs = peer_docs(&pool).await;
    let partial = Value::Array(docs.as_array().unwrap()[..2].to_vec());
    assert_eq!(
        register_peer(&app, &partial, PEER_EMAIL, "+254711220001")
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let created = register_peer(&app, &peer_docs(&pool).await, PEER_EMAIL, "+254711220001").await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = json_body(created).await;
    assert_eq!(created["data"]["approval_status"], "PENDING");
    let peer_id = created["data"]["peer_id"].as_str().unwrap().to_string();
    let accepted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM legal_acceptances la JOIN peer_profiles p ON p.user_id = la.user_id          WHERE p.id = $1 AND la.document_type::text LIKE 'PEER_%'",
    )
    .bind(Uuid::parse_str(&peer_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(accepted, 3, "each Peer document acceptance is recorded");
    assert_eq!(
        register_peer(&app, &peer_docs(&pool).await, PEER_EMAIL, "+254711220002")
            .await
            .status(),
        StatusCode::CONFLICT,
        "the same email cannot register twice"
    );

    let peer = peer_login(&app).await;
    let profile = json_body(get(&app, "/api/v1/peer/profile", Some(&peer)).await).await;
    assert_eq!(profile["data"]["approval_status"], "PENDING");
    assert_eq!(profile["data"]["full_name"], "Neema Kamau");

    // Pending: no slots, and patients do not see the peer.
    let slot = serde_json::json!({"start_time": in_minutes(120)});
    assert_eq!(
        post(&app, "/api/v1/peer/slots", Some(&peer), &slot)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let listed = json_body(get(&app, "/api/v1/peers", Some(&patient)).await).await;
    assert!(listed["data"].as_array().unwrap().is_empty());

    // Admin approves.
    let admin = token_for_real_role(&pool, "ADMIN").await;
    let pending =
        json_body(get(&app, "/api/v1/admin/peers?status=PENDING", Some(&admin)).await).await;
    assert_eq!(pending["data"][0]["id"], peer_id.as_str());
    assert_eq!(pending["data"][0]["email"], PEER_EMAIL);
    let approved = post(
        &app,
        &format!("/api/v1/admin/peers/{peer_id}/approve"),
        Some(&admin),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(approved.status(), StatusCode::OK);

    // Slots are 10 minutes and cannot overlap or start in the past.
    let made = post(&app, "/api/v1/peer/slots", Some(&peer), &slot).await;
    assert_eq!(made.status(), StatusCode::CREATED);
    let made = json_body(made).await;
    let slot_id = made["data"]["id"].as_str().unwrap().to_string();
    let start =
        chrono::DateTime::parse_from_rfc3339(made["data"]["start_time"].as_str().unwrap()).unwrap();
    let end =
        chrono::DateTime::parse_from_rfc3339(made["data"]["end_time"].as_str().unwrap()).unwrap();
    assert_eq!((end - start).num_minutes(), 10);
    assert_eq!(
        post(
            &app,
            "/api/v1/peer/slots",
            Some(&peer),
            &serde_json::json!({"start_time": in_minutes(125)})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/peer/slots",
            Some(&peer),
            &serde_json::json!({"start_time": in_minutes(-5)})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let own = json_body(get(&app, "/api/v1/peer/slots", Some(&peer)).await).await;
    assert_eq!(own["data"].as_array().unwrap().len(), 1);

    // Patients now see the peer (display fields only) and the open slot.
    let listed = json_body(get(&app, "/api/v1/peers", Some(&patient)).await).await;
    let entry = &listed["data"][0];
    assert_eq!(entry["id"], peer_id.as_str());
    assert!(entry["next_available"].is_string());
    assert!(entry.get("email").is_none() && entry.get("phone_number").is_none());
    let open = json_body(
        get(
            &app,
            &format!("/api/v1/peers/{peer_id}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert_eq!(open["data"][0]["id"], slot_id.as_str());

    // Removing the open slot hides it from patients.
    assert_eq!(
        delete(&app, &format!("/api/v1/peer/slots/{slot_id}"), Some(&peer))
            .await
            .status(),
        StatusCode::OK
    );
    let open = json_body(
        get(
            &app,
            &format!("/api/v1/peers/{peer_id}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(open["data"].as_array().unwrap().is_empty());

    // Suspending needs a note and removes the peer from the patient list.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/peers/{peer_id}/suspend"),
            Some(&admin),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/admin/peers/{peer_id}/suspend"),
            Some(&admin),
            &serde_json::json!({"note": "Paused pending refresher training."})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let listed = json_body(get(&app, "/api/v1/peers", Some(&patient)).await).await;
    assert!(listed["data"].as_array().unwrap().is_empty());
    let audit: Vec<(String, String)> = sqlx::query_as(
        "SELECT action, result FROM audit_events WHERE resource_type='PEER_NAVIGATOR' AND resource_id=$1 \
         ORDER BY created_at",
    )
    .bind(Uuid::parse_str(&peer_id).unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    // Sign-up, approve and suspend, plus the refused suspend without a note
    // (recorded as a failed Admin action).
    let audit: Vec<(&str, &str)> = audit
        .iter()
        .map(|(a, r)| (a.as_str(), r.as_str()))
        .collect();
    assert_eq!(
        audit,
        [
            ("PEER_NAVIGATOR_CREATED", "SUCCESS"),
            ("PEER_NAVIGATOR_APPROVED", "SUCCESS"),
            ("ADMIN_ACTION", "FAILED"),
            ("PEER_NAVIGATOR_SUSPENDED", "SUCCESS"),
        ]
    );
    // The sign-up row tells Admins a new application is waiting, without PII.
    let created: serde_json::Value = sqlx::query_scalar(
        "SELECT after_data FROM audit_events WHERE resource_type='PEER_NAVIGATOR'          AND resource_id=$1 AND action='PEER_NAVIGATOR_CREATED'",
    )
    .bind(Uuid::parse_str(&peer_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(created["approval_status"], "PENDING");
    assert!(created.get("full_name").is_none() && created.get("email").is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn peer_endpoints_are_isolated_by_role(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    assert_eq!(
        register_peer(&app, &peer_docs(&pool).await, PEER_EMAIL, "+254711220003")
            .await
            .status(),
        StatusCode::CREATED
    );
    let peer = peer_login(&app).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let admin = token_for_real_role(&pool, "ADMIN").await;

    for token in [&patient, &doctor, &admin] {
        assert_eq!(
            get(&app, "/api/v1/peer/profile", Some(token))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    for token in [&peer, &doctor, &patient] {
        assert_eq!(
            get(&app, "/api/v1/admin/peers", Some(token)).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    // A peer is not a patient or a doctor.
    assert_eq!(
        get(&app, "/api/v1/peers", Some(&peer)).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&app, "/api/v1/doctor/slots", Some(&peer))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    // A doctor's password cannot sign in as a peer.
    let wrong = post(
        &app,
        "/api/v1/auth/peer/login",
        None,
        &serde_json::json!({"email": SEED_DOCTOR1_EMAIL, "password": SEED_DOCTOR1_PASSWORD}),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    // Approval columns are Admin-only even if a peer writes the row directly.
    let peer_id: Uuid = sqlx::query_scalar("SELECT id FROM peer_profiles LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.current_role','PEER',true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let forged = sqlx::query("UPDATE peer_profiles SET approval_status='APPROVED' WHERE id=$1")
        .bind(peer_id)
        .execute(&mut *tx)
        .await;
    assert!(forged.is_err(), "a peer cannot approve themself");
}

/// An approved peer with `n` open slots; returns (peer token, peer id, slot ids).
async fn approved_peer_with_slots(
    app: &axum::Router,
    pool: &PgPool,
    email: &str,
    phone: &str,
    n: i64,
) -> (String, String, Vec<String>) {
    let created = post(
        app,
        "/api/v1/auth/peer/register",
        None,
        &serde_json::json!({
            "full_name": "Neema Kamau",
            "email": email,
            "phone_number": phone,
            "password": PEER_PASSWORD,
            "county": "Nairobi",
            "accepted_documents": peer_docs(pool).await
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let peer_id = json_body(created).await["data"]["peer_id"]
        .as_str()
        .unwrap()
        .to_string();
    let admin = token_for_real_role(pool, "ADMIN").await;
    let approved = post(
        app,
        &format!("/api/v1/admin/peers/{peer_id}/approve"),
        Some(&admin),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(approved.status(), StatusCode::OK);
    let started = json_body(
        post(
            app,
            "/api/v1/auth/peer/login",
            None,
            &serde_json::json!({"email": email, "password": PEER_PASSWORD}),
        )
        .await,
    )
    .await;
    let token = json_body(
        post(
            app,
            "/api/v1/auth/peer/verify-otp",
            None,
            &serde_json::json!({
                "session_token": started["data"]["session_token"],
                "otp_code": started["data"]["debug_otp_code"]
            }),
        )
        .await,
    )
    .await["data"]["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let mut slots = Vec::new();
    for i in 0..n {
        let made = post(
            app,
            "/api/v1/peer/slots",
            Some(&token),
            &serde_json::json!({"start_time": in_minutes(120 + i * 15)}),
        )
        .await;
        assert_eq!(made.status(), StatusCode::CREATED);
        slots.push(
            json_body(made).await["data"]["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    (token, peer_id, slots)
}

async fn request_slot(app: &axum::Router, patient: &str, slot: &str) -> axum::http::Response<Body> {
    post(
        app,
        "/api/v1/peer-requests",
        Some(patient),
        &serde_json::json!({
            "slot_id": slot,
            "support_type": "CHAT",
            "topic": "Staying on track with my PrEP"
        }),
    )
    .await
}

#[sqlx::test(migrations = "./migrations")]
async fn patient_requests_peer_accepts_declines_and_patient_cancels(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (peer, peer_id, slots) =
        approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220011", 3).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    // Request: PENDING, slot held, peer notified without the topic.
    let made = request_slot(&app, &patient, &slots[0]).await;
    assert_eq!(made.status(), StatusCode::CREATED);
    let made = json_body(made).await;
    assert_eq!(made["data"]["status"], "PENDING");
    assert_eq!(made["data"]["peer_name"], "Neema Kamau");
    let first = made["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        request_slot(&app, &patient, &slots[0]).await.status(),
        StatusCode::CONFLICT,
        "a held slot cannot be requested again"
    );
    let open = json_body(
        get(
            &app,
            &format!("/api/v1/peers/{peer_id}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert_eq!(open["data"].as_array().unwrap().len(), 2);

    // The peer sees a display name and age band, never the patient's identity.
    let inbox =
        json_body(get(&app, "/api/v1/peer/requests?status=PENDING", Some(&peer)).await).await;
    let item = &inbox["data"][0];
    assert_eq!(item["id"], first.as_str());
    assert_eq!(item["patient_display_name"], "Patient U.");
    assert!(item["patient_age_range"].is_string());
    assert_eq!(item["topic"], "Staying on track with my PrEP");
    assert!(item.get("patient_id").is_none() && item.get("patient_user_id").is_none());
    let note: (String, String) = sqlx::query_as(
        "SELECT title, body FROM notifications WHERE type='PEER_REQUEST_NEW' AND related_entity_id=$1",
    )
    .bind(Uuid::parse_str(&first).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !note.0.contains("PrEP") && !note.1.contains("PrEP"),
        "no topic in notifications"
    );

    // Accept: slot booked, patient told.
    let accepted = post(
        &app,
        &format!("/api/v1/peer/requests/{first}/accept"),
        Some(&peer),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(json_body(accepted).await["data"]["status"], "ACCEPTED");
    let status: String = sqlx::query_scalar("SELECT status FROM peer_slots WHERE id=$1")
        .bind(Uuid::parse_str(&slots[0]).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "BOOKED");
    let told: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notifications WHERE type='PEER_REQUEST_ACCEPTED' AND user_id=$1",
    )
    .bind(uid(PATIENT_USER_ID))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(told, 1);
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer-requests/{first}/cancel"),
            Some(&patient),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT,
        "an accepted request is not cancelled from here"
    );

    // Decline with a note frees the slot.
    let second = json_body(request_slot(&app, &patient, &slots[1]).await).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let declined = post(
        &app,
        &format!("/api/v1/peer/requests/{second}/decline"),
        Some(&peer),
        &serde_json::json!({"note": "I'm away at that time, please pick another."}),
    )
    .await;
    assert_eq!(declined.status(), StatusCode::OK);
    let mine = json_body(get(&app, "/api/v1/peer-requests", Some(&patient)).await).await;
    let declined_row = mine["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == second.as_str())
        .unwrap()
        .clone();
    assert_eq!(declined_row["status"], "DECLINED");
    assert_eq!(
        declined_row["decline_note"],
        "I'm away at that time, please pick another."
    );
    let open = json_body(
        get(
            &app,
            &format!("/api/v1/peers/{peer_id}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(open["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == slots[1].as_str()));

    // The patient cancels a waiting request; it cannot then be accepted.
    let third = json_body(request_slot(&app, &patient, &slots[2]).await).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let cancelled = post(
        &app,
        &format!("/api/v1/peer-requests/{third}/cancel"),
        Some(&patient),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(cancelled.status(), StatusCode::OK);
    assert_eq!(json_body(cancelled).await["data"]["status"], "CANCELLED");
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer/requests/{third}/accept"),
            Some(&peer),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let open = json_body(
        get(
            &app,
            &format!("/api/v1/peers/{peer_id}/slots"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(open["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == slots[2].as_str()));
}

#[sqlx::test(migrations = "./migrations")]
async fn peer_requests_are_limited_and_isolated(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (_peer, _peer_id, slots) =
        approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220021", 4).await;
    let (other_peer, _, _) =
        approved_peer_with_slots(&app, &pool, "other.peer@example.test", "+254711220022", 0).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");

    let mut ids = Vec::new();
    for slot in &slots[..3] {
        let made = request_slot(&app, &patient, slot).await;
        assert_eq!(made.status(), StatusCode::CREATED);
        ids.push(
            json_body(made).await["data"]["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    assert_eq!(
        request_slot(&app, &patient, &slots[3]).await.status(),
        StatusCode::CONFLICT,
        "at most three requests may wait at once"
    );

    // Another peer cannot answer it, and a patient cannot accept it.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer/requests/{}/accept", ids[0]),
            Some(&other_peer),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer/requests/{}/accept", ids[0]),
            Some(&patient),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let other_inbox = json_body(get(&app, "/api/v1/peer/requests", Some(&other_peer)).await).await;
    assert!(other_inbox["data"].as_array().unwrap().is_empty());

    // The database refuses a patient forging acceptance.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.current_role','PATIENT',true), set_config('app.current_user_id',$1,true)")
        .bind(PATIENT_USER_ID)
        .execute(&mut *tx)
        .await
        .unwrap();
    let forged = sqlx::query("UPDATE peer_requests SET status='ACCEPTED' WHERE id=$1")
        .bind(Uuid::parse_str(&ids[0]).unwrap())
        .execute(&mut *tx)
        .await;
    assert!(forged.is_err(), "a patient cannot accept their own request");
}

#[test]
fn peer_sees_only_a_display_name_and_age_band() {
    use hellodoctor_backend::api::v1::peer_handlers::{peer_age_range, peer_display_name};
    assert_eq!(peer_display_name("Amina Wanjiru Otieno"), "Amina O.");
    assert_eq!(peer_display_name("amina"), "amina");
    let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 14).unwrap();
    let born = |y, m, d| chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
    assert_eq!(
        peer_age_range(born(2008, 11, 15), today).as_deref(),
        Some("15-19")
    );
    assert_eq!(
        peer_age_range(born(2004, 1, 1), today).as_deref(),
        Some("20-24")
    );
    assert_eq!(
        peer_age_range(born(1980, 1, 1), today).as_deref(),
        Some("40+")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn accepted_request_opens_a_chat_that_never_closes(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (peer, _peer_id, slots) =
        approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220031", 1).await;
    let (other_peer, _, _) =
        approved_peer_with_slots(&app, &pool, "other.peer@example.test", "+254711220032", 0).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");

    let request = json_body(request_slot(&app, &patient, &slots[0]).await).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // No chat before acceptance.
    let none = json_body(get(&app, "/api/v1/peer-chats", Some(&patient)).await).await;
    assert!(none["data"].as_array().unwrap().is_empty());
    post(
        &app,
        &format!("/api/v1/peer/requests/{request}/accept"),
        Some(&peer),
        &serde_json::json!({}),
    )
    .await;
    let mine = json_body(get(&app, "/api/v1/peer-requests", Some(&patient)).await).await;
    let chat = mine["data"][0]["conversation_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Patient writes; the send is idempotent by key.
    let msg = serde_json::json!({"content": "Hi Neema, thanks for accepting."});
    let sent = send(
        &app,
        "POST",
        &format!("/api/v1/peer-chats/{chat}/messages"),
        Some(&patient),
        Some(&msg),
        Some("k-1"),
    )
    .await;
    assert_eq!(sent.status(), StatusCode::CREATED);
    let first_id = json_body(sent).await["data"]["id"].clone();
    let replay = send(
        &app,
        "POST",
        &format!("/api/v1/peer-chats/{chat}/messages"),
        Some(&patient),
        Some(&msg),
        Some("k-1"),
    )
    .await;
    assert_eq!(json_body(replay).await["data"]["id"], first_id);

    // The peer sees it as unread, with the patient's display name only.
    let peer_list = json_body(get(&app, "/api/v1/peer-chats", Some(&peer)).await).await;
    assert_eq!(peer_list["data"][0]["counterpart_name"], "Patient U.");
    assert_eq!(peer_list["data"][0]["unread_count"], 1);
    let patient_list = json_body(get(&app, "/api/v1/peer-chats", Some(&patient)).await).await;
    assert_eq!(patient_list["data"][0]["counterpart_name"], "Neema Kamau");
    let notice: String = sqlx::query_scalar(
        "SELECT body FROM notifications WHERE type='NEW_CHAT_MESSAGE' AND related_entity_type='PEER_CHAT' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !notice.contains("thanks for accepting"),
        "no message text in notifications"
    );

    // Read receipt clears the unread count.
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer-chats/{chat}/read"),
            Some(&peer),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let peer_list = json_body(get(&app, "/api/v1/peer-chats", Some(&peer)).await).await;
    assert_eq!(peer_list["data"][0]["unread_count"], 0);

    // Marking the session complete keeps the chat open (no lock).
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer/requests/{request}/complete"),
            Some(&peer),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let later = send(
        &app,
        "POST",
        &format!("/api/v1/peer-chats/{chat}/messages"),
        Some(&peer),
        Some(&serde_json::json!({"content": "Reach out any time."})),
        None,
    )
    .await;
    assert_eq!(later.status(), StatusCode::CREATED);
    let thread = json_body(
        get(
            &app,
            &format!("/api/v1/peer-chats/{chat}/messages"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert_eq!(thread["data"].as_array().unwrap().len(), 2);

    // An image goes through the same compression and encrypted storage.
    let image = post(
        &app,
        &format!("/api/v1/peer-chats/{chat}/attachments"),
        Some(&patient),
        &serde_json::json!({"mime_type": "image/jpeg", "base64_content": minimal_jpeg_base64()}),
    )
    .await;
    assert_eq!(image.status(), StatusCode::CREATED);
    let image_id = json_body(image).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let viewed = get(
        &app,
        &format!("/api/v1/peer-chats/{chat}/messages/{image_id}/attachment"),
        Some(&peer),
    )
    .await;
    assert_eq!(viewed.status(), StatusCode::OK);
    assert_eq!(viewed.headers()["cache-control"], "no-store, private");

    // Nobody else can read or write it.
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/peer-chats/{chat}/messages"),
            Some(&other_peer)
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/peer-chats/{chat}/messages"),
            Some(&doctor)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let other_list = json_body(get(&app, "/api/v1/peer-chats", Some(&other_peer)).await).await;
    assert!(other_list["data"].as_array().unwrap().is_empty());

    // Admin Peer Directory: counts and who was supported, never the topic
    // or any message.
    let peer_id = _peer_id;
    let admin = token_for_real_role(&pool, "ADMIN").await;
    let stored_messages: i64 = sqlx::query_scalar("SELECT count(*) FROM peer_messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    let directory = json_body(get(&app, "/api/v1/admin/peer-directory", Some(&admin)).await).await;
    let row = directory["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == peer_id.as_str())
        .expect("the peer is listed")
        .clone();
    assert_eq!(row["patients_supported"], 1);
    assert_eq!(row["sessions_completed"], 1);
    assert_eq!(row["chat_sessions"], 1);
    assert_eq!(row["messages"], stored_messages);
    let history = get(
        &app,
        &format!("/api/v1/admin/peers/{peer_id}/history"),
        Some(&admin),
    )
    .await;
    assert_eq!(history.status(), StatusCode::OK);
    let history = json_body(history).await;
    let session = &history["data"]["sessions"][0];
    assert_eq!(session["request_id"], request.as_str());
    assert!(!session["patient_name"].as_str().unwrap().is_empty());
    assert_eq!(session["messages"], stored_messages);
    assert!(session.get("topic").is_none());
    let raw = history.to_string();
    assert!(!raw.contains("thanks for accepting"), "no message content");
    let events: Vec<&str> = history["data"]["status_history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["event"].as_str().unwrap())
        .collect();
    assert_eq!(events, ["CREATED", "APPROVED"]);
    // Peers and patients cannot use the Admin views.
    for token in [&peer, &patient] {
        assert_eq!(
            get(&app, "/api/v1/admin/peer-directory", Some(token))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let viewed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action = 'PEER_NAVIGATOR_HISTORY_VIEWED'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(viewed, 1, "viewing a peer's history is audited");
}

async fn peer_presence(
    state: &AppState,
    channel: &str,
    uid: u32,
    id: &str,
    joined: bool,
    seq: i64,
) -> bool {
    use hellodoctor_backend::services::consultation_presence_service::{
        apply_provider_event, ProviderPresenceEvent, ProviderPresenceKind,
    };
    apply_provider_event(
        state,
        ProviderPresenceEvent {
            provider_event_id: id.into(),
            channel_name: channel.into(),
            participant_uid: uid,
            kind: if joined {
                ProviderPresenceKind::Joined
            } else {
                ProviderPresenceKind::Left
            },
            provider_sequence: seq,
            provider_event_at: chrono::Utc::now(),
        },
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn peer_call_uses_the_doctor_join_window_and_empty_room_rules(pool: PgPool) {
    use hellodoctor_backend::services::agora_token_service::derive_uid;
    use hellodoctor_backend::services::peer_call_service::PeerCallService;
    let (app, pool, state) = test_app_and_state(pool).await;
    let (peer, _peer_id, slots) =
        approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220041", 1).await;
    let (other_peer, _, _) =
        approved_peer_with_slots(&app, &pool, "other.peer@example.test", "+254711220042", 0).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let request = json_body(
        post(
            &app,
            "/api/v1/peer-requests",
            Some(&patient),
            &serde_json::json!({"slot_id": slots[0], "support_type": "CALL", "topic": "Talking things through"}),
        )
        .await,
    )
    .await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Not joinable until the peer accepts.
    let call = json_body(
        get(
            &app,
            &format!("/api/v1/peer-requests/{request}/call"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert_eq!(call["data"]["join"]["eligible"], false);
    assert_eq!(call["data"]["join"]["reason"], "INVALID_STATE");
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer-requests/{request}/rtc-token"),
            Some(&patient),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    post(
        &app,
        &format!("/api/v1/peer/requests/{request}/accept"),
        Some(&peer),
        &serde_json::json!({}),
    )
    .await;

    // Inside the window both parties get tokens for the same peer_ channel.
    let call = json_body(
        get(
            &app,
            &format!("/api/v1/peer-requests/{request}/call"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert_eq!(call["data"]["join"]["eligible"], true);
    let patient_token = json_body(
        post(
            &app,
            &format!("/api/v1/peer-requests/{request}/rtc-token"),
            Some(&patient),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    let peer_token = json_body(
        post(
            &app,
            &format!("/api/v1/peer-requests/{request}/rtc-token"),
            Some(&peer),
            &serde_json::json!({}),
        )
        .await,
    )
    .await;
    let channel = patient_token["data"]["channel_name"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(channel.starts_with("peer_"));
    assert_eq!(peer_token["data"]["channel_name"], channel.as_str());
    assert_ne!(patient_token["data"]["uid"], peer_token["data"]["uid"]);
    assert!(patient_token["data"]["token"].as_str().unwrap().len() > 20);
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer-requests/{request}/rtc-token"),
            Some(&other_peer),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND,
        "nobody else can join"
    );

    // Presence from the Agora webhook: both join, both leave -> grace period.
    let peer_user: Uuid = sqlx::query_scalar("SELECT peer_user_id FROM peer_requests WHERE id=$1")
        .bind(Uuid::parse_str(&request).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let patient_uid = derive_uid(&channel, uid(PATIENT_USER_ID).into());
    let peer_uid = derive_uid(&channel, peer_user.into());
    assert!(peer_presence(&state, &channel, patient_uid, "e1", true, 1).await);
    assert!(peer_presence(&state, &channel, peer_uid, "e2", true, 1).await);
    assert!(
        !peer_presence(&state, &channel, peer_uid, "e2-dup", true, 1).await,
        "out-of-order ignored"
    );
    let status: String =
        sqlx::query_scalar("SELECT status FROM peer_call_sessions WHERE channel_name=$1")
            .bind(&channel)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "ACTIVE");
    assert!(peer_presence(&state, &channel, patient_uid, "e3", false, 2).await);
    assert!(peer_presence(&state, &channel, peer_uid, "e4", false, 2).await);
    let (recovery, seconds): (String, f64) = sqlx::query_as(
        "SELECT recovery_state, extract(epoch FROM empty_room_deadline - clock_timestamp())::float8 \
         FROM peer_call_sessions WHERE channel_name=$1",
    )
    .bind(&channel)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recovery, "GRACE_PERIOD");
    assert!(
        (170.0..=181.0).contains(&seconds),
        "the doctor 180-second grace: {seconds}"
    );

    // A rejoin inside the grace keeps the room.
    assert!(peer_presence(&state, &channel, peer_uid, "e5", true, 3).await);
    let recovery: String =
        sqlx::query_scalar("SELECT recovery_state FROM peer_call_sessions WHERE channel_name=$1")
            .bind(&channel)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recovery, "NONE");
    assert!(peer_presence(&state, &channel, peer_uid, "e6", false, 4).await);

    // Past the grace the sweep ends the room; no new call token, chat stays open.
    sqlx::query("UPDATE peer_call_sessions SET empty_room_deadline = clock_timestamp() - interval '1 second' WHERE channel_name=$1")
        .bind(&channel)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        PeerCallService::expire_empty_rooms(&state).await.unwrap(),
        1
    );
    let call = json_body(
        get(
            &app,
            &format!("/api/v1/peer-requests/{request}/call"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert_eq!(call["data"]["session_status"], "ENDED");
    assert_eq!(call["data"]["join"]["eligible"], false);
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer-requests/{request}/rtc-token"),
            Some(&patient),
            &serde_json::json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let chat: Uuid = sqlx::query_scalar("SELECT id FROM peer_conversations WHERE request_id=$1")
        .bind(Uuid::parse_str(&request).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        post(
            &app,
            &format!("/api/v1/peer-chats/{chat}/messages"),
            Some(&patient),
            &serde_json::json!({"content": "Thanks for the call."})
        )
        .await
        .status(),
        StatusCode::CREATED
    );
}

async fn care_sign_in(
    app: &axum::Router,
    email: &str,
    password: &str,
) -> axum::http::Response<Body> {
    let started = post(
        app,
        "/api/v1/auth/care/login",
        None,
        &serde_json::json!({"email": email, "password": password}),
    )
    .await;
    if started.status() != StatusCode::OK {
        return started;
    }
    let started = json_body(started).await;
    post(
        app,
        "/api/v1/auth/care/verify-otp",
        None,
        &serde_json::json!({
            "session_token": started["data"]["session_token"],
            "otp_code": started["data"]["debug_otp_code"]
        }),
    )
    .await
}

#[sqlx::test(migrations = "./migrations")]
async fn one_care_sign_in_serves_doctors_and_peers(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220051", 0).await;
    expire_otp_delivery_cooldown(&state, PEER_EMAIL).await;
    let peer = care_sign_in(&app, PEER_EMAIL, PEER_PASSWORD).await;
    assert_eq!(peer.status(), StatusCode::OK);
    assert_eq!(json_body(peer).await["data"]["user"]["role"], "PEER");
    let doctor = care_sign_in(&app, SEED_DOCTOR1_EMAIL, SEED_DOCTOR1_PASSWORD).await;
    assert_eq!(doctor.status(), StatusCode::OK);
    assert_eq!(json_body(doctor).await["data"]["user"]["role"], "DOCTOR");
    assert_eq!(
        care_sign_in(&app, PEER_EMAIL, "Wrong-Password-1!")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    give_seed_patient_email_credentials(&pool).await;
    assert_eq!(
        care_sign_in(&app, SEED_PATIENT_TEST_EMAIL, SEED_PATIENT_TEST_PASSWORD)
            .await
            .status(),
        StatusCode::UNAUTHORIZED,
        "patients cannot use the Care app sign-in"
    );
}

/// Peer account emails are queued with the business event: sign-up
/// ("application received") and each Admin decision. Names in account
/// emails resolve for every role (they used to come back empty, so doctors
/// were greeted "Dr. Colleague").
#[sqlx::test(migrations = "./migrations")]
async fn peer_emails_are_queued_and_account_names_resolve(pool: PgPool) {
    let (app, pool, state) = test_app_and_state(pool).await;
    let created = register_peer(&app, &peer_docs(&pool).await, PEER_EMAIL, "+254711220091").await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let peer_id = Uuid::parse_str(
        json_body(created).await["data"]["peer_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let queued = |pool: PgPool| async move {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT email_type, recipient_email FROM email_outbox              WHERE related_entity_type = 'PEER_NAVIGATOR' AND related_entity_id = $1              ORDER BY created_at",
        )
        .bind(peer_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        rows
    };
    assert_eq!(
        queued(pool.clone()).await,
        vec![(
            "PEER_APPLICATION_RECEIVED".to_string(),
            PEER_EMAIL.to_string()
        )]
    );

    let admin = token_for_real_role(&pool, "ADMIN").await;
    for (action, note) in [
        ("approve", None),
        ("suspend", Some("Paused for refresher training.")),
        ("reinstate", None),
    ] {
        let decided = post(
            &app,
            &format!("/api/v1/admin/peers/{peer_id}/{action}"),
            Some(&admin),
            &serde_json::json!({ "note": note }),
        )
        .await;
        assert_eq!(decided.status(), StatusCode::OK, "{action}");
    }
    let types: Vec<String> = queued(pool.clone())
        .await
        .into_iter()
        .map(|r| r.0)
        .collect();
    assert_eq!(
        types,
        [
            "PEER_APPLICATION_RECEIVED",
            "PEER_APPROVED",
            "PEER_SUSPENDED",
            "PEER_REINSTATED"
        ]
    );

    // The dispatch worker's pool can read the peer it renders for.
    let rendered_from =
        hellodoctor_backend::repository::peer_repo::find_by_id(&state.system_db, peer_id)
            .await
            .unwrap()
            .expect("system worker reads peer_profiles");
    assert_eq!(rendered_from.full_name, "Neema Kamau");

    // Account-email greetings resolve for peers, doctors and patients.
    let first_name = |user_id: Uuid| {
        let state = state.clone();
        async move {
            let user = hellodoctor_backend::repository::user_repo::find_by_id(
                &state.system_db,
                user_id.into(),
            )
            .await
            .unwrap()
            .unwrap();
            hellodoctor_backend::services::password_reset_service::account_first_name(&state, &user)
                .await
        }
    };
    assert_eq!(
        first_name(rendered_from.user_id.into_inner()).await,
        "Neema"
    );
    let doctor_user: Uuid = sqlx::query_scalar(
        "UPDATE doctor_profiles SET full_name = 'Dr. Jane Mwangi'          WHERE id = (SELECT id FROM doctor_profiles WHERE deleted_at IS NULL LIMIT 1) RETURNING user_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first_name(doctor_user).await, "Jane");
    let (patient_user, patient_name): (Uuid, String) = sqlx::query_as(
        "SELECT user_id, full_name FROM patient_profiles          WHERE user_id IS NOT NULL AND guardian_patient_id IS NULL LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        first_name(patient_user).await,
        patient_name.split_whitespace().next().unwrap()
    );
}

/// Every Admin action is audited: handlers that write their own row are not
/// doubled; record reads and changes without one get a generic row; opening
/// a record the console already loaded is recorded through
/// `/admin/audit/views`. The patient record lists peer sessions.
#[sqlx::test(migrations = "./migrations")]
async fn admin_actions_and_record_views_are_audited(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let (_peer, peer_id, slots) =
        approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220071", 1).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let request = json_body(request_slot(&app, &patient, &slots[0]).await).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let admin = token_for_real_role(&pool, "ADMIN").await;
    let count = |pool: PgPool, action: &'static str| async move {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM audit_events WHERE action = $1")
            .bind(action)
            .fetch_one(&pool)
            .await
            .unwrap()
    };

    // The approval (in approved_peer_with_slots) wrote its own row only.
    assert_eq!(count(pool.clone(), "PEER_NAVIGATOR_APPROVED").await, 1);
    assert_eq!(count(pool.clone(), "ADMIN_ACTION").await, 0);

    // A record read without its own audit row gets a generic one.
    let patient_id: Uuid = sqlx::query_scalar("SELECT patient_id FROM peer_requests WHERE id = $1")
        .bind(Uuid::parse_str(&request).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    let dossier = get(
        &app,
        &format!("/api/v1/admin/patients/{patient_id}/dossier"),
        Some(&admin),
    )
    .await;
    assert!(dossier.status().is_success() || dossier.status().is_client_error());
    let (route, resource): (Value, Option<Uuid>) = sqlx::query_as(
        "SELECT after_data, resource_id FROM audit_events WHERE action = 'PATIENT_RECORD_VIEWED' \
         ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(route["route"], "/admin/patients/:id/dossier");
    assert_eq!(resource, Some(patient_id));

    // Lists are not recorded.
    let before = count(pool.clone(), "PATIENT_RECORD_VIEWED").await;
    get(&app, "/api/v1/admin/patients", Some(&admin)).await;
    assert_eq!(count(pool.clone(), "PATIENT_RECORD_VIEWED").await, before);

    // Opening records already loaded in the console.
    for (kind, id, action, expected) in [
        (
            "PATIENT",
            patient_id.to_string(),
            "PATIENT_RECORD_VIEWED",
            2,
        ),
        (
            "PEER_NAVIGATOR",
            peer_id.clone(),
            "PEER_NAVIGATOR_RECORD_VIEWED",
            1,
        ),
    ] {
        let viewed = post(
            &app,
            "/api/v1/admin/audit/views",
            Some(&admin),
            &serde_json::json!({"resource_type": kind, "resource_id": id}),
        )
        .await;
        assert_eq!(viewed.status(), StatusCode::OK);
        assert_eq!(count(pool.clone(), action).await, expected, "{action}");
    }
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/audit/views",
            Some(&admin),
            &serde_json::json!({"resource_type": "WHATEVER", "resource_id": peer_id}),
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            "/api/v1/admin/audit/views",
            Some(&patient),
            &serde_json::json!({"resource_type": "PATIENT", "resource_id": patient_id}),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    // Successful views audited themselves; only the rejected attempt got a
    // generic row, marked FAILED with its status.
    let failed: Vec<(String, Value)> =
        sqlx::query_as("SELECT result, after_data FROM audit_events WHERE action = 'ADMIN_ACTION'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].0, "FAILED");
    assert_eq!(failed[0].1["status"], 422);

    // The patient record lists peer sessions, without topic or messages.
    let sessions = get(
        &app,
        &format!("/api/v1/admin/patients/{patient_id}/peer-sessions"),
        Some(&admin),
    )
    .await;
    assert_eq!(sessions.status(), StatusCode::OK);
    let sessions = json_body(sessions).await;
    assert_eq!(sessions["data"][0]["request_id"], request.as_str());
    assert_eq!(sessions["data"][0]["peer_name"], "Neema Kamau");
    assert_eq!(sessions["data"][0]["status"], "PENDING");
    assert!(sessions["data"][0].get("topic").is_none());
    assert_eq!(
        get(
            &app,
            &format!("/api/v1/admin/patients/{patient_id}/peer-sessions"),
            Some(&patient),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}

/// Hostile input is refused or stored as plain text; peer chat takes photos
/// only.
#[sqlx::test(migrations = "./migrations")]
async fn peer_inputs_reject_spoofing_files_and_treat_sql_as_text(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    // A bidirectional override in a name is refused.
    let spoofed = post(
        &app,
        "/api/v1/auth/peer/register",
        None,
        &serde_json::json!({
            "full_name": "Neema \u{202E}Kamau",
            "email": "spoof.peer@example.test",
            "phone_number": "+254711220081",
            "password": PEER_PASSWORD,
            "county": "Nairobi",
            "accepted_documents": peer_docs(&pool).await
        }),
    )
    .await;
    assert_eq!(spoofed.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let (peer, _peer_id, slots) =
        approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220082", 1).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    // SQL in a topic is just text: stored verbatim, nothing executed.
    let injection = "x'); DROP TABLE peer_requests; --";
    let made = post(
        &app,
        "/api/v1/peer-requests",
        Some(&patient),
        &serde_json::json!({"slot_id": slots[0], "support_type": "CHAT", "topic": injection}),
    )
    .await;
    assert_eq!(made.status(), StatusCode::CREATED);
    let request = json_body(made).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let stored: String = sqlx::query_scalar("SELECT topic FROM peer_requests WHERE id = $1")
        .bind(Uuid::parse_str(&request).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, injection);

    // Accept, then try to send a PDF and a script renamed as a photo.
    let accepted = post(
        &app,
        &format!("/api/v1/peer/requests/{request}/accept"),
        Some(&peer),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let mine = json_body(get(&app, "/api/v1/peer-requests", Some(&patient)).await).await;
    let chat = mine["data"][0]["conversation_id"]
        .as_str()
        .unwrap()
        .to_string();
    for (mime, bytes) in [
        (
            "application/pdf",
            b"%PDF-1.4\n1 0 obj << >> endobj\n%%EOF".as_slice(),
        ),
        ("text/plain", b"hello".as_slice()),
        ("image/png", b"<script>alert(1)</script>".as_slice()),
    ] {
        let sent = post(
            &app,
            &format!("/api/v1/peer-chats/{chat}/attachments"),
            Some(&patient),
            &serde_json::json!({"mime_type": mime, "base64_content": STANDARD.encode(bytes)}),
        )
        .await;
        assert_eq!(sent.status(), StatusCode::UNPROCESSABLE_ENTITY, "{mime}");
    }
}

/// Peer-support sessions email both sides like a consultation does: the peer
/// on a new request, a booking and a cancellation; the patient on a
/// confirmation and a decline. No email carries the topic or, to the peer,
/// the patient's name.
#[sqlx::test(migrations = "./migrations")]
async fn peer_session_emails_reach_both_sides_without_the_topic(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    let transport = capture_transport();
    state.email_transport = transport.clone();
    let app = hellodoctor_backend::api::create_router(state);
    let patient_email = "session.patient@example.test";
    sqlx::query("UPDATE users SET email = $1 WHERE id = $2")
        .bind(patient_email)
        .bind(uid(PATIENT_USER_ID))
        .execute(&pool)
        .await
        .unwrap();
    let (peer, _peer_id, slots) =
        approved_peer_with_slots(&app, &pool, PEER_EMAIL, "+254711220051", 3).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let mut ids = Vec::new();
    for slot in &slots {
        let made = request_slot(&app, &patient, slot).await;
        assert_eq!(made.status(), StatusCode::CREATED);
        ids.push(
            json_body(made).await["data"]["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    for (path, token) in [
        (format!("/api/v1/peer/requests/{}/accept", ids[0]), &peer),
        (format!("/api/v1/peer/requests/{}/decline", ids[1]), &peer),
        (format!("/api/v1/peer-requests/{}/cancel", ids[2]), &patient),
    ] {
        let response = post(&app, &path, Some(token), &serde_json::json!({})).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    let types = [
        "PEER_SESSION_REQUESTED",
        "PEER_SESSION_BOOKED",
        "PEER_SESSION_CANCELLED",
        "PEER_SESSION_CONFIRMED",
        "PEER_SESSION_DECLINED",
    ];
    wait_for_sent_emails(&pool, &types).await;

    let mut queued: Vec<(String, String)> = sqlx::query_as(
        "SELECT email_type, recipient_email FROM email_outbox WHERE email_type LIKE 'PEER_SESSION_%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    queued.sort();
    let mut expected: Vec<(String, String)> = [
        ("PEER_SESSION_BOOKED", PEER_EMAIL),
        ("PEER_SESSION_CANCELLED", PEER_EMAIL),
        ("PEER_SESSION_CONFIRMED", patient_email),
        ("PEER_SESSION_DECLINED", patient_email),
        ("PEER_SESSION_REQUESTED", PEER_EMAIL),
        ("PEER_SESSION_REQUESTED", PEER_EMAIL),
        ("PEER_SESSION_REQUESTED", PEER_EMAIL),
    ]
    .iter()
    .map(|(t, r)| (t.to_string(), r.to_string()))
    .collect();
    expected.sort();
    assert_eq!(queued, expected);

    let sent = transport.sent.lock().unwrap().clone();
    let subjects: Vec<&str> = sent.iter().map(|m| m.1.as_str()).collect();
    for subject in [
        "You have a new support request.",
        "Your support session is booked.",
        "A support request was cancelled.",
        "Your support session is confirmed.",
        "Your support request wasn't accepted.",
    ] {
        assert!(subjects.contains(&subject), "missing: {subject}");
    }
    for (recipient, subject, html, text) in sent
        .iter()
        .filter(|m| m.1.contains("support session") || m.1.contains("support request"))
    {
        for body in [html, text] {
            assert!(
                !body.contains("PrEP"),
                "{subject}: the topic never reaches an email"
            );
            assert!(body.contains("EAT"), "{subject}: the session time is shown");
            if recipient == PEER_EMAIL {
                assert!(
                    !body.contains("Patient User"),
                    "{subject}: no patient name to the peer"
                );
            }
        }
    }
    let confirmed = sent
        .iter()
        .find(|m| m.1 == "Your support session is confirmed.")
        .unwrap();
    assert!(confirmed.3.contains("Hello Patient,") && confirmed.3.contains("Neema"));
}
