# HelloDoctor Doctor Web

This is the single Doctor Web project. It ports `prototype/index.html` and uses
the current Rust APIs through an HttpOnly cookie BFF. The Flutter app is the
functional reference; its source is not modified by this Web implementation.

Use the existing installed Node runtime and restore the project lockfile with
`npm ci`. Copy `.env.local.example` to `.env.local` and configure `BACKEND_URL`
and the exact `DOCTOR_WEB_ORIGIN`. Neither variable contains browser tokens.
Production requires HTTPS so Secure cookies can operate correctly.

Run `npm run build`, then `npm run start`. The approved development preview
listens only on `127.0.0.1:3001`. No host runtime, browser, global package,
service or object-storage infrastructure is installed by this project.

Checks: `npm run format:check`, `npm run lint`, `npm run typecheck`, `npm test`,
and `npm run build`. The safety/format check is the project's local source
checker; it is not an ESLint or Prettier installation. Browser evidence scripts
under `docs/audit_evidence/doctor_web` use the already-installed Python
Playwright package and Microsoft Edge. They never download a browser binary.

Sensitive responses remain in runtime memory and use no-store policies.
Session changes abort pending requests and clear account state. Document bytes
are reauthorized through the backend; signed storage URLs stay server-side.
The BFF refresh/revocation coordination is currently process-local. Scaling it
across workers requires the existing secure web session infrastructure to
provide shared coordination; multiworker behavior is not verified here.

Unconfigured storage is reported using the current backend response. Rx
uploads preserve failed and successful pages separately. Agora calls, real
payout-provider onboarding, notifications, articles and structured laboratory
panels must not be represented as integrated without their actual capabilities.

Build success is not visual acceptance. The acceptance and parity reports in
`docs/DOCTOR_WEB_*` must be read alongside the rendered browser evidence.
