-- Doctor Web redesign (2026-11-14, product-owner approved): Doctor articles gain an
-- optional health topic, an optional complete Swahili version and an optional cover
-- image. Covers live in their own encrypted media namespace with the same limits as
-- Admin covers; access mirrors doctor_articles (owner, or anyone once published).
CREATE TABLE doctor_article_media (
    id UUID PRIMARY KEY,
    article_id UUID NOT NULL REFERENCES doctor_articles(id),
    object_key TEXT NOT NULL UNIQUE CHECK (object_key = 'doctor-article-media/' || id::text || '.jpg'),
    content_hash TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 512000),
    ready BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (article_id, id)
);
CREATE INDEX doctor_article_media_cleanup ON doctor_article_media(created_at, id);

ALTER TABLE doctor_articles
    ADD COLUMN topic_key TEXT CHECK (topic_key IN ('HIV_BASICS','PREP','CONDOMS','SRH','STIGMA_DISCLOSURE','HEALTHY_RELATIONSHIPS')),
    ADD COLUMN title_sw TEXT CHECK (char_length(title_sw) BETWEEN 1 AND 160),
    ADD COLUMN summary_sw TEXT CHECK (char_length(summary_sw) BETWEEN 1 AND 600),
    ADD COLUMN body_sw TEXT CHECK (char_length(body_sw) BETWEEN 1 AND 30000),
    ADD COLUMN cover_image_id UUID,
    ADD CONSTRAINT doctor_articles_swahili_complete CHECK (
        (title_sw IS NULL AND summary_sw IS NULL AND body_sw IS NULL)
        OR (title_sw IS NOT NULL AND summary_sw IS NOT NULL AND body_sw IS NOT NULL)),
    ADD CONSTRAINT doctor_articles_cover_fk FOREIGN KEY (id, cover_image_id) REFERENCES doctor_article_media(article_id, id);

ALTER TABLE doctor_article_media ENABLE ROW LEVEL SECURITY;
ALTER TABLE doctor_article_media FORCE ROW LEVEL SECURITY;
-- Owner: the verified Doctor who wrote the article (same identity test as doctor_articles).
CREATE POLICY doctor_article_media_owner_read ON doctor_article_media FOR SELECT TO hellodoctor_api USING (
    current_setting('app.current_role', true) = 'DOCTOR' AND EXISTS (
      SELECT 1 FROM doctor_articles a JOIN doctor_profiles d ON d.id = a.author_id
      WHERE a.id = article_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid));
CREATE POLICY doctor_article_media_published_read ON doctor_article_media FOR SELECT TO hellodoctor_api USING (
    ready AND EXISTS (SELECT 1 FROM doctor_articles a WHERE a.id = article_id AND a.status = 'PUBLISHED' AND a.cover_image_id = doctor_article_media.id));
CREATE POLICY doctor_article_media_owner_insert ON doctor_article_media FOR INSERT TO hellodoctor_api WITH CHECK (
    current_setting('app.current_role', true) = 'DOCTOR' AND NOT ready AND EXISTS (
      SELECT 1 FROM doctor_articles a JOIN doctor_profiles d ON d.id = a.author_id JOIN users u ON u.id = d.user_id
      WHERE a.id = article_id AND a.status <> 'ARCHIVED'
        AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid
        AND d.professional_verification_status = 'VERIFIED' AND d.deleted_at IS NULL AND u.is_active AND u.deleted_at IS NULL));
CREATE POLICY doctor_article_media_owner_update ON doctor_article_media FOR UPDATE TO hellodoctor_api USING (
    current_setting('app.current_role', true) = 'DOCTOR' AND EXISTS (
      SELECT 1 FROM doctor_articles a JOIN doctor_profiles d ON d.id = a.author_id
      WHERE a.id = article_id AND d.user_id = NULLIF(current_setting('app.current_user_id', true), '')::uuid)
) WITH CHECK (ready);
-- Cleanup of abandoned uploads (not the current cover of any article) after 24 hours.
CREATE POLICY doctor_article_media_worker_read ON doctor_article_media FOR SELECT TO hellodoctor_system_worker USING (true);
CREATE POLICY doctor_article_media_worker_lock ON doctor_article_media FOR UPDATE TO hellodoctor_system_worker USING (
    created_at < now() - interval '24 hours' AND NOT EXISTS (SELECT 1 FROM doctor_articles a WHERE a.cover_image_id = doctor_article_media.id)) WITH CHECK (false);
CREATE POLICY doctor_article_media_worker_delete ON doctor_article_media FOR DELETE TO hellodoctor_system_worker USING (
    created_at < now() - interval '24 hours' AND NOT EXISTS (SELECT 1 FROM doctor_articles a WHERE a.cover_image_id = doctor_article_media.id));
GRANT SELECT, INSERT ON doctor_article_media TO hellodoctor_api;
GRANT UPDATE (ready) ON doctor_article_media TO hellodoctor_api;
GRANT SELECT, DELETE ON doctor_article_media TO hellodoctor_system_worker;
GRANT UPDATE (ready) ON doctor_article_media TO hellodoctor_system_worker;
-- The cleanup worker must see every current cover (not just published ones), or it
-- would treat a draft's cover as abandoned. It reads only the cover reference.
GRANT SELECT (id, cover_image_id) ON doctor_articles TO hellodoctor_system_worker;
CREATE POLICY doctor_articles_worker_cover_read ON doctor_articles FOR SELECT TO hellodoctor_system_worker USING (true);
