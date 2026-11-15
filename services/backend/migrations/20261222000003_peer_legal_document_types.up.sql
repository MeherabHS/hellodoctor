-- Peer Navigator legal documents (product-owner instruction 2026-11-14).
-- Separate migration: a new enum value cannot be used until commit.
ALTER TYPE legal_document_type_enum ADD VALUE IF NOT EXISTS 'PEER_TERMS';
ALTER TYPE legal_document_type_enum ADD VALUE IF NOT EXISTS 'PEER_PRIVACY_NOTICE';
ALTER TYPE legal_document_type_enum ADD VALUE IF NOT EXISTS 'PEER_CONDUCT_SAFEGUARDING';
