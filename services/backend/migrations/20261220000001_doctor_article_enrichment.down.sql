DROP POLICY IF EXISTS doctor_articles_worker_cover_read ON doctor_articles;
REVOKE SELECT (id, cover_image_id) ON doctor_articles FROM hellodoctor_system_worker;
-- doctor_article_media's policies read doctor_articles.cover_image_id, so the
-- media table (and with it those policies) must go before that column.
ALTER TABLE doctor_articles DROP CONSTRAINT IF EXISTS doctor_articles_cover_fk;
DROP TABLE IF EXISTS doctor_article_media;
ALTER TABLE doctor_articles
    DROP CONSTRAINT IF EXISTS doctor_articles_swahili_complete,
    DROP COLUMN IF EXISTS cover_image_id,
    DROP COLUMN IF EXISTS body_sw,
    DROP COLUMN IF EXISTS summary_sw,
    DROP COLUMN IF EXISTS title_sw,
    DROP COLUMN IF EXISTS topic_key;
