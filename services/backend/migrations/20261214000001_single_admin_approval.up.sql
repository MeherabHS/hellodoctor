-- Product-owner instruction, 2026-11-13: no second-Admin approval anywhere in
-- content publishing. Any Admin may approve and publish content they wrote,
-- including sensitive health content and quiz revisions. The submit ->
-- approve -> publish steps, version checks and audit trail are unchanged; the
-- `sensitive` flags remain as content classification only.

-- 1. Article table: drop "sensitive articles cannot be approved by their author".
DO $$
DECLARE c TEXT;
BEGIN
 FOR c IN SELECT conname FROM pg_constraint
   WHERE conrelid = 'admin_articles'::regclass AND contype = 'c'
     AND pg_get_constraintdef(oid) ILIKE '%approved_by <> author_id%'
 LOOP
  EXECUTE format('ALTER TABLE admin_articles DROP CONSTRAINT %I', c);
 END LOOP;
END $$;

-- 2. Article guard: same as 20261211000001 without the independent-approval clause.
CREATE OR REPLACE FUNCTION guard_admin_article() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE changed BOOLEAN;
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
  IF current_setting('app.current_role',true)<>'ADMIN' OR NEW.approved_by IS DISTINCT FROM NULLIF(current_setting('app.current_user_id',true),'')::uuid
   OR NEW.approved_at IS NULL OR NEW.approved_version IS DISTINCT FROM NEW.content_version THEN
   RAISE EXCEPTION 'Approval must be recorded by the approving Admin' USING ERRCODE='23514';
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

-- 3. Quiz revision guard: same as 20261128000001 without "a sensitive quiz
--    cannot be approved by its creator or last editor".
CREATE OR REPLACE FUNCTION guard_article_quiz_revision() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE parent_version INTEGER; parent_status TEXT; actor UUID;
BEGIN
 actor:=NULLIF(current_setting('app.current_user_id',true),'')::uuid;
 IF current_setting('app.current_role',true) IS DISTINCT FROM 'ADMIN' THEN RAISE EXCEPTION 'Admin required' USING ERRCODE='23514'; END IF;
 SELECT a.content_version,a.status INTO parent_version,parent_status FROM admin_articles a WHERE a.id=NEW.article_id FOR UPDATE OF a;
 IF parent_status IS NULL OR parent_status='ARCHIVED' THEN RAISE EXCEPTION 'Article unavailable' USING ERRCODE='23514'; END IF;
 IF TG_OP='INSERT' THEN
  IF NEW.status<>'DRAFT' OR NEW.version<>1 OR NEW.content_version<>1 OR NEW.article_content_version<>parent_version OR NEW.created_by IS DISTINCT FROM actor OR NEW.edited_by IS DISTINCT FROM actor THEN RAISE EXCEPTION 'Invalid quiz draft' USING ERRCODE='23514'; END IF;
  RETURN NEW;
 END IF;
 IF ROW(NEW.id,NEW.article_id,NEW.created_by,NEW.created_at) IS DISTINCT FROM ROW(OLD.id,OLD.article_id,OLD.created_by,OLD.created_at) OR OLD.status='PUBLISHED' OR NEW.version<>OLD.version+1 THEN RAISE EXCEPTION 'Immutable quiz or stale version' USING ERRCODE='23514'; END IF;
 IF NEW.content_version=OLD.content_version+1 THEN
  IF NEW.status<>'DRAFT' OR NEW.approved_version IS NOT NULL OR NEW.edited_by IS DISTINCT FROM actor OR NEW.article_content_version<>parent_version THEN RAISE EXCEPTION 'Quiz edits must invalidate approval' USING ERRCODE='23514'; END IF;
 ELSE
  IF ROW(NEW.content_version,NEW.include_quiz,NEW.article_content_version,NEW.edited_by) IS DISTINCT FROM ROW(OLD.content_version,OLD.include_quiz,OLD.article_content_version,OLD.edited_by) OR NOT(
   (OLD.status='DRAFT' AND NEW.status='PENDING_REVIEW') OR
   (OLD.status='PENDING_REVIEW' AND NEW.status IN ('DRAFT','APPROVED')) OR
   (OLD.status='APPROVED' AND NEW.status IN ('DRAFT','PUBLISHED'))
  ) THEN RAISE EXCEPTION 'Invalid quiz transition' USING ERRCODE='23514'; END IF;
 END IF;
 IF NEW.status IN ('PENDING_REVIEW','APPROVED','PUBLISHED') THEN
  IF NEW.article_content_version<>parent_version OR (NEW.include_quiz AND NOT article_quiz_complete(NEW.id)) THEN RAISE EXCEPTION 'Incomplete quiz or stale article version' USING ERRCODE='23514'; END IF;
 END IF;
 IF NEW.status='APPROVED' AND OLD.status='PENDING_REVIEW' THEN
  IF NEW.approved_by IS DISTINCT FROM actor OR NEW.approved_version IS DISTINCT FROM NEW.content_version OR NEW.approved_at IS NULL THEN RAISE EXCEPTION 'Approval must be recorded by the approving Admin' USING ERRCODE='23514'; END IF;
 ELSIF NEW.approved_version IS NOT NULL AND ROW(NEW.approved_version,NEW.approved_by,NEW.approved_at) IS DISTINCT FROM ROW(OLD.approved_version,OLD.approved_by,OLD.approved_at) THEN RAISE EXCEPTION 'Approval can only be assigned during review' USING ERRCODE='23514'; END IF;
 IF NEW.status='PUBLISHED' AND parent_status<>'PUBLISHED' THEN RAISE EXCEPTION 'Publish the approved article first' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
