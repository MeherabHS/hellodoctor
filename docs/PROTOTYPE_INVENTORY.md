# Phase 1 — Complete Prototype Inventory

> **Document Version:** 1.0.0  
> **Source Baseline:** Repository Root `c:\Users\CUBE\Desktop\helodoc old site`  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Executive Summary

This inventory documents all files present in the HelloDoctor repository. Every file has been inspected to evaluate its architectural role, business logic, visual components, mock data, and obsolescence status.

Classification Labels used throughout this audit:
- `OBSERVED`: Explicitly verified in the file contents or code.
- `INFERRED`: Derived logically from system structure and integration patterns.
- `RECOMMENDED`: Action suggested for the production Flutter/Rust implementation.
- `UNKNOWN`: Insufficient evidence to determine without user confirmation.

---

## 2. Comprehensive File Inventory Table

| Filename | Path | Type | Apparent Purpose | Dependencies | Screens Affected | Important Functions | Important Data | Biz Logic? | UI Logic? | Mock Backend? | Reusable Comp? | Assets? | Obsolete/Duplicate? | Confidence |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `index.html` | `/index.html` | HTML / CSS / JS | Primary monolithic web application served by GitHub Pages. Contains all 4 shells. | FontAwesome, Google Fonts (Plus Jakarta Sans) | `view-0` to `view-14`, `doc-view-0` to `doc-view-5`, `#doctorWebShell`, `adminPage_*` | `switchScreen`, `switchDoctorTab`, `switchAdminPage`, `renderDoctorEarnings`, `submitPatientGrievance`, `handlePatientFileSelection`, etc. | `doctorCatalog`, `doctorEarningsStore`, `adminTransactionStore`, `adminPatientGrievancesStore`, `adminDoctorHistoryStore` | YES | YES | YES | YES | NO (inline SVGs) | NO (PRIMARY SOURCE OF TRUTH) | 100% |
| `prototype/index.html` | `/prototype/index.html` | HTML / CSS / JS | Legacy path copy kept synchronized with root `index.html` for backward test compatibility. | FontAwesome, Google Fonts | Same as `index.html` | Identical to `index.html` | Identical to `index.html` | YES | YES | YES | YES | NO | DUPLICATE (Mirrors `/index.html`) | 100% |
| `BUSINESS_LOGIC.md` | `/BUSINESS_LOGIC.md` | Markdown | Exhaustive functional business logic specification covering all business rules, 20% platform charge, 1-5 Rx intake, grievance adjudication, and localization. | None | All | None | Formulas, fee splits, status definitions | YES | NO | NO | NO | NO | NO (CRITICAL SPECIFICATION) | 100% |
| `css/tokens.css` | `/css/tokens.css` | CSS | Design token definitions: colors, typography scales, 8pt spacing grid, shadows, radii, animation curves. | None | Global stylesheet consumer | None | CSS custom properties (`--color-primary`, `--space-16`, etc.) | NO | YES | NO | YES | NO | NO (DESIGN TOKENS) | 100% |
| `css/base.css` | `/css/base.css` | CSS | CSS reset, base typographic rendering, accessibility defaults, utility classes. | `tokens.css` | Global | None | Standard element styling | NO | YES | NO | YES | NO | NO | 100% |
| `css/components.css` | `/css/components.css` | CSS | Reusable UI component styling: cards, buttons, pill chips, badges, input fields, modals. | `tokens.css` | Core components | None | Component class rules (`.card`, `.btn-primary`, `.badge-status`) | NO | YES | NO | YES | NO | NO | 100% |
| `css/screens.css` | `/css/screens.css` | CSS | Layout and screen-specific container styles for mobile viewports. | `components.css` | Screen views | None | Layout rules (`.screen-container`, `.header-bar`) | NO | YES | NO | YES | NO | NO | 100% |
| `js/app.js` | `/js/app.js` | JavaScript | Application controller for discrete/wellness views, toast notifications, and modal triggers. | `data.js` | Wellness / discrete modes | `toggleDiscreetMode`, `openModal`, `closeModal`, `showToast` | View state flags | YES | YES | NO | YES | NO | NO | 95% |
| `js/data.js` | `/js/data.js` | JavaScript | Data dictionary containing patient profiles, ART regimen data, doctor catalog, and masked terminology. | None | Specialist catalog, patient details | None | `AppData.patient`, `AppData.doctors`, `AppData.discreetDictionary` | YES | NO | YES | NO | NO | NO | 95% |
| `Hello Doctor [B].fig` | `/Hello Doctor [B].fig` | Figma Binary | Original vector UI design file containing all artboards, typography scales, and component designs. | Figma Application | All mobile screens | None | Visual layouts, color palettes, vector icons | NO | YES | NO | YES | YES | NO (PRIMARY UI ASSET) | 100% |
| `Hello Doctor [B].pdf` | `/Hello Doctor [B].pdf` | PDF Document | Complete multi-page vector export of original mobile app designs. | PDF Viewer | All screens | None | Screen mockups, navigation flows, visual states | NO | YES | NO | NO | YES | NO | 100% |
| `Hello Doctor [B] (1).pdf` to `(15).pdf` | Root directory | PDF Slices | Individual exported slices of Figma design screens (Patient profile, Rx intake, doctor booking, consultations). | PDF Viewer | Various screens | None | Visual specifications for individual screens | NO | YES | NO | NO | YES | NO | 100% |
| `all_pdf_texts.txt` | `/all_pdf_texts.txt` | Plain Text | Extracted textual content and annotations from design PDF exports. | None | Reference | None | Text strings, microcopy, label translations | NO | NO | NO | NO | NO | REFERENCE | 90% |
| `pdf_summary.txt` | `/pdf_summary.txt` | Text (UTF-16LE) | Summary text dump of PDF analysis. | None | Reference | None | Screen page listings | NO | NO | NO | NO | NO | DUPLICATE (Use `pdf_summary_utf8.txt`) | 85% |
| `pdf_summary_utf8.txt`| `/pdf_summary_utf8.txt`| Text (UTF-8) | UTF-8 decoded text analysis of PDF design specifications. | None | Reference | None | Color hex codes, typography specs, screen titles | NO | NO | NO | NO | NO | REFERENCE | 95% |
| `scratch/images/*.png` | `/scratch/images/` | PNG Images (68 files) | Extracted UI images, doctor portrait avatars, banners, and visual iconography from Figma design files. | Image renderers | Doctor profiles, banners, waiting room | None | Raw bitmap assets | NO | NO | NO | NO | YES | NO (BUNDLE ASSETS) | 100% |
| `scratch/thumbnail.png`| `/scratch/thumbnail.png`| PNG Image | Thumbnail preview of the HelloDoctor Figma artboards. | Image renderers | Documentation | None | Visual overview | NO | NO | NO | NO | YES | REFERENCE | 90% |
| `scratch/test_*.js` | `/scratch/` (11 files) | Node.js Test Suites | Automated regression suites validating tag balance, JS syntax, earnings math, multi-Rx uploads, and admin pages. | Node.js `fs`, `vm`, `assert` | All | `runTestSuite` | Test fixtures, expected DOM element lists | YES | NO | YES | NO | NO | NO (REGRESSION TEST SUITES) | 100% |

---

## 3. Detailed Component & Code Evaluation

### 3.1 `index.html` (Monolithic Reference Architecture)
- **Status:** `OBSERVED` Primary Source of Truth.
- **Size:** 1,119,111 bytes (2,420 balanced `<div>` tags, 2 comprehensive `<script>` blocks).
- **Embedded Portals:**
  1. `#patientAppShell`: Mobile patient viewport encompassing `view-0` through `view-14`.
  2. `#doctorAppShell`: Mobile physician viewport encompassing `doc-view-0` through `doc-view-5`.
  3. `#doctorWebShell`: Clinical desktop workstation featuring dual-pane teleconsultation and EMR.
  4. `#adminPortalShell`: 9 central administrative consoles (`command`, `doctors`, `patients`, `bmdc`, `slots`, `compliance`, `logs`, `grievances`, `finance`).

### 3.2 `css/tokens.css` (Visual Design Tokens)
- **Status:** `OBSERVED` Design Token Source.
- **Palette:**
  - Teal Primary: `#0D5C52`, Active Mint: `#14B8A6`, Primary Surface: `#E6F4F1`.
  - Terracotta Accent: `#D95D39`, Gold: `#D97706`, Emergency Red: `#DC2626`.
  - Background: `#F8F6F0`, Card Surface: `#FFFFFF`, Ink: `#182220`.
- **Typography Scale:** Plus Jakarta Sans display (28px/34px), H1 (22px/28px), H2 (18px/24px), H3 (16px/22px), Body (14px/20px), Caption (11px/15px).
- **Spacing Grid:** 8pt base scale (`--space-8` to `--space-40`).
- **Radii:** 8px, 14px, 20px, 28px, 9999px.

### 3.3 Extracted Image Assets (`scratch/images/`)
- Contains 68 distinct PNG image files extracted from Figma artboards.
- Includes doctor avatar portraits, medical equipment illustrations, service icons, and banner graphics.
- Fully cataloged in [`docs/ASSET_SPEC.md`](ASSET_SPEC.md).
