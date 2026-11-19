-- Reverses 20261222000014_counterpart_photos.up.sql. Uploaded peer photo
-- objects stay in the object store; only the references are removed.
DROP FUNCTION IF EXISTS peer_request_counterpart_photo(UUID);
DROP FUNCTION IF EXISTS appointment_counterpart_photo(UUID);

ALTER TABLE peer_profiles
    DROP COLUMN profile_picture_object_key,
    DROP COLUMN profile_picture_bucket,
    DROP COLUMN profile_picture_content_hash,
    DROP COLUMN profile_picture_mime_type,
    DROP COLUMN profile_picture_size_bytes,
    DROP COLUMN profile_picture_updated_at;
