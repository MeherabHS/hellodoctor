# ADR-007: Real-Time Signaling & Clinical Chat Architecture

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Real-Time Systems Specialist

---

## 1. Context
WebRTC requires an out-of-band signaling mechanism to exchange SDP offers, answers, and ICE routing candidates between patient mobile devices and physician workstations. Additionally, post-consultation 24-hour follow-up chat requires instant delivery with typing status.

## 2. Decision
We implement a lightweight **WebSocket Signaling & Chat Hub** directly inside the Rust backend using `tokio::sync::broadcast` channels, partitioned by `room_id` and `conversation_id`.

## 3. Alternatives Considered
1. **Third-Party Messaging (Pusher / PubNub):** Fast to set up, but introduces recurring per-message costs, external data residency compliance risks for healthcare data, and third-party downtime dependencies.
2. **Firebase Realtime Database / Firestore:** Proprietary lock-in; lacks compile-time verification and native integration with the Rust backend state machine.

## 4. Rationale
- **Low Memory Overhead:** Rust handles thousands of persistent WebSocket connections in a single process using Tokio green tasks with minimal RAM overhead.
- **Zero Third-Party Dependency:** Chat messages and signaling packets remain entirely within the sovereign HeloDoc cloud boundary, ensuring full compliance with health privacy regulations.
- **Direct Database Integration:** Chat messages can be validated and persisted asynchronously into PostgreSQL within the same application process.

## 5. Consequences
- **Positive:** High performance, zero per-message cost, full control over healthcare data residency.
- **Trade-off:** Multi-instance clustering requires a Redis Pub/Sub backplane when scaling horizontally across multiple server nodes.
