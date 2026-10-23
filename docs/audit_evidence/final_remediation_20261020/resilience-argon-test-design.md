# Argon2 cancellation regression design

Only `services/backend/src/crypto.rs`'s `#[cfg(test)]` module changed. `resilience-argon-source-manifest.json` records before/after hashes and equality of the production text outside that module. Argon2 parameters, production permit acquisition, blocking work and error decisions are unchanged.

Three new named cases cover hash, real password verification and dummy verification. A private current-thread Tokio runtime permits only one blocking thread. An event-controlled blocking closure occupies that thread. Three existing semaphore permits are held by the test; first polling the actual production password future acquires the fourth and queues its real work. An explicit first-poll notification establishes that state without sleeping. Aborting and awaiting the caller must leave all four permits unavailable. A fifth production hashing call must still return `CapacityLimitReached`. Only after releasing the controlled worker and observing the actual semaphore-release event may capacity return. Final capacity is strictly four.

All waits use events and 30-second error deadlines; there are no arbitrary sleeps. A release-on-drop guard prevents private-runtime shutdown from hanging on assertion failure. A test-only Tokio mutex serializes these saturation tests with the preserved original fail-fast test, including when the harness uses parallel test threads. No production mutex was introduced.

Command:

```text
cargo test --offline --locked --lib crypto::password_capacity_tests -- --test-threads=4
```

The first scratch compilation failed because the scratch source copy omitted the `sqlx::migrate!` compile-time migration directory; unchanged migrations were then copied. That failure is preserved under `resilience-argon-cancellation.*`. The corrected fixture execution is recorded under `resilience-argon-cancellation-fixture.*`; its actual exit status and named result, rather than this design document, determine execution acceptance. The shared cached Cargo target can serialize this command behind the independently running full API build. A deadline while waiting for that lock is a tooling delay, not an executed failing Argon2 assertion.

Corrected fixture result: **4 passed, zero failed, zero ignored, 69 filtered**, test runtime 1.59 seconds, command exit zero, total 122.1444252 seconds including Cargo lock wait and compilation. These four include the original fail-fast case and three new cancellation cases; they overlap root's final full-library inventory and must not be counted twice. Default test-module parallelism was explicitly exercised with four test threads.

Single-file formatting check initially found one long new test invocation; it was wrapped without logic changes. `resilience-argon-format-corrected.json` records `rustfmt --edition 2021 --check services/backend/src/crypto.rs`, exit zero. Root's final full-library run remains required to establish combined regression acceptance with all concurrent source changes.
