//! Cross-account access sweep (product owner, 2026-11-24): every API route,
//! called by every kind of caller, against another person's real records.
//!
//! The route list is read from `src/api/router.rs` at compile time, so a
//! route added later is covered without editing this file.
//!
//! One patient (the victim) gets a consultation with marker text planted in
//! every clinical place: the intake complaint and notes, a chat message, the
//! doctor's prescription note and a correction note, plus a marker name.
//! Then each route is called, with every path parameter filled with the
//! victim's real ids, by:
//!   * nobody (no token) and a garbage token: protected routes answer 401;
//!   * another patient, an unrelated doctor and a peer navigator: no
//!     response may contain the victim's name or any clinical marker, and
//!     afterwards the victim's records must be unchanged;
//!   * an Admin: no response may contain clinical text (Admin may see names,
//!     never conditions, chat or prescriptions).
//!
//! No route may answer 500 to anyone.
//!
//! Limits, stated so nobody over-reads a pass: write routes get an empty JSON
//! body, so many answer 422 before reaching their ownership check; the
//! leak check catches what is *returned*, and the integrity check catches
//! changes to the victim's planted records only.
use super::*;
use serde_json::json;

const ROUTER_SOURCE: &str = include_str!("../../src/api/router.rs");
const CLINICAL_MARKER: &str = "Kx7ClinicalMarker";
const IDENTITY_MARKER: &str = "Victimmarker";

#[derive(Debug, Clone)]
struct RouteDef {
    path: String,
    methods: Vec<&'static str>,
    public: bool,
}

/// Every `.route("…", …)` inside `public_routes()` and `protected_routes()`,
/// with the HTTP methods registered on it.
fn api_routes() -> Vec<RouteDef> {
    let public_start = ROUTER_SOURCE
        .find("fn public_routes()")
        .expect("public_routes in router.rs");
    let protected_start = ROUTER_SOURCE
        .find("fn protected_routes()")
        .expect("protected_routes in router.rs");
    let mut routes = Vec::new();
    let mut at = 0;
    while let Some(found) = ROUTER_SOURCE[at..].find(".route(") {
        let start = at + found;
        at = start + 7;
        if start < public_start.min(protected_start) {
            continue; // health/metrics routers, outside /api/v1
        }
        // The route call's own text: up to its matching ')'.
        let bytes = ROUTER_SOURCE.as_bytes();
        let (mut depth, mut end) = (0i32, start + 6);
        while end < bytes.len() {
            match bytes[end] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            end += 1;
        }
        let call = &ROUTER_SOURCE[start..end];
        let Some(q1) = call.find('"') else { continue };
        let Some(q2) = call[q1 + 1..].find('"') else {
            continue;
        };
        let path = call[q1 + 1..q1 + 1 + q2].to_string();
        let rest = &call[q1 + 2 + q2..];
        let mut methods = Vec::new();
        for (name, method) in [
            ("get(", "GET"),
            ("post(", "POST"),
            ("put(", "PUT"),
            ("patch(", "PATCH"),
            ("delete(", "DELETE"),
        ] {
            let mut i = 0;
            while let Some(f) = rest[i..].find(name) {
                let pos = i + f;
                let before = rest[..pos].chars().last();
                if !matches!(before, Some(c) if c.is_alphanumeric() || c == '_')
                    && !methods.contains(&method)
                {
                    methods.push(method);
                }
                i = pos + name.len();
            }
        }
        // Routes defined between `public_routes()` and the next function.
        let public = if public_start < protected_start {
            start > public_start && start < protected_start
        } else {
            start > public_start
        };
        if !methods.is_empty() {
            routes.push(RouteDef {
                path,
                methods,
                public,
            });
        }
    }
    routes
}

/// The victim's real ids, used to fill path parameters.
#[derive(Clone)]
struct Victim {
    appointment: Uuid,
    conversation: Uuid,
    prescription: Uuid,
    patient_profile: Uuid,
    patient_user: Uuid,
    session: Uuid,
    slot: Uuid,
    message: Uuid,
    doctor_profile: Uuid,
}

/// Concrete URIs for one route: each `:param` gets the victim id its name
/// suggests; a generic `:id` is tried as each of the victim's main records.
fn concrete_paths(path: &str, v: &Victim) -> Vec<String> {
    let generic = [
        v.appointment,
        v.conversation,
        v.patient_profile,
        v.prescription,
    ];
    let fill = |variant: usize| -> String {
        path.split('/')
            .map(|seg| {
                let Some(name) = seg.strip_prefix(':') else {
                    return seg.to_string();
                };
                let name = name.to_ascii_lowercase();
                let id = if name.contains("appointment") {
                    Some(v.appointment)
                } else if name.contains("conversation") || name.contains("chat") {
                    Some(v.conversation)
                } else if name.contains("document")
                    || name.contains("prescription")
                    || name.contains("rx")
                {
                    Some(v.prescription)
                } else if name.contains("message") {
                    Some(v.message)
                } else if name.contains("session") || name.contains("consultation") {
                    Some(v.session)
                } else if name.contains("slot") {
                    Some(v.slot)
                } else if name.contains("patient") || name.contains("profile") {
                    Some(v.patient_profile)
                } else if name.contains("user") {
                    Some(v.patient_user)
                } else if name.contains("doctor") {
                    Some(v.doctor_profile)
                } else if name == "action" {
                    return "publish".to_string();
                } else if name == "revision" {
                    return "1".to_string();
                } else {
                    None
                };
                id.unwrap_or(generic[variant]).to_string()
            })
            .collect::<Vec<_>>()
            .join("/")
    };
    let mut out: Vec<String> = (0..generic.len())
        .map(|i| format!("/api/v1{}", fill(i)))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Retries the server's own throughput limits like a client would, so the
/// final answer is the authorisation decision itself.
async fn settled(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
) -> (StatusCode, String) {
    let body = json!({});
    for _ in 0..10 {
        // A fresh idempotency key each time, so no call replays another's.
        let key = Uuid::new_v4().to_string();
        let res = send(
            app,
            method,
            uri,
            token,
            if method == "GET" { None } else { Some(&body) },
            if method == "GET" {
                None
            } else {
                Some(key.as_str())
            },
        )
        .await;
        let status = res.status();
        if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
            tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
            continue;
        }
        let bytes = axum::body::to_bytes(res.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap_or_default();
        return (status, String::from_utf8_lossy(&bytes).into_owned());
    }
    (StatusCode::TOO_MANY_REQUESTS, String::new())
}

async fn new_user(pool: &PgPool, role: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO users(id,phone_number,role,is_active,is_verified,created_at,updated_at) \
         VALUES($1,$2,'{role}'::user_role_enum,true,true,NOW(),NOW())"
    ))
    .bind(id)
    .bind(format!(
        "+2547{:08}",
        rand::thread_rng().gen_range(0..100_000_000)
    ))
    .execute(pool)
    .await
    .unwrap();
    fixture_session(pool, id).await;
    id
}

/// What the victim's planted records look like; compared before and after
/// the strangers' calls.
async fn victim_state(pool: &PgPool, v: &Victim) -> serde_json::Value {
    let row = sqlx::query(
        "SELECT a.status::text AS status, a.clinical_outcome, i.chief_complaint, i.clinical_notes, \
                p.full_name, u.is_active, u.deleted_at IS NULL AS present, \
                (SELECT COUNT(*) FROM prescriptions WHERE appointment_id=a.id) AS pages, \
                (SELECT COUNT(*) FROM prescription_corrections WHERE appointment_id=a.id) AS notes, \
                (SELECT COUNT(*) FROM chat_messages WHERE conversation_id=$2) AS messages \
           FROM appointments a \
           LEFT JOIN appointment_clinical_intake i ON i.appointment_id=a.id \
           JOIN patient_profiles p ON p.id=a.patient_id \
           JOIN users u ON u.id=p.user_id \
          WHERE a.id=$1",
    )
    .bind(v.appointment)
    .bind(v.conversation)
    .fetch_one(pool)
    .await
    .unwrap();
    json!({
        "status": row.get::<String, _>("status"),
        "outcome": row.get::<Option<String>, _>("clinical_outcome"),
        "complaint": row.get::<Option<String>, _>("chief_complaint"),
        "notes": row.get::<Option<String>, _>("clinical_notes"),
        "name": row.get::<String, _>("full_name"),
        "active": row.get::<bool, _>("is_active"),
        "present": row.get::<bool, _>("present"),
        "pages": row.get::<i64, _>("pages"),
        "corrections": row.get::<i64, _>("notes"),
        "messages": row.get::<i64, _>("messages"),
    })
}

#[test]
fn the_router_is_parsed_completely() {
    let routes = api_routes();
    assert!(
        routes.len() >= 150,
        "router parsing found only {} routes",
        routes.len()
    );
    assert!(routes.iter().any(|r| r.public), "public routes found");
    let corrections = routes
        .iter()
        .find(|r| r.path == "/consultations/:appointment_id/prescription/corrections")
        .expect("a known route");
    assert!(corrections.methods.contains(&"GET") && corrections.methods.contains(&"POST"));
    assert!(!corrections.public);
}

#[sqlx::test(migrations = "./migrations")]
async fn no_account_reaches_another_accounts_records_on_any_route(pool: PgPool) {
    let mut config = test_config();
    config.global_ip_rate_limit_per_min = 100_000;
    let redis = deadpool_redis::Config::from_url(&config.redis_url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let (app, pool) = test_app_with_config(pool, config).await;

    // ---- The victim: a completed consultation with markers everywhere.
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    open_clinical_window(&pool, appointment).await;
    start_consultation(&pool, appointment).await;
    sqlx::query("UPDATE patient_profiles SET full_name=$2 WHERE id=$1")
        .bind(uid(PATIENT_PROFILE_ID))
        .bind(format!("Zebedayo {IDENTITY_MARKER}"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO appointment_clinical_intake(appointment_id,patient_id,chief_complaint,clinical_notes) \
         SELECT id,patient_id,$2,$2 FROM appointments WHERE id=$1 \
         ON CONFLICT (appointment_id) DO UPDATE SET chief_complaint=$2, clinical_notes=$2",
    )
    .bind(appointment)
    .bind(format!("{CLINICAL_MARKER} intake"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO chat_conversations(appointment_id,patient_id,doctor_id,expires_at) \
         SELECT id,patient_id,doctor_id,NOW()+INTERVAL '1 day' FROM appointments WHERE id=$1 \
         ON CONFLICT (appointment_id) DO NOTHING",
    )
    .bind(appointment)
    .execute(&pool)
    .await
    .unwrap();
    let conversation: Uuid =
        sqlx::query_scalar("SELECT id FROM chat_conversations WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    let message: Uuid = sqlx::query_scalar(
        "INSERT INTO chat_messages(conversation_id,sender_id,content) VALUES($1,$2,$3) RETURNING id",
    )
    .bind(conversation)
    .bind(uid(PATIENT_USER_ID))
    .bind(format!("{CLINICAL_MARKER} chat"))
    .fetch_one(&pool)
    .await
    .unwrap();
    let page = post(
        &app,
        &format!("/api/v1/consultations/{appointment}/prescription"),
        Some(&doctor),
        &json!({"base64_photo": minimal_jpeg_base64(), "doctor_notes": format!("{CLINICAL_MARKER} rx")}),
    )
    .await;
    assert_eq!(page.status(), StatusCode::CREATED);
    let prescription: Uuid =
        serde_json::from_value(json_body(page).await["data"]["id"].clone()).unwrap();
    let complete = post(
        &app,
        &format!("/api/v1/consultations/{appointment}/complete"),
        Some(&doctor),
        &json!({"outcome": "COMPLETED_WITH_RX"}),
    )
    .await;
    assert_eq!(complete.status(), StatusCode::OK);
    let note = post(
        &app,
        &format!("/api/v1/consultations/{appointment}/prescription/corrections"),
        Some(&doctor),
        &json!({"note": format!("{CLINICAL_MARKER} correction")}),
    )
    .await;
    assert_eq!(note.status(), StatusCode::CREATED);
    let (session, slot): (Uuid, Uuid) = sqlx::query_as(
        "SELECT cs.id, a.slot_id FROM appointments a JOIN consultation_sessions cs ON cs.appointment_id=a.id WHERE a.id=$1",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();
    let victim = Victim {
        appointment,
        conversation,
        prescription,
        patient_profile: uid(PATIENT_PROFILE_ID),
        patient_user: uid(PATIENT_USER_ID),
        session,
        slot,
        message,
        doctor_profile: uid(DOCTOR1_PROFILE_ID),
    };
    // The victim's own view really contains the markers (so a clean sweep
    // means "refused", not "the marker was never there").
    let own = json_body(
        get(
            &app,
            &format!("/api/v1/consultations/{appointment}/prescription/corrections"),
            Some(&patient),
        )
        .await,
    )
    .await;
    assert!(own.to_string().contains(CLINICAL_MARKER), "{own}");

    // ---- The strangers.
    let other_patient = new_user(&pool, "PATIENT").await;
    sqlx::query("INSERT INTO patient_profiles (id,user_id,full_name,date_of_birth,gender,created_at,updated_at) VALUES (gen_random_uuid(),$1,'Other Patient','1991-02-02','OTHER',NOW(),NOW())")
        .bind(other_patient)
        .execute(&pool)
        .await
        .unwrap();
    let peer = new_user(&pool, "PEER").await;
    fixture_session(&pool, uid(DOCTOR2_USER_ID)).await;
    let strangers: Vec<(&str, Uuid, String)> = vec![
        (
            "another patient",
            other_patient,
            make_token(other_patient, "PATIENT"),
        ),
        (
            "an unrelated doctor",
            uid(DOCTOR2_USER_ID),
            make_token(uid(DOCTOR2_USER_ID), "DOCTOR"),
        ),
        ("a peer navigator", peer, make_token(peer, "PEER")),
    ];
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let stranger_ids: Vec<Uuid> = strangers.iter().map(|s| s.1).collect();
    let before = victim_state(&pool, &victim).await;

    let routes = api_routes();
    let mut failures: Vec<String> = Vec::new();
    let mut calls = 0usize;
    let mut skipped_rate_limited = 0usize;
    // Reads first, so a write that wrongly succeeds cannot hide a later leak.
    for pass in ["GET", "WRITE"] {
        for route in &routes {
            for method in route.methods.iter().copied() {
                if (pass == "GET") != (method == "GET") {
                    continue;
                }
                for uri in concrete_paths(&route.path, &victim) {
                    // Fresh rate-limit counters and live strangers for each call
                    // group (a stranger's own logout or deletion must not hide
                    // the routes after it).
                    let mut conn = redis.get().await.unwrap();
                    let _: i64 = deadpool_redis::redis::cmd("EVAL")
                        .arg("local n=0 for _,k in ipairs(redis.call('KEYS','ratelimit:*')) do redis.call('DEL',k) n=n+1 end return n")
                        .arg(0)
                        .query_async(&mut conn)
                        .await
                        .unwrap();
                    drop(conn);
                    sqlx::query("UPDATE auth_sessions SET revoked_at=NULL, rotated_at=NULL, expires_at=NOW()+INTERVAL '7 days' WHERE user_id = ANY($1)")
                        .bind(&stranger_ids)
                        .execute(&pool)
                        .await
                        .unwrap();
                    sqlx::query(
                        "UPDATE users SET is_active=true, deleted_at=NULL WHERE id = ANY($1)",
                    )
                    .bind(&stranger_ids)
                    .execute(&pool)
                    .await
                    .unwrap();

                    // Nobody, and a garbage token.
                    if !route.public {
                        for (who, token) in
                            [("anonymous", None), ("garbage token", Some("not.a.jwt"))]
                        {
                            let (status, _) = settled(&app, method, &uri, token).await;
                            calls += 1;
                            if status != StatusCode::UNAUTHORIZED {
                                failures.push(format!(
                                    "{who} {method} {uri} -> {status} (expected 401)"
                                ));
                            }
                        }
                    }
                    // Strangers: no name, no clinical text, no 500.
                    for (who, _, token) in &strangers {
                        let (status, body) = settled(&app, method, &uri, Some(token)).await;
                        calls += 1;
                        if status == StatusCode::TOO_MANY_REQUESTS {
                            skipped_rate_limited += 1;
                        }
                        if status == StatusCode::INTERNAL_SERVER_ERROR {
                            failures.push(format!("{who} {method} {uri} -> 500"));
                        }
                        for marker in [CLINICAL_MARKER, IDENTITY_MARKER] {
                            if body.contains(marker) {
                                failures.push(format!(
                                    "LEAK: {who} {method} {uri} -> {status} returned the victim's {}",
                                    if marker == CLINICAL_MARKER { "clinical text" } else { "name" }
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    // The strangers changed nothing of the victim's.
    let after = victim_state(&pool, &victim).await;
    if before != after {
        failures.push(format!(
            "INTEGRITY: the victim's records changed during the strangers' calls\nbefore {before}\nafter  {after}"
        ));
    }
    // Admin: every read route, never any clinical text.
    for route in &routes {
        if !route.methods.contains(&"GET") {
            continue;
        }
        for uri in concrete_paths(&route.path, &victim) {
            let mut conn = redis.get().await.unwrap();
            let _: i64 = deadpool_redis::redis::cmd("EVAL")
                .arg("local n=0 for _,k in ipairs(redis.call('KEYS','ratelimit:*')) do redis.call('DEL',k) n=n+1 end return n")
                .arg(0)
                .query_async(&mut conn)
                .await
                .unwrap();
            drop(conn);
            let (status, body) = settled(&app, "GET", &uri, Some(&admin)).await;
            calls += 1;
            if status == StatusCode::INTERNAL_SERVER_ERROR {
                failures.push(format!("admin GET {uri} -> 500"));
            }
            if body.contains(CLINICAL_MARKER) {
                failures.push(format!(
                    "LEAK: admin GET {uri} -> {status} returned clinical text"
                ));
            }
        }
    }
    println!(
        "cross-account sweep: {} routes, {calls} calls, {skipped_rate_limited} still rate-limited after retries",
        routes.len()
    );
    assert!(
        failures.is_empty(),
        "{} problem(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Found by the sweep (2026-11-24): the consultation chat endpoints accepted
/// any signed-in role, and row-level security lets the ADMIN role see chat
/// rows (the Admin trace needs their metadata), so Admin could read the chat
/// text and open its clinical images. Only the consultation's doctor and
/// patient may now; Admin gets 403 on both.
#[sqlx::test(migrations = "./migrations")]
async fn admin_cannot_read_consultation_chat_text_or_images(pool: PgPool) {
    let (app, pool) = test_app(pool).await;
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let admin = make_token(uid(ADMIN_USER_ID), "ADMIN");
    let (appointment, _) = book_and_confirm(&app, &patient).await;
    sqlx::query(
        "INSERT INTO chat_conversations(appointment_id,patient_id,doctor_id,expires_at) \
         SELECT id,patient_id,doctor_id,NOW()+INTERVAL '1 day' FROM appointments WHERE id=$1 \
         ON CONFLICT (appointment_id) DO NOTHING",
    )
    .bind(appointment)
    .execute(&pool)
    .await
    .unwrap();
    let conversation: Uuid =
        sqlx::query_scalar("SELECT id FROM chat_conversations WHERE appointment_id=$1")
            .bind(appointment)
            .fetch_one(&pool)
            .await
            .unwrap();
    let message: Uuid = sqlx::query_scalar(
        "INSERT INTO chat_messages(conversation_id,sender_id,content,attachment_bucket,attachment_object_key,attachment_mime,attachment_size,attachment_hash) \
         VALUES($1,$2,$3,'b','chat/x.jpg','image/jpeg',10,'h') RETURNING id",
    )
    .bind(conversation)
    .bind(uid(PATIENT_USER_ID))
    .bind(format!("{CLINICAL_MARKER} chat"))
    .fetch_one(&pool)
    .await
    .unwrap();

    let messages = format!("/api/v1/chat/{conversation}/messages");
    let read = get(&app, &messages, Some(&admin)).await;
    assert_eq!(read.status(), StatusCode::FORBIDDEN);
    let body = axum::body::to_bytes(read.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&body).contains(CLINICAL_MARKER));
    let image = get(
        &app,
        &format!("/api/v1/chat/{conversation}/messages/{message}/attachment"),
        Some(&admin),
    )
    .await;
    assert_eq!(image.status(), StatusCode::FORBIDDEN);
    // The patient still reads their own chat.
    let own = json_body(get(&app, &messages, Some(&patient)).await).await;
    assert!(own.to_string().contains(CLINICAL_MARKER), "{own}");
}
