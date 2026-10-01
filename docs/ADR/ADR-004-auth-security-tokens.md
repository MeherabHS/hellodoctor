# ADR-004: Authentication Strategy & Asymmetric Token Architecture

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Security Reviewer

---

## 1. Context
The platform must support high-volume mobile access while adhering to healthcare security standards. Patients log in primarily using mobile phone numbers and SMS OTPs, while physicians and administrators authenticate using credentials with multi-factor verification.

## 2. Decision
We adopt **RS256/Ed25519 JWT Access Tokens (15-minute lifespan)** paired with **single-use opaque Refresh Tokens (7-day lifespan)** stored in PostgreSQL/Redis and encrypted client-side using `flutter_secure_storage`.

## 3. Alternatives Considered
1. **Stateful Server-Side Sessions:** Requires every microservice/endpoint to query the database/Redis on every request, increasing latency and database load.
2. **Long-Lived JWT Tokens (24h+):** Vulnerable to replay attacks if intercepted; cannot be revoked without maintaining a global blocklist that defeats the stateless benefit.

## 4. Rationale
- **Short-Lived Access Tokens (15m):** Minimizes the exposure window if a token is intercepted on compromised client devices.
- **Asymmetric Signing:** Public keys can be cached by satellite services (e.g., WebRTC signaling server) to verify tokens without querying the central database.
- **Secure Token Rotation:** Refresh tokens are single-use; reusing a previously consumed token triggers automatic session invalidation across all user devices.

## 5. Consequences
- **Positive:** Stateless verification, fast response times, tight security posture.
- **Trade-off:** Client must handle silent token refresh via HTTP interceptors.
