# HelloDoctor final system defect log — 20 October 2026

## Current campaign findings

No new application defect was confirmed during the current cross-platform and backend rerun. The only current-run failures were deliberate fixture/configuration guards: the first API invocation omitted the required owned Redis endpoint, and the full API run omitted the synthetic Admin assertion secret. Both guards behaved correctly; the exact HMAC case passed when its test secret was supplied.

The earlier confirmed production defect remains the admission-capacity sizing correction in `services/backend/src/repository/mod.rs`, already present before this run. Its four explicit A/B benchmarks passed at the current source revision without changing fixed per-lane ceilings.

| Issue | Severity | Root cause | Fix/result |
|---|---|---|---|
| Admission capacity sized from one role pool | Medium | capacity calculation ignored the five independent role pools | corrected before this run; A/B benchmarks 4/4 PASS |
| Admin refresh tests unsafe without fixture | Safety guard | missing explicit Redis endpoint/owner marker | no application change; all five refresh tests PASS with owned fixture |
| API security suite missing assertion secret | Test configuration | full process did not export the required synthetic BFF secret | no application change; focused test PASS |
| 19,750 historical transport errors | Unresolved operational finding | retained producer rows lack OS exception/phase data | no source change; classification remains INCONCLUSIVE |

## Change-control invariants

`EXISTING_TESTS_DELETED=0`, `EXISTING_ASSERTIONS_WEAKENED=0`, `NEW_UNEXPLAINED_SKIPS=0`, `SECURITY_MIDDLEWARE_DISABLED=0`, `RLS_DISABLED=0`, `FINANCIAL_INVARIANTS_WEAKENED=0`, `PRODUCTION_RATE_LIMITS_RELAXED_TO_PASS_TESTS=0`, `SHARED_RESOURCES_MODIFIED=0`, and `PRODUCTION_RESOURCES_MODIFIED=0`.

Historical remediation details remain in [FINAL_DEFECT_REMEDIATION_LOG.md](FINAL_DEFECT_REMEDIATION_LOG.md); this report does not replace them.
