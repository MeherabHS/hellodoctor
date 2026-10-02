# ADR-010: Notification Engine & Offline Resilience Architecture

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Mobile Lead

---

## 1. Context
Healthcare consultations require timely reminders (e.g., 30 minutes before appointment start) even when the mobile app is terminated or in the background. Furthermore, intermittent cellular drops must not cause duplicate financial transactions or data loss.

## 2. Decision
1. **Push Notifications:** Use **Firebase Cloud Messaging (FCM)** for Android and **Apple Push Notification Service (APNs)** via an abstraction service in Rust. Push notification payloads must NEVER contain PHI (diagnosis, prescription details, lab results, complaints). Use generic messages.
2. **Offline Resilience:** Implement **Drift (local SQLite)** for caching doctor directories and health vault records, combined with an **Offline Action Guard Modal** that intercepts destructive transactions (booking, payment, signing) when `connectivity_plus` detects no network connectivity. Note: `connectivity_plus` is a UX hint, not transaction authority.
3. **Medical Images Cache:** Medical images and prescriptions must use encrypted lifecycle-controlled local storage, NOT generic disk cache (`cached_network_image`). Doctor portraits can use normal caching.

## 3. Alternatives Considered
1. **WebSocket Heartbeat Pushes:** Only works while the application is active in the foreground; fails completely when the user locks their screen or kills the app.
2. **Allowing Offline Payment Queuing:** High risk of double-debiting or booking stale slots that were taken by another user during the offline window.

## 4. Rationale
- **Reliable Push Delivery:** FCM/APNs deliver high-priority background wake-up triggers for incoming video calls and consultation countdown alerts.
- **Fail-Safe Offline Mode:** Caching read-only data (past prescriptions, doctor profiles) allows offline review, while blocking write mutations protects clinical and financial integrity.

## 5. Consequences
- **Positive:** Reliable notification delivery, minimal risk of double booking or payment drift during offline periods.
- **Trade-off:** Requires configuring APNs certificates and FCM service accounts.
