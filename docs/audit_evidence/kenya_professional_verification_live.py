"""Live manual-review walkthrough using installed Edge and existing services.

Creates exactly two clearly synthetic Doctors, no slots/payments/practitioner data.
Never records authentication tokens, passwords or OTPs in artifacts.
"""
import asyncio
import base64
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import secrets
import struct
import subprocess
import time
from playwright.async_api import async_playwright, expect

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'services/backend/target/kenya-professional-verification'
RESULT = OUT / 'live-results.json'
SEED = (ROOT / 'services/backend/src/seed.rs').read_text(encoding='utf-8')
results = json.loads(RESULT.read_text()) if RESULT.exists() else {'nonce': secrets.token_hex(5), 'checks': {}, 'fixtures': {}}
nonce = results['nonce']

def credential(name):
    return re.search(r'pub const ' + name + r': &str = "([^"]+)"', SEED).group(1)

def otp():
    raw = hmac.new(base64.b32decode(credential('SEED_ADMIN_TOTP_SECRET_BASE32')), struct.pack('>Q', int(time.time()) // 30), hashlib.sha1).digest()
    offset = raw[-1] & 15
    return str((struct.unpack('>I', raw[offset:offset+4])[0] & 0x7fffffff) % 1000000).zfill(6)

def save():
    RESULT.write_text(json.dumps(results, indent=2), encoding='utf-8')

async def json_response(response, expected):
    assert response.status == expected, 'Unexpected HTTP status: ' + str(response.status)
    return (await response.json())['data']

async def browser_api(page, path, body=None, expected=200):
    # Exercise browser cookie handling, including Secure cookies on loopback;
    # APIRequestContext uses different HTTP-cookie rules from installed Edge.
    reply = await page.evaluate("""async ({path,body})=>{
      const response=await fetch(path,{method:body===null?'GET':'POST',credentials:'include',
        headers:{'Content-Type':'application/json'},...(body===null?{}:{body:JSON.stringify(body)})});
      return {status:response.status,body:await response.json()};
    }""", {'path':path, 'body':body})
    assert reply['status'] == expected, 'Unexpected browser HTTP status: ' + str(reply['status'])
    return reply['body']['data']

async def walkthrough():
    save()
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel='msedge', headless=True)
        errors = []
        admin_ctx = await browser.new_context()
        # This walkthrough tests manual review, not background polling. Avoid
        # multiplying real dossier requests against the shared dev IP ceiling.
        await admin_ctx.add_init_script('window.setInterval = () => 0;')
        admin = await admin_ctx.new_page()
        admin.on('pageerror', lambda error: errors.append(type(error).__name__))
        await admin.goto('http://127.0.0.1:3000/login', wait_until='networkidle')
        await admin.locator('#email').fill(credential('SEED_ADMIN_EMAIL'))
        await admin.locator('#password').fill(credential('SEED_ADMIN_PASSWORD'))
        await admin.locator('#totp').fill(otp())
        await admin.get_by_role('button', name='Sign in', exact=True).click()
        await admin.wait_for_url('http://127.0.0.1:3000/', timeout=45000)
        await admin.locator('#adminNav-verification').click()
        await expect(admin.locator('#adminPage_verification')).to_be_visible()
        results['checks']['admin_generic_verification_navigation'] = 'PASS'
        save()

        for index, (label, terminal, action, button) in enumerate([
            ('Reject', 'REJECTED', 'reject', 'Reject Application'),
            ('Approve', 'VERIFIED', 'verify', 'Verify Registration'),
        ]):
            context = await browser.new_context()
            doctor = await context.new_page()
            doctor.on('pageerror', lambda error: errors.append(type(error).__name__))
            await doctor.goto('http://127.0.0.1:3001', wait_until='networkidle')
            name = f'Dr. Synthetic Kenya Review {nonce} {label}'
            email = f'kenya-review-{nonce}-{label.lower()}@example.test'
            # Only a non-privileged, synthetic test account; credential never exported.
            password = f'Synthetic-Review-{nonce}-Aa1!'
            payload = {'full_name': name, 'email': email, 'password': password,
                       'license_number': f'SYNTH-KE-REVIEW-{nonce}-{index}',
                       'phone_number': '+2547' + str((int(nonce, 16) + index) % 100000000).zfill(8),
                       'primary_specialty': 'General Medicine', 'experience_years': 1,
                       'current_hospital': 'Synthetic Kenya Review Clinic',
                       'qualifications': ['Synthetic MBChB'], 'residential_address': 'Synthetic Nairobi fixture'}
            headers = {'Origin': 'http://127.0.0.1:3001'}
            if label not in results['fixtures']:
                registered = await browser_api(doctor, '/api/auth/register', payload, 201)
                assert registered['professional_verification_status'] == 'PENDING'
                assert registered['license_authority'] == 'KMPDC' and registered['license_country'] == 'KE'
                results['fixtures'][label] = {'doctor_id': registered['doctor_id'], 'initial_status': 'PENDING'}
                results['checks'][label + '_registration_pending_kmpdc_ke'] = 'PASS'
                save()
            login = await browser_api(doctor, '/api/auth/login', {'email':email, 'password':password})
            assert login['development_otp'], 'Development fixture requires the configured development OTP path'
            await browser_api(doctor, '/api/auth/verify', {'otp_code': login['development_otp']})
            profile_url = '/api/proxy/doctor/profile'
            profile = await browser_api(doctor, profile_url)
            doctor_id = results['fixtures'][label]['doctor_id']
            assert profile['id'] == doctor_id
            assert profile['license_authority'] == 'KMPDC' and profile['license_country'] == 'KE'
            if profile['professional_verification_status'] == 'PENDING':
                await doctor.reload(wait_until='networkidle')
                await expect(doctor.get_by_role('heading', name='Pending review', exact=True)).to_be_visible()
                results['checks'][label + '_doctor_pending_screen'] = 'PASS'
                queue = await browser_api(admin, '/api/proxy/admin/doctors/pending-verification')
                assert any(row['id'] == doctor_id and row['professional_verification_status'] == 'PENDING' for row in queue)
                results['checks'][label + '_included_in_admin_pending_queue'] = 'PASS'
                print(label + ': pending registration and queue confirmed; pacing shared-dev requests', flush=True)
                await asyncio.sleep(35)
                await admin.reload(wait_until='networkidle')
                await admin.locator('#adminNav-verification').click()
                await admin.get_by_text(name, exact=True).click(timeout=45000)
                await expect(admin.locator('#adminPage_verification')).to_contain_text('Kenya Medical Practitioners and Dentists Council')
                await asyncio.sleep(35)
                async with admin.expect_response(lambda response: response.url.endswith('/' + doctor_id + '/' + action)) as review:
                    await admin.get_by_role('button', name=re.compile(button)).click()
                assert (await review.value).status == 200
                results['checks'][label + '_admin_ui_terminal_transition'] = 'PASS'
                save()

            for _ in range(2):
                queue = await browser_api(admin, '/api/proxy/admin/doctors/pending-verification')
                assert all(row['id'] != doctor_id for row in queue)
                profile = await browser_api(doctor, profile_url)
                assert profile['professional_verification_status'] == terminal
                assert profile['is_verified'] == (terminal == 'VERIFIED')
                assert profile['professional_verification_reviewed_at'] and profile['professional_verification_reviewed_by']
                assert not profile['is_on_duty']
            results['checks'][label + '_persisted_terminal_status_and_queue_exclusion_after_refresh'] = 'PASS'
            await doctor.reload(wait_until='networkidle')
            if terminal == 'REJECTED':
                await expect(doctor.get_by_role('heading', name='Professional registration rejected', exact=True)).to_be_visible()
                await expect(doctor.get_by_text('KMPDC Registration:', exact=False)).to_be_visible()
            else:
                await doctor.locator('#doctorWebSidebar [data-page="settings"]').click()
                await doctor.get_by_role('button', name='Professional Details', exact=True).click()
                await expect(doctor.locator('#settings-professional')).to_contain_text('Kenya Medical Practitioners and Dentists Council')
                await expect(doctor.locator('#settings-professional')).to_contain_text('Kenya (KE)')
                await expect(doctor.locator('#settings-professional')).to_contain_text('Professionally verified')
            assert not re.search(r'BMDC|Bangladesh', await doctor.locator('body').inner_text(), re.I)
            results['checks'][label + '_doctor_web_current_kenya_status_display'] = 'PASS'
            await doctor.screenshot(path=str(OUT / (label.lower() + '-doctor-status.png')), full_page=True)
            save()
            await browser_api(doctor, '/api/auth/logout', {})
            await context.close()
            save()
            print(label + ': terminal status, queue exclusion and Doctor display confirmed', flush=True)

        await admin.reload(wait_until='networkidle')
        await admin.locator('#adminNav-verification').click()
        await expect(admin.locator('#adminPage_verification')).to_contain_text('No Doctors are awaiting')
        assert not re.search(r'BMDC|Bangladesh', await admin.locator('body').inner_text(), re.I)
        await admin.screenshot(path=str(OUT / 'admin-pending-queue.png'), full_page=True)
        results['checks']['admin_refreshed_queue_excludes_both_terminal_fixtures'] = 'PASS'
        ids = [record['doctor_id'] for record in results['fixtures'].values()]
        assert all(re.fullmatch(r'[0-9a-f-]{36}', value) for value in ids)
        query = "SELECT json_build_object('id',id,'status',professional_verification_status,'authority',license_authority,'country',license_country,'is_verified',is_verified,'slots',(SELECT count(*) FROM doctor_schedule_slots s WHERE s.doctor_id=d.id)) FROM doctor_profiles d WHERE id IN (" + ','.join("'"+value+"'" for value in ids) + ") ORDER BY id"
        raw = subprocess.run(['docker','exec','hellodoctor-postgres','psql','-X','-At','-v','ON_ERROR_STOP=1','-U','hellodoctor','-d','hellodoctor','-c',query], check=True, capture_output=True, text=True, creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0).stdout
        persisted = [json.loads(line) for line in raw.splitlines() if line.startswith('{')]
        assert len(persisted) == 2 and {row['status'] for row in persisted} == {'VERIFIED','REJECTED'}
        assert all(row['authority']=='KMPDC' and row['country']=='KE' and row['slots']==0 for row in persisted)
        results['persisted_fixtures'] = persisted
        results['browser_exceptions'] = errors
        assert not errors
        results['verdict'] = 'PASS'
        save()
        # Admin logout returns a top-level success envelope without a data field.
        await admin.evaluate("fetch('/api/logout',{method:'POST'})")
        await browser.close()
        print(json.dumps({'verdict': results['verdict'], 'checks_passed': len(results['checks']), 'synthetic_doctors': len(results['fixtures']), 'browser_exceptions': len(errors)}))

if __name__ == '__main__':
    asyncio.run(walkthrough())
