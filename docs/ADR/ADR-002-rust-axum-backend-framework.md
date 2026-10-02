# ADR-002: Rust & Axum for Core Backend Telehealth Service

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Backend Lead

---

## 1. Context
The backend service must handle concurrent Agora token minting and telemetry ingestion, high-frequency teleconsultation telemetry streams, financial escrow transactions, and multi-file medical image ingestion without memory leaks or CPU bottlenecks. Healthcare software mandates extreme reliability, zero crash tolerance, and strict data confidentiality.

## 2. Decision
We choose **Rust** as the backend language and **Axum** (built on `tokio`, `tower`, and `hyper`) as the asynchronous HTTP and WebSocket framework.

## 3. Alternatives Considered
1. **Node.js / Express or NestJS:** Fast prototyping, but single-threaded event loop can suffer during intensive cryptographic operations (HMAC-SHA256 prescription integrity verification) and high-concurrency WebSocket signaling.
2. **Go / Gin or Fiber:** Excellent concurrency and fast compile times, but Go's garbage collector introduces latency spikes, and absence of an expressive type system (enums with payloads, algebraic data types) increases domain modeling errors.
3. **Actix-web (Rust):** Highly capable, but Axum provides tighter integration with the official `tokio` ecosystem and cleaner macro-free ergonomics.

## 4. Rationale
- **Memory Safety Without Garbage Collection:** Rust guarantees elimination of null pointer exceptions, data races, and use-after-free bugs at compile time.
- **Predictable Performance:** Low-latency route handling and minimal memory footprint (~25-50 MB RAM under load).
- **Type-Safe Domain Modeling:** Enums with payloads represent state machines (`AppointmentStatus`, `EscrowStatus`) deterministically.
- **Agora Token Service:** Axum backend is responsible for minting short-lived secure Agora Dynamic RTC Tokens.

## 5. Consequences
- **Positive:** Maximum throughput, minimal hosting costs, near-zero runtime crash potential, compile-time verified database queries with `sqlx`.
- **Trade-off:** Slower initial compile times and stricter borrow-checker discipline.
