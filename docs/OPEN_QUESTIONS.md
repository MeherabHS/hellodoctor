# Phase 27 — Open Questions & Technical Ambiguities

> **Document Version:** 1.0.0  
> **Objective:** Minimize questions Claude Code needs to ask the user by establishing safe, deterministic defaults for all edge cases.  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Resolved Questions (By Prototype Inspection)

The following items were clarified by reverse-engineering `index.html` and need **NOT** be asked again:
- **Q:** *Are star ratings allowed for doctors?*  
  **A:** **NO.** All star ratings (`⭐`, `★`) and reviews were eliminated in favor of BMDC verification and grievance adjudication.
- **Q:** *What is the doctor earnings platform charge?*  
  **A:** **Strictly 20% withheld platform charge**, displayed via a 3-tier calculation ($G \to C \to N$) with mandatory basis note.
- **Q:** *How many prescriptions can a patient upload?*  
  **A:** **1 to 5 images maximum** (`MAX_PRESCRIPTION_IMAGES = 5`).
- **Q:** *Where is the emergency call button placed?*  
  **A:** **Directly above Upcoming Consultation** on `view-0`, labeled "Emergency Contact" (no "24/7" prefix), dialing `16263`.
- **Q:** *What languages are supported?*  
  **A:** **English (`en`)** and **Swahili (`sw`)**.

---

## 2. Genuinely Unresolved Technical Questions

### Q-001: Third-Party MFS Gateway Provider Integration Mode
- **Question:** Will live production payments integrate directly via direct telco MFS APIs (bKash Direct Merchant API, Safaricom Daraja API for M-Pesa) or through a unified regional payment aggregator (e.g., Shurjopay, SSLCommerz, Flutterwave)?
- **Why It Matters:** Affects the webhook signature verification algorithm and callback payload schemas in the Rust backend.
- **Affected Components:** `services/backend/src/services/payment_service.rs`, `adminPage_finance`.
- **Safe Default:** Implement an abstract `PaymentGateway` trait in Rust with a direct bKash/M-Pesa simulator for development and pluggable drivers for aggregators.
- **Blocks Implementation?** **NON-BLOCKER.** Implementation can proceed using the simulator driver.

---

### Q-002: LiveKit SFU vs. Self-Hosted Coturn for WebRTC Relays
- **Question:** Does the production deployment have dedicated compute infrastructure to host a LiveKit SFU cluster, or should 1-on-1 consultations use standard P2P with a managed STUN/TURN traversal relay?
- **Why It Matters:** Dictates whether the backend embeds the LiveKit server-side token generation SDK or simple STUN/TURN credential issuance.
- **Affected Components:** `services/backend/src/api/v1/telehealth_handlers.rs`, `apps/mobile/lib/features/consultation/`.
- **Safe Default:** Implement standard WebRTC P2P with STUN/TURN credentials (compatible with `coturn` and Twilio Network Traversal).
- **Blocks Implementation?** **NON-BLOCKER.**

---

### Q-003: SMS Telco Gateway Credentials for Production OTP
- **Question:** Which SMS provider credentials (e.g., Twilio, Infobip, local Bangladesh/Kenya bulk SMS gateway) will be provisioned in the production environment?
- **Why It Matters:** Final production SMS OTP delivery requires provider API tokens.
- **Affected Components:** `services/backend/src/services/auth_service.rs`.
- **Safe Default:** Implement mock OTP logger in development mode (fixed code `584920` or printed to console logs) while providing configurable environment variables (`SMS_API_KEY`, `SMS_SENDER_ID`) for production.
- **Blocks Implementation?** **NON-BLOCKER.**
