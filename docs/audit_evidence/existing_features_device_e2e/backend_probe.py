"""Read/check real synthetic-account backend state; sessions stay in ignored target.

No payment activation or database mutations are performed by this helper.
"""
import json
import base64
import re
import sys
import time
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).parent
RUNTIME = ROOT / 'services/backend/target/existing_features_e2e_runtime.json'
BASE = 'http://127.0.0.1:8080/api/v1/'


def seed(name):
    source = (ROOT / 'services/backend/src/seed.rs').read_text(encoding='utf-8')
    return re.search(rf'{name}: &str = "([^"]+)"', source)[1]


def request(path, body=None, token=None, expected=200, method=None, idempotency_key=None):
    headers = {'Content-Type': 'application/json'}
    if token:
        headers['Authorization'] = 'Bearer '+token
    if idempotency_key:
        headers['Idempotency-Key'] = idempotency_key
    req = Request(BASE+path, data=json.dumps(body).encode() if body is not None else None,
                  headers=headers, method=method or ('POST' if body is not None else 'GET'))
    try:
        with urlopen(req, timeout=20) as response:
            status, payload = response.status, json.load(response)
    except HTTPError as error:
        status, payload = error.code, json.load(error)
    assert status == expected, f'{path}: HTTP {status}, expected {expected}'
    return payload.get('data') if status < 300 else payload


def session():
    return json.loads(RUNTIME.read_text(encoding='utf-8'))


def own(role, path, **kwargs):
    current = session()
    token = current[role+'_token']
    payload = token.split('.')[1]
    claims = json.loads(base64.urlsafe_b64decode(payload+'='*(-len(payload)%4)))
    if claims['exp'] < time.time()+60:
        rotated = request('auth/refresh', {'refresh_token': current[role+'_refresh']})
        current[role+'_token'] = token = rotated['access_token']
        current[role+'_refresh'] = rotated['refresh_token']
        RUNTIME.write_text(json.dumps(current), encoding='utf-8')
    return request(path, token=token, **kwargs)


def bootstrap():
    env = (ROOT / 'services/backend/.env').read_text()
    assert re.search(r'(?m)^ENVIRONMENT=development\s*$', env)
    guide = (ROOT / 'releases/PATIENT_CONSULTATION_TESTING_20261014.md').read_text(encoding='utf-8')
    email, password = re.search(r'\| Patient \| `([^`]+)` \| `([^`]+)`', guide).groups()
    patient = request('auth/patient/login', {'email': email, 'password': password})
    patient = request('auth/patient/verify-otp', {
        'challenge_token': patient['challenge_token'], 'otp_code': patient['debug_otp_code']})
    doctor = request('auth/doctor/login', {
        'email': seed('SEED_DOCTOR1_EMAIL'), 'password': seed('SEED_DOCTOR1_PASSWORD')})
    doctor = request('auth/doctor/verify-otp', {
        'session_token': doctor['session_token'], 'otp_code': doctor['debug_otp_code']})
    RUNTIME.write_text(json.dumps({'patient_token': patient['access_token'],
                                  'patient_refresh': patient['refresh_token'],
                                  'doctor_token': doctor['access_token'],
                                  'doctor_refresh': doctor['refresh_token']}), encoding='utf-8')
    p = own('patient', 'patients/me')
    d = own('doctor', 'doctor/profile')
    (OUT / 'account_identity.json').write_text(json.dumps({
        'patient': {'id': p['id'], 'user_id': p['user_id'], 'name': p['full_name']},
        'doctor': {'id': d['id'], 'name': d['full_name']},
    }, indent=2), encoding='utf-8')
    print('Patient/Doctor password and development OTP sessions issued; tokens not printed.')


if __name__ == '__main__':
    if sys.argv[1] == 'bootstrap':
        bootstrap()
    elif sys.argv[1] == 'get':
        print(json.dumps(own(sys.argv[2], sys.argv[3]), ensure_ascii=True))
    else:
        raise SystemExit('Unknown operation')
