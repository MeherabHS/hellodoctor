//! First-Admin bootstrap (`services::admin_bootstrap_service`) — positive,
//! duplicate, and concurrency cases. Exercises the real `hellodoctor_auth_worker`
//! capability role's pool (via `build_role_pools`, the same helper the rest of
//! this suite uses to prove grants/RLS actually work, not a superuser bypass)
//! so these tests prove the same privilege boundary the production tool runs
//! under, not an idealized one. `auth_worker` only has INSERT on `audit_events`
//! (by least-privilege design, see migration 20261013000001), so the two tests
//! that read back an audit row use the `system_worker` pool for that
//! verification SELECT only — the actual bootstrap write path always stays on
//! `auth_worker`, matching production. That SELECT also requires
//! `app.current_role = 'ADMIN'` to be set on the session first (the
//! `audit_events_select` RLS policy checks it, see migration
//! 20261106000002_single_admin) — the GRANT alone isn't sufficient, so those
//! two verification reads run inside a short transaction that sets it via
//! `set_config(..., true)` before querying, then rolls back.
//!
//! The CLI binary itself (`src/bin/admin_bootstrap.rs`) is intentionally not
//! exercised here: its `rpassword` prompts open `/dev/tty` directly and
//! cannot be driven by piped stdin in an automated test. Everything it does
//! beyond collecting that input is covered below.
use super::*;
use hellodoctor_backend::error::AppError;
use hellodoctor_backend::services::admin_bootstrap_service::bootstrap_first_admin;

const VALID_PASSWORD: &str = "Bootstrap1!";

/// `build_role_pools` alone — deliberately does NOT call `seed::seed_dev_data`
/// (unlike `test_app_and_state`), so this is a genuinely admin-free database,
/// matching the real first-boot condition this tool exists for.
async fn bare_auth_pool(pool: PgPool) -> PgPool {
    build_role_pools(&pool).await.auth
}

#[sqlx::test(migrations = "./migrations")]
async fn creates_the_first_admin_on_an_empty_database(pool: PgPool) {
    let pools = build_role_pools(&pool).await;
    let auth = pools.auth;

    let outcome = bootstrap_first_admin(
        &auth,
        "admin.bootstrap@example.invalid",
        "+254712345601",
        VALID_PASSWORD,
    )
    .await
    .expect("bootstrap should succeed on an admin-free database");

    let (role, is_active, is_verified, password_hash): (String, bool, bool, Option<String>) =
        sqlx::query_as(
            "SELECT role::text, is_active, is_verified, password_hash FROM users WHERE id = $1",
        )
        .bind(outcome.user_id)
        .fetch_one(&auth)
        .await
        .unwrap();
    assert_eq!(role, "ADMIN");
    assert!(is_active);
    assert!(is_verified);
    assert!(hellodoctor_backend::crypto::verify_password_async(
        VALID_PASSWORD.to_string(),
        password_hash.expect("password hash must be set"),
    )
    .await
    .unwrap());

    // Audit event is recorded, and never attributed to the newly created
    // account as though it had already authenticated itself.
    let (actor_id, action, resource_id, result): (
        Option<uuid::Uuid>,
        String,
        Option<uuid::Uuid>,
        String,
    ) = {
        // audit_events' RLS SELECT policy requires the session to assert
        // `app.current_role = 'ADMIN'` (the same set_config convention
        // production uses via repository::begin_authed_as) — the GRANT alone
        // is not sufficient. `is_local=true` scopes it to this transaction.
        let mut tx = pools.system.begin().await.unwrap();
        sqlx::query("SELECT set_config('app.current_role', 'ADMIN', true)")
            .execute(&mut *tx)
            .await
            .unwrap();
        let row = sqlx::query_as(
            "SELECT actor_id, action, resource_id, result FROM audit_events \
                 WHERE resource_id = $1 AND action = 'ADMIN_BOOTSTRAP_INITIAL_ACCOUNT_CREATED'",
        )
        .bind(outcome.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        row
    };
    assert_eq!(
        actor_id, None,
        "bootstrap must not attribute creation to an authenticated actor"
    );
    assert_eq!(action, "ADMIN_BOOTSTRAP_INITIAL_ACCOUNT_CREATED");
    assert_eq!(resource_id, Some(outcome.user_id));
    assert_eq!(result, "SUCCESS");
}

#[sqlx::test(migrations = "./migrations")]
async fn refuses_when_an_admin_already_exists(pool: PgPool) {
    // test_app_and_state seeds the fixed dev Admin fixture (seed::SEED_ADMIN_EMAIL)
    // before returning — exactly the "an Admin already exists" condition.
    let (_, pool, _state) = test_app_and_state(pool).await;
    let auth = build_role_pools(&pool).await.auth;

    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE role = 'ADMIN'")
        .fetch_one(&auth)
        .await
        .unwrap();
    assert_eq!(before, 1);

    let error = bootstrap_first_admin(
        &auth,
        "second.admin@example.invalid",
        "+254712345602",
        VALID_PASSWORD,
    )
    .await
    .expect_err("must refuse when an Admin already exists");
    assert!(matches!(error, AppError::Conflict(_)));

    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE role = 'ADMIN'")
        .fetch_one(&auth)
        .await
        .unwrap();
    assert_eq!(after, 1, "no second Admin row may be created");
    assert_eq!(
        0,
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM users WHERE email = 'second.admin@example.invalid'"
        )
        .fetch_one(&auth)
        .await
        .unwrap(),
        "the rejected attempt must not leave a partial row behind"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn refuses_duplicate_email_or_phone_even_for_a_non_admin_account(pool: PgPool) {
    let auth = bare_auth_pool(pool).await;

    bootstrap_first_admin(
        &auth,
        "first.admin@example.invalid",
        "+254712345603",
        VALID_PASSWORD,
    )
    .await
    .expect("first bootstrap must succeed");

    // Same email, different phone — rejected by the duplicate-contact check,
    // not just the admin-exists check (proves both independent guards exist).
    let by_email = bootstrap_first_admin(
        &auth,
        "first.admin@example.invalid",
        "+254712345604",
        VALID_PASSWORD,
    )
    .await
    .expect_err("duplicate email must be refused");
    assert!(matches!(by_email, AppError::Conflict(_)));

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE role = 'ADMIN'")
        .fetch_one(&auth)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_bootstrap_attempts_create_at_most_one_admin(pool: PgPool) {
    let pools = build_role_pools(&pool).await;
    let auth = pools.auth.clone();
    let auth_a = auth.clone();
    let auth_b = auth.clone();

    // Two attempts racing the same admin-free database with different
    // identities — the advisory lock must serialize them so exactly one
    // succeeds, never both and never neither.
    let (result_a, result_b) = tokio::join!(
        bootstrap_first_admin(
            &auth_a,
            "racer.a@example.invalid",
            "+254712345605",
            VALID_PASSWORD,
        ),
        bootstrap_first_admin(
            &auth_b,
            "racer.b@example.invalid",
            "+254712345606",
            VALID_PASSWORD,
        ),
    );

    let successes = [&result_a, &result_b].iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 1, "exactly one concurrent attempt must succeed");

    let total_admins: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE role = 'ADMIN'")
        .fetch_one(&auth)
        .await
        .unwrap();
    assert_eq!(
        total_admins, 1,
        "concurrency must never produce more than one initial Admin"
    );

    let audit_rows: i64 = {
        let mut tx = pools.system.begin().await.unwrap();
        sqlx::query("SELECT set_config('app.current_role', 'ADMIN', true)")
            .execute(&mut *tx)
            .await
            .unwrap();
        let count = sqlx::query_scalar(
            "SELECT count(*) FROM audit_events WHERE action = 'ADMIN_BOOTSTRAP_INITIAL_ACCOUNT_CREATED'",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        count
    };
    assert_eq!(
        audit_rows, 1,
        "exactly one audit event, matching the one successful creation"
    );
}
