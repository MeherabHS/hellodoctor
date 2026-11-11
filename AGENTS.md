# Repository instructions

Follow the user's current scope and repository-safety restrictions. Do not stage,
commit, push, reset, clean, deploy, or repair historical data without authorization.

## Mandatory cross-module regression gate

Any change affecting money, account identity, clinical information, or another
application's state requires evidence for all three gates before acceptance:

1. **Local correctness:** exercise the changed behavior, validation and authorization.
2. **System consistency:** identify every affected record, ledger/accounting effect,
   service, audit/outbox event and client contract. Verify that they agree after the
   operation. An endpoint returning success is not sufficient evidence.
3. **Failure recovery:** test relevant interruption, retry, timeout, partial failure,
   concurrency and restart paths. Prove truthful pending/failure outcomes, rollback
   or durable recovery, and absence of duplicate financial or identity effects.

Record the affected-module map, invariants, actual tests/counts and remaining gaps
in the completion report. Explain any genuinely inapplicable check. Source review
alone is not runtime proof; missing provider/device evidence must stay explicitly
unverified. A scoped application gate does not certify an untested provider or
production deployment.

Use approved disposable fixtures. Preserve RLS, ABAC, separated database roles,
encryption, audit history and production timeout/pool settings. Never weaken an
assertion or security control to obtain a pass. Coordinate long regression runs;
do not overlap suites that mutate cluster-wide roles or shared fixture state.

For financial/identity repairs, inventory historical inconsistencies read-only.
Treat historical reconciliation as a separately reviewed operation rather than an
automatic cleanup migration.

See `docs/DEFINITION_OF_DONE.md` and `docs/TESTING_STRATEGY.md` for the acceptance
matrix. Existing examples do not override current requirements or user instructions.
