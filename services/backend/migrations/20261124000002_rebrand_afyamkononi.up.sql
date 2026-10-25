-- Product rebrand: HelloDoctor -> AfyaMkononi, user-facing text only
-- (display copy, notification templates, legal document content).
-- Deliberately does NOT touch any technical identifier — Postgres role
-- names (hellodoctor_api, hellodoctor_system_worker, etc.), package/bundle
-- identifiers, folder or file names all remain unchanged; renaming those is
-- a separate, much larger exercise this migration does not attempt.

-- ---------------------------------------------------------------------
-- notification_templates: the three Milestone-3-seeded rows that still
-- named the old brand (registration verified/rejected, professional
-- verification revoked — EN and SW). An ordinary UPDATE, not a new
-- version — unlike `legal_documents` below, template copy carries no
-- acceptance/audit history, so there is nothing to preserve by superseding
-- rather than editing in place.
-- ---------------------------------------------------------------------
UPDATE notification_templates
   SET title_template = replace(title_template, 'HelloDoctor', 'AfyaMkononi'),
       body_template = replace(body_template, 'HelloDoctor', 'AfyaMkononi')
 WHERE template_key IN ('registration_verified', 'registration_rejected', 'professional_verification_revoked')
   AND (title_template LIKE '%HelloDoctor%' OR body_template LIKE '%HelloDoctor%');

-- ---------------------------------------------------------------------
-- legal_documents: Terms of Service / Privacy Notice / consent text are
-- versioned, audit-sensitive content — a user's historical acceptance
-- record must keep meaning exactly what they actually read and agreed to
-- at the time. The correct way to rebrand this content is therefore a NEW
-- superseding version (exactly as any other legal-document amendment would
-- be shipped), never an in-place edit of the existing version row. Done in
-- three ordered steps because `legal_documents_one_current` is a
-- non-deferrable partial unique index per `document_type`:
--   1. Insert each new version as NOT current (no uniqueness conflict).
--   2. Mark the old HelloDoctor-named version(s) not current.
--   3. Mark the newly inserted AfyaMkononi version(s) current.
-- ---------------------------------------------------------------------
INSERT INTO legal_documents (document_type, version, title, required, is_current, body)
SELECT document_type,
       version || '-afyamkononi',
       replace(title, 'HelloDoctor', 'AfyaMkononi'),
       required,
       false,
       replace(body, 'HelloDoctor', 'AfyaMkononi')
  FROM legal_documents
 WHERE is_current = true
   AND (title LIKE '%HelloDoctor%' OR body LIKE '%HelloDoctor%');

UPDATE legal_documents
   SET is_current = false
 WHERE is_current = true
   AND (title LIKE '%HelloDoctor%' OR body LIKE '%HelloDoctor%');

UPDATE legal_documents
   SET is_current = true
 WHERE version LIKE '%-afyamkononi'
   AND (title LIKE '%AfyaMkononi%' OR body LIKE '%AfyaMkononi%');
