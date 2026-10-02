# ADR-004: Authentication Strategy & Asymmetric Token Architecture

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Security Reviewer

---

## 1. Context
The platform must support high-volume mobile access while adhering to healthcare security standards. Patients log in primarily using mobile phone numbers and SMS OTP-only (no MFA), while physicians and administrators authenticate using credentials with true multi-factor verification (MFA).

## 2. Decision
We adopt **Ed25519 JWT Access Tokens (15-minute lifespan)** paired with **single-use opaque Refresh Tokens (7-day lifespan)** stored in PostgreSQL/Redis and encrypted client-side using `flutter_secure_storage`. Sessions are tracked in the `auth_sessions` table with a `token_family_id`.

## 3. Alternatives Considered
1. **Stateful Server-Side Sessions:** Requires every microservice/endpoint to query the database/Redis on every request, increasing latency and database load.
2. **Long-Lived JWT Tokens (24h+):** Vulnerable to replay attacks if intercepted; cannot be revoked without maintaining a global blocklist that defeats the stateless benefit.

## 4. Rationale
- **Short-Lived Access Tokens (15m):** Minimizes the exposure window if a token is intercepted on compromised client devices.
- **Asymmetric Signing (Ed25519):** Public keys can be cached by satellite services to verify tokens without querying the central database. Key generation uses `ed25519-dalek`. Keys are securely stored and rotated, leveraging the `kid` (Key ID) header in JWTs to identify the active key.
- **Secure Token Rotation & Reuse Detection:** Refresh tokens are single-use. The `auth_sessions` table tracks `token_family_id`. If a previously consumed token is reused (indicating a potential breach or stolen token), the backend detects the reuse and triggers automatic session invalidation across all user devices for that family.

## 5. Consequences
- **Positive:** Stateless verification, fast response times, tight security posture.
- **Trade-off:** Client must handle silent token refresh via HTTP interceptors.
