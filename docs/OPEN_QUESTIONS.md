# Phase 27 — Open Questions & Technical Ambiguities

> **Document Version:** 1.0.0  
> **Objective:** Minimize questions Claude Code needs to ask the user by establishing safe, deterministic defaults for all edge cases.  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Resolved Questions (By Prototype Inspection)

The following items were clarified by reverse-engineering `index.html` and need **NOT** be asked again:
- **Q:** *Are star ratings allowed for doctors?*  
  **A:** **NO.** All star ratings (`⭐`, `★`) and reviews were eliminated in favor of KMPDC verification and grievance adjudication.
- **Q:** *What is the doctor earnings platform charge?*  
  **A:** **Strictly 20% withheld platform charge**, displayed via a 3-tier calculation ($G \to C \to N$) with mandatory basis note.
- **Q:** *How many prescriptions can a patient upload?*  
  **A:** **1 to 20 images maximum per consultation** (`PATIENT_MAX_IMAGES_PER_CONSULTATION = 20`), each independently compressed to at most 600 KB (`MAX_PRESCRIPTION_IMAGE_BYTES = 600 * 1024`; 40 KB until 2026-11-18), never a shared/divided batch budget. See docs/PRESCRIPTION_UPLOAD_40KB_20_IMAGE_VERIFICATION.md.
- **Q:** *Where is the emergency call button placed?*  
  **A:** **Directly above Upcoming Consultation** on `view-0`, labeled "Emergency Contact" (no "24/7" prefix), dialing `16263` (or local equivalent).
- **Q:** *What languages are supported?*  
- **Q:** *What video infrastructure will be used for live teleconsultation?*  
  **A:** **Agora RTC SDK** (`agora_rtc_engine` in Flutter + dynamic token generation in Rust). No custom WebRTC server will be built.

---

## 2. Genuinely Unresolved Technical Questions

### Q-001: Third-Party MFS Gateway Provider Integration Mode
- **Question:** Will live production payments integrate directly via direct telco MFS APIs (bKash Direct Merchant API, Safaricom Daraja API for M-Pesa) or through a unified regional payment aggregator (e.g., Shurjopay, SSLCommerz, Flutterwave)?
- **Why It Matters:** Affects the webhook signature verification algorithm and callback payload schemas in the Rust backend.
- **Affected Components:** `services/backend/src/services/payment_service.rs`, `adminPage_finance`.
- **Safe Default:** Implement an abstract `PaymentGateway` trait in Rust with a direct bKash/M-Pesa simulator for development and pluggable drivers for aggregators.
- **Blocks Implementation?** **NON-BLOCKER.** Implementation can proceed using the simulator driver.
- **Decision (2026-11-14, product owner):** Finance comes later, not now. The financial module stays deferred (`financial_module_deferred`); no gateway is chosen yet.

---

### Q-003: SMS Telco Gateway Credentials for Production OTP
- **Question:** Which SMS provider credentials (e.g., Twilio, Infobip, local Kenya bulk SMS gateway) will be provisioned in the production environment?
- **Why It Matters:** Final production SMS OTP delivery requires provider API tokens.
- **Affected Components:** `services/backend/src/services/auth_service.rs`.
- **Safe Default:** Implement mock OTP logger in development mode (fixed code `584920` or printed to console logs) while providing configurable environment variables (`SMS_API_KEY`, `SMS_SENDER_ID`) for production.
- **Blocks Implementation?** **NON-BLOCKER.**
- **Decision (2026-11-14, product owner):** No SMS. No SMS provider will be provisioned.

---

### Q-NEW-1: What jurisdiction-specific medical record retention periods apply? (KE)
- **Question:** How long must clinical data be stored legally based on jurisdiction?
- **Blocks Implementation?** **NON-BLOCKER.**
- **Status (2026-11-14):** Still open for the stored records themselves: prescriptions and patient uploads are kept indefinitely and are not purged. Separately decided: a doctor can open their own consultation's prescriptions and patient uploads only until 20 days after the consultation ended (migration `20261221000002_doctor_document_20_day_window`); patients and Admins keep access.

### Q-NEW-2: Payment-hold terminology
- **Decision:** Use `PAYMENT_HELD` and payment-hold terminology throughout product, API, database, and financial operations documents. Do not describe the arrangement as legal escrow unless counsel and the payment provider explicitly approve that classification.
- **Blocks Implementation?** **NON-BLOCKER.**

### Q-NEW-3: Deployment scope
- **Decision:** Kenya only; English and Swahili. No other-country rollout is in this product scope.

### Q-NEW-4: What Agora data-region configuration is needed for KE compliance?
- **Question:** What Agora data-region configuration is needed for KE compliance?
- **Blocks Implementation?** **NON-BLOCKER.**
- **Decision (2026-11-14, product owner):** Decided by the product owner; no open question remains.

### Q-NEW-5: Product decisions recorded 2026-11-14 (product owner)
- **Patient intake uploads:** a patient can remove their own uploaded page and upload the right one while the consultation still accepts uploads (live payment hold, CONFIRMED, WAITING, IN_CONSULTATION). After that the upload stays in the record. Migration `20261221000001_patient_intake_removal`; `GET`/`DELETE /appointments/:id/prescriptions[/:document_id]`.
- **Health Records:** the reference panel's Share Access tab is removed, not shown as unavailable.
- **Reminders:** stay on the device only (encrypted local storage, no server copy); they do not survive a reinstall or a new phone. Accepted as is.
- **Doctor Web earnings note:** Doctor Web's Earnings page and Statement intentionally do not render the "Total calculation is based including the platform charge 20%" note; the Doctor Web dashboard, Doctor mobile and Admin Web still do.
- **Book Consultation wording:** "Book a Video Consultation" (slot-booked) instead of the reference's "Start Video Call" is correct.

### Q-NEW-6: Product decisions recorded 2026-11-20 (product owner)
- **Chat consultation window:** a CHAT consultation can be messaged only until its booked slot ends; afterwards the history stays readable to both parties but nobody can send, and every surface shows "Expired". A VIDEO consultation keeps its 24-hour follow-up chat. Backend `AppointmentService::chat_window_end`; existing chats moved by migration `20261222000015`. Patient copy no longer calls chat consultations "24h".
- **In-call chat:** the Patient and Care apps have a chat button on the video call (beside mute, camera and switch camera) that opens the consultation chat over the running call, as Doctor Web already did. Peer support calls open the session chat the same way.
- **Chat screens:** the conversation lists use the Peer Navigator list design; inside a chat, the existing message, paperclip and message-bar design stays, with an Active / Expired pill.
- **Offline Health Records:** documents are saved on the phone AES-256-GCM encrypted (key in secure storage), opened offline, and wiped with the key at sign-out or session expiry (`core/cache/offline_documents.dart`, 60 MB cap).
- **Incident log:** every error response, including refused 4xx requests, is recorded on the Admin incident log (category `CLIENT_REQUEST` for 4xx), grouped by route and code with ids stripped (`api/middleware/incident_reporting.rs`).
