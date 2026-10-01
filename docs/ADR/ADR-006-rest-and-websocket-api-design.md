# ADR-006: Hybrid REST and WebSocket API Communication Protocol

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, API Designer

---

## 1. Context
HelloDoctor includes transactional operations (appointment booking, payment escrow authorization, prescription signing) as well as continuous bi-directional streams (WebRTC call signaling, 24-hour asynchronous clinical messaging). Selecting a single transport protocol for all interactions would be suboptimal.

## 2. Decision
We adopt a **Hybrid Protocol Strategy**:
1. **REST APIs (JSON over HTTPS):** For all standard CRUD, directory queries, booking, authentication, and financial transactions.
2. **WebSockets (WSS):** Exclusively for real-time WebRTC signaling and active 24-hour consultation chat message exchange.
3. **Server-Sent Events (SSE) / Polling:** For waiting room queue countdowns and background presence checks.

## 3. Alternatives Considered
1. **Pure GraphQL:** Unified schema, but adds unnecessary query parsing overhead, complicates file uploads (multipart requests for 1–5 prescription photos), and makes edge caching and rate limiting more complex.
2. **gRPC:** High performance, but requires specialized mobile HTTP/2 proxies and complicates web browser support for the Admin Portal and Doctor Workstation.

## 4. Rationale
- **REST for Determinism:** Financial transactions, slot reservations, and prescription signing require standard HTTP semantics (`Idempotency-Key`, cache-control headers, status codes `201`, `409`, `422`).
- **WebSockets Where Justified:** Low-latency signaling and live chat benefit from persistent socket framing without HTTP polling overhead.

## 5. Consequences
- **Positive:** Clear separation of transactional vs. streaming traffic; optimal battery and bandwidth consumption.
- **Trade-off:** Client must maintain both an HTTP client (`Dio`) and a WebSocket manager.
