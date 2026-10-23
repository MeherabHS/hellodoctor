# Upload sanitizer capacity regression design

Scope: `services/backend/src/services/upload_security_service.rs` only. The existing function-local `OnceLock<Arc<Semaphore>>` initialization and clone were extracted identically to private `decoder_semaphore()`. Capacity remains four; `try_acquire_owned` still fails immediately with the same `RateLimitExceeded` error; size/MIME/magic validation still precedes capacity acquisition. The owned permit remains inside the blocking closure. Image allocation/dimension/compression limits and all PDF checks are unchanged. No runtime setting or dependency changed.

`resilience-upload-source-manifest.json` includes before/after hashes and verifies the complete original synchronous test module is unchanged. Three new cases use a real two-by-two PNG encoded through the existing image dependency:

1. Holding four decoder permits makes a valid upload fail with `RateLimitExceeded` within a 100 ms error deadline, with no queued waiter. Releasing them permits actual sanitization; the resulting PNG decodes to two-by-two, remains within the original byte ceiling, and restores all four permits.
2. Wrong MIME, invalid magic and too-short bytes return `UnprocessableEntity` while all decoder permits are held. A capacity error would fail the assertion, proving validation remains before expensive-work admission.
3. A controlled worker occupies a private runtime's only blocking thread. Three decoder permits are held; first polling the actual PNG sanitizer future acquires the fourth and queues real blocking work. An event establishes this exact state. Aborting and awaiting the caller must not release its permit; another valid upload still fails immediately. Releasing the controlled worker and awaiting the actual semaphore-release event restores capacity. Release-on-drop prevents private-runtime shutdown hangs on assertion failure. Event waits have 30-second deadlines; no arbitrary sleeps are used.

A private test-only Tokio mutex serializes the three saturation cases under parallel test execution, without affecting the original synchronous content tests or production admission.

Focused command:

```text
cargo test --offline --locked --lib services::upload_security_service::async_capacity_tests -- --test-threads=3
```

Execution status comes from `resilience-upload-capacity.json` and its logs; root's final full-library run is required for integrated acceptance. The tests use the owner-authorized ordinary local execution exception, cached dependencies and scratch source, with no network services, provider, upload flood or shared data. This proves sanitizer admission/cancellation behavior only; it does not certify HTTP multipart authorization, storage cleanup, document quota races or upload availability under attack.

Focused result: **3 passed, zero failed, zero ignored, 73 filtered** (76 total library cases discovered), harness-reported test duration 0.00 seconds, command exit zero, command duration 23.6458563 seconds including 22.85 seconds of compilation. These cases overlap root's final full-library inventory and must not be added twice. The focused compiled snapshot preceded the final formatting-only expression reflow; the final original source separately passes rustfmt and root's final full-library run will include its exact formatting.

Single-file formatting initially requested only expression reflow after the private helper extraction. `resilience-upload-format-corrected.json` records the unchanged-logic correction and successful non-mutating check.
