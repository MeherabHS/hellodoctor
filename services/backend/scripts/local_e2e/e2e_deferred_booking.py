"""Repeatable local E2E: Patient booking -> Doctor visibility -> Agora token authorization -> completion
in FINANCIAL_MODULE_MODE=deferred, against the REAL backend binary and a DISPOSABLE PostgreSQL.

Run via ./run_local_e2e.sh (it provisions the disposable DB/Redis, generates throwaway secrets, boots the
backend and tears everything down). Synthetic fixtures only; no real patient data; no credentials are stored
in this file (the doctor password is generated per run; dev seed identities come from src/seed.rs).

Scope limits (by design): development environment, not production; Agora tokens are signed with throwaway
credentials, no audio/video media is exchanged; doctor verification is applied directly to the disposable DB.
"""
import json, os, re, secrets, subprocess, sys, urllib.error, urllib.request, uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

BASE = os.environ.get('E2E_API_BASE', 'http://127.0.0.1:8090/api/v1/')
PG_CONTAINER = os.environ.get('E2E_PG_CONTAINER', 'hd-e2e-pg')
assert re.match(r'^http://(127\.0\.0\.1|localhost):\d+/', BASE), 'E2E refuses any non-loopback target'
seed = (Path(__file__).resolve().parents[2] / 'src' / 'seed.rs').read_text(encoding='utf-8')
const = lambda n: re.search(rf'pub const {n}: &str = "([^"]+)"', seed)[1]
results = []


def call(method, path, body=None, token=None, ok=(200, 201), key=None):
    headers = {'Content-Type': 'application/json'}
    if token: headers['Authorization'] = 'Bearer ' + token
    if key: headers['Idempotency-Key'] = key
    req = urllib.request.Request(BASE + path, method=method, headers=headers,
                                 data=json.dumps(body).encode() if body is not None else None)
    try:
        with urllib.request.urlopen(req) as r: status, raw = r.status, r.read()
    except urllib.error.HTTPError as e: status, raw = e.code, e.read()
    try: data = json.loads(raw)
    except Exception: data = {'raw': raw[:200].decode(errors='replace')}
    if ok is not None and status not in ok:
        print(f'!! {method} {path} -> {status} {json.dumps(data)[:400]}'); sys.exit(1)
    return status, data


def check(name, cond, detail=''):
    results.append((name, bool(cond)))
    print(('PASS ' if cond else 'FAIL ') + name + (f'  [{detail}]' if detail else ''))
    if not cond: sys.exit(1)


def sql(q):
    return subprocess.check_output(['docker', 'exec', PG_CONTAINER, 'psql', '-U', 'hellodoctor', '-d', 'hellodoctor', '-tAc', q], text=True).strip()


# ---- 0. authentication is required for the policy; never assumed
st, _ = call('GET', 'config/booking-policy', ok=None)
check('policy endpoint rejects unauthenticated callers (client must not assume a mode)', st == 401, st)

# ---- doctor self-registers through the real production endpoint (fees are created as 0), then manual verification
legal = {r.split('=')[0]: r.split('=')[1] for r in sql("select document_type||'='||version from legal_documents where is_current").splitlines()}
n = uuid.uuid4().hex[:8]
email = f'e2e-{n}@example.test'; temp_pw = 'Aa1!' + secrets.token_urlsafe(18)
phone = '+2547' + str(int(n, 16) % 100000000).zfill(8)
docs = [{'type': t, 'version': legal[t]} for t in ('DOCTOR_TERMS', 'PRIVACY_NOTICE', 'PROFESSIONAL_VERIFICATION_DECLARATION', 'TELEMEDICINE_PRACTITIONER_POLICY')]
_, reg = call('POST', 'auth/doctor/register', {'full_name': f'E2E Doctor {n}', 'license_number': f'E2E-{n}', 'phone_number': phone, 'email': email,
    'password': temp_pw, 'primary_specialty': 'General Medicine', 'experience_years': 3, 'current_hospital': 'Synthetic Clinic',
    'qualifications': ['MBChB'], 'residential_address': 'Nairobi', 'accepted_documents': docs})
doctor_id = sql(f"select id from doctor_profiles where license_number='E2E-{n}'")
print('doctor fixture', doctor_id, 'fees', sql(f"select consultation_fee_video||'/'||consultation_fee_chat from doctor_profiles where id='{doctor_id}'"))
# Manual KMPDC-style verification (admin action) applied to the disposable fixture DB
sql(f"update doctor_profiles set professional_verification_status='VERIFIED', is_verified=true, professional_verification_reviewed_at=now() where id='{doctor_id}'")

# ---- doctor login
_, lg = call('POST', 'auth/doctor/login', {'email': email, 'password': temp_pw})
d = lg['data']
if 'access_token' not in d:
    code = d.get('debug_otp_code') or d.get('otp_code') or d.get('debug_code')
    _, v = call('POST', 'auth/doctor/verify-otp', {'session_token': d['session_token'], 'otp_code': code})
    d = v['data']
doctor_tok = d['access_token']
if d.get('must_change_password') or d.get('password_change_required'):
    print('NOTE: password change flagged by server (not required for this flow)')

# ---- doctor creates a future slot (Doctor Web: POST /doctor/slots) ~6 min ahead
start = (datetime.now(timezone.utc) + timedelta(minutes=6)).replace(second=0, microsecond=0)
end = start + timedelta(minutes=10)
fmt = lambda t: t.isoformat().replace('+00:00', 'Z')
_, sl = call('POST', 'doctor/slots', {'start_time': fmt(start), 'end_time': fmt(end)}, doctor_tok)
slot_id = sl['data']['id']
# also an expired slot cannot even be created (server clock), and a past row is never offered
st, _ = call('POST', 'doctor/slots', {'start_time': fmt(start - timedelta(hours=2)), 'end_time': fmt(start - timedelta(hours=2) + timedelta(minutes=10))}, doctor_tok, ok=None)
check('server refuses creating a past slot', st in (400, 422), st)

# ---- patient (seeded phone-OTP login, same endpoints the Flutter app uses)
PH = const('SEED_PATIENT_PHONE')
_, ro = call('POST', 'auth/register-otp', {'phone_number': PH, 'role': 'PATIENT'})
code = ro['data'].get('debug_otp_code') or ro['data'].get('debug_code') or ro['data'].get('otp_code')
_, pl = call('POST', 'auth/verify-otp-and-login', {'phone_number': PH, 'otp_code': code, 'accepted_documents': [
    {'type': t, 'version': legal[t]} for t in ('PATIENT_DATA_SHARING_CONSENT','PATIENT_TERMS','PATIENT_PRIVACY_NOTICE','PATIENT_DATA_RIGHTS')]}, ok=(200, 201, 400, 422))
patient_tok = pl['data']['access_token']

# ---- Gate 3: the policy as the Flutter client consumes it
st, pol = call('GET', 'config/booking-policy', token=patient_tok)
check('authenticated patient gets payment_required=false (deferred)', pol['data'] == {'payment_required': False}, pol['data'])

# ---- Patient sees the doctor (VIDEO directory) and the slot, though fees are 0
_, dl = call('GET', 'doctors?modality=VIDEO', token=patient_tok)
check('unpriced verified doctor listed in VIDEO directory (deferred)', any(x['id'] == doctor_id for x in dl['data']))
_, ds = call('GET', f'doctors/{doctor_id}/slots', token=patient_tok)
check('patient sees the future slot', any(x['id'] == slot_id for x in ds['data']))

# ---- Book (no payment method, no gateway)
before_tx = sql('select count(*) from transactions')
before_wallet = sql('select coalesce(sum(lifetime_gross),0)||\'/\'||count(*) from doctor_wallets')
st, bk = call('POST', 'appointments/book', {'doctor_id': doctor_id, 'slot_id': slot_id, 'modality': 'VIDEO', 'chief_complaint': 'Synthetic E2E'}, patient_tok, key=str(uuid.uuid4()))
b = bk['data']; apt = b['appointment_id']
check('booking returns CONFIRMED / NOT_REQUIRED with no payment session', b['status'] == 'CONFIRMED' and b['payment_policy'] == 'NOT_REQUIRED' and b['payment_session_id'] is None, b['status'])
check('DB: appointment CONFIRMED + NOT_REQUIRED', sql(f"select status||'/'||payment_policy from appointments where id='{apt}'") == 'CONFIRMED/NOT_REQUIRED')
check('DB: no transaction row for the appointment', sql(f"select count(*) from transactions where appointment_id='{apt}'") == '0' and sql('select count(*) from transactions') == before_tx)
check('DB: slot is BOOKED', sql(f"select status from doctor_schedule_slots where id='{slot_id}'") == 'BOOKED')
st, _ = call('POST', 'appointments/book', {'doctor_id': doctor_id, 'slot_id': slot_id, 'modality': 'VIDEO'}, patient_tok, ok=None)
check('second booking of the same slot rejected (409)', st == 409, st)

# ---- Doctor Web sees it
_, la = call('GET', 'doctor/appointments?limit=50', token=doctor_tok)
row = next((x for x in la['data'] if x.get('id') == apt or x.get('appointment_id') == apt), None)
check('confirmed appointment visible in Doctor Web list', row is not None)
print('doctor row status/payment:', row.get('status'), row.get('payment_policy'))

# ---- Agora: role-validated tokens (credentials are throwaway; media itself is NOT exercised here)
_, pt = call('POST', f'consultations/{apt}/rtc-token', {}, patient_tok)
_, dt = call('POST', f'consultations/{apt}/rtc-token', {}, doctor_tok)
check('patient and doctor are issued tokens for the SAME channel', pt['data']['channel_name'] == dt['data']['channel_name'] and pt['data']['channel_name'].startswith('rtc_'), pt['data']['channel_name'])
check('tokens differ per identity (uid/role bound)', pt['data'].get('token') != dt['data'].get('token'))
# another doctor / another patient must not get a token
_, other = call('POST', 'auth/doctor/login', {'email': const('SEED_DOCTOR1_EMAIL'), 'password': const('SEED_DOCTOR1_PASSWORD')})
od = other['data']
if 'access_token' not in od:
    c = od.get('debug_otp_code') or od.get('otp_code') or od.get('debug_code')
    _, od = call('POST', 'auth/doctor/verify-otp', {'session_token': od['session_token'], 'otp_code': c}); od = od['data']
st, _ = call('POST', f'consultations/{apt}/rtc-token', {}, od['access_token'], ok=None)
check("a different doctor is refused a token for this appointment", st in (403, 404), st)
st, _ = call('POST', f'consultations/{apt}/complete', {'outcome': 'COMPLETED_NO_RX'}, patient_tok, ok=None)
check('patient cannot complete a consultation', st in (401, 403), st)

# ---- clinical completion by the authorized doctor
st, cp = call('POST', f'consultations/{apt}/complete', {'outcome': 'COMPLETED_NO_RX'}, doctor_tok, ok=None)
check('doctor completes without prescription', st == 200, st)
check('DB: appointment COMPLETED', sql(f"select status from appointments where id='{apt}'") == 'COMPLETED')
check('DB: still no transaction row', sql(f"select count(*) from transactions where appointment_id='{apt}'") == '0')
check('DB: wallet totals unchanged', sql('select coalesce(sum(lifetime_gross),0)||\'/\'||count(*) from doctor_wallets') == before_wallet)
print('\nALL', len(results), 'CHECKS PASSED')
