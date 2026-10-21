# Clinical Chat security and retained history

## History and pagination

The initial backend page contains the latest 50 messages, returned chronologically. Older pages use the stable `(sent_at,id)` cursor, not OFFSET. Newer pages catch up from the last known cursor. Both Flutter clients and Doctor Web merge by authoritative message ID, preserving already loaded older pages while polling. New messages arriving during an older-page request are retained. Clients render latest messages at the bottom and expose earlier history without an arbitrary larger message cap.

## Replay and notifications

The logical send identity is authenticated sender + conversation + hashed idempotency key. A transaction advisory lock and unique database constraint serialize duplicate requests. Same key/same message returns the committed logical message; key reuse with different content returns 409. Identical intentional text with different keys remains valid. Clients retain the key through uncertain network failure and issue a new key for a new successful send intent.

Message, notification intent and accepted trace are committed together. Notification identity is derived from message + recipient, so replay does not duplicate it. Notification text contains no clinical message content. Existing Notification Center, foreground/background best-effort sync and local notifications are retained; there is no external push service or guaranteed OS delivery claim.

## Exact lock and isolation

`clock_timestamp() < expires_at` is required at database mutation time, including after waiting for locks. At equality/after expiry, inserts fail. The application check, restrictive RLS and BEFORE INSERT trigger provide layered enforcement. Maintenance persists locked_at/reason and CHAT_LOCKED. Read access remains available to the Patient/guardian and assigned treating Doctor for their own encounter; ADMIN has deliberate audited privileged access. Chat attachment uploads are also denied after lock.

The same Patient's Doctor A and Doctor B encounters remain isolated by appointment/conversation/document/Rx context. Request DTOs cannot supply trusted sender identity. Unknown privileged acknowledgement fields are rejected. Read acknowledgements are per authenticated account and message; repeated acknowledgements cannot overwrite the first server timestamp. Read evidence is client display acknowledgement, not proof of human comprehension.

## Attachments and Rx

The Doctor paperclip uses the existing JPEG/PNG clinical upload path, private encrypted DocumentStorageService and bounded UploadSecurityService decoder. Existing MIME/magic/decode/dimension/metadata removal and normalized 40 KiB rules remain. Raw object keys/credentials and public URLs are not returned. Authorized retrieval records grant/access and actual backend retrieval separately. History reads survive Chat expiry. A failed upload does not create a successful sent bubble; uncertain COMMIT does not destructively delete possibly committed ciphertext.

The Doctor's own persisted sender identity renders green/right; Patient messages render white/left. Patient applies the same ownership principle to their own outgoing messages. No display-name or insertion-order inference is used.

## Tests and limitations

Added integration tests cover eight simultaneous same-key sends, conflicting reuse, same text/different keys, atomic privacy-safe notification counts, 200-message pages, first read acknowledgement, request/session correlation, a transaction crossing expiry, retained own history, same-Patient/two-Doctor denial and trace cleanup isolation. Added Flutter tests cover pagination during concurrent arrival and retry-key preservation. Existing attachment/Rx/BOLA tests remain intact.

Final gate counts and test-diff evidence are in `audit_evidence/lifecycle_milestone/`. Physical send/receive, keyboard screenshots and local OS delivery for this new build remain pending a connected phone. Existing earlier device observations are historical evidence only. Azure integration and real external payment are not verified. Video/Agora is deferred.

Text and image sends now share the same sender/conversation key namespace; reusing a key across content types returns 409 and cannot create another message or notification. The retained four-way attachment concurrency test caught a nested-connection timeout in the new trace wiring. Production now commits the authorization-stage trace before decoding, then opens the final mutation transaction and revalidates DB wall time. Request timeout, pool size and four concurrent requests were not relaxed. The focused 21-test rerun passes.

Doctor Web retains a selected historical encounter context beyond the first 100 operational rows, so reopening older Chat history preserves the authorized Patient name and action context. Two additional Web mapping regressions pass.

## Final boundary corrections

An active paid CHAT encounter authorizes its attachment mutation through the actual Chat window; it does not inherit the Video pre-slot treatment gate. VIDEO retains its existing gate. Text and attachment sends share the same replay-key namespace. A database-level attachment expiry rejection maps to the existing 403 business denial, without a false sent message or infrastructure error. The four-way concurrent upload test retains the original request count, pool size and request timeout.

Legacy prescription retrieval grants lacking an authenticated session are rejected. The regression separately checks actual grant TTL, revoked issuing session and missing-session denial; retained own-encounter access does not override those controls.

Doctor Flutter and Web hide mutation controls in read-only historical Chat. Timestamp merging compares real instants, including offset equivalence and PostgreSQL microseconds, so whole-second messages and fractional timestamps retain their chronology. Admin timeline ordering also retains sub-millisecond precision. The focused final four-test backend boundary run passed with 231 unrelated cases filtered; it is supplemental evidence, not a substitute for the unfiltered gate.

Doctor Web has a dedicated clinical outgoing-message color token. Switching the app's sage, nordic or lavender theme cannot turn outgoing clinical messages blue/purple; rendered browser checks require the same green and incoming white in all three themes.


## Subsequent API traffic correction (2026-10-16)

See [API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md](API_TRAFFIC_CACHE_AND_POLLING_VERIFICATION.md) for route/background polling, in-flight coalescing, bounded pagination, selected-day Schedule reads and the separate post-milestone test/device evidence. Earlier counts and screenshots above remain historical evidence for their recorded revision. No new physical-device claim is implied.
