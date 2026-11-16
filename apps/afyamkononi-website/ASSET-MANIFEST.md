# AfyaMkononi Product Asset Manifest & Screenshot Registry

This document records the provenance, capture details, dimensions, and integration destinations for all real product assets across the updated AfyaMkononi website.

---

## 1. Patient Flutter Application Screenshots (`hellodoctor_mobile_patient`)

All Patient App screenshots were captured from genuine runs of the `hellodoctor_mobile_patient` Flutter Android client under controlled synthetic testing conditions without real patient data or personal health information (PHI).

| Asset File | Format | Native Resolution | Feature / Route | Provenance & Validation |
| :--- | :--- | :--- | :--- | :--- |
| `assets/patient-carousel-01-home.png`<br>`assets/patient-carousel-01-home.webp` | PNG / WebP | 1080 &times; 2400 px | Patient Home & Dashboard | Device capture (`patient_home.png`). Displays bilingual care shortcuts, verified doctor directory entry, health advice articles, and emergency disclaimer banner. |
| `assets/patient-carousel-02-discovery.png`<br>`assets/patient-carousel-02-discovery.webp` | PNG / WebP | 1080 &times; 2400 px | Doctor Discovery | Device capture (`patient_discovery.png`). Shows specialty taxonomy (General, Paediatrics, Internal Medicine), filter chips, and practitioner cards with verified status badges. |
| `assets/patient-carousel-03-doctor-profile.png`<br>`assets/patient-carousel-03-doctor-profile.webp` | PNG / WebP | 1080 &times; 2400 px | Clinician Profile & Credentials | Device capture (`patient_doctor_profile.png`). Details KMPDC medical licensing credentials, hospital affiliations, patient reviews, and chat/video consultation booking fees. |
| `assets/patient-carousel-04-booking-slots.png`<br>`assets/patient-carousel-04-booking-slots.webp` | PNG / WebP | 1080 &times; 2400 px | Appointment Booking | Device capture (`patient_booking_slots.png`). Real-time calendar slot picker with morning, afternoon, and evening availability options. |
| `assets/patient-carousel-05-consultation-chat.png`<br>`assets/patient-carousel-05-consultation-chat.webp` | PNG / WebP | 1080 &times; 2400 px | Encrypted Clinical Chat | Device capture (`patient_chat_open.png`). Secure, end-to-end encrypted messaging session with treating physician, time-stamped clinical inquiries, and photo attachment capability. |
| `assets/patient-carousel-06-payment-confirmed.png`<br>`assets/patient-carousel-06-payment-confirmed.webp` | PNG / WebP | 1080 &times; 2400 px | M-Pesa Payment Confirmation | Device capture (`patient_payment_confirmed.png`). M-Pesa STK Push payment receipt, booking reference number, and virtual waiting room access gateway. |
| `assets/patient-carousel-07-care-continuity.png`<br>`assets/patient-carousel-07-care-continuity.webp` | PNG / WebP | 1080 &times; 2400 px | Active Consultations & Care Continuity | Device capture (`patient_active_chat_list.png`). Historical and active consultation threads, clinician follow-up notes, and longitudinal health encounter tracking. |

---

## 2. Doctor Flutter Application Screenshots (`hellodoctor_mobile`)

All Doctor App screenshots were captured from genuine runs of the `hellodoctor_mobile` Flutter Android client under controlled clinician test accounts. They contain no patient identifiers, confidential diagnostic details, or live financial credentials.

| Asset File | Format | Native Resolution | Feature / Route | Provenance & Validation |
| :--- | :--- | :--- | :--- | :--- |
| `assets/doctor-carousel-01-patient-queue.png`<br>`assets/doctor-carousel-01-patient-queue.webp` | PNG / WebP | 1080 &times; 2400 px | Clinical Triage Queue | Device capture (`doctor_chat_patient_queue.png`). Shows incoming patient queue, triage urgency indicators, intake notes, and consultation start triggers. |
| `assets/doctor-carousel-02-schedule-management.png`<br>`assets/doctor-carousel-02-schedule-management.webp` | PNG / WebP | 1080 &times; 2400 px | Schedule & Availability Controls | Device capture (`doctor_schedule_before.png`). Displays daily calendar management, slot blocking/unblocking, buffer interval settings, and calendar sync. |
| `assets/doctor-carousel-03-consultation-inbox.png`<br>`assets/doctor-carousel-03-consultation-inbox.webp` | PNG / WebP | 1080 &times; 2400 px | Consultation Inbox | Device capture (`doctor_chat_list.png`). Centralized communication inbox displaying active, queued, and archived patient dialogues with triage badges. |
| `assets/doctor-carousel-04-consultation-interface.png`<br>`assets/doctor-carousel-04-consultation-interface.webp` | PNG / WebP | 1080 &times; 2400 px | Clinician Encounter Interface | Device capture (`doctor_reply_sent.png`). Focused clinical consultation screen with structured physician response compose bar, message history, and attachment review. |
| `assets/doctor-carousel-05-smart-rx.png`<br>`assets/doctor-carousel-05-smart-rx.webp` | PNG / WebP | 1080 &times; 2400 px | Smart Rx Electronic Prescriptions | Mobile client capture (`smart_rx_390.png`). Demonstrates digital prescription authoring with medication formulary, dosage, duration, and tamper-proof electronic signing. |
| `assets/doctor-carousel-06-qna-advice.png`<br>`assets/doctor-carousel-06-qna-advice.webp` | PNG / WebP | 1080 &times; 2400 px | Community Health Q&A | Mobile client capture (`qna_390.png`). Shows verified doctor response tools for public health inquiries, educational medical content, and verified badges. |
| `assets/doctor-carousel-07-earnings-wallet.png`<br>`assets/doctor-carousel-07-earnings-wallet.webp` | PNG / WebP | 1080 &times; 2400 px | Practice Wallet & Settlements | Mobile client capture (`wallet_390.png`). Practice financial ledger showing itemized consultation fee breakdown, automated escrow status, and direct M-Pesa payout settlement. |

---

## 3. Browser Preview Assets

Generated using headless Microsoft Edge (`1440px` desktop viewport, `390px` mobile viewport):

- `previews/sauti-v4-product-home-desktop-1440.png` (Desktop Homepage with Patient & Doctor Carousels)
- `previews/sauti-v4-product-home-mobile-390.png` (Mobile Homepage view with responsive touch carousels)
- `previews/sauti-v4-about-desktop-1440.png` (Desktop About AfyaMkononi page)
- `previews/sauti-v4-support-desktop-1440.png` (Desktop Support & Safety Hub)
- `previews/sauti-v4-privacy-desktop-1440.png` (Desktop Privacy Policy with sticky TOC)
- `previews/sauti-v4-consent-desktop-1440.png` (Desktop Patient Consent page with sticky TOC)

---

## 4. Pending Production Assets for Review

The following items are designated for final organizational and commercial sign-off prior to external release:
1. **Licensed Editorial Hero Photograph**: The hero section currently features the approved visual CSS image treatment slot until final commercial licensing of the Kenyan portrait photography is secured.
2. **Production App Store Badges**: Google Play badge and download destination URLs are linked to the verified technical support and app discovery anchors pending live Play Store listing activation by Circle Technology Ltd.
3. **Formal Regulatory Numbers**: Final ODPC Registration Certificate number and KMPDC enterprise license number to be inserted into the document headers upon issuance.
