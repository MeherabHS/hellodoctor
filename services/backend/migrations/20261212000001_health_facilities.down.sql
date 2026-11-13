DROP TRIGGER IF EXISTS health_facility_guard ON health_facilities;
DROP FUNCTION IF EXISTS guard_health_facility();
ALTER TABLE IF EXISTS health_facilities DROP CONSTRAINT IF EXISTS health_facilities_image_fk;
DROP TABLE IF EXISTS facility_media;
DROP TABLE IF EXISTS health_facilities;
DROP TABLE IF EXISTS facility_service_categories;
