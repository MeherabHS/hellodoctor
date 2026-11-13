-- Patient education (product-owner instruction, 2026-11-13):
-- 1. An Admin article category may be linked to one of the six fixed HIV
--    Prevention & Sexual Health topics, so the Patient app can list a topic's
--    articles by a stable key instead of by text search.
-- 2. An Admin article may carry a YouTube link. Such an article is a video:
--    it goes through the same draft/review/publish workflow, appears under the
--    Videos tab rather than among articles, and opens YouTube when tapped.
-- Both columns are nullable and additive; existing rows are unaffected.

ALTER TABLE article_categories
  ADD COLUMN topic_key TEXT
  CHECK (topic_key IS NULL OR topic_key IN (
    'HIV_BASICS','PREP','CONDOMS','SRH','STIGMA_DISCLOSURE','HEALTHY_RELATIONSHIPS'));

-- The API stores only the canonical form, https://www.youtube.com/watch?v=<11-char id>.
ALTER TABLE admin_articles
  ADD COLUMN video_url TEXT
  CHECK (video_url IS NULL OR video_url ~ '^https://www\.youtube\.com/watch\?v=[A-Za-z0-9_-]{11}$');

CREATE INDEX article_categories_topic ON article_categories(topic_key) WHERE topic_key IS NOT NULL;

-- Same function as 20261127000001, with video_url added to the content fields
-- whose change must invalidate approval and schedule.
CREATE OR REPLACE FUNCTION guard_admin_article() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE changed BOOLEAN; required_review BOOLEAN;
BEGIN
 IF TG_OP='INSERT' THEN
  RETURN NEW;
 END IF;
 changed := ROW(NEW.title_en,NEW.title_sw,NEW.body_en,NEW.body_sw,NEW.category_id,NEW.tags,NEW.featured,NEW.sensitive,NEW.cover_image_id,NEW.video_url)
   IS DISTINCT FROM ROW(OLD.title_en,OLD.title_sw,OLD.body_en,OLD.body_sw,OLD.category_id,OLD.tags,OLD.featured,OLD.sensitive,OLD.cover_image_id,OLD.video_url);
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
