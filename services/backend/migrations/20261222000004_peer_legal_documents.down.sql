-- Acceptance evidence is immutable: a document version someone accepted is
-- retained on rollback and only stops being offered. Versions nobody has
-- accepted are removed, so re-applying the migration can insert them again.
DELETE FROM legal_documents d
WHERE d.document_type IN ('PEER_TERMS', 'PEER_PRIVACY_NOTICE', 'PEER_CONDUCT_SAFEGUARDING')
  AND NOT EXISTS (
      SELECT 1 FROM legal_acceptances a
      WHERE a.document_type = d.document_type AND a.document_version = d.version);
UPDATE legal_documents SET is_current = false
WHERE document_type IN ('PEER_TERMS', 'PEER_PRIVACY_NOTICE', 'PEER_CONDUCT_SAFEGUARDING');
