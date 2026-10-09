# Doctor Articles - direct publication

Current approved policy: verified Doctors publish structured educational content related to their specialty themselves. **No Admin review, moderation queue or publication approval is required.** There is no slot approval workflow either.

The structured fields are title (1-160 characters), summary (1-600), article body (1-30,000) and up to ten optional references (1-500 each). Author identity and specialty come from the authenticated Doctor/profile. References and body are plain text, rendered as text; no HTML renderer, scraping, remote fetch or content-generation service is involved. Do not include identifiable patient content. Articles are educational and do not replace an individual consultation.

## Lifecycle

`DRAFT -> PUBLISHED -> ARCHIVED`, with `DRAFT -> ARCHIVED` also allowed. Doctors can edit their own drafts and published articles. Published edits appear in the feed immediately and retain the original publication date. Archived articles remain visible to their author as read-only history and cannot be republished; a new article can be written separately. No `PENDING_REVIEW` or `REJECTED` article state exists.

Only professionally VERIFIED Doctors may author/publish. Existing account suspension and session checks apply independently. Patient-facing reads include only PUBLISHED articles whose author remains professionally verified, active and not deleted. Drafts/archives do not appear in the feed. An Admin cannot impersonate an author or publish through Doctor routes.

## API and persistence

| Route under `/api/v1` | Behavior |
|---|---|
| GET/POST `/doctor/articles` | own paginated list / create private draft |
| GET/PATCH `/doctor/articles/:id` | own detail / edit with current version |
| POST `/doctor/articles/:id/publish` | direct Doctor publication with current version |
| POST `/doctor/articles/:id/archive` | archive with current version |
| GET `/articles` | authenticated published feed, limit up to 50 and offset |
| GET `/articles/:id` | authenticated published article only |

Mutation DTOs deny unknown fields, including author/status/review metadata. Creation takes `{creation_key, content}`; the key is generated internally by clients and isolated by author. Identical concurrent retries return one draft and one create audit event; changed payload reuse conflicts. Edits take `{version, content}`; publish/archive take `{version}`. Conditional writes increment the version, so stale edits and simultaneous publish/archive have one winner and defined 409 conflicts. Ownership is rechecked on every request; private cache replay cannot bypass it.

Normal SQLx migration `20261025000001_doctor_articles` adds the table and status enum. Forced RLS separates own-author writes/reads from published-only reads. A DB trigger preserves immutable identity/version rules and terminal archives. Existing five capability roles are preserved. Each mutation and its operational audit metadata commit together; publication/archive also create deduplicated existing `ARTICLE_STATUS_CHANGED` notifications in that transaction. Audit snapshots omit article text.

Doctor Web has **My Articles** in the existing sidebar. Flutter has **My Articles** in Profile & Settings. Both include empty/loading/error states, title/summary/body/reference editing, draft save, direct publish, published edits, archive, paginated lists and reload-latest conflict recovery. No Admin article screen is needed for this policy.

## Patient verification boundary

The repository still has no patient client. Published feed APIs exist and are exercised with authenticated patients. The synthetic backend journey covers OTP login -> article read -> available slot -> idempotent booking -> signed development M-Pesa callback -> Doctor completion -> patient appointment read -> grievance -> logout/access-and-refresh rejection. Fixtures open the clinical time window and resolve the consultation-session identity; this is not a complete patient UX or live provider acceptance test. No money or external M-Pesa/Agora provider is involved. Complete patient app journey/device validation remains open.

Permanent API tests cover lifecycle/content round-trip, ownership/ABAC/verification/mass-assignment, concurrent creation, stale edits, competing publication/archive, RLS, inactive-author feed exclusion, validation bounds and rollback when audit persistence fails. Client tests cover article permissions/contracts and real Flutter editor interaction with its HTTP adapter.
