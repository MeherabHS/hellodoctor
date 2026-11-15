-- Facilities & Services (product-owner instruction, 2026-11-13): an
-- Admin-managed directory of health facilities that patients can browse.
-- Admins create, publish, unpublish and archive; patients and doctors read
-- published facilities only. Photos live in the article-media object store
-- under facility-media/<uuid>.jpg (never in PostgreSQL).

-- Service categories are data, so a new category needs a row, not an app release.
CREATE TABLE facility_service_categories (
 code TEXT PRIMARY KEY CHECK (code ~ '^[A-Z][A-Z0-9_]{1,39}$'),
 name_en TEXT NOT NULL CHECK (char_length(btrim(name_en)) BETWEEN 1 AND 60),
 name_sw TEXT NOT NULL CHECK (char_length(btrim(name_sw)) BETWEEN 1 AND 60),
 icon_key TEXT NOT NULL CHECK (icon_key ~ '^[a-z_]{1,40}$'),
 sort_order INTEGER NOT NULL DEFAULT 0,
 active BOOLEAN NOT NULL DEFAULT true
);
INSERT INTO facility_service_categories(code,name_en,name_sw,icon_key,sort_order) VALUES
 ('TESTING','Testing (HIV/STIs)','Upimaji (VVU/Magonjwa ya zinaa)','testing',10),
 ('PREP','PrEP','PrEP','prep',20),
 ('ART','ART (Treatment)','ART (Matibabu)','art',30),
 ('COUNSELLING','Counselling & Support','Ushauri na Msaada','counselling',40);

-- Drafts may be incomplete; a PUBLISHED row must be complete (checked here and,
-- with clearer messages, by the API).
CREATE TABLE health_facilities (
 id UUID PRIMARY KEY,
 name TEXT NOT NULL CHECK (char_length(btrim(name)) BETWEEN 2 AND 120),
 description TEXT NOT NULL DEFAULT '' CHECK (char_length(description) <= 2000),
 county TEXT NOT NULL DEFAULT '' CHECK (char_length(county) <= 40),
 location_text TEXT NOT NULL DEFAULT '' CHECK (char_length(location_text) <= 200),
 contact_phone TEXT NOT NULL DEFAULT '' CHECK (contact_phone = '' OR contact_phone ~ '^\+254[0-9]{9}$'),
 google_maps_url TEXT NOT NULL DEFAULT '' CHECK (google_maps_url = '' OR (google_maps_url LIKE 'https://%' AND char_length(google_maps_url) <= 2048)),
 services TEXT[] NOT NULL DEFAULT '{}' CHECK (cardinality(services) <= 20),
 youth_friendly BOOLEAN NOT NULL DEFAULT false,
 timezone TEXT NOT NULL DEFAULT 'Africa/Nairobi' CHECK (timezone IN ('Africa/Nairobi')),
 opening_hours JSONB CHECK (opening_hours IS NULL OR jsonb_typeof(opening_hours) = 'object'),
 image_media_id UUID,
 publication_status TEXT NOT NULL DEFAULT 'DRAFT' CHECK (publication_status IN ('DRAFT','PUBLISHED','ARCHIVED')),
 version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
 published_at TIMESTAMPTZ,
 archived_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 created_by UUID NOT NULL REFERENCES users(id),
 updated_by UUID NOT NULL REFERENCES users(id),
 CHECK ((publication_status = 'ARCHIVED') = (archived_at IS NOT NULL)),
 CHECK (publication_status <> 'PUBLISHED' OR (
   published_at IS NOT NULL AND county <> '' AND btrim(location_text) <> '' AND contact_phone <> ''
   AND google_maps_url <> '' AND image_media_id IS NOT NULL AND cardinality(services) >= 1))
);
CREATE INDEX health_facilities_directory ON health_facilities(lower(name), id) WHERE publication_status = 'PUBLISHED';
CREATE INDEX health_facilities_admin ON health_facilities(updated_at DESC, id);
CREATE INDEX health_facilities_county ON health_facilities(county) WHERE publication_status = 'PUBLISHED';

CREATE TABLE facility_media (
 id UUID PRIMARY KEY,
 facility_id UUID NOT NULL REFERENCES health_facilities(id),
 object_key TEXT NOT NULL UNIQUE CHECK (object_key = 'facility-media/' || id::text || '.jpg'),
 content_hash TEXT NOT NULL,
 size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 512000),
 ready BOOLEAN NOT NULL DEFAULT false,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE (facility_id, id)
);
ALTER TABLE health_facilities ADD CONSTRAINT health_facilities_image_fk
  FOREIGN KEY (id, image_media_id) REFERENCES facility_media(facility_id, id);

ALTER TABLE facility_service_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE facility_service_categories FORCE ROW LEVEL SECURITY;
ALTER TABLE health_facilities ENABLE ROW LEVEL SECURITY;
ALTER TABLE health_facilities FORCE ROW LEVEL SECURITY;
ALTER TABLE facility_media ENABLE ROW LEVEL SECURITY;
ALTER TABLE facility_media FORCE ROW LEVEL SECURITY;

CREATE POLICY facility_category_read ON facility_service_categories FOR SELECT TO hellodoctor_api
  USING (current_setting('app.current_role', true) IN ('ADMIN','PATIENT','DOCTOR'));

CREATE POLICY facility_read ON health_facilities FOR SELECT TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN'
     OR (current_setting('app.current_role', true) IN ('PATIENT','DOCTOR') AND publication_status = 'PUBLISHED'));
CREATE POLICY facility_insert ON health_facilities FOR INSERT TO hellodoctor_api
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN' AND publication_status = 'DRAFT'
     AND created_by = NULLIF(current_setting('app.current_user_id', true), '')::uuid);
CREATE POLICY facility_update ON health_facilities FOR UPDATE TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN')
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN'
     AND updated_by = NULLIF(current_setting('app.current_user_id', true), '')::uuid);

CREATE POLICY facility_media_read ON facility_media FOR SELECT TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN'
     OR (ready AND EXISTS (SELECT 1 FROM health_facilities f WHERE f.id = facility_id
         AND f.publication_status = 'PUBLISHED' AND f.image_media_id = facility_media.id)));
CREATE POLICY facility_media_insert ON facility_media FOR INSERT TO hellodoctor_api
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN');
CREATE POLICY facility_media_update ON facility_media FOR UPDATE TO hellodoctor_api
  USING (current_setting('app.current_role', true) = 'ADMIN')
  WITH CHECK (current_setting('app.current_role', true) = 'ADMIN');

GRANT SELECT ON facility_service_categories TO hellodoctor_api;
GRANT SELECT, INSERT, UPDATE ON health_facilities, facility_media TO hellodoctor_api;

-- Archived is final; identity and audit columns never change.
CREATE FUNCTION guard_health_facility() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.id <> OLD.id OR NEW.created_at <> OLD.created_at OR NEW.created_by <> OLD.created_by
    OR OLD.publication_status = 'ARCHIVED' OR NEW.version <> OLD.version + 1 THEN
  RAISE EXCEPTION 'Invalid facility identity, version or archived edit' USING ERRCODE = '23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER health_facility_guard BEFORE UPDATE ON health_facilities
  FOR EACH ROW EXECUTE FUNCTION guard_health_facility();
