-- Standalone quizzes (product-owner instruction, 2026-11-13): a quiz that is
-- not attached to a blog article. Stored as an Admin article flagged
-- `standalone_quiz`, so it reuses the existing quiz revisions, independent
-- review, publication and patient answer-check functions unchanged. The
-- article body is the quiz's short introduction. A standalone quiz never
-- appears among articles or videos; patients reach it from the Quizzes tab.
ALTER TABLE admin_articles
  ADD COLUMN standalone_quiz BOOLEAN NOT NULL DEFAULT false,
  ADD CONSTRAINT admin_articles_quiz_not_video CHECK (NOT (standalone_quiz AND video_url IS NOT NULL));

-- The kind is fixed at creation.
CREATE FUNCTION guard_admin_article_kind() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.standalone_quiz IS DISTINCT FROM OLD.standalone_quiz THEN
  RAISE EXCEPTION 'An entry cannot change between article and standalone quiz' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER admin_article_kind_guard BEFORE UPDATE ON admin_articles
  FOR EACH ROW EXECUTE FUNCTION guard_admin_article_kind();
