"""Correct only the fixed, proven synthetic seed identities. Never run in production."""
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'services/backend/target/kenya-professional-verification'
config = dict(line.split('=', 1) for line in (ROOT / 'services/backend/.env').read_text(encoding='utf-8-sig').splitlines() if '=' in line and not line.lstrip().startswith('#'))
assert config.get('ENVIRONMENT', '').strip().strip('"\'') == 'development', 'Development environment required'
flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0

def sql(query):
    result = subprocess.run(['docker', 'exec', '-i', 'hellodoctor-postgres', 'psql', '-X', '-v', 'ON_ERROR_STOP=1', '-At', '-U', 'hellodoctor', '-d', 'hellodoctor'], input=query, text=True, capture_output=True, creationflags=flags)
    if result.returncode:
        raise RuntimeError('Guarded fixture correction failed; no unknown identities may be rewritten: ' + result.stderr)
    return result.stdout.strip()

query = r"""
BEGIN;
CREATE TEMP TABLE known_doctor_fixture_corrections (
 id uuid, user_id uuid, old_name text, new_name text, old_license text, new_license text,
 old_hospital text, new_hospital text, old_phone text, new_phone text
) ON COMMIT DROP;
INSERT INTO known_doctor_fixture_corrections VALUES
 ('da000001-0000-0000-0000-000000000001','10000001-0000-0000-0000-000000000001',
 'Dr. Sabrina Akter','Dr. Synthetic Doctor One','KMPDC #45821','SYNTH-KE-0001',
 'Kenyatta National Hospital','Synthetic Nairobi Clinic One','+254 711 884920','+254700000101'),
 ('da000002-0000-0000-0000-000000000002','10000002-0000-0000-0000-000000000002',
 'Dr. Anika Rahman','Dr. Synthetic Doctor Two','KMPDC #52891','SYNTH-KE-0002',
 'Nairobi Hospital','Synthetic Nairobi Clinic Two','+254 712 445566','+254700000102');
SELECT d.id FROM doctor_profiles d JOIN known_doctor_fixture_corrections f USING(id) FOR UPDATE OF d;
DO $$ BEGIN
 IF (SELECT count(*) FROM doctor_profiles d JOIN known_doctor_fixture_corrections f USING(id)
     WHERE d.user_id=f.user_id AND d.full_name IN (f.old_name,f.new_name)
       AND d.license_number IN (f.old_license,f.new_license)
       AND d.current_hospital IN (f.old_hospital,f.new_hospital)
       AND d.verified_phone IN (f.old_phone,f.new_phone)
       AND d.license_authority='KMPDC' AND d.license_country='KE') <> 2 THEN
   RAISE EXCEPTION 'Known seed identity has changed; refusing to rewrite unknown data';
 END IF;
END $$;
WITH before_rows AS MATERIALIZED (
 SELECT d.id, jsonb_build_object('full_name',d.full_name,'license_number',d.license_number,
   'current_hospital',d.current_hospital,'verified_phone',d.verified_phone) AS before_data
 FROM doctor_profiles d JOIN known_doctor_fixture_corrections f USING(id)
 WHERE d.full_name<>f.new_name OR d.license_number<>f.new_license
   OR d.current_hospital<>f.new_hospital OR d.verified_phone<>f.new_phone
), changed AS (
 UPDATE doctor_profiles d SET full_name=f.new_name, license_number=f.new_license,
   current_hospital=f.new_hospital, verified_phone=f.new_phone, updated_at=NOW()
 FROM known_doctor_fixture_corrections f, before_rows b WHERE d.id=f.id AND d.id=b.id
 RETURNING d.id, d.full_name, d.license_number, d.current_hospital, d.verified_phone
), audited AS (
 INSERT INTO audit_events(id,actor_role,action,resource_type,resource_id,result,reason,before_data,after_data,changed_fields)
 SELECT gen_random_uuid(),'PLATFORM_ADMIN','SYNTHETIC_KENYA_FIXTURE_CORRECTED','DOCTOR_PROFILE',c.id,'SUCCESS',
   'Approved Kenya correction of exact source-owned synthetic seed identities; user accounts, schedules and financial history preserved',
   b.before_data, jsonb_build_object('full_name',c.full_name,'license_number',c.license_number,'current_hospital',c.current_hospital,'verified_phone',c.verified_phone),
   ARRAY['full_name','license_number','current_hospital','verified_phone']
 FROM changed c JOIN before_rows b USING(id) RETURNING resource_id
)
SELECT json_build_object('doctor_profiles_updated',count(*),'doctor_ids',json_agg(resource_id)) FROM audited;
WITH changed AS (
 UPDATE users u SET phone_number=f.new_phone, updated_at=NOW()
 FROM known_doctor_fixture_corrections f WHERE u.id=f.user_id AND u.phone_number=f.old_phone
 RETURNING u.id
) SELECT json_build_object('doctor_user_phones_updated',count(*)) FROM changed;
WITH changed AS (
 UPDATE users SET phone_number=CASE id
   WHEN 'a0000001-0000-0000-0000-000000000001'::uuid THEN '+254700000001'
   ELSE '+254700000002' END, updated_at=NOW()
 WHERE (id='a0000001-0000-0000-0000-000000000001' AND phone_number='+8801700000000')
    OR (id='b0000001-0000-0000-0000-000000000001' AND phone_number='+8801712345678')
 RETURNING id
) SELECT json_build_object('admin_patient_seed_phones_updated',count(*)) FROM changed;
COMMIT;
"""
result = sql(query)
records = [json.loads(line) for line in result.splitlines() if line.startswith('{')]
(OUT / 'persistent-fixture-correction.json').write_text(json.dumps(records, indent=2), encoding='utf-8')
print(json.dumps(records))
