-- Exact inverse of the up migration: restores the pre-fix (stale) six-role
-- definitions byte-for-byte as they existed after
-- `20261112000001_profile_pictures`.
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

CREATE OR REPLACE FUNCTION doctor_public_photo(p_doctor_id UUID)
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
