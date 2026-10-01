# CLAUDE.md — Autonomous Engineering Operating Manual for HelloDoctor

> **Role for Claude Code:** Lead Autonomous Systems & Full-Stack Engineer  
> **Target Production Tech Stack:**  
> - **Mobile Application:** Flutter (Dart) — Cross-platform iOS, Android, and responsive tablet  
> - **Backend Core Service:** Rust (Axum, Tokio, SQLx, Tower) — High-performance, memory-safe REST & WebSocket server  
> - **Persistent Database:** PostgreSQL 16+ (Normalized schema, connection pooling, ACID transactions)  
> - **Real-Time Video & Chat:** Agora RTC SDK (`agora_rtc_engine` in Flutter + dynamic token minting in Rust) for live teleconsultation; WebSockets for 24-hour clinical chats  
> - **Reference Baseline:** HTML/CSS/JavaScript prototype located at `index.html` (and `prototype/index.html`)

---

## 1. Prime Directive & Operating Rules

1. **Read Before Writing**: You must read all documentation in [`docs/`](docs/) before making any architectural or structural decisions.
2. **Prototype is the Source of Truth**: The prototype in `index.html` represents the exact intended product behavior, screen hierarchy, and visual design. Do not redesign, simplify, or reinvent the product.
3. **Strict Technology Boundaries**:
   - The mobile application must be written in **Flutter**. Do not use web wrappers (e.g., Capacitor, Cordova, WebView shells).
   - The backend service must be written in **Rust** using **Axum** and **SQLx**.
   - Database operations must target **PostgreSQL**.
4. **No Unilateral Feature Inventions**: If a behavior is not found in the prototype or specified in the documentation, do not invent it. Follow the guidance in [`docs/OPEN_QUESTIONS.md`](docs/OPEN_QUESTIONS.md) or use documented safe defaults.
5. **Milestone Discipline**: Implement feature-by-feature following [`docs/IMPLEMENTATION_ROADMAP.md`](docs/IMPLEMENTATION_ROADMAP.md). Track progress in [`docs/PROGRESS.md`](docs/PROGRESS.md).
6. **No Phantom Completions**: Never declare a feature, milestone, or task complete if automated tests fail or if compilation errors exist.
7. **Healthcare Security & Privacy**:
   - Never store passwords in plaintext (use Argon2id).
   - Never hardcode secrets, API keys, or tokens in source code or client apps.
   - Never log sensitive Protected Health Information (PHI), patient medical complaints, or raw national identity credentials.
8. **Reversible Migrations**: All PostgreSQL schema migrations must include corresponding down-migrations. Never drop production columns destructively without deprecation windows.
9. **Autonomous Loop**:
   ```
   READ SPEC ──> INSPECT PROTOTYPE ──> READ ROADMAP ──> SELECT TASK
         │                                                    │
         ▼                                                    ▼
   UPDATE PROGRESS <── COMMIT <── RUN AUDIT <── FIX <── RUN TESTS <── IMPLEMENT
   ```

---

## 2. Core Business Rules That Must Never Be Broken

1. **Zero Star Ratings & Vanity Reviews**:
   - The platform strictly prohibits 5-star ratings, reviews, and popularity algorithms.
   - Physician qualification is represented exclusively by BMDC registration, clinical experience years, and verified hospital affiliations.
   - Patient dissatisfaction is handled exclusively via the structured **Clinical Grievance Redressal System**.
2. **Doctor Earnings 20% Debarred Settlement**:
   - Doctor earnings must always be computed and presented using the 3-step formula:
     $$\text{Gross Earnings} \longrightarrow \text{20\% Withheld Platform Charge} \longrightarrow \text{Final Net Take-Home}$$
   - The mandatory note: *"Total calculation is based including the platform charge 20%"* must be rendered on all earnings views.
3. **Multi-Prescription Upload Cap (1 to 5 Images)**:
   - Patient pre-consultation intake allows uploading between 1 and 5 prescription/lab photos (`MAX_PRESCRIPTION_IMAGES = 5`). Selections beyond 5 must be rejected or capped with user notification.
4. **Emergency Contact Helpline**:
   - The Emergency Contact banner (`Call 16263`) must be positioned immediately above the Upcoming Consultation card on the patient home screen. It must never be labeled with the prefix *"24/7"*.
5. **Bilingual Regional Localization**:
   - The patient interface must support instantaneous toggling between **English** and **Swahili (Kiswahili)**.

---

## 3. Project Directory Structure to Build

When implementing the system, Claude Code must adhere to this monorepo layout:

```
hellodoctor/
├── CLAUDE.md                           # This operating manual
├── docs/                               # Comprehensive specification package
│   ├── PROTOTYPE_INVENTORY.md
│   ├── SCREEN_INVENTORY.md
│   ├── USER_FLOWS.md
│   ├── INTERACTION_SPEC.md
│   ├── DOMAIN_MODEL.md
│   ├── DATABASE_SCHEMA.md
│   ├── API_SPEC.md
│   ├── AUTH_SECURITY.md
│   ├── FLUTTER_ARCHITECTURE.md
│   ├── RUST_BACKEND_ARCHITECTURE.md
│   ├── REALTIME.md
│   ├── UI_DESIGN_SYSTEM.md
│   ├── ASSET_SPEC.md
│   ├── STATE_MACHINES.md
│   ├── VALIDATION_RULES.md
│   ├── ERROR_HANDLING.md
│   ├── TESTING_STRATEGY.md
│   ├── ACCEPTANCE_CRITERIA.md
│   ├── VISUAL_FIDELITY.md
│   ├── REQUIREMENT_TRACEABILITY.md
│   ├── IMPLEMENTATION_ROADMAP.md
│   ├── DEFINITION_OF_DONE.md
│   ├── PROGRESS.md
│   ├── RISKS.md
│   ├── OPEN_QUESTIONS.md
│   ├── PROTOTYPE_TO_PRODUCTION_MAPPING.md
│   ├── DATA_FLOW.md
│   ├── SPECIFICATION_AUDIT.md
│   └── ADR/                            # Architecture Decision Records (001-010)
│
├── apps/
│   ├── mobile/                         # Flutter Mobile App (Patient & Doctor)
│   │   ├── lib/
│   │   │   ├── app.dart
│   │   │   ├── core/                   # Network, theme, localization, constants
│   │   │   ├── features/               # Feature-first modules (auth, booking, consult)
│   │   │   └── main.dart
│   │   ├── pubspec.yaml
│   │   └── test/
│   │
│   └── web/                            # Admin Portal & Doctor Web Workstation (Next.js / TS)
│       ├── src/
│       ├── package.json
│       └── tsconfig.json
│
├── services/
│   └── backend/                        # Rust Core Backend
│       ├── Cargo.toml
│       ├── migrations/                 # SQLx migration files (.sql)
│       └── src/
│           ├── api/                    # HTTP routes & Axum handlers
│           ├── domain/                 # Core entities, business logic, validation
│           ├── repository/             # Database access traits & SQLx implementations
│           ├── services/               # Orchestration & business services
│           ├── telemetry/              # Tracing & metrics
│           └── main.rs
│
└── index.html                          # Reference Prototype
```

---

## 4. Development Workflow & Commands

### Rust Backend
- Format code: `cargo fmt`
- Run linter: `cargo clippy --all-targets -- -D warnings`
- Run database migrations: `sqlx migrate run`
- Run test suite: `cargo test`
- Run server: `cargo run`

### Flutter Mobile
- Get packages: `flutter pub get`
- Run code generator: `dart run build_runner build --delete-conflicting-outputs`
- Analyze code: `flutter analyze`
- Run unit & widget tests: `flutter test`
- Run integration tests: `flutter test integration_test/app_test.dart`
- Run on device/simulator: `flutter run`

### Web Services (TypeScript / Next.js)
- Install dependencies: `npm install`
- Run linter: `npm run lint`
- Run tests: `npm test`
- Run development server: `npm run dev`

---

## 5. Verification Checklist Before Marking Milestones Complete

Before marking any milestone or task complete in [`docs/PROGRESS.md`](docs/PROGRESS.md), verify:
- [ ] Code compiles with zero errors and zero unhandled warnings.
- [ ] Automated unit, service, and integration tests have been written and pass 100%.
- [ ] Form inputs validate on both client (Flutter) and server (Rust).
- [ ] Database entities and migrations reflect the specification in [`docs/DATABASE_SCHEMA.md`](docs/DATABASE_SCHEMA.md).
- [ ] UI visually matches the reference prototype screens per [`docs/VISUAL_FIDELITY.md`](docs/VISUAL_FIDELITY.md).
- [ ] Acceptance criteria in [`docs/ACCEPTANCE_CRITERIA.md`](docs/ACCEPTANCE_CRITERIA.md) are fully satisfied.
- [ ] No sensitive credentials, tokens, or PII are exposed in logs or test fixtures.
