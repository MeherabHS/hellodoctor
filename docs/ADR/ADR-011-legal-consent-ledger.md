# ADR-011: Doctor Registration Legal & Professional Agreements Consent Gate

> **Status:** APPROVED
> **Date:** 2026-10-27
> **Deciders:** Principal Systems Architect, Compliance Reviewer

---

## 1. Context
Kenya ODPC guidance requires informed, specific notice before processing
health data, including disclosure of cross-border/cloud processing, and
recognizes multiple lawful bases for processing (not only consent).
Doctor registration (`auth_handlers::doctor_register`) had no legal
acknowledgement step at all — no Terms, Privacy Notice, professional
verification declaration, or telemedicine/confidentiality declaration was
ever presented or recorded. A single `accepted_terms = true` boolean would
not establish *which version* was accepted, *when*, by *which canonical
identity*, or *what text was actually shown* — exactly the evidence a
regulator, court, or internal compliance review would ask for.

## 2. Decision
We adopt a **server-owned document registry** (`legal_documents`) paired with
an **immutable, append-only acceptance ledger** (`legal_acceptances`),
enforced atomically inside the existing `doctor_register` transaction.

- `legal_documents` holds the current and historical text/version for five
  document types (`DOCTOR_TERMS`, `PRIVACY_NOTICE`,
  `PROFESSIONAL_VERIFICATION_DECLARATION`,
  `TELEMEDICINE_PRACTITIONER_POLICY`, optional `MARKETING_COMMUNICATIONS`),
  with at most one `is_current` row per type. `document_hash` is recomputed
  by Postgres itself (a trigger, since a strictly-`IMMUTABLE` generated
  column can't call `convert_to()`) — never supplied by application code.
- `legal_acceptances` records one row per document a Doctor actually
  accepted: canonical `users.id`, document type/version/hash, timestamp, and
  request context. No role is ever granted `UPDATE` or `DELETE` on this
  table — a changed document version produces a new row, never a mutation of
  an old one.
- The client submits only `{type, version}` pairs, never a hash. The server
  independently re-validates that every currently required type is present
  with its current version (inline in the handler, matching how that handler
  already checks duplicate license/phone/email), and a `BEFORE INSERT`
  trigger on `legal_acceptances` re-validates currency again at the database
  layer and overwrites the hash regardless of what was supplied — a direct
  API call cannot bypass either layer.

## 3. Alternatives Considered
1. **A boolean `accepted_terms` flag on `doctor_profiles`:** Rejected — tells
   you nothing about which version or text was presented, cannot represent
   per-document acceptance, and is trivially overwritten (not an evidence
   trail).
2. **A generated `document_hash` column:** Rejected — Postgres requires
   generated-column expressions to be strictly `IMMUTABLE`; `convert_to()` is
   only `STABLE`. A `BEFORE INSERT/UPDATE` trigger gives the identical
   guarantee (server-computed, never app-supplied) without that restriction.
3. **Client-supplied document hash:** Rejected — defeats the purpose of
   server-independent validation; the server must be the sole source of
   truth for what text a given version actually contains.
4. **Forcing re-consent at next login when a document version is bumped:**
   Deferred, not rejected — the data model (`is_current` + the immutable
   ledger) fully supports detecting staleness, but the login-time
   blocking screen for already-registered doctors is a separate feature,
   out of scope for this pass.

## 4. Rationale
- **Atomicity:** acceptance rows are written inside the same
  `auth_tx` transaction as the `users`/`doctor_profiles`/`doctor_wallets`
  inserts — the account and its acknowledgements land together or not at
  all.
- **RLS:** a Doctor reads only their own acceptance rows; Compliance/
  PlatformAdmin/ClinicalAdmin read any Doctor's as evidence
  (`abac::Action::ViewLegalAcceptances`). Only `hellodoctor_auth_worker` may
  `INSERT`, and only for the authenticating subject itself.
- **No star-rating/marketing conflation:** `MARKETING_COMMUNICATIONS` is a
  separate, optional, independently-presented document type — never bundled
  into the mandatory set.

## 5. Consequences
- **Positive:** immutable, versioned, queryable evidence of exactly what a
  Doctor agreed to and when; server-independent enforcement that a direct
  API call cannot bypass; a registry doctors and future features (e.g. a
  login-time re-consent gate) can build on without re-architecting.
- **Trade-off:** registration now requires an extra round trip
  (`GET /legal/documents`) before submission, and the Flutter registration
  flow gained one additional screen between the existing form and account
  creation.
- **Deferred:** login-time forced re-consent on a version bump, and a
  dedicated marketing-consent revoke endpoint, are not built yet — tracked as
  follow-up work, not silently dropped.
