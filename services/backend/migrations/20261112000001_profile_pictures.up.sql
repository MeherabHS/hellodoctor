-- Profile pictures for Patient and Doctor profiles.
--
-- Mirrors `prescription_intake_documents`' object-reference column shape
-- (bucket/key/hash/mime/size) rather than storing bytes in the database — the
-- actual encrypted image bytes live in the existing private S3-compatible
-- object store (`DocumentStorageService`), unchanged by this migration.
--
-- All six columns are nullable and have no default, so existing rows (and
-- `patient_repo::insert`/`doctor_repo::insert`, which do not list these
-- columns) are unaffected: a profile simply has no picture until one is
-- uploaded. No RLS policy changes are needed — `patient_profiles` and
-- `doctor_profiles` already enforce owner-only write and owner/admin-only
-- private read (see `20261003000001_rls_hardening.up.sql` and
-- `20261026000001_canonical_user_isolation.up.sql`); a new column on an
-- already-protected row inherits the existing policy automatically.

ALTER TABLE patient_profiles
    ADD COLUMN profile_picture_object_key TEXT,
    ADD COLUMN profile_picture_bucket TEXT,
    ADD COLUMN profile_picture_content_hash TEXT,
    ADD COLUMN profile_picture_mime_type TEXT,
    ADD COLUMN profile_picture_size_bytes INTEGER,
    ADD COLUMN profile_picture_updated_at TIMESTAMPTZ;

ALTER TABLE doctor_profiles
    ADD COLUMN profile_picture_object_key TEXT,
    ADD COLUMN profile_picture_bucket TEXT,
    ADD COLUMN profile_picture_content_hash TEXT,
    ADD COLUMN profile_picture_mime_type TEXT,
    ADD COLUMN profile_picture_size_bytes INTEGER,
    ADD COLUMN profile_picture_updated_at TIMESTAMPTZ;

-- Deliberately exposes only `profile_picture_size_bytes`/`_updated_at`
-- through the public directory view, never the private `object_key`/
-- `bucket`/`content_hash`/`mime_type` — the directory-visible photo is
-- served through a dedicated, authenticated endpoint that looks up the full
-- private row itself; API clients never see or need the raw storage key.
-- PostgreSQL does not allow CREATE OR REPLACE VIEW to append columns. The
-- existing view is referenced by the doctor-article RLS policy, so remove and
-- restore that dependent policy inside this migration transaction before
-- replacing the projection. No committed request can observe the gap.
DROP POLICY doctor_articles_published_read ON doctor_articles;
DROP VIEW doctor_public_directory;

CREATE VIEW doctor_public_directory WITH (security_barrier=true) AS
 SELECT d.id,d.full_name,d.license_number,d.license_authority,d.license_country,
 d.primary_specialty,d.experience_years,d.current_hospital,d.qualifications,
 d.consultation_fee_video,d.consultation_fee_chat,d.is_on_duty,d.is_verified,
 d.professional_verification_status,d.created_at,d.updated_at,u.is_active AS account_active,
 d.profile_picture_size_bytes,d.profile_picture_updated_at
 FROM doctor_profiles d JOIN users u ON u.id=d.user_id
 WHERE d.deleted_at IS NULL AND u.deleted_at IS NULL AND (u.is_active OR current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','COMPLIANCE','SECURITY_ADMIN','SUPPORT'))
 ;
GRANT SELECT ON doctor_public_directory TO hellodoctor_api;

CREATE POLICY doctor_articles_published_read ON doctor_articles FOR SELECT USING (
    status='PUBLISHED' AND current_setting('app.current_role',true) IN
      ('PATIENT','DOCTOR','PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','SUPPORT','COMPLIANCE','SECURITY_ADMIN')
    AND EXISTS (SELECT 1 FROM doctor_public_directory d WHERE d.id=author_id
      AND d.professional_verification_status='VERIFIED' AND d.account_active)
);

-- `doctor_profiles` has FORCE ROW LEVEL SECURITY (see
-- `20261026000001_canonical_user_isolation.up.sql`): a plain authenticated
-- SELECT from any OTHER account's session can only ever see that account's
-- own row, so a Patient browsing the doctor directory cannot read another
-- doctor's full row (which is where the private `object_key`/`bucket`/
-- `content_hash`/`mime_type` needed to actually serve the photo bytes live —
-- the public view above deliberately omits them). This mirrors
-- `doctor_authorized_document` (see
-- `20261014000001_rx_pages_document_authorization.up.sql`), the existing,
-- already-proven pattern in this codebase for a SECURITY DEFINER function
-- granting narrowly-scoped cross-account read access to specific private
-- columns, gated by an explicit visibility condition evaluated inside the
-- function itself (never a blanket RLS bypass) — here, the identical
-- visibility condition `doctor_public_directory` already uses.
CREATE FUNCTION doctor_public_photo(p_doctor_id UUID)
RETURNS JSONB LANGUAGE sql SECURITY DEFINER SET search_path=public STABLE AS $$
    SELECT jsonb_build_object('bucket',d.profile_picture_bucket,'key',d.profile_picture_object_key,
        'hash',d.profile_picture_content_hash,'mime',d.profile_picture_mime_type)
    FROM doctor_profiles d JOIN users u ON u.id=d.user_id
    WHERE d.id=p_doctor_id AND d.deleted_at IS NULL AND u.deleted_at IS NULL
      AND (u.is_active OR current_setting('app.current_role',true) IN ('PLATFORM_ADMIN','CLINICAL_ADMIN','FINANCE_ADMIN','COMPLIANCE','SECURITY_ADMIN','SUPPORT'))
      AND d.profile_picture_object_key IS NOT NULL
    LIMIT 1
$$;
REVOKE ALL ON FUNCTION doctor_public_photo(UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION doctor_public_photo(UUID) TO hellodoctor_api;
