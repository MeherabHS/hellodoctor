-- Additive editorial CMS. Doctor publishing, policies and notifications are unchanged.
CREATE TABLE article_categories (
 id UUID PRIMARY KEY,
 name_en TEXT NOT NULL CHECK (char_length(btrim(name_en)) BETWEEN 1 AND 80),
 name_sw TEXT NOT NULL CHECK (char_length(btrim(name_sw)) BETWEEN 1 AND 80),
 sensitive BOOLEAN NOT NULL DEFAULT true,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX article_categories_en ON article_categories(lower(btrim(name_en)));
CREATE UNIQUE INDEX article_categories_sw ON article_categories(lower(btrim(name_sw)));
CREATE TABLE admin_articles (
 id UUID PRIMARY KEY,
 author_id UUID NOT NULL REFERENCES users(id),
 title_en TEXT NOT NULL CHECK (char_length(btrim(title_en)) BETWEEN 1 AND 160),
 title_sw TEXT NOT NULL DEFAULT '' CHECK (char_length(title_sw)<=160),
 body_en TEXT NOT NULL CHECK (char_length(btrim(body_en)) BETWEEN 1 AND 30000),
 body_sw TEXT NOT NULL DEFAULT '' CHECK (char_length(body_sw)<=30000),
 category_id UUID NOT NULL REFERENCES article_categories(id),
 tags TEXT[] NOT NULL DEFAULT '{}' CHECK (cardinality(tags)<=10),
 featured BOOLEAN NOT NULL DEFAULT false,
 sensitive BOOLEAN NOT NULL DEFAULT true,
 cover_image_id UUID,
 status TEXT NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT','PENDING_REVIEW','APPROVED','SCHEDULED','PUBLISHED','ARCHIVED')),
 version INTEGER NOT NULL DEFAULT 1 CHECK (version>0),
 content_version INTEGER NOT NULL DEFAULT 1 CHECK (content_version>0),
 approved_version INTEGER,
 approved_by UUID REFERENCES users(id),
 approved_at TIMESTAMPTZ,
 scheduled_for TIMESTAMPTZ,
 published_at TIMESTAMPTZ,
 archived_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK ((title_sw='')=(body_sw='')),
 CHECK ((approved_version IS NULL)=(approved_by IS NULL) AND (approved_by IS NULL)=(approved_at IS NULL)),
 CHECK (approved_version IS NULL OR approved_version=content_version),
 CHECK (status NOT IN ('APPROVED','SCHEDULED','PUBLISHED') OR approved_version IS NOT NULL),
 CHECK (status NOT IN ('DRAFT','PENDING_REVIEW') OR approved_version IS NULL),
 CHECK ((status='SCHEDULED')=(scheduled_for IS NOT NULL)),
 CHECK ((status='ARCHIVED')=(archived_at IS NOT NULL)),
 CHECK (status<>'PUBLISHED' OR published_at IS NOT NULL),
 CHECK (NOT sensitive OR approved_by IS NULL OR approved_by<>author_id)
);
CREATE INDEX admin_articles_feed ON admin_articles(published_at DESC,id) WHERE status='PUBLISHED';
CREATE INDEX admin_articles_list ON admin_articles(created_at DESC,id);
CREATE INDEX admin_articles_status ON admin_articles(status,created_at DESC,id);
CREATE INDEX admin_articles_category ON admin_articles(category_id,created_at DESC,id);
CREATE INDEX admin_articles_due ON admin_articles(scheduled_for,id) WHERE status='SCHEDULED';
CREATE TABLE article_media (
 id UUID PRIMARY KEY,
 article_id UUID NOT NULL REFERENCES admin_articles(id),
 object_key TEXT NOT NULL UNIQUE CHECK (object_key='article-media/'||id::text||'.jpg'),
 content_hash TEXT NOT NULL,
 size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 512000),
 ready BOOLEAN NOT NULL DEFAULT false,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(article_id,id)
);
ALTER TABLE admin_articles ADD CONSTRAINT admin_articles_cover_fk FOREIGN KEY(id,cover_image_id) REFERENCES article_media(article_id,id);
CREATE INDEX article_media_cleanup ON article_media(created_at,id);

ALTER TABLE article_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE article_categories FORCE ROW LEVEL SECURITY;
ALTER TABLE admin_articles ENABLE ROW LEVEL SECURITY;
ALTER TABLE admin_articles FORCE ROW LEVEL SECURITY;
ALTER TABLE article_media ENABLE ROW LEVEL SECURITY;
ALTER TABLE article_media FORCE ROW LEVEL SECURITY;
CREATE POLICY category_read ON article_categories FOR SELECT TO hellodoctor_api USING (current_setting('app.current_role',true) IN ('ADMIN','PATIENT','DOCTOR'));
CREATE POLICY category_insert ON article_categories FOR INSERT TO hellodoctor_api WITH CHECK (current_setting('app.current_role',true)='ADMIN');
CREATE POLICY category_update ON article_categories FOR UPDATE TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN') WITH CHECK (current_setting('app.current_role',true)='ADMIN');
CREATE POLICY category_delete ON article_categories FOR DELETE TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN');
CREATE POLICY admin_article_read ON admin_articles FOR SELECT TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN' OR (current_setting('app.current_role',true) IN ('PATIENT','DOCTOR') AND status='PUBLISHED' AND approved_version=content_version));
CREATE POLICY admin_article_insert ON admin_articles FOR INSERT TO hellodoctor_api WITH CHECK (current_setting('app.current_role',true)='ADMIN' AND author_id=NULLIF(current_setting('app.current_user_id',true),'')::uuid AND status='DRAFT' AND version=1 AND content_version=1 AND approved_version IS NULL AND cover_image_id IS NULL);
CREATE POLICY admin_article_update ON admin_articles FOR UPDATE TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN') WITH CHECK (current_setting('app.current_role',true)='ADMIN');
CREATE POLICY admin_article_worker_read ON admin_articles FOR SELECT TO hellodoctor_system_worker USING (true);
CREATE POLICY admin_article_worker_update ON admin_articles FOR UPDATE TO hellodoctor_system_worker USING (status='SCHEDULED') WITH CHECK (status='PUBLISHED');
CREATE POLICY article_media_read ON article_media FOR SELECT TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN' OR (ready AND EXISTS(SELECT 1 FROM admin_articles a WHERE a.id=article_id AND a.status='PUBLISHED' AND a.cover_image_id=article_media.id)));
CREATE POLICY article_media_insert ON article_media FOR INSERT TO hellodoctor_api WITH CHECK (current_setting('app.current_role',true)='ADMIN');
CREATE POLICY article_media_update ON article_media FOR UPDATE TO hellodoctor_api USING (current_setting('app.current_role',true)='ADMIN') WITH CHECK (current_setting('app.current_role',true)='ADMIN');
CREATE POLICY article_media_worker_read ON article_media FOR SELECT TO hellodoctor_system_worker USING (true);
CREATE POLICY article_media_worker_delete ON article_media FOR DELETE TO hellodoctor_system_worker USING (created_at<now()-interval '24 hours' AND NOT EXISTS(SELECT 1 FROM admin_articles a WHERE a.cover_image_id=article_media.id));
-- SELECT FOR UPDATE requires an UPDATE privilege/policy. Permit locking cleanup
-- candidates only; WITH CHECK(false) prohibits any actual worker UPDATE.
CREATE POLICY article_media_worker_lock ON article_media FOR UPDATE TO hellodoctor_system_worker USING (created_at<now()-interval '24 hours' AND NOT EXISTS(SELECT 1 FROM admin_articles a WHERE a.cover_image_id=article_media.id)) WITH CHECK(false);
GRANT SELECT,INSERT,UPDATE,DELETE ON article_categories TO hellodoctor_api;
GRANT SELECT,INSERT,UPDATE ON admin_articles,article_media TO hellodoctor_api;
GRANT SELECT ON admin_articles,article_media TO hellodoctor_system_worker;
GRANT UPDATE(status,version,published_at,scheduled_for,updated_at) ON admin_articles TO hellodoctor_system_worker;
GRANT DELETE ON article_media TO hellodoctor_system_worker;
GRANT UPDATE(ready) ON article_media TO hellodoctor_system_worker;
-- Scheduler audit uses the existing transaction-bound audit writer.
GRANT SELECT,INSERT ON audit_events TO hellodoctor_system_worker;

CREATE FUNCTION guard_admin_article() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE changed BOOLEAN; required_review BOOLEAN;
BEGIN
 IF TG_OP='INSERT' THEN
  RETURN NEW;
 END IF;
 changed := ROW(NEW.title_en,NEW.title_sw,NEW.body_en,NEW.body_sw,NEW.category_id,NEW.tags,NEW.featured,NEW.sensitive,NEW.cover_image_id)
   IS DISTINCT FROM ROW(OLD.title_en,OLD.title_sw,OLD.body_en,OLD.body_sw,OLD.category_id,OLD.tags,OLD.featured,OLD.sensitive,OLD.cover_image_id);
 IF NEW.id<>OLD.id OR NEW.author_id<>OLD.author_id OR NEW.created_at<>OLD.created_at OR OLD.status='ARCHIVED' OR NEW.version<>OLD.version+1 THEN
  RAISE EXCEPTION 'Invalid article identity or version' USING ERRCODE='23514';
 END IF;
 IF changed THEN
  IF NEW.content_version<>OLD.content_version+1 OR NEW.status<>'DRAFT' OR NEW.approved_version IS NOT NULL OR NEW.scheduled_for IS NOT NULL THEN
   RAISE EXCEPTION 'Edits must invalidate approval and schedule' USING ERRCODE='23514';
  END IF;
 ELSE
  IF NEW.content_version<>OLD.content_version OR NOT (
   (OLD.status='DRAFT' AND NEW.status IN ('DRAFT','PENDING_REVIEW','ARCHIVED')) OR
   (OLD.status='PENDING_REVIEW' AND NEW.status IN ('DRAFT','APPROVED','ARCHIVED')) OR
   (OLD.status='APPROVED' AND NEW.status IN ('DRAFT','PUBLISHED','SCHEDULED','ARCHIVED')) OR
   (OLD.status='SCHEDULED' AND NEW.status IN ('DRAFT','PUBLISHED','ARCHIVED')) OR
   (OLD.status='PUBLISHED' AND NEW.status IN ('DRAFT','ARCHIVED'))
  ) THEN RAISE EXCEPTION 'Invalid article transition' USING ERRCODE='23514'; END IF;
 END IF;
 IF NEW.status='APPROVED' AND OLD.status='PENDING_REVIEW' THEN
  SELECT sensitive INTO required_review FROM article_categories WHERE id=NEW.category_id;
  IF current_setting('app.current_role',true)<>'ADMIN' OR NEW.approved_by IS DISTINCT FROM NULLIF(current_setting('app.current_user_id',true),'')::uuid
   OR NEW.approved_at IS NULL OR NEW.approved_version IS DISTINCT FROM NEW.content_version OR ((NEW.sensitive OR required_review) AND NEW.approved_by=NEW.author_id) THEN
   RAISE EXCEPTION 'Independent approval required' USING ERRCODE='23514';
  END IF;
 ELSIF NEW.approved_version IS NOT NULL AND ROW(NEW.approved_version,NEW.approved_by,NEW.approved_at) IS DISTINCT FROM ROW(OLD.approved_version,OLD.approved_by,OLD.approved_at) THEN
  RAISE EXCEPTION 'Approval cannot be assigned outside review' USING ERRCODE='23514';
 END IF;
 IF NEW.status='PUBLISHED' AND OLD.status='SCHEDULED' AND NEW.published_at<OLD.scheduled_for THEN
  RAISE EXCEPTION 'Schedule is not due' USING ERRCODE='23514';
 END IF;
 IF NEW.cover_image_id IS NOT NULL AND changed AND NOT EXISTS(SELECT 1 FROM article_media WHERE id=NEW.cover_image_id AND article_id=NEW.id AND ready) THEN
  RAISE EXCEPTION 'Cover is not ready' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER admin_article_guard BEFORE INSERT OR UPDATE ON admin_articles FOR EACH ROW EXECUTE FUNCTION guard_admin_article();

-- Check all Doctor IDs without granting Admins access to Doctor drafts or altering Doctor RLS.
CREATE FUNCTION guard_admin_article_id() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,public AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM public.doctor_articles WHERE id=NEW.id) THEN
  RAISE EXCEPTION 'Article ID already exists' USING ERRCODE='23505';
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION guard_admin_article_id() FROM PUBLIC;
CREATE TRIGGER admin_article_id_guard BEFORE INSERT ON admin_articles FOR EACH ROW EXECUTE FUNCTION guard_admin_article_id();

-- Category review classification is immutable: changing it must not bypass already-reviewed content.
CREATE FUNCTION guard_article_category() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.id<>OLD.id OR NEW.created_at<>OLD.created_at OR NEW.sensitive<>OLD.sensitive THEN
  RAISE EXCEPTION 'Category identity and review requirement are immutable' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER article_category_guard BEFORE UPDATE ON article_categories FOR EACH ROW EXECUTE FUNCTION guard_article_category();
