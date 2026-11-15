-- Patients give a residential address at registration (product-owner
-- instruction 2026-11-14). Nullable because accounts created before this
-- change, and guardianed dependent profiles, have none; new self
-- registrations require it in the API. Sensitive: shown only to the patient
-- themself and in their own data-rights export, and cleared on erasure.
ALTER TABLE patient_profiles
    ADD COLUMN residential_address TEXT
    CHECK (residential_address IS NULL OR char_length(residential_address) BETWEEN 3 AND 300);
