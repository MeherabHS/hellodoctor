DROP TRIGGER IF EXISTS admin_article_kind_guard ON admin_articles;
DROP FUNCTION IF EXISTS guard_admin_article_kind();
ALTER TABLE admin_articles DROP CONSTRAINT IF EXISTS admin_articles_quiz_not_video;
ALTER TABLE admin_articles DROP COLUMN IF EXISTS standalone_quiz;
