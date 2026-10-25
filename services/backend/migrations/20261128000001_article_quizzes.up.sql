-- Optional, independently reviewed assessment revisions. Existing article lifecycle is unchanged.
CREATE TABLE article_quiz_revisions (
 id UUID PRIMARY KEY,
 article_id UUID NOT NULL REFERENCES admin_articles(id),
 created_by UUID NOT NULL REFERENCES users(id),
 edited_by UUID NOT NULL REFERENCES users(id),
 include_quiz BOOLEAN NOT NULL DEFAULT false,
 status TEXT NOT NULL DEFAULT 'DRAFT' CHECK(status IN ('DRAFT','PENDING_REVIEW','APPROVED','PUBLISHED')),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0),
 content_version INTEGER NOT NULL DEFAULT 1 CHECK(content_version>0),
 article_content_version INTEGER NOT NULL CHECK(article_content_version>0),
 approved_version INTEGER,
 approved_by UUID REFERENCES users(id),
 approved_at TIMESTAMPTZ,
 published_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(article_id,id),
 CHECK((approved_version IS NULL)=(approved_by IS NULL) AND (approved_by IS NULL)=(approved_at IS NULL)),
 CHECK(approved_version IS NULL OR approved_version=content_version),
 CHECK((status IN ('APPROVED','PUBLISHED'))=(approved_version IS NOT NULL)),
 CHECK((status='PUBLISHED')=(published_at IS NOT NULL))
);
CREATE UNIQUE INDEX article_quiz_one_draft ON article_quiz_revisions(article_id) WHERE status<>'PUBLISHED';
CREATE TABLE article_quiz_questions (
 revision_id UUID NOT NULL REFERENCES article_quiz_revisions(id),
 id UUID NOT NULL,
 position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 49),
 text_en TEXT NOT NULL CHECK(char_length(text_en)<=2000),
 text_sw TEXT NOT NULL DEFAULT '' CHECK(char_length(text_sw)<=2000),
 explanation_en TEXT NOT NULL DEFAULT '' CHECK(char_length(explanation_en)<=4000),
 explanation_sw TEXT NOT NULL DEFAULT '' CHECK(char_length(explanation_sw)<=4000),
 correct_option_id UUID,
 PRIMARY KEY(revision_id,id),
 UNIQUE(revision_id,position) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE article_quiz_options (
 revision_id UUID NOT NULL,
 question_id UUID NOT NULL,
 id UUID NOT NULL,
 position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 5),
 text_en TEXT NOT NULL CHECK(char_length(text_en)<=500),
 text_sw TEXT NOT NULL DEFAULT '' CHECK(char_length(text_sw)<=500),
 PRIMARY KEY(revision_id,id),
 UNIQUE(revision_id,question_id,id),
 UNIQUE(revision_id,question_id,position) DEFERRABLE INITIALLY DEFERRED,
 FOREIGN KEY(revision_id,question_id) REFERENCES article_quiz_questions(revision_id,id)
);
ALTER TABLE article_quiz_questions ADD CONSTRAINT article_quiz_answer_fk
 FOREIGN KEY(revision_id,id,correct_option_id) REFERENCES article_quiz_options(revision_id,question_id,id)
 DEFERRABLE INITIALLY DEFERRED;
-- Deliberately contains no questions, explanations or answer keys.
CREATE TABLE article_quiz_publications (
 article_id UUID PRIMARY KEY REFERENCES admin_articles(id),
 revision_id UUID NOT NULL,
 article_content_version INTEGER NOT NULL,
 included BOOLEAN NOT NULL,
 question_count INTEGER NOT NULL CHECK(question_count BETWEEN 0 AND 50),
 schema_version INTEGER NOT NULL DEFAULT 1 CHECK(schema_version=1),
 FOREIGN KEY(article_id,revision_id) REFERENCES article_quiz_revisions(article_id,id),
 CHECK((included AND question_count>0) OR (NOT included AND question_count=0))
);

ALTER TABLE article_quiz_revisions ENABLE ROW LEVEL SECURITY;
ALTER TABLE article_quiz_revisions FORCE ROW LEVEL SECURITY;
ALTER TABLE article_quiz_questions ENABLE ROW LEVEL SECURITY;
ALTER TABLE article_quiz_questions FORCE ROW LEVEL SECURITY;
ALTER TABLE article_quiz_options ENABLE ROW LEVEL SECURITY;
ALTER TABLE article_quiz_options FORCE ROW LEVEL SECURITY;
ALTER TABLE article_quiz_publications ENABLE ROW LEVEL SECURITY;
ALTER TABLE article_quiz_publications FORCE ROW LEVEL SECURITY;
CREATE POLICY quiz_revision_admin ON article_quiz_revisions FOR ALL TO hellodoctor_api USING(current_setting('app.current_role',true)='ADMIN') WITH CHECK(current_setting('app.current_role',true)='ADMIN');
CREATE POLICY quiz_question_admin ON article_quiz_questions FOR ALL TO hellodoctor_api USING(current_setting('app.current_role',true)='ADMIN') WITH CHECK(current_setting('app.current_role',true)='ADMIN');
CREATE POLICY quiz_option_admin ON article_quiz_options FOR ALL TO hellodoctor_api USING(current_setting('app.current_role',true)='ADMIN') WITH CHECK(current_setting('app.current_role',true)='ADMIN');
CREATE POLICY quiz_publication_admin ON article_quiz_publications FOR ALL TO hellodoctor_api USING(current_setting('app.current_role',true)='ADMIN') WITH CHECK(current_setting('app.current_role',true)='ADMIN');
CREATE POLICY quiz_publication_read ON article_quiz_publications FOR SELECT TO hellodoctor_api USING(current_setting('app.current_role',true) IN ('PATIENT','DOCTOR') AND EXISTS(SELECT 1 FROM admin_articles a WHERE a.id=article_id AND a.status='PUBLISHED' AND a.approved_version=a.content_version AND a.content_version=article_content_version));
GRANT SELECT,INSERT,UPDATE ON article_quiz_revisions,article_quiz_publications TO hellodoctor_api;
GRANT SELECT,INSERT,UPDATE,DELETE ON article_quiz_questions,article_quiz_options TO hellodoctor_api;

CREATE FUNCTION article_quiz_complete(revision UUID) RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
 SELECT (SELECT count(*) BETWEEN 1 AND 50 FROM article_quiz_questions WHERE revision_id=revision)
 AND NOT EXISTS(SELECT 1 FROM article_quiz_questions q WHERE q.revision_id=revision AND (
 btrim(q.text_en)='' OR btrim(q.explanation_en)='' OR q.correct_option_id IS NULL
 OR (SELECT count(*) FROM article_quiz_options o WHERE o.revision_id=q.revision_id AND o.question_id=q.id) NOT BETWEEN 2 AND 6
 OR EXISTS(SELECT 1 FROM article_quiz_options o WHERE o.revision_id=q.revision_id AND o.question_id=q.id AND btrim(o.text_en)='')
 OR (btrim(q.text_sw)='')<>(btrim(q.explanation_sw)='')
 OR EXISTS(SELECT 1 FROM article_quiz_options o WHERE o.revision_id=q.revision_id AND o.question_id=q.id AND (btrim(q.text_sw)='')<>(btrim(o.text_sw)=''))));
$$;
CREATE FUNCTION guard_article_quiz_revision() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE parent_version INTEGER; parent_status TEXT; sensitive BOOLEAN; article_author UUID; actor UUID;
BEGIN
 actor:=NULLIF(current_setting('app.current_user_id',true),'')::uuid;
 IF current_setting('app.current_role',true) IS DISTINCT FROM 'ADMIN' THEN RAISE EXCEPTION 'Admin required' USING ERRCODE='23514'; END IF;
 SELECT a.content_version,a.status,(a.sensitive OR c.sensitive),a.author_id INTO parent_version,parent_status,sensitive,article_author FROM admin_articles a JOIN article_categories c ON c.id=a.category_id WHERE a.id=NEW.article_id FOR UPDATE OF a;
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
  IF NEW.approved_by IS DISTINCT FROM actor OR NEW.approved_version IS DISTINCT FROM NEW.content_version OR NEW.approved_at IS NULL OR (sensitive AND actor IN (NEW.created_by,NEW.edited_by)) THEN RAISE EXCEPTION 'Independent approval required' USING ERRCODE='23514'; END IF;
 ELSIF NEW.approved_version IS NOT NULL AND ROW(NEW.approved_version,NEW.approved_by,NEW.approved_at) IS DISTINCT FROM ROW(OLD.approved_version,OLD.approved_by,OLD.approved_at) THEN RAISE EXCEPTION 'Approval can only be assigned during review' USING ERRCODE='23514'; END IF;
 IF NEW.status='PUBLISHED' AND parent_status<>'PUBLISHED' THEN RAISE EXCEPTION 'Publish the approved article first' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER article_quiz_revision_guard BEFORE INSERT OR UPDATE ON article_quiz_revisions FOR EACH ROW EXECUTE FUNCTION guard_article_quiz_revision();

CREATE FUNCTION guard_article_quiz_child() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE revision UUID; state TEXT;
BEGIN
 revision:=CASE WHEN TG_OP='DELETE' THEN OLD.revision_id ELSE NEW.revision_id END;
 IF current_setting('app.current_role',true) IS DISTINCT FROM 'ADMIN' THEN RAISE EXCEPTION 'Admin required' USING ERRCODE='42501'; END IF;
 IF TG_OP='UPDATE' AND NEW.revision_id<>OLD.revision_id THEN RAISE EXCEPTION 'Revision ownership is immutable' USING ERRCODE='23514'; END IF;
 SELECT status INTO state FROM article_quiz_revisions WHERE id=revision FOR UPDATE;
 IF state IS DISTINCT FROM 'DRAFT' THEN RAISE EXCEPTION 'Only draft questions can change' USING ERRCODE='23514'; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER article_quiz_question_guard BEFORE INSERT OR UPDATE OR DELETE ON article_quiz_questions FOR EACH ROW EXECUTE FUNCTION guard_article_quiz_child();
CREATE TRIGGER article_quiz_option_guard BEFORE INSERT OR UPDATE OR DELETE ON article_quiz_options FOR EACH ROW EXECUTE FUNCTION guard_article_quiz_child();
CREATE FUNCTION check_article_quiz_order() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE revision UUID;
BEGIN
 revision:=CASE WHEN TG_OP='DELETE' THEN OLD.revision_id ELSE NEW.revision_id END;
 IF EXISTS(SELECT 1 FROM article_quiz_questions WHERE revision_id=revision HAVING count(*)>50 OR min(position)<>0 OR max(position)<>count(*)-1)
 OR EXISTS(SELECT 1 FROM article_quiz_options WHERE revision_id=revision GROUP BY question_id HAVING count(*)>6 OR min(position)<>0 OR max(position)<>count(*)-1) THEN RAISE EXCEPTION 'Invalid question or option order/count' USING ERRCODE='23514'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER article_quiz_question_order AFTER INSERT OR UPDATE OR DELETE ON article_quiz_questions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_article_quiz_order();
CREATE CONSTRAINT TRIGGER article_quiz_option_order AFTER INSERT OR UPDATE OR DELETE ON article_quiz_options DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_article_quiz_order();
CREATE FUNCTION guard_article_quiz_publication() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM article_quiz_revisions r JOIN admin_articles a ON a.id=r.article_id WHERE r.id=NEW.revision_id AND r.article_id=NEW.article_id AND r.status='PUBLISHED' AND r.approved_version=r.content_version AND r.article_content_version=a.content_version AND a.status='PUBLISHED' AND a.approved_version=a.content_version AND NEW.article_content_version=r.article_content_version AND NEW.included=r.include_quiz AND NEW.question_count=CASE WHEN r.include_quiz THEN (SELECT count(*) FROM article_quiz_questions WHERE revision_id=r.id) ELSE 0 END) THEN RAISE EXCEPTION 'Only the approved quiz revision may be published' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER article_quiz_publication_guard BEFORE INSERT OR UPDATE ON article_quiz_publications FOR EACH ROW EXECUTE FUNCTION guard_article_quiz_publication();
