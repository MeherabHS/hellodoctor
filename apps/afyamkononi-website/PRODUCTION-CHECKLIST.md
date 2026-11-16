# AfyaMkononi Production Readiness Checklist & Scope Freeze

## 1. Final Website Scope & Full Legal Architecture

AfyaMkononi is a focused, product-facing website in Kenya. It does not have separate `/patients` or `/doctors` pages; both are substantial, interactive sections of the homepage.
The legal, compliance, policy, and support documentation from AfyaMkononi.app has been 100% migrated and preserved with zero omissions, simplifications, or templating, adapted to the approved Sauti V4 design system.

### Core Product Routes
- [x] **`/` (Home — `index.html`)**: Brand identity, Sauti V4 editorial hero, how it works, **interactive Patient App carousel**, **interactive Doctor App carousel**, trust & safety, institutional governance preview, download CTA.
- [x] **`/#patients`**: Direct deep link to Patient showcase section and interactive carousel.
- [x] **`/#doctors`**: Direct deep link to Clinician showcase section and interactive carousel.
- [x] **`/about` (`about.html`)**: Mission, health equity, language dignity (English + Kiswahili), 4-party governance structure (GHI, Circle Technology Ltd, AfyaMkononi.app, RMCL), and platform architecture.

### Dedicated Legal, Compliance & Support Routes
- [x] **`/privacy` (`privacy.html`)**: Full 27-section Kenya Data Protection Act 2019 privacy policy, 5-day temporary consultation trace rule, retention principles, and Google Play health data compliance.
- [x] **`/terms` (`terms.html`)**: Full 29-section Terms of Service, medical device disclaimer, emergency limitations, consultation booking, payments, cancellations, and Republic of Kenya governing law.
- [x] **`/doctor-terms` (`doctor-terms.html`)**: Full 23-section Doctor Terms and Professional Declaration, 4-status verification lifecycle (PENDING, VERIFIED, REJECTED, REVOKED), 20% platform fee terms, and solemn attestation under KMPDC rules.
- [x] **`/patient-consent` (`patient-consent.html` / `consent.html`)**: Full 18-section Patient Telemedicine and Health Data Consent under the Kenya Health Act 2017 & Digital Health Act 2023.
- [x] **`/data-retention` (`data-retention.html`)**: Full 11-section Data Retention and Erasure Policy including the complete 4-column Data Retention Schedule Matrix table and 5-day temporary trace rule.
- [x] **`/account-deletion` (`account-deletion.html`)**: Full 9-section Google Play-compliant public web deletion resource, in-app deletion instructions, identity verification rules, and retained data exceptions.
- [x] **`/support` (`support.html`)**: Support & Safety Hub with Emergency Casualty Warning (red alert box), technical support, patient grievances, doctor support, billing inquiries, and ODPC/KMPDC escalation.
- [x] **`/cookies` (`cookies.html`)**: Cookie & Tracking Policy detailing essential-only cookies, zero commercial advertising cookies, and zero third-party tracking pixels.
- [x] **`/contact` (`contact.html`)**: Solutions architecture inquiry directory, regional focus (South Asia • Africa), and office location.

---

## 2. Interactive App Screenshot Carousels

- [x] **Patient App Carousel (7 Genuine Flutter Screens)**:
  - 01. Home & Everyday Care (`patient-carousel-01-home.png / .webp`)
  - 02. Doctor Discovery & Specialties (`patient-carousel-02-discovery.png / .webp`)
  - 03. Doctor Profile & Licensure Transparency (`patient-carousel-03-doctor-profile.png / .webp`)
  - 04. Appointment Booking & Slot Selection (`patient-carousel-04-booking-slots.png / .webp`)
  - 05. Encrypted Clinical Chat & Attachments (`patient-carousel-05-consultation-chat.png / .webp`)
  - 06. M-Pesa Payment & Confirmation (`patient-carousel-06-payment-confirmed.png / .webp`)
  - 07. Care Continuity & Active History (`patient-carousel-07-care-continuity.png / .webp`)
- [x] **Doctor App Carousel (7 Genuine Flutter Screens)**:
  - 01. Real-time Patient Queue & Triage (`doctor-carousel-01-patient-queue.png / .webp`)
  - 02. Sovereign Schedule & Slot Controls (`doctor-carousel-02-schedule-management.png / .webp`)
  - 03. Unified Consultation Inbox (`doctor-carousel-03-consultation-inbox.png / .webp`)
  - 04. Focused Clinical Encounter Interface (`doctor-carousel-04-consultation-interface.png / .webp`)
  - 05. Smart Rx Digital Prescriptions (`doctor-carousel-05-smart-rx.png / .webp`)
  - 06. Community Health Q&A (`doctor-carousel-06-qna-advice.png / .webp`)
  - 07. Practice Wallet & Settlement Ledger (`doctor-carousel-07-earnings-wallet.png / .webp`)
- [x] **Carousel Accessibility & Interactions**:
  - Touch/swipe gesture support (smooth horizontal swipe detection)
  - Previous / Next buttons with visible focus rings
  - Keyboard navigation (Left, Right, Home, End arrows on focused phone frame)
  - ARIA attributes (`aria-roledescription="carousel"`, `role="region"`, `role="group"`, `aria-roledescription="slide"`, `aria-live="polite"`)
  - Pagination / slide-position indicators (`01 / 07` counter + active pill track)
  - Responsive layout (fluid on mobile, tablet, and desktop viewports)
  - No horizontal page overflow (`overflow-x: hidden`)
  - Reduced-motion support (`prefers-reduced-motion: reduce`)
  - No unnecessary autoplay

---

## 3. Legal Documents Parity & Technical Features

- [x] **100% Content Parity Verified**: Exact reproduction of all 147 numbered sections/subsections across the legal suite (see `CONTENT-PARITY-REPORT.md`).
- [x] **Legal Architecture Replicated**:
  - Breadcrumb with back-to-home navigation and print button (`window.print()`).
  - Document header hero with Official Public Document, Version 2.0, and Effective Date badge pills.
  - Entity disclosure box detailing the 4-party governance structure (GHI, Circle Technology Ltd, AfyaMkononi.app, RMCL) and direct inquiry contacts (Privacy, Support, DPO).
  - Sticky Table of Contents sidebar with section counter and smooth scroll navigation.
  - Interactive Mobile Table of Contents Accordion (`Jump to Section (X items)`).
  - Scrollspy active-section tracking in the TOC as the user reads.
  - Floating "Back to Top" button with smooth scroll.
  - Document ID attestation (`AM-KE-{SLUG}-v2.0`) and Republic of Kenya regulatory notice.
  - Bottom Related Documents Navigation Bar linking all governance documents.
  - Print stylesheet (`@media print`) stripping navigation/decorations for paper printing.
- [x] **Sauti V4 Visual System Applied**:
  - Newsreader display serif typography for major headings.
  - Plus Jakarta Sans / Manrope interface typography for legal prose.
  - Warm ivory background (`#fcf9f2`), forest-green accents (`#164e43`), warm sand cards, and terracotta highlights.
- [x] **Zero Broken Links**: All 964 internal links and section anchors verified valid.

---

## 4. Pending Final Commercial Sign-offs (Pre-deployment)

- [ ] Commercial licensing sign-off for Kenyan healthcare hero photography (clean editorial placeholder preserved in hero).
- [ ] Circle Technology Ltd Google Play live store listing link confirmation for the patient CTA.
- [ ] Insertion of formal ODPC Data Controller certificate registration number upon administrative issuance.
