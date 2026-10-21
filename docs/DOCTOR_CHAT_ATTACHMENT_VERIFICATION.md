# Doctor chat ownership and attachments — 2026-10-16

Doctor Flutter and Doctor Web now render the authenticated Doctor's own `sender_id == users.id` messages green/right, and Patient messages white/left with a subtle border. Patient Flutter keeps the complementary own-green rule. Reloaded history uses sender identity, never display-name guessing or insertion order. Chat timestamps display in Nairobi time.

Doctor chat has a separate 48dp paperclip, multiline composer, accessible Send action disabled for empty input, retryable image uploads, and safe-area/keyboard protection. The Take after meals / Check fever canned advice controls were removed. Existing clinical history and prescription workflows remain accessible; previously sent messages are preserved.

## Attachment architecture and security

The existing `chat_messages` attachment columns already provide the persistence boundary; no migration or parallel document/chat system was added. The smallest extension is:

- `POST /chat/:conversation_id/attachments`: Doctor-only, verified-Doctor, consultation RLS/clinical-window authorization, active/unlocked chat, upload rate limit, required idempotency key.
- Existing `UploadSecurityService`: JPEG/PNG MIME and magic validation, bounded decoding, re-encoding/EXIF stripping and independent **40 KiB normalized image ceiling**. Only these implemented image types are offered; arbitrary PDFs, clinician-only records and structured Rx issuance are not invented.
- Existing `DocumentStorageService`: encrypted private object storage, server-generated opaque object key, integrity hash and rollback cleanup if message insertion/commit fails.
- Deterministic scoped message identity plus a transaction advisory lock: concurrent retries of one image/key create exactly one object/message. A reused key for a different normalized image conflicts. Flutter and Web retain the same retry key while a failed attempt remains pending.
- `GET /chat/:conversation_id/messages/:message_id/attachment`: request-bound authenticated/RLS authorization on every view, Doctor clinical-window recheck, no-store/private/nosniff response. Returns the authorized image bytes in JSON to existing authenticated transports; no public URL, SAS token, storage key or filesystem path is returned.
- Public chat DTOs expose only attachment ID, generic Clinical image name, MIME and byte size. Existing raw storage metadata is no longer serialized by chat endpoints.

The same authoritative message is rendered by Doctor Flutter, Doctor Web and Patient Flutter. Image viewers hold bytes only in route memory, evict decoded Flutter image providers on disposal, and do not use generic disk image caches. Native picker temporary copies are cleared after reading, on oversized selection and after route closure during selection. An upload failure leaves a retryable draft; it never adds a sent bubble.

Backend files: new `chat_attachment_handlers.rs`, existing chat handler/repository, router/module wiring. Flutter files: Doctor repository/models/chat page plus ClinicalMessageBubble/ClinicalImageViewer; Patient chat mapping/repository/page/viewer. Web files: Chat, ClinicalAttachment, contracts/BFF allowlist/timeout, and scoped chat CSS.

## Evidence and limits

Real PostgreSQL/RLS HTTP integration tests exercise production handlers/services using the repository's established test-only private in-memory ObjectStore. Four concurrent requests return the same message ID; row count remains exactly one. Doctor and Patient histories match; both authorized viewers retrieve identical sanitized bytes with no-store headers. Tests also cover unrelated Doctor/Patient denial, Patient upload denial, expired chat denial, MIME spoofing, corruption and oversized request rejection. The storage fixture is never enabled in the running development service.

The running environment still has **no configured private object storage**. Successful live/device attachment upload and persistence therefore remain unverified. Production Azure Blob wiring remains a separate deployment acceptance concern. No storage service, host software or emulator was installed and no local fake sent/upload-success state was added.

The real Edge walkthrough verified existing Patient/Doctor message history, own/incoming computed colors, right alignment, paperclip, removed canned controls and reload. [Live results](audit_evidence/existing_features_device_e2e/correction_live_web.json), [real backend chat screenshot](audit_evidence/existing_features_device_e2e/correction_doctor_web_chat.png).

The narrow Doctor widget test loads the bundled font/icons and production theme, checks 48dp targets, empty-send state, composer position above a simulated keyboard/system inset, and inspects actual rendering. Its messages are declared synthetic fixtures; these are **not physical-device screenshots**: [closed keyboard](audit_evidence/existing_features_device_e2e/correction_doctor_chat_composer_320.png), [open keyboard](audit_evidence/existing_features_device_e2e/correction_doctor_chat_keyboard_320.png).

## Status

| Required status | Result |
| --- | --- |
| DOCTOR_OWN_MESSAGE_GREEN_VERIFIED | Flutter identity/style regression + real Web |
| PATIENT_MESSAGE_WHITE_IN_DOCTOR_CHAT_VERIFIED | Flutter identity/style regression + real Web |
| DOCTOR_CHAT_HISTORY_OWNERSHIP_VERIFIED | Reload regressions + real Web history |
| DOCTOR_CHAT_ATTACHMENT_ACTION_IMPLEMENTED | Native sheet/file picker + Web paperclip/file picker |
| DOCTOR_MEDICAL_DOCUMENT_UPLOAD_VERIFIED | Backend test storage only; live storage NOT_CONFIGURED |
| CHAT_ATTACHMENT_AUTHORIZATION_VERIFIED | Real PostgreSQL/RLS HTTP tests |
| CHAT_ATTACHMENT_PERSISTENCE_VERIFIED | Backend test storage/database only; live NOT_VERIFIED |
| DOCTOR_WEB_ATTACHMENT_CONSISTENCY_VERIFIED | Shared DTO/routes implemented; live attachment upload NOT_VERIFIED |
| DOCTOR_CHAT_PHYSICAL_DEVICE_VERIFIED | DEFERRED_BY_USER |

Exact suite counts and every intentional existing UI expectation change are recorded in [the shared quality/audit report](PATIENT_PAYMENT_UX_VERIFICATION.md). Full ecosystem simulation remains on hold; video/Agora remains outside this correction pass.
