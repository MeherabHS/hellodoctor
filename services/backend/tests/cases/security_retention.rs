//! Security-data retention purge (2026-11-21, "security data only").
use super::*;
use hellodoctor_backend::services::security_retention::run_security_retention_purge;

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_codes_and_old_ended_sessions_are_purged_evidence_is_kept(pool: PgPool) {
    let (_app, pool, state) = test_app_and_state(pool).await;
    let user = uid(PATIENT_USER_ID);

    // Codes: one expired 8 days ago (purged), one 6 days ago and one still
    // valid (both kept).
    for (phone, age) in [
        ("+254700000081", "8 days"),
        ("+254700000082", "6 days"),
        ("+254700000083", "-1 hour"),
    ] {
        sqlx::query(&format!(
            "INSERT INTO phone_otps (phone_number, code_hash, expires_at) VALUES ($1, 'h', now() - interval '{age}')"
        ))
        .bind(phone)
        .execute(&pool)
        .await
        .unwrap();
    }
    for (token, age) in [("mfa-old", "8 days"), ("mfa-new", "1 day")] {
        sqlx::query(&format!(
            "INSERT INTO pending_mfa_challenges (token, user_id, code_hash, expires_at) VALUES ($1, $2, 'h', now() - interval '{age}')"
        ))
        .bind(token)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    }
    // One reset request per user: the expired one for the patient, a fresh
    // one for a doctor.
    for (token, owner, age) in [
        ("reset-old", user, "8 days"),
        ("reset-new", uid(DOCTOR1_USER_ID), "1 day"),
    ] {
        sqlx::query(&format!(
            "INSERT INTO password_reset_requests (token, user_id, code_hash, otp_expires_at) VALUES ($1, $2, 'h', now() - interval '{age}')"
        ))
        .bind(token)
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Sessions: ended 100 days ago (purged, with its device key), ended 100
    // days ago but named in the audit log (kept), ended 30 days ago (kept),
    // and live (kept).
    let insert_session = |family: Uuid, ended: &'static str| {
        let pool = pool.clone();
        async move {
            let id: Uuid = sqlx::query_scalar(&format!(
                "INSERT INTO auth_sessions (user_id, token_family_id, refresh_token_hash, expires_at) \
                 VALUES ($1, $2, 'retention-'||$2::text, now() - interval '{ended}') RETURNING id"
            ))
            .bind(user)
            .bind(family)
            .fetch_one(&pool)
            .await
            .unwrap();
            id
        }
    };
    let old_family = Uuid::new_v4();
    let evidence_family = Uuid::new_v4();
    let recent_family = Uuid::new_v4();
    let live_family = Uuid::new_v4();
    let old = insert_session(old_family, "100 days").await;
    let evidence = insert_session(evidence_family, "100 days").await;
    let recent = insert_session(recent_family, "30 days").await;
    let live = insert_session(live_family, "-3 days").await;
    sqlx::query(
        "INSERT INTO auth_session_device_keys (token_family_id, key_thumbprint) VALUES ($1, repeat('a', 43)), ($2, repeat('b', 43))",
    )
    .bind(old_family)
    .bind(live_family)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO audit_events (actor_role, action, resource_type, session_id) VALUES ('PATIENT', 'TEST', 'AUTH_SESSION', $1)",
    )
    .bind(evidence)
    .execute(&pool)
    .await
    .unwrap();

    let counts = run_security_retention_purge(&state).await.unwrap();
    assert_eq!(counts.phone_otps, 1);
    assert_eq!(counts.pending_mfa_challenges, 1);
    assert_eq!(counts.password_reset_requests, 1);
    assert_eq!(counts.auth_sessions, 1);

    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM phone_otps WHERE phone_number LIKE '+25470000008%'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM pending_mfa_challenges WHERE token='mfa-new'"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM password_reset_requests WHERE token='reset-new'"
        )
        .await,
        1
    );
    let remaining: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM auth_sessions WHERE id = ANY($1)")
            .bind(vec![old, evidence, recent, live])
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!remaining.contains(&old));
    assert!(
        remaining.contains(&evidence) && remaining.contains(&recent) && remaining.contains(&live)
    );
    assert_eq!(
        count(&pool, &format!("SELECT count(*) FROM auth_session_device_keys WHERE token_family_id = '{old_family}'")).await,
        0
    );
    assert_eq!(
        count(&pool, &format!("SELECT count(*) FROM auth_session_device_keys WHERE token_family_id = '{live_family}'")).await,
        1
    );

    // A second run finds nothing more.
    assert_eq!(
        run_security_retention_purge(&state).await.unwrap().total(),
        0
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn session_purge_refuses_a_shorter_period_than_approved(pool: PgPool) {
    let (_app, _pool, state) = test_app_and_state(pool).await;
    let result: Result<i32, _> = sqlx::query_scalar("SELECT purge_ended_auth_sessions(30, 100)")
        .fetch_one(&state.auth_db)
        .await;
    assert!(result.is_err());
    // The API role cannot call it at all.
    let denied: Result<i32, _> = sqlx::query_scalar("SELECT purge_ended_auth_sessions(90, 100)")
        .fetch_one(&state.db)
        .await;
    assert!(denied.is_err());
}
