# Prescription/Rx Image Storage Contract Change — 40 KB Per Image, 20 Images Per Consultation

## Old contract (what was wrong)

`services/backend/src/services/upload_security_service.rs` previously defined:

```rust
const TOTAL_BATCH_BUDGET_BYTES: usize = 400 * 1024; // 400 KB
fn per_image_target_bytes(images_in_batch: usize) -> usize {
    TOTAL_BATCH_BUDGET_BYTES / images_in_batch.max(1)
}
```

Every image in a single upload request shared one 400 KB budget, divided by how many images were
submitted together: a 1-image request got the full 400 KB, but a 5-image request got only
400 KB / 5 = 80 KB each. This was a product mistake, not a safety requirement — a patient
submitting more photos in one request should not cause each photo to be compressed harder.

The patient pre-consultation intake cap was also 5 images per appointment
(`existing_count as usize + files.len() > 5` in `prescription_service.rs`), and Doctor Web's
client-side pre-compression (`rx-compression.ts`) mirrored the old 400 KB figure.

## New contract

- **`MAX_PRESCRIPTION_IMAGE_BYTES = 40 * 1024` (40,960 bytes)** — every accepted
  prescription/intake image, Patient or Doctor, gets this fixed ceiling independently. It is never
  divided by how many images are in the same request.
- **`PATIENT_MAX_IMAGES_PER_CONSULTATION = 20`** — Patient pre-consultation intake allows up to 20
  images per appointment (was 5). The cap is read from `prescription_repo::count_intake_documents`,
  which is already scoped `WHERE appointment_id = $1` — i.e. per-consultation by construction, never
  lifetime/account-wide and never summed across consultations.
- **Resulting Patient aggregate ceiling: 20 × 40 KB = 800 KB per consultation**, by construction of
  the per-image guarantee plus the count cap — no separate runtime aggregate-byte check was added or
  is needed.
- **Doctor's Rx page-count cap stays at 5 pages per appointment** (`count >= 5` in
  `prescription_service.rs::upload_doctor_prescription_photo`) — explicitly out of scope and
  unchanged. Only the Doctor's per-image compression target moved from the old divided-batch-of-1
  value to the same fixed 40 KB.

## Bug found during verification: a database-level CHECK constraint also capped the count at 5

The application-level cap in `prescription_service.rs` was only half the enforcement. The initial
schema migration (`20261001000001_initial_schema.up.sql`) also defines, independently:

```sql
page_number SMALLINT NOT NULL CHECK (page_number >= 1 AND page_number <= 5),
```

on `prescription_intake_documents`. Raising only the Rust-level comparison left this DB constraint
at the old value, so any insert past page 5 failed with a raw Postgres `23514` check-violation,
surfacing to callers as an unhandled `500` rather than the intended `422`. Caught by the full
serialized integration run, not by the unit tests (which never touch Postgres) or by `cargo
check`/`clippy` (which can't see a database constraint). Fixed with a new paired migration,
`20261030000001_prescription_intake_20_image_cap.{up,down}.sql`, which drops and recreates
`prescription_intake_documents_page_number_check` as `page_number BETWEEN 1 AND 20`. The Doctor's
own separate `prescriptions.page_number` CHECK (`20261014000001_rx_pages_document_authorization`,
`BETWEEN 1 AND 5`) was deliberately left untouched — confirmed by direct read, not assumption, since
it belongs to a different table backing the Doctor's own unrelated 5-page cap.

## Test-design issue found and fixed: an unrelated pre-existing rate limiter

The first version of `test_twenty_document_cap_enforced_under_concurrent_uploads` sent 25 concurrent
*single-file* requests, expecting 20 successes. It failed (`left: 5, right: 20`) even after the
constraint fix above, because `prescription_handlers.rs`'s upload endpoints already call
`rate_limit::check_upload_limit` — a **pre-existing, unrelated** fixed-window limiter of **10 upload
API calls per 10 minutes per user** (`src/rate_limit.rs`). That limiter counts *requests*, not
*images*, and was never in tension with the old 5-image cap (5 single-file requests is under 10).
At the new 20-image cap, a one-request-per-image test design collides with it. This is not a
production bug — a real client can submit up to 20 images in one batched request, well inside the
10-request limit — it was only a flaw in that specific test's shape. Fixed by batching 5 files per
concurrent request (8 concurrent batches, well under the 10-request limit) so exactly 4 of 8
batches succeed (4 × 5 = 20, the exact cap) regardless of race order, which both avoids the
unrelated rate limiter and gives a deterministic expected outcome.

## Backend enforcement

- `services/backend/src/services/upload_security_service.rs`: `sanitize_async`/
  `validate_and_sanitize` no longer take an `images_in_batch` parameter; every image compresses
  toward `MAX_PRESCRIPTION_IMAGE_BYTES` via the existing iterative JPEG-quality-then-downscale loop
  (`compress_to_target`), which is a hard guarantee (never rejects for being hard to compress; the
  only floors are the pre-existing technical minimums — JPEG quality 1, 8px long edge — which exist
  to avoid a degenerate encoder call, not to protect legibility).
- `services/backend/src/services/prescription_service.rs`: `PATIENT_MAX_IMAGES_PER_CONSULTATION = 20`
  constant added; `upload_patient_intake_documents`'s count check now compares against it instead of
  the literal `5`; both call sites to `sanitize_async` (patient batch loop and the doctor's single-photo
  path) dropped the removed 3rd argument. The doctor's own `if count >= 5` page check is untouched.
- Backend remains authoritative regardless of any client-side claim about size — the client-side
  compression in Doctor Web (below) is a transfer-size optimization only, not a trust boundary.

## Concurrency protection (preserved, re-verified against the new cap)

`appointment_repo::lock_for_document_upload` takes a `pg_advisory_xact_lock` keyed on the
appointment's UUID, held for the duration of the transaction that does count-check-then-insert. This
was already correct before this change (serializes the whole read-count/compare/insert sequence per
appointment) and needed no code change — only the numeric comparison inside the locked section moved
from `5` to `PATIENT_MAX_IMAGES_PER_CONSULTATION` (20). Re-verified under the new cap with an updated
concurrency regression test (8 concurrent 5-file-batch uploads against one appointment must yield
exactly 4 successful batches / 20 stored documents, never more — see the rate-limiter note above for
why batches of 5, not 20 single-file requests).

## Client-side compression (Doctor Web)

- `apps/doctor-web/src/lib/rx-compression.ts`: `RX_PAGE_MAX_BYTES` changed from `400 * 1024` to
  `40 * 1024`; the "unable to fit" error message text updated to say 40 KB.
- `apps/doctor-web/src/components/Rx.tsx`: the three "400 KB" UI copy strings (compressing-progress
  message, the `PHOTO_TOO_LARGE` client guard message, and the help text under the file picker)
  updated to say 40 KB. The Doctor's own "Select 1–5 ... photos" page-count copy is unchanged (that
  describes the unrelated, unchanged 5-page cap).
- Doctor Flutter has no client-side compression to update — it sends raw bytes; all compression is
  backend-side (confirmed by repo-wide grep against `apps/mobile/lib/features/doctor`).
- No Patient Flutter code exists in this repo (`apps/mobile/lib/features` contains only `auth`,
  `doctor`, `notifications`, `settings`) — nothing to update there.

## Consultation scoping

`prescription_repo::count_intake_documents` is `SELECT COUNT(*) FROM prescription_intake_documents
WHERE appointment_id = $1` — scoped to one appointment (= one consultation) by construction. A
second, fresh appointment for the same patient starts at count 0 regardless of how many images a
different appointment already holds; this is proven directly by
`test_prescription_image_count_cap_is_isolated_per_consultation`.

## Storage behavior

The per-image compression guarantee (iterative JPEG-quality-then-downscale, or PNG downscale-only)
was already architected to always hit its target rather than ever risk rejecting an upload for being
hard to compress — this is unchanged behavior, just re-pointed at the new 40 KB figure instead of the
old divided-batch figure. No code path exists that would silently store an image over budget; the
unit tests below exercise this against both a realistic document-photo fixture and a deliberately
worst-case incompressible-noise fixture.

## Tests added/updated

Backend unit tests (`services/backend/src/services/upload_security_service.rs`, `#[cfg(test)] mod
tests`), all rewritten off the old batch-division semantics onto the new fixed-per-image semantics:
- `single_image_uses_close_to_its_full_budget_not_needlessly_over_compressed`
- `five_images_each_independently_fit_the_40kb_ceiling_not_a_divided_share` — proves each of 5
  images independently fits 40 KB AND that their summed total exceeds a single image's budget
  (disproving any batch-wide division).
- `large_png_is_downscaled_and_compressed_toward_its_own_40kb_budget`
- `small_image_is_not_needlessly_degraded`
- `worst_case_noise_jpeg_never_exceeds_the_guaranteed_per_image_ceiling`
- `worst_case_noise_png_never_exceeds_its_budget`
- Plus 4 unchanged PDF-structure tests (`accepts_clean_pdf`, `rejects_pdf_with_javascript`,
  `rejects_pdf_with_openaction`, `rejects_encrypted_pdf`) updated only to drop the removed 3rd
  argument.

Backend integration tests (`services/backend/tests/api_tests.rs`):
- `test_multi_prescription_intake_and_20_photo_limit` (replaces
  `test_multi_prescription_intake_and_5_photo_limit`): uploads 15 then 5 more (lands exactly on the
  20-image boundary, must succeed, asserted via a direct DB count), then one more (21st) must be
  rejected (422) with the stored count still exactly 20 (no partial persistence from the rejected
  call).
- `test_twenty_document_cap_enforced_under_concurrent_uploads` (replaces
  `test_five_document_cap_enforced_under_concurrent_uploads`): 8 concurrent 5-file-batch uploads
  against one fresh appointment must yield exactly 4 successful batches (20 images) and a final
  stored count of exactly 20, the other 4 batches rejected (422) — the same race the original
  5-image regression test guarded, re-proven at the new cap. Batched rather than one-request-per-image
  specifically to stay under the separate, unrelated `check_upload_limit` rate limiter (10 requests /
  10 minutes per user) — see the dedicated note above.
- `test_prescription_image_count_cap_is_isolated_per_consultation` (new): fills one appointment to
  its full 20-image cap, then proves a second, unrelated appointment for the same patient starts
  fresh at 0 and accepts an upload.
- `test_doctor_rx_pages_encrypted_download_retry_and_expired_grant` (Doctor's own 5-page cap test):
  confirmed unaffected and left unchanged, per the explicit out-of-scope instruction on the Doctor
  page-count cap.

Doctor Web tests (`apps/doctor-web/tests/rx-compression.test.ts`): updated the hard-coded
`409600`/`409601`/`200000` byte literals and the `/400 KB/` string-match assertions to use the
`RX_PAGE_MAX_BYTES` constant (now 40,960) and `/40 KB/`, and renamed the boundary test's title.

## Actual test results

- Backend unit tests (`cargo test --offline --lib upload_security_service`): **10/10 passed**
  (115.78s — the tighter 40 KB budget requires more downscale-and-retry iterations under the
  worst-case-noise fixtures than the old 400 KB budget did, so these two tests are noticeably slower
  than before; still well within normal `cargo test` timeouts).
- Backend full library check (`cargo check --offline --lib`): clean, zero errors.
- Backend test-binary check (`cargo check --offline --tests`): clean, zero errors.
- Backend full serialized integration suite
  (`cargo test --offline --test api_tests -- --test-threads=1`): first full run surfaced the 3
  prescription-related failures described above (149 tests: 146 passed, 3 failed). After the
  migration fix and the concurrency test rewrite, a second full run showed those 3 now passing but
  surfaced 5 *different* failures, all in tests unrelated to prescriptions
  (`test_doctor_duty_credentials_and_modality_contract`,
  `test_health_check_reports_current_migration_state`,
  `test_professional_review_migration_backfill_uses_latest_evidence_and_conservative_pending`,
  `test_security_refresh_migration_revokes_legacy_rotated_families`,
  `test_slot_block_payment_lock_protection`), all with the identical error: `sqlx-core`'s
  `#[sqlx::test]` harness failed to apply migrations against its own fresh ephemeral database with a
  raw Postgres `XX000 tuple concurrently updated` error from `heapam.c`/`simple_heap_update` — a
  race in the test harness's own template-database cloning, not application code. Re-run individually:
  4 of the 5 passed immediately in isolation, and the 5th
  (`test_doctor_duty_credentials_and_modality_contract`) passed cleanly on a single retry. None of
  the 5 touch prescriptions, images, or the migration added in this change, so this is classified as
  pre-existing test-infrastructure flakiness, not a regression — stated plainly rather than hidden.
  A third full run was started to see whether a single pass can come back 149/149 clean; its actual
  result is recorded at the end of this document rather than assumed.
- Doctor Web (`npm test`): **44/44 passed** after the rewrite (1 test initially failed —
  `quality exhaustion causes resizing...` — because its mock encoder's fixed `200000`-byte stand-in
  no longer fit under the new 40,960-byte budget; fixed by expressing it relative to
  `RX_PAGE_MAX_BYTES` instead of a hard-coded literal).
- Doctor Web typecheck (`npm run typecheck`): clean.
- Doctor Web lint/format check (`npm run lint`): clean.

## Scope audit (what was and wasn't touched)

- `services/backend/src/services/upload_security_service.rs`, `prescription_service.rs` — changed.
- `services/backend/migrations/20261030000001_prescription_intake_20_image_cap.{up,down}.sql` — new
  paired migration widening the `prescription_intake_documents_page_number_check` CHECK constraint
  from 5 to 20 (see the dedicated bug note above). The dev backend's own long-running `cargo run`
  process, if one is active, must be restarted before this migration is trusted against it, per the
  project's standing migration-discipline rule (`CLAUDE.md` §4) — `#[sqlx::test]`'s ephemeral
  per-test databases already pick it up automatically and need no such restart.
- `services/backend/tests/api_tests.rs` — the two affected tests rewritten, one new test added; the
  Doctor 5-page test confirmed unaffected.
- `apps/doctor-web/src/lib/rx-compression.ts`, `src/components/Rx.tsx`, `tests/rx-compression.test.ts`
  — changed.
- `CLAUDE.md` §2 item 3, `docs/VALIDATION_RULES.md` (Prescription Photos matrix row + §2.1 narrative),
  `docs/OPEN_QUESTIONS.md` — updated as live product-rule specifications.
- `docs/SCREEN_INVENTORY.md`, `docs/USER_FLOWS.md` — these literally catalog `index.html`'s own
  prototype source code (which still hard-codes `MAX_PRESCRIPTION_IMAGES = 5` and was **not**
  modified, since the prototype file itself was out of scope for this change). Rather than falsely
  claim the prototype now says 20, a superseding note was added next to each reference, following the
  same convention already used elsewhere in this doc set (e.g.
  `docs/DOCTOR_WEB_PROTOTYPE_INVENTORY.md`'s "Kenya correction" note) for exactly this situation.
- Historical/dated reports (`docs/CHECKPOINT.md`, `docs/PROGRESS.md`,
  `docs/DOCTOR_WEB_FINAL_ACCEPTANCE.md`, `docs/DOCTOR_WEB_RX_UPLOAD_REPORT.md`,
  `docs/DOCTOR_WEB_PROTOTYPE_INVENTORY.md`) were deliberately left untouched — they record what was
  actually built/tested/accepted at a specific past date under the old 400 KB/5-image contract, and
  rewriting them after the fact would misrepresent that history. Future work describing the current
  state should cite this document instead.
- Apps/mobile (Flutter): no Patient feature code exists in this repo, and Doctor Flutter has no
  client-side compression — confirmed by direct grep, nothing to change.

## Final status

- `PRESCRIPTION_40KB_PER_IMAGE_CONTRACT_IMPLEMENTED` — proven by the passing backend unit tests
  above, which directly assert the 40 KB (`MAX_PRESCRIPTION_IMAGE_BYTES`) ceiling applies per image
  and is never divided by batch size.
- `PATIENT_20_IMAGE_PER_CONSULTATION_CAP_IMPLEMENTED` — proven by
  `test_multi_prescription_intake_and_20_photo_limit`'s passing 20-boundary/21-rejection assertions.
- `PATIENT_800KB_PER_CONSULTATION_AGGREGATE_CAP_IMPLEMENTED` — holds by construction (20 × 40 KB) of
  the two items above; no separate runtime aggregate check exists or was needed, consistent with the
  per-image + count-cap design.
- `CONCURRENT_IMAGE_CAP_ENFORCEMENT_VERIFIED` — proven by
  `test_twenty_document_cap_enforced_under_concurrent_uploads` (8 concurrent 5-file batches → exactly
  4 succeed / 20 stored) and `test_prescription_image_count_cap_is_isolated_per_consultation`.
- `DOCTOR_RX_40KB_COMPRESSION_VERIFIED` — proven by the Doctor single-photo call site in
  `prescription_service.rs` now sharing the same `MAX_PRESCRIPTION_IMAGE_BYTES` path as the patient
  side (backend), and by the passing Doctor Web `rx-compression.test.ts`/`npm run lint`/
  `npm run typecheck` runs (client-side pre-compression).

**Final confirmed result**: a clean, isolated, single run of
`cargo test --offline --test api_tests -- --test-threads=1` (no concurrent invocation this time —
two earlier attempts were invalidated when overlapping invocations against the same Postgres
instance were mistakenly killed mid-run) completed in 1069.44s with
**149 passed; 0 failed; 0 ignored**. This is the authoritative result for the whole prescription
contract change, superseding the earlier partial/interrupted runs described above.
