# Phase 19 — Visual Fidelity & Pixel-Precision Audit Checklist

> **Document Version:** 1.0.1  
> **Mandate:** The Flutter mobile application must reproduce the visual layout, color palette, component geometries, and micro-interactions of the prototype with high fidelity.  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Visual Fidelity Principles

1. **No Unilateral Redesigns:** The implementation agent must not alter typography scales, spacing tokens, card layouts, or color hierarchies.
2. **Side-by-Side Verification:** For every implemented screen, run visual comparisons against the HTML prototype rendered at mobile viewport (`390x844px` — iPhone 14/15 reference frame).

---

## 2. Screen-by-Screen Visual Audit Checklist

| Screen ID | Prototype DOM View | Visual Requirement Checklist | Pass / Fail |
|---|---|---|---|
| `HD-PATIENT-HOME` | `view-0` | • Greeting with wave emoji: "Rafiq Ahmed 👋"<br/>• Core Services 2x2 grid with soft borders and icons<br/>• **Emergency Contact strip positioned immediately above Upcoming Appointments**<br/>• Strip labeled "Emergency Contact" (no "24/7" prefix)<br/>• Upcoming consultation card with doctor avatar and countdown | [ ] |
| `HD-PATIENT-SERVICES` | `view-1` | • Clean search input with grey placeholder<br/>• Full-width service cards with price badges<br/>• **Single "Book a Specialist Doctor" card under "All Available Services" (no duplicate top card)** | [ ] |
| `HD-PATIENT-FIND-SPECIALIST` | `view-3` | • Modality toggle tabs ('All', 'Video', 'Chat')<br/>• Horizontal scrolling specialty filter chips<br/>• **Doctor cards with BMDC badges, experience years, hospital name, and consultation fees**<br/>• **ZERO star symbols (⭐/★) or review ratings** | [ ] |
| `HD-PATIENT-BOOK-SLOT` | `view-4` | • Date picker carousel with active date highlighted in Deep Teal (`#0D5C52`)<br/>• 15-minute slot chips arranged in morning/afternoon/evening sections<br/>• **Multi-prescription dropzone with dashed border**<br/>• **Thumbnail gallery displaying page badges ("Page 1 of 3") and delete trash buttons**<br/>• Payment gateway radio chips with logos (bKash, Nagad, M-Pesa)<br/>• Fixed bottom checkout bar with "Confirm & Pay ৳ 800" button | [ ] |
| `HD-PATIENT-WAITING-ROOM` | `view-10` | • Live countdown timer card with circular progress arc<br/>• Queue status badge ("You are #1 in line")<br/>• **Attached prescription horizontal strip with count badge ("3 Photos Attached")**<br/>• Hardware readiness indicators (Camera & Mic Ready with green checkmarks) | [ ] |
| `HD-PATIENT-POST-CONSULT` | `view-11` | • Emerald checkmark hero icon<br/>• Digital prescription download button<br/>• **Raw telemetry card strictly hidden (display: none; aria-hidden="true")**<br/>• Discrete grievance filing button ("Having an issue? Report to Medical Administration") | [ ] |
| `HD-PATIENT-ACCOUNT-SETTINGS`| `view-7` | • Profile avatar, phone number, and national ID<br/>• Family profile dependent pills<br/>• **Language menu item displaying "Language (English / Swahili)"**<br/>• **Active badge displaying current selection ("English" or "Swahili")**<br/>• Red logout button | [ ] |
| `HD-DOC-QUEUE` | `doc-view-0` | • Doctor profile header with on-duty switch<br/>• Patient queue cards with multi-prescription count badges<br/>• Start video consultation CTA | [ ] |
| `HD-DOC-WALLET-EARNINGS` | `doc-view-3` | • **Hero card with 3-tier breakdown: Gross (৳ 35,562.50) -> 20% Fee (- ৳ 7,112.50) -> Final Net (৳ 28,450.00)**<br/>• **Mandatory italicized note: "Total calculation is based including the platform charge 20%."**<br/>• Itemized consultation ledger rows showing gross, 20% withheld, and net take-home | [ ] |
| `HD-DOCWEB-WORKSTATION` | `#doctorWebShell` | • Dual-pane desktop layout: Left 1080p Agora RTC video, Right EMR / Rx Composer<br/>• Header revenue widget: "Total Gross: ৳ 10,500 \| 20% Fee: - ৳ 2,100 \| Net Final: ৳ 8,400" | [ ] |
| `HD-ADMIN-DOCTORS` | `adminPage_doctors` | • Table columns: BMDC ID, Doctor Name, Verified Phone, Residence, Specialty, Actions<br/>• Modal `#adminDoctorHistoryModal` showing verified phone, residence, and past disciplinary logs | [ ] |
| `HD-ADMIN-GRIEVANCES` | `adminPage_grievances`| • Grievance docket showing ID, Target, Category, Telemetry Status, Actions<br/>• Adjudication buttons: "Disburse Refund" (Purple) and "Issue Warning" (Amber) (for Internal Platform Compliance) | [ ] |
| `HD-ADMIN-FINANCE` | `adminPage_finance` | • KPI cards: GMV, Escrow Reserves In-Flight, 20% Platform Revenue, Disbursed Payouts<br/>• Master transaction ledger with filter tabs and CSV export button | [ ] |
| `HD-ADMIN-LOGS` | `adminPage_logs` | • Diagnostics log viewer. Note: Failure simulator is for dev/staging only. | [ ] |
| `HD-MODAL-LANGUAGE` | `#languageModal` | • Modal dialog with English (UK flag) and Swahili (Kenya flag) options<br/>• Radio checkmarks and "Apply Language" CTA | [ ] |
| `HD-MODAL-RX-VIEWER` | `#labReportModal` | • Full-screen document preview with black overlay<br/>• Floating pagination bar at bottom ("Page 1 of 3") with Previous and Next arrows | [ ] |
| `HD-MODAL-OFFLINE-GUARD` | `#offlineActionGuardModal` | • Alert modal with disconnected WiFi icon<br/>• Explanatory text advising that internet connection is required for payments and bookings | [ ] |
