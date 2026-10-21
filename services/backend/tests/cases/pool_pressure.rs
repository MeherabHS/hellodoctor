//! Controlled, in-process integration regression with independently observed
//! real capability connections. This is not a network load/capacity benchmark.
use super::*;

#[sqlx::test(migrations = "./migrations")]
async fn capability_pool_exhaustion_rejects_before_waiting_and_recovers(pool: PgPool) {
    let (_, pool, mut state) = test_app_and_state(pool).await;
    // Use the unchanged production default, rather than enlarging a pool to
    // hide contention. Only this test's pools differ from the small API fixture.
    for role in [DbRole::Api, DbRole::AuthWorker] {
        let previous = match role {
            DbRole::Api => state.db.clone(),
            DbRole::AuthWorker => state.auth_db.clone(),
            _ => unreachable!(),
        };
        let replacement = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .idle_timeout(std::time::Duration::from_millis(100))
            .connect_with(previous.connect_options().as_ref().clone())
            .await
            .unwrap();
        previous.close().await;
        match role {
            DbRole::Api => state.db = replacement,
            DbRole::AuthWorker => state.auth_db = replacement,
            _ => unreachable!(),
        }
    }
    let mut config = (*state.config).clone();
    config.db_pool_max_connections_per_role = 10;
    state.config = Arc::new(config);
    state.admission = Arc::new(hellodoctor_backend::admission::Admission::new(10));
    let app = create_router(state.clone());
    let patient = make_token(uid(PATIENT_USER_ID), "PATIENT");
    let doctor = make_token(uid(DOCTOR1_USER_ID), "DOCTOR");
    assert_eq!(
        get(&app, "/api/v1/patients/me", Some(&patient))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        get(&app, "/api/v1/doctor/appointments", Some(&doctor))
            .await
            .status(),
        StatusCode::OK
    );
    for role in [DbRole::AuthWorker, DbRole::Api] {
        let target = match role {
            DbRole::Api => &state.db,
            DbRole::AuthWorker => &state.auth_db,
            _ => unreachable!(),
        };
        let mut held = Vec::new();
        for _ in 0..9 {
            held.push(target.acquire().await.unwrap());
        }
        assert_eq!(
            get(&app, "/api/v1/patients/me", Some(&patient))
                .await
                .status(),
            StatusCode::OK,
            "one remaining connection must still serve an authorized patient"
        );
        // SQLx returns a committed transaction's connection asynchronously.
        // Establish the stated one-idle-connection precondition by awaiting
        // that return event, without retrying or relaxing the HTTP assertion.
        let mut available = target.acquire().await.unwrap();
        available.return_to_pool().await;
        assert_eq!(
            get(&app, "/api/v1/doctor/chats", Some(&doctor))
                .await
                .status(),
            StatusCode::OK,
            "one remaining connection must still serve an authorized doctor"
        );
        held.push(target.acquire().await.unwrap());
        assert_eq!(target.size(), 10);
        assert_eq!(target.num_idle(), 0);
        let role_name = match role {
            DbRole::AuthWorker => "hellodoctor_auth_worker",
            DbRole::Api => "hellodoctor_api",
            _ => unreachable!(),
        };
        let observed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND usename=$1",
        )
        .bind(role_name)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            observed, 10,
            "independent DB observer must see all ten connections"
        );
        for (path, token) in [
            ("/api/v1/patients/me", patient.as_str()),
            ("/api/v1/doctor/appointments", doctor.as_str()),
            ("/api/v1/doctor/chats", doctor.as_str()),
        ] {
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                get(&app, path, Some(token)),
            )
            .await
            .expect("pool pressure must reject before queuing for a connection");
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()["Retry-After"], "1");
            assert_eq!(
                json_body(response).await["error"]["code"],
                "SERVICE_OVERLOADED"
            );
        }
        let ctx = hellodoctor_backend::auth_context::AuthContext {
            user_id: UserId::from(uid(PATIENT_USER_ID)),
            role: UserRole::Patient,
            permissions: vec!["patient:self".into()],
            session_id: None,
            request_id: None,
        };
        let acquisition = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            state.begin_authed_as(&ctx, role),
        )
        .await
        .expect("transaction acquisition must respect its 250ms deadline");
        assert!(acquisition.is_err());
        let health = get(&app, "/healthz", None).await;
        assert_eq!(
            health.status(),
            if matches!(role, DbRole::Api) {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::OK
            }
        );
        // Return explicitly so the recovery check waits on a completion event,
        // not an arbitrary delay or a retry that could conceal a regression.
        for mut connection in held {
            connection.return_to_pool().await;
        }
        assert_eq!(
            get(&app, "/api/v1/patients/me", Some(&patient))
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            get(&app, "/api/v1/doctor/chats", Some(&doctor))
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(get(&app, "/healthz", None).await.status(), StatusCode::OK);
    }
}
