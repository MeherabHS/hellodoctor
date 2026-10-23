# Web remediation evidence, 20 October 2026

This is owner-authorized local development verification, not a formally sandbox-certified audit. No providers were contacted, dependencies downloaded, shared Redis databases used, or production configuration changed. Baseline hashes cover 132 original web files. Seven original Admin files changed, two test-only fixture files were added, and Doctor Web source did not change. See `web-baseline.json`, `web-changes.json` and `web-minimal-fixes.patch`.

| Issue | Reproduced failure | Minimal correction | Verification |
| --- | --- | --- | --- |
| WEB-01 | JSX apostrophes: three `react/no-unescaped-entities` errors | JSX entities preserve the displayed wording | Strict ESLint: zero errors/warnings |
| WEB-02 | Toast `Date.now()` purity warning | Component-local event-driven monotonic sequence also avoids same-millisecond ID collisions | Typecheck, lint, full tests, build |
| WEB-03 | Two relative `window.location.href` warnings | Explicit same-origin absolute document navigation preserves the security boundary of discarding authenticated JS/runtime caches | Typecheck, lint, full tests, build |
| WEB-04 | Render-time callback ref writes, two warnings | Callback refs update in commit effects registered before the polling effects | Typecheck, lint, full tests, build; polling/recovery tests retained |
| WEB-05 | Two cleanup ref-capture warnings | Capture the stable epoch ref in effect scope; retain the same increment/invalidation and timer cancellation | Typecheck, lint, full tests, build |
| WEB-06 | Investigation effect reset warning | Keyed investigation subtree initializes empty/loading state for each grievance; retains active-request guard and authorized data loading | Typecheck, lint, full tests, build |
| WEB-07 | BotChallenge effect state warning | Missing-key error derives from configuration; external SDK throw reports via teardown-aware microtask; successful proof callback clears failure | Typecheck, lint, full tests, build; 11 synthetic Turnstile tests retained |
| WEB-08 | Refresh suite hardcoded shared port 6379/14 and could silently skip four cases | Test-only explicit `TEST_REDIS_URL`, literal loopback nondefault port, DB14, no URL credentials/options, and matching `hd:test-fixture:owner` marker before connections | All five original refresh cases execute against real disposable Redis; four new fixture guard tests pass |

The original Admin lint result was reproduced as three errors and nine warnings in `web-admin-lint-baseline.*`. All Admin source files in that original scratch copy match the current campaign baseline; only an unrelated Doctor README differs. The first new scratch lint attempt raced an unfinished cached dependency copy and failed to resolve `type-check`. Completing the cached copy fixed this fixture issue without installing anything. All initial failure evidence remains retained.

Final executed commands use the exact isolated source copies in `scratch/final-remediation-20261020/web/{admin-web,doctor-web}`. The command metadata files record executable, arguments, working directory, actual child exit status, duration and allowlisted environment names.

| Gate | Admin | Doctor |
| --- | --- | --- |
| `node --max-old-space-size=1536 node_modules/typescript/bin/tsc --noEmit --incremental false` | PASS, 5.931s | PASS, 4.604s |
| Actual lint | `node ... node_modules/eslint/bin/eslint.js . --max-warnings 0`: PASS, 11.171s | `node ... scripts/check-source.mjs`: PASS, 0.118s; custom repository checker, not ESLint |
| `node --max-old-space-size=1536 --experimental-strip-types --test tests/*.test.ts` | 49/49 PASS, 0 failed/skipped, 9.214s | 78/78 PASS, 0 failed/skipped, 0.714s |
| `node --max-old-space-size=1536 node_modules/next/dist/bin/next build` | PASS, 31.177s | PASS, 47.233s |

Admin's 49 named tests comprise all 45 original cases and four additional fixture-safety cases. All 11 original assertion lines in `refresh.test.ts` are byte-equivalent at line level; see `web-refresh-assertion-preservation.json`. Four conditional skip clauses were removed, no new skip was introduced, and no original test was deleted. Doctor's 78 tests are unchanged. Focused reruns, SQL operations and HTTP assertions are not added to these named-test totals.

Admin refresh fixture: owner-provisioned container `hd-remediation-redis-20261020`, Docker label `hellodoctor.remediation.run=final-20261020`, URL `redis://127.0.0.1:16380/14`, marker value `final-remediation-20261020`. This suite never uses `FLUSHDB` or `FLUSHALL`. Redis failure/missing configuration/ownership mismatch now fails the suite rather than skipping security tests or falling back to a shared service. The root coordinator owns fixture cleanup.

Turnstile tests use injected synthetic provider responses; BFF HMAC and seven recovery cases per application are unit-level evidence, not browser/provider acceptance. Admin build retains existing Next16 React18 and middleware-convention deprecation notices. Node test execution retains the existing `MODULE_TYPELESS_PACKAGE_JSON` notice. No lint rules were suppressed, no dependency upgrade was made, and no new build error/warning was introduced.
