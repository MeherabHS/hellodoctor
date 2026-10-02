# ADR-007: Real-Time Clinical Chat Architecture

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Real-Time Systems Specialist

---

## 1. Context
Post-consultation 24-hour follow-up chat requires instant delivery with typing status to enable effective asynchronous communication between patient and physician.

## 2. Decision
We implement a lightweight **WebSocket Chat Hub** directly inside the Rust backend using `tokio::sync::broadcast` channels, partitioned by `conversation_id`.

## 3. Alternatives Considered
1. **Third-Party Messaging (Pusher / PubNub):** Fast to set up, but introduces recurring per-message costs, external data residency compliance risks for healthcare data, and third-party downtime dependencies.
2. **Firebase Realtime Database / Firestore:** Proprietary lock-in; lacks compile-time verification and native integration with the Rust backend state machine.

## 4. Rationale
- **Low Memory Overhead:** Rust handles thousands of persistent WebSocket connections in a single process using Tokio green tasks with minimal RAM overhead.
- **Controlled Third-Party Boundary:** Chat messages remain within the approved HeloDoc cloud boundary and are subject to documented privacy, security, and vendor-assurance controls.
- **Direct Database Integration:** Chat messages can be validated and persisted asynchronously into PostgreSQL within the same application process.

## 5. Consequences
- **Positive:** High performance, zero per-message cost, full control over healthcare data residency, reliable 24-hour auto-lock enforcement.
- **Trade-off:** Multi-instance clustering requires a Redis Pub/Sub backplane when scaling horizontally across multiple server nodes.
