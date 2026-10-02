# Phase 10 — Rust Backend Architecture & Service Engineering

> **Document Version:** 1.0.0  
> **Core Framework:** Rust 1.80+ (Edition 2021), `axum` 0.7+, `tokio` 1.40+, `sqlx` 0.8+  
> **Database:** PostgreSQL 16+ via async connection pool (`sqlx::PgPool`)  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Architectural Principles & Crate Layout

The HelloDoctor backend is engineered for memory safety, concurrency, sub-millisecond route latency, and strong type safety. It adheres to the **Repository-Service-Handler Pattern**.

```
services/backend/
├── Cargo.toml                          # Crate dependencies & build profiles
├── migrations/                         # SQLx forward/backward migrations
│   ├── 20261001000001_initial_schema.sql
│   └── 20261001000002_add_telemetry_and_dossiers.sql
│
└── src/
    ├── main.rs                         # Application entrypoint & server bootstrap
    ├── config.rs                       # Strongly typed environment configuration
    ├── error.rs                        # AppError enum & Axum IntoResponse mapper
    │
    ├── api/                            # HTTP & WebSocket routing layer
    │   ├── router.rs                   # Master router combining all sub-routers
    │   ├── middleware/                 # Auth JWT, RateLimit, RequestId, Trace
    │   └── v1/                         # API version 1 route handlers
    │       ├── auth_handlers.rs
    │       ├── doctor_handlers.rs
    │       ├── appointment_handlers.rs
    │       ├── prescription_handlers.rs
    │       ├── grievance_handlers.rs
    │       ├── wallet_handlers.rs
    │       ├── admin_handlers.rs
    │       └── realtime_ws.rs
    │
    ├── domain/                         # Domain models, validation & business rules
    │   ├── models/                     # Patient, Doctor, Appointment, Escrow, etc.
    │   └── validation/                 # Custom phone, Internal Platform Compliance Warning, slot validator rules
    │
    ├── services/                       # Business logic orchestration
    │   ├── auth_service.rs             # JWTs and auth_sessions management
    │   ├── appointment_service.rs      # Atomic slot locking & escrow hold
    │   ├── prescription_service.rs     # Doctor prescription photo upload, patient intake document management & integrity verification
    │   ├── grievance_service.rs        # Telemetry binding & board adjudication (Internal Platform Compliance Warning)
    │   ├── settlement_service.rs       # 20% platform charge debarment calculations, two-stage payment flow & monthly batch disbursement orchestration
    │   ├── disbursement_service.rs     # Monthly batch disbursement orchestration for finance admins
    │   ├── agora_token_service.rs      # Agora token minting (manual HMAC token generation)
    │   ├── audit_service.rs            # Audit event logging service
    │   └── upload_security_service.rs  # File upload pipeline (magic bytes, malware scan, re-encoding, quarantine)
    │
    ├── repository/                     # Database access layer (SQLx queries)
    │   ├── user_repo.rs
    │   ├── doctor_repo.rs
    │   ├── appointment_repo.rs
    │   ├── prescription_repo.rs
    │   ├── grievance_repo.rs
    │   ├── session_repo.rs             # auth_sessions tracking
    │   └── transaction_repo.rs
    │
    └── telemetry/                      # Tracing subscriber, Prometheus metrics
```

---

## 2. Core Dependencies (`Cargo.toml`)

```toml
[package]
name = "hellodoctor-backend"
version = "2.4.0"
edition = "2021"

[dependencies]
# Async Runtime & HTTP Server
tokio = { version = "1.40", features = ["full"] }
axum = { version = "0.7", features = ["multipart", "ws"] }
tower = { version = "0.4", features = ["timeout", "load-shed"] }
tower-http = { version = "0.5", features = ["cors", "trace", "compression-full"] }

# Database & Migrations
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "bigdecimal", "migrate"] }

# Serialization & Validation
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
validator = { version = "0.18", features = ["derive"] }
bigdecimal = { version = "0.4", features = ["serde"] }
uuid = { version = "1.10", features = ["serde", "v4", "v7"] }
chrono = { version = "0.4", features = ["serde"] }

# Security & Cryptography
argon2 = "0.5"
jsonwebtoken = "9.3"
rand = "0.8"
sha2 = "0.10"
hmac = "0.12"

# Logging & Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
thiserror = "1.0"
dotenvy = "0.15"
```

---

## 3. Atomic Slot Reservation & Escrow Engine

To prevent concurrent double-booking and ensure financial integrity, slot reservation runs within an **ACID database transaction** with row-level locks (`SELECT ... FOR UPDATE`):

```rust
pub async fn book_appointment(
    pool: &PgPool,
    patient_id: Uuid,
    doctor_id: Uuid,
    slot_id: Uuid,
    fee: BigDecimal,
    gateway: PaymentGateway,
) -> Result<AppointmentBookingResult, AppError> {
    let mut tx = pool.begin().await?;

    // 1. Acquire exclusive lock on target slot
    let slot = sqlx::query!(
        r#"
        SELECT id, status as "status: SlotStatus"
        FROM doctor_schedule_slots
        WHERE id = $1 AND doctor_id = $2
        FOR UPDATE
        "#,
        slot_id,
        doctor_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound("Slot not found".into()))?;

    if slot.status != SlotStatus::Available {
        return Err(AppError::Conflict("Selected slot is no longer available.".into()));
    }

    // 2. Mark slot as booked
    sqlx::query!(
        "UPDATE doctor_schedule_slots SET status = 'BOOKED' WHERE id = $1",
        slot_id
    )
    .execute(&mut *tx)
    .await?;

    // 3. Compute 20% platform charge debarment
    let platform_fee = &fee * BigDecimal::from_str("0.20").unwrap();
    let net_doctor_amount = &fee - &platform_fee;

    // 4. Create appointment
    let apt_id = Uuid::new_v4();
    let apt_num = format!("APT-{}", &apt_id.to_string()[0..8].to_uppercase());

    sqlx::query!(
        r#"
        INSERT INTO appointments (id, appointment_number, patient_id, doctor_id, slot_id, status, consultation_fee)
        VALUES ($1, $2, $3, $4, $5, 'CONFIRMED', $6)
        "#,
        apt_id,
        apt_num,
        patient_id,
        doctor_id,
        slot_id,
        fee
    )
    .execute(&mut *tx)
    .await?;

    // 5. Deposit into Escrow Ledger
    let txn_num = format!("TXN-{}", Uuid::new_v4().to_string()[0..8].to_uppercase());
    sqlx::query!(
        r#"
        INSERT INTO transactions (transaction_number, appointment_id, gateway, gross_amount, platform_fee_amount, net_amount, escrow_status)
        VALUES ($1, $2, $3, $4, $5, $6, 'ESCROW_HELD')
        "#,
        txn_num,
        apt_id,
        gateway as _,
        fee,
        platform_fee,
        net_doctor_amount
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(AppointmentBookingResult {
        appointment_id: apt_id,
        appointment_number: apt_num,
        escrow_held: fee,
    })
}
```

---

## 4. Error Handling & Standard Responses

The backend maps all internal domain errors into clean HTTP responses using `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR", msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "RESOURCE_CONFLICT", msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::Database(_) | AppError::Internal(_) => {
                tracing::error!("Internal server error: {:?}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "An unexpected error occurred.".into())
            }
        };

        let body = serde_json::json!({
            "success": false,
            "error": {
                "code": code,
                "message": message
            }
        });

        (status, axum::Json(body)).into_response()
    }
}
```

---

## 5. Graceful Shutdown & Health Checks

The backend provides `/healthz` for load balancers and handles `SIGTERM` / `Ctrl+C`:

```rust
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, shutting down"),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }
}
```
