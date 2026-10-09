-- Doctor-authored educational articles publish directly. There is no Admin
-- review, approval, or external registry integration in this workflow.
CREATE TYPE doctor_article_status AS ENUM ('DRAFT', 'PUBLISHED', 'ARCHIVED');
CREATE TABLE doctor_articles (
    id UUID PRIMARY KEY,
    author_id UUID NOT NULL REFERENCES doctor_profiles(id),
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 160),
    summary TEXT NOT NULL CHECK (char_length(summary) BETWEEN 1 AND 600),
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 30000),
    references_list JSONB NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(references_list) = 'array'),
    status doctor_article_status NOT NULL DEFAULT 'DRAFT',
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    creation_key TEXT NOT NULL CHECK (char_length(creation_key) BETWEEN 16 AND 64),
    creation_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at TIMESTAMPTZ,
    UNIQUE (author_id, creation_key),
    CHECK ((status = 'PUBLISHED' AND published_at IS NOT NULL) OR status <> 'PUBLISHED')
);
CREATE INDEX doctor_articles_author ON doctor_articles(author_id, created_at DESC, id);
CREATE INDEX doctor_articles_feed ON doctor_articles(published_at DESC, id) WHERE status='PUBLISHED';
ALTER TABLE doctor_articles ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_articles FORCE ROW LEVEL SECURITY;

CREATE POLICY doctor_articles_published_read ON doctor_articles FOR SELECT USING (
    status='PUBLISHED' AND current_setting('app.current_role',true) IN
      ('PATIENT','DOCTOR','PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN')
    AND EXISTS (SELECT 1 FROM doctor_profiles d JOIN users u ON u.id=d.user_id
      WHERE d.id=author_id AND d.professional_verification_status='VERIFIED'
        AND d.deleted_at IS NULL AND u.is_active AND u.deleted_at IS NULL)
);
CREATE POLICY doctor_articles_owner_read ON doctor_articles FOR SELECT USING (
    current_setting('app.current_role',true)='DOCTOR' AND EXISTS (
      SELECT 1 FROM doctor_profiles d WHERE d.id=author_id
      AND d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid)
);
CREATE POLICY doctor_articles_owner_insert ON doctor_articles FOR INSERT WITH CHECK (
    current_setting('app.current_role',true)='DOCTOR' AND status='DRAFT' AND EXISTS (
      SELECT 1 FROM doctor_profiles d JOIN users u ON u.id=d.user_id WHERE d.id=author_id
      AND d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
      AND d.professional_verification_status='VERIFIED' AND d.deleted_at IS NULL
      AND u.is_active AND u.deleted_at IS NULL)
);
CREATE POLICY doctor_articles_owner_update ON doctor_articles FOR UPDATE USING (
    current_setting('app.current_role',true)='DOCTOR' AND EXISTS (
      SELECT 1 FROM doctor_profiles d WHERE d.id=author_id
      AND d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid)
) WITH CHECK (
    current_setting('app.current_role',true)='DOCTOR' AND EXISTS (
      SELECT 1 FROM doctor_profiles d JOIN users u ON u.id=d.user_id WHERE d.id=author_id
      AND d.user_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid
      AND d.professional_verification_status='VERIFIED' AND d.deleted_at IS NULL
      AND u.is_active AND u.deleted_at IS NULL)
);

-- Keep identity immutable and archived content terminal, including direct SQL
-- through the ordinary API capability role. No sixth database role is added.
CREATE FUNCTION guard_doctor_article_update() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.id IS DISTINCT FROM OLD.id OR NEW.author_id IS DISTINCT FROM OLD.author_id
    OR NEW.creation_key IS DISTINCT FROM OLD.creation_key OR NEW.creation_hash IS DISTINCT FROM OLD.creation_hash
    OR NEW.created_at IS DISTINCT FROM OLD.created_at OR OLD.status='ARCHIVED'
    OR NEW.version <> OLD.version+1
    OR (OLD.published_at IS NOT NULL AND NEW.published_at IS DISTINCT FROM OLD.published_at)
    OR (OLD.status='PUBLISHED' AND NEW.status NOT IN ('PUBLISHED','ARCHIVED')) THEN
    RAISE EXCEPTION 'Invalid article transition or identity change' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER doctor_article_update_guard BEFORE UPDATE ON doctor_articles
FOR EACH ROW EXECUTE FUNCTION guard_doctor_article_update();
GRANT SELECT, INSERT, UPDATE ON doctor_articles TO hellodoctor_api;
