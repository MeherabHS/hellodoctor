# ADR-006: Hybrid REST and WebSocket API Communication Protocol

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, API Designer

---

## 1. Context
HelloDoctor includes transactional operations (appointment booking, payment-hold authorization, secure prescription-photo submission) as well as continuous bi-directional streams (24-hour asynchronous clinical messaging). Selecting a single transport protocol for all interactions would be suboptimal.

## 2. Decision
We adopt a **Hybrid Protocol Strategy**:
1. **REST APIs (JSON over HTTPS):** For all standard CRUD, directory queries, booking, authentication, and financial transactions.
2. **WebSockets (WSS):** Exclusively for active 24-hour consultation chat message exchange.
3. **Server-Sent Events (SSE) / Polling:** For waiting room queue countdowns and background presence checks.

## 3. Alternatives Considered
1. **Pure GraphQL:** Unified schema, but adds unnecessary query parsing overhead, complicates file uploads (multipart requests for 1–5 prescription photos), and makes edge caching and rate limiting more complex.
2. **gRPC:** High performance, but requires specialized mobile HTTP/2 proxies and complicates web browser support for the Admin Portal and Doctor Workstation.

## 4. Rationale
- **REST for Determinism:** Financial transactions, slot reservations, and prescription-photo submission require standard HTTP semantics (`Idempotency-Key`, cache-control headers, status codes `201`, `409`, `422`).
- **WebSockets Where Justified:** Live chat benefits from persistent socket framing without HTTP polling overhead. Video/audio RTC is handled entirely by Agora SDK — WebSockets are NOT used for RTC signaling.

## 5. Consequences
- **Positive:** Clear separation of transactional vs. streaming traffic; optimal battery and bandwidth consumption.
- **Trade-off:** Client must maintain both an HTTP client (`Dio`) and a WebSocket manager.
