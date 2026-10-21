//! Database adversarial contract tests. Run only against a disposable, isolated
//! PostgreSQL cluster, single-threaded (migrations create cluster-wide roles).
//! These exercise the actual repository/SQL gates, not an HTTP/provider simulator.
//! The separate observer reads committed tables; worker return values do not grade
//! the invariant. HTTP/provider availability remains a separate acceptance gate.
use hellodoctor_backend::{
    domain::models::Gateway,
    repository::{payment_event_repo, transaction_repo, wallet_repo},
    seed::{self, ADMIN_USER_ID, DOCTOR1_PROFILE_ID, DOCTOR1_USER_ID, PATIENT_PROFILE_ID},
};
use sqlx::PgPool;
use tokio::task::JoinSet;
use uuid::Uuid;

fn id(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}

async fn obligation(pool: &PgPool) -> Uuid {
    let appointment = Uuid::new_v4();
    sqlx::query("INSERT INTO appointments(id,appointment_number,patient_id,doctor_id,slot_id,modality,status,consultation_fee) SELECT $1,$1::text,$2,$3,id,'CHAT','PENDING_PAYMENT',100 FROM doctor_schedule_slots WHERE doctor_id=$3 ORDER BY start_time LIMIT 1")
        .bind(appointment).bind(id(PATIENT_PROFILE_ID)).bind(id(DOCTOR1_PROFILE_ID))
        .execute(pool).await.unwrap();
    appointment
}

async fn insert_attempt(pool: &PgPool, appointment: Uuid, failed: bool) -> Result<(), sqlx::Error> {
    let transaction = Uuid::new_v4();
    sqlx::query("INSERT INTO transactions(id,transaction_number,appointment_id,payment_session_id,gateway,gross_amount,platform_fee_amount,net_amount,payment_status,currency) VALUES($1,$1::text,$2,$3,'MPESA',100,20,80,CASE WHEN $4 THEN 'FAILED' ELSE 'INITIATED' END::payment_status_enum,'KES')")
        .bind(transaction).bind(appointment).bind(Uuid::new_v4()).bind(failed)
        .execute(pool).await.map(|_| ())
}

async fn observer(pool: &PgPool, appointment: Uuid) -> (i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM transactions WHERE appointment_id=$1 AND payment_status<>'FAILED'),(SELECT count(*) FROM financial_economic_effects WHERE business_id=$1 AND effect_type='PAYMENT_CONFIRMED'),(SELECT count(*) FROM financial_economic_effects WHERE business_id=$1 AND effect_type='CONSULTATION_SETTLED')")
        .bind(appointment).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn business_obligation_events_settlement_and_admin_races(pool: PgPool) {
    seed::seed_dev_data(&pool).await.unwrap();
    let appointment = obligation(&pool).await;
    // History is legitimate and must not consume the active obligation slot.
    insert_attempt(&pool, appointment, true).await.unwrap();
    insert_attempt(&pool, appointment, true).await.unwrap();
    let mut workers = JoinSet::new();
    for _ in 0..100 {
        let pool = pool.clone();
        workers.spawn(async move { insert_attempt(&pool, appointment, false).await });
    }
    while let Some(result) = workers.join_next().await {
        if let Err(error) = result.unwrap() {
            assert_eq!(
                error.as_database_error().and_then(|e| e.code()).as_deref(),
                Some("23505")
            );
        }
    }
    assert_eq!(observer(&pool, appointment).await, (1, 0, 0));
    let transaction: Uuid = sqlx::query_scalar(
        "SELECT id FROM transactions WHERE appointment_id=$1 AND payment_status='INITIATED'",
    )
    .bind(appointment)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Same-event and distinct-event concurrent callbacks both share the same
    // conditional repository transition. No attacker-side success counter is used.
    for distinct_events in [false, true] {
        let mut workers = JoinSet::new();
        for i in 0..100 {
            let pool = pool.clone();
            workers.spawn(async move {
                let mut tx = pool.begin().await.unwrap();
                let event = if distinct_events {
                    format!("synthetic-event-{i}")
                } else {
                    "synthetic-same-event".into()
                };
                let new = payment_event_repo::try_record_event(
                    &mut *tx,
                    transaction,
                    Gateway::Mpesa,
                    &event,
                    "PAYMENT_SUCCESS",
                )
                .await
                .unwrap();
                if new {
                    transaction_repo::mark_payment_held(&mut *tx, transaction, "SYNTHETIC-SUCCESS")
                        .await
                        .unwrap();
                }
                tx.commit().await.unwrap();
            });
        }
        while let Some(result) = workers.join_next().await {
            result.unwrap();
        }
        assert_eq!(observer(&pool, appointment).await, (1, 1, 0));
    }
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM payment_events WHERE transaction_id=$1")
            .bind(transaction)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 101);

    sqlx::query("UPDATE appointments SET status='CONFIRMED' WHERE id=$1")
        .bind(appointment)
        .execute(&pool)
        .await
        .unwrap();
    let mut workers = JoinSet::new();
    for _ in 0..100 {
        let pool = pool.clone();
        workers.spawn(async move {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("SELECT set_config('app.current_role','DOCTOR',true),set_config('app.current_user_id',$1,true)")
                .bind(DOCTOR1_USER_ID).execute(&mut *tx).await.unwrap();
            let result: String = sqlx::query_scalar("SELECT doctor_complete_consultation($1,'COMPLETED_NO_RX')")
                .bind(appointment).fetch_one(&mut *tx).await.unwrap();
            assert!(matches!(result.as_str(), "COMPLETED" | "ALREADY_COMPLETED"));
            tx.commit().await.unwrap();
        });
    }
    while let Some(result) = workers.join_next().await {
        result.unwrap();
    }
    assert_eq!(observer(&pool, appointment).await, (1, 1, 1));
    let net: bigdecimal::BigDecimal =
        sqlx::query_scalar("SELECT lifetime_net FROM doctor_wallets WHERE doctor_id=$1")
            .bind(id(DOCTOR1_PROFILE_ID))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(net, bigdecimal::BigDecimal::from(80));
    // A later valid callback cannot regress settlement or create another effect.
    assert!(
        !transaction_repo::mark_payment_held(&pool, transaction, "SYNTHETIC-SUCCESS")
            .await
            .unwrap()
    );
    assert_eq!(observer(&pool, appointment).await, (1, 1, 1));

    let other_admin = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,phone_number,email,role) VALUES($1,'+254799999991','synthetic-admin@example.invalid','ADMIN')")
        .bind(other_admin).execute(&pool).await.unwrap();
    let batch = Uuid::new_v4();
    sqlx::query("INSERT INTO disbursement_batches(id,batch_number,initiated_by,period_start,period_end,total_doctors,total_gross,total_platform_fee,total_net_disbursed,status) VALUES($1,$1::text,$2,'2026-01-01','2026-01-31',1,100,20,80,'PENDING_APPROVAL')")
        .bind(batch).bind(id(ADMIN_USER_ID)).execute(&pool).await.unwrap();
    let item = Uuid::new_v4();
    sqlx::query("INSERT INTO disbursement_items(id,batch_id,doctor_id,gross_amount,platform_fee,net_amount,gateway,status,currency,provider_code) VALUES($1,$2,$3,100,20,80,'MPESA','PENDING_APPROVAL','KES','SYNTHETIC_LOCAL')")
        .bind(item).bind(batch).bind(id(DOCTOR1_PROFILE_ID)).execute(&pool).await.unwrap();
    let mut workers = JoinSet::new();
    for i in 0..100 {
        let pool = pool.clone();
        workers.spawn(async move {
            let admin = if i % 2 == 0 {
                id(ADMIN_USER_ID)
            } else {
                other_admin
            };
            wallet_repo::approve_disbursement_item(&pool, item, admin)
                .await
                .unwrap()
        });
    }
    while let Some(result) = workers.join_next().await {
        result.unwrap();
    }
    let (status, winner): (String, Uuid) =
        sqlx::query_as("SELECT status,approved_by FROM disbursement_items WHERE id=$1")
            .bind(item)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "APPROVED");
    assert!([id(ADMIN_USER_ID), other_admin].contains(&winner));
    let mut workers = JoinSet::new();
    for _ in 0..100 {
        let pool = pool.clone();
        workers.spawn(async move {
            wallet_repo::confirm_disbursement_item_paid(&pool, item, "SYNTHETIC-PAYOUT")
                .await
                .unwrap();
        });
    }
    while let Some(result) = workers.join_next().await {
        result.unwrap();
    }
    let (status, effects): (String, i64) = sqlx::query_as("SELECT status,(SELECT count(*) FROM financial_economic_effects WHERE effect_type='PAYOUT_PAID' AND business_id=$1) FROM disbursement_items WHERE id=$1")
        .bind(item).fetch_one(&pool).await.unwrap();
    assert_eq!((status.as_str(), effects), ("PAID", 1));
}
