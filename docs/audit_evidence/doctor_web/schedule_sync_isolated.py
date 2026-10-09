"""Exercise both production portals against a newly provisioned throwaway DB.

Never targets the shared backend. Uses installed Edge and existing local tools;
all created slots and logins are confined to the owned, disposable database.
"""
import asyncio
import base64
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import socket
import struct
import subprocess
import time
from datetime import datetime, timedelta, timezone
from urllib.parse import urlsplit, urlunsplit
from urllib.request import urlopen
from playwright.async_api import async_playwright, expect

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'services/backend/target/schedule-sync-followup'
OUT.mkdir(parents=True, exist_ok=True)
DB = 'hd_schedule_sync_' + str(int(time.time()))
assert re.fullmatch(r'hd_schedule_sync_[0-9]+', DB)
SEED = (ROOT / 'services/backend/src/seed.rs').read_text(encoding='utf-8')
FLAGS = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0

def credential(name):
    return re.search(r'pub const ' + name + r': &str = "([^"]+)"', SEED).group(1)

def totp():
    raw = hmac.new(base64.b32decode(credential('SEED_ADMIN_TOTP_SECRET_BASE32')),
                   struct.pack('>Q', int(time.time()) // 30), hashlib.sha1).digest()
    offset = raw[-1] & 15
    return str((struct.unpack('>I', raw[offset:offset + 4])[0] & 0x7fffffff) % 1000000).zfill(6)

def command(args):
    return subprocess.run(args, check=True, capture_output=True, text=True, creationflags=FLAGS).stdout.strip()

def sql(query, database='hellodoctor'):
    return command(['docker', 'exec', 'hellodoctor-postgres', 'psql', '-v', 'ON_ERROR_STOP=1', '-At',
                    '-U', 'hellodoctor', '-d', database, '-c', query])

def dotenv(path):
    values = {}
    for line in path.read_text(encoding='utf-8-sig').splitlines():
        line = line.strip()
        if not line or line.startswith('#') or '=' not in line:
            continue
        key, value = line.split('=', 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values

async def ready(url, processes):
    for _ in range(120):
        assert all(process.poll() is None for process in processes), 'Isolated server exited; inspect local logs.'
        try:
            with urlopen(url, timeout=1) as response:
                if response.status == 200:
                    return
        except Exception:
            pass
        await asyncio.sleep(0.5)
    raise AssertionError('Isolated server readiness timeout: ' + url)

async def walkthrough():
    result = {}
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel='msedge', headless=True)
        doctor = await browser.new_context(timezone_id='Asia/Dhaka')
        page = await doctor.new_page()
        errors = []
        page.on('pageerror', lambda error: errors.append(type(error).__name__))
        doctor_base = 'http://127.0.0.1:3002'
        await page.goto(doctor_base, wait_until='networkidle')
        login = await page.evaluate("""async body=>{const r=await fetch('/api/auth/login',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});return {status:r.status,body:await r.json()};}""", {
            'email': credential('SEED_DOCTOR1_EMAIL'), 'password': credential('SEED_DOCTOR1_PASSWORD')})
        assert login['status'] == 200
        verify = await page.evaluate("""async otp_code=>{const r=await fetch('/api/auth/verify',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({otp_code})});return r.status;}""", login['body']['data']['development_otp'])
        assert verify == 200
        await page.goto(doctor_base, wait_until='networkidle')
        schedule_nav = page.locator('#doctorWebSidebar').get_by_role('button', name='Schedule & Slots', exact=True)
        await expect(schedule_nav).to_be_visible()
        navigation = page.locator('#doctorWebSidebar .dw-nav-item')
        entries = await navigation.evaluate_all('(nodes)=>nodes.map(node=>node.dataset.page)')
        assert entries.index('schedule') + 1 == entries.index('appointments')
        await schedule_nav.click()
        await expect(schedule_nav).to_have_attribute('aria-current', 'page')
        assert await page.locator('#doctorWebSidebar [data-page="appointments"]').get_attribute('aria-current') is None
        await expect(page.locator('#webScheduleWorkspace')).to_be_visible()
        await page.go_back()
        await expect(page.locator('[data-web-screen="dashboard"]')).to_be_visible()
        await page.go_forward()
        await expect(page.locator('#webScheduleWorkspace')).to_be_visible()
        result['schedule_sidebar_before_appointments_direct_access_and_history'] = 'PASS'
        await page.set_viewport_size({'width': 390, 'height': 844})
        await page.get_by_role('button', name='Open navigation', exact=True).click()
        await schedule_nav.click()
        await expect(page.get_by_role('button', name='Open navigation', exact=True)).to_have_attribute('aria-expanded', 'false')
        await page.get_by_role('button', name='+ Add Schedule Slot', exact=True).click()
        await expect(page.get_by_label('Slot date', exact=True)).to_be_visible()
        await page.keyboard.press('Escape')
        await expect(page.get_by_role('dialog')).to_have_count(0)
        result['narrow_sidebar_direct_slot_creation_and_drawer_cleanup'] = 'PASS'
        await page.set_viewport_size({'width': 1440, 'height': 900})
        admin = await browser.new_context(timezone_id='Asia/Dhaka')
        admin_base = 'http://localhost:3003'
        response = await admin.request.post(admin_base + '/api/login', data={
            'email': credential('SEED_ADMIN_EMAIL'), 'password': credential('SEED_ADMIN_PASSWORD'), 'totp_code': totp()})
        assert response.status == 200
        admin_page = await admin.new_page()
        admin_page.on('pageerror', lambda error: errors.append(type(error).__name__))
        await admin_page.goto(admin_base, wait_until='networkidle')
        await admin_page.locator('#adminNav-slots').click()
        day = (datetime.now(timezone(timedelta(hours=3))) + timedelta(days=35)).date().isoformat()
        await admin_page.get_by_label('Schedule date', exact=True).fill(day)

        await page.get_by_role('button', name='+ Add Schedule Slot', exact=True).click()
        await page.get_by_label('Slot date', exact=True).fill(day)
        await page.get_by_label('Generate sequential 10-minute slots', exact=True).uncheck()
        await page.get_by_label('Start time', exact=True).fill('13:00')
        await page.get_by_label('End time', exact=True).fill('13:10')
        async with page.expect_response(lambda r: r.url.endswith('/api/proxy/doctor/slots') and r.request.method == 'POST') as pending:
            await page.get_by_role('button', name=re.compile('Generate & Publish')).click()
        response = await pending.value
        assert response.status == 201
        slot = (await response.json())['data']
        slot_id = slot['id']
        await page.keyboard.press('Escape')
        card = page.locator(f'[data-slot-id="{slot_id}"]')
        chip = admin_page.locator(f'[data-slot-id="{slot_id}"]')
        await expect(card).to_have_attribute('data-slot-status', 'AVAILABLE')
        await expect(page.get_by_test_id('available-selected-date')).to_have_text('1 available on this date')
        await expect(chip).to_have_attribute('data-slot-status', 'AVAILABLE', timeout=22000)
        assert '13:00' in await chip.inner_text() and '13:00' in await card.inner_text()
        assert 'Open for booking' in await card.inner_text()
        assert sql(f"SELECT count(*) FROM appointments WHERE slot_id='{slot_id}'", DB) == '0'
        result['doctor_creation_reaches_admin_without_manual_refresh'] = 'PASS'
        result['same_eat_time_on_dhaka_device'] = 'PASS'
        result['available_slot_is_not_a_booking'] = 'PASS'
        print('Isolated creation and time-zone agreement: PASS', flush=True)

        await chip.click()
        await expect(chip).to_have_attribute('data-slot-status', 'BLOCKED')
        await expect(card).to_have_attribute('data-slot-status', 'BLOCKED', timeout=22000)
        await expect(page.get_by_test_id('available-selected-date')).to_have_text('0 available on this date')
        result['admin_block_reaches_doctor_without_manual_refresh'] = 'PASS'
        await card.get_by_role('button', name='Make available', exact=True).click()
        await page.get_by_role('dialog').get_by_role('button', name='Make available', exact=True).click()
        await expect(card).to_have_attribute('data-slot-status', 'AVAILABLE')
        await expect(chip).to_have_attribute('data-slot-status', 'AVAILABLE', timeout=22000)
        result['doctor_unblock_reaches_admin_without_manual_refresh'] = 'PASS'
        print('Isolated cross-portal block and unblock: PASS', flush=True)

        async def broken_feed(route):
            await route.fulfill(status=503, content_type='application/json', body='{"success":false}')
        await admin_page.route('**/api/proxy/admin/slots', broken_feed)
        await admin_page.get_by_role('button', name='Refresh schedule', exact=True).click()
        await expect(admin_page.get_by_role('alert').filter(has_text='Schedule refresh failed')).to_be_visible()
        await expect(chip).to_have_attribute('data-slot-status', 'AVAILABLE')
        before = sql(f"SELECT count(*) FROM audit_events WHERE resource_id='{slot_id}'", DB)
        await chip.click()
        assert sql(f"SELECT count(*) FROM audit_events WHERE resource_id='{slot_id}'", DB) == before
        result['failed_admin_read_is_visible_preserves_snapshot_and_disables_stale_action'] = 'PASS'
        await admin_page.unroute('**/api/proxy/admin/slots', broken_feed)
        await admin_page.get_by_role('button', name='Refresh schedule', exact=True).click()
        await expect(admin_page.get_by_role('alert').filter(has_text='Schedule refresh failed')).to_have_count(0)

        await page.route('**/api/proxy/doctor/slots?*', broken_feed)
        await page.get_by_role('button', name='Refresh', exact=True).click()
        await expect(page.get_by_role('alert').filter(has_text='Schedule refresh failed')).to_be_visible()
        await expect(card).to_have_attribute('data-slot-status', 'AVAILABLE')
        result['failed_doctor_read_is_visible_and_preserves_snapshot'] = 'PASS'
        await page.unroute('**/api/proxy/doctor/slots?*', broken_feed)
        await page.get_by_role('button', name='Refresh', exact=True).click()
        await expect(page.get_by_role('alert').filter(has_text='Schedule refresh failed')).to_have_count(0)

        await card.get_by_role('button', name='Cancel slot', exact=True).click()
        await page.get_by_role('button', name='Confirm cancellation', exact=True).click()
        await expect(card).to_have_attribute('data-slot-status', 'CANCELLED')
        await expect(page.get_by_test_id('available-selected-date')).to_have_text('0 available on this date')
        result['availability_count_follows_selected_date_and_block_cancel_state'] = 'PASS'
        await expect(chip).to_have_attribute('data-slot-status', 'CANCELLED', timeout=22000)
        result['doctor_cancellation_reaches_admin_without_manual_refresh'] = 'PASS'
        queue_reads = []
        page.on('request', lambda r: queue_reads.append(r.url) if '/api/proxy/doctor/appointments?' in r.url else None)
        await page.wait_for_timeout(16500)
        assert queue_reads, 'Appointment queue did not poll'
        result['appointment_queue_also_refreshes'] = 'PASS'
        assert not errors, errors
        assert sql(f"SELECT count(*) FROM doctor_schedule_slots WHERE id='{slot_id}'") == '0'
        result['test_slot_absent_from_shared_database'] = 'PASS'
        result['browser_exceptions'] = len(errors)
        (OUT / 'isolated-browser-results.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
        print(json.dumps(result), flush=True)
        await browser.close()

async def main():
    processes = []
    logs = []
    created = False
    redis_owned = False
    for port in (8081, 3002, 3003):
        with socket.socket() as sock:
            assert sock.connect_ex(('127.0.0.1', port)) != 0, f'Test port already occupied: {port}'
    assert command(['docker', 'exec', 'hellodoctor-redis', 'redis-cli', '-n', '14', 'DBSIZE']) == '0', 'Test Redis DB must be empty'
    try:
        sql(f'CREATE DATABASE {DB}')
        created = True
        env = {**os.environ, **dotenv(ROOT / 'services/backend/.env')}
        parsed = urlsplit(env['DATABASE_URL'])
        env.update(DATABASE_URL=urlunsplit(parsed._replace(path='/' + DB)), APP_DB_NAME=DB,
                   ENVIRONMENT='development', SEED_DEV_DATA='true', HOST='127.0.0.1', PORT='8081',
                   REDIS_URL='redis://127.0.0.1:6379/14', DB_POOL_MAX_CONNECTIONS_PER_ROLE='2',
                   GLOBAL_IP_RATE_LIMIT_PER_MIN='10000', RUST_LOG='error', PUBLIC_API_BASE_URL='http://127.0.0.1:8081')
        redis_owned = True
        log = (OUT / 'isolated-backend.log').open('w', encoding='utf-8')
        logs.append(log)
        processes.append(subprocess.Popen([str(ROOT / 'services/backend/target/debug/hellodoctor-backend.exe')],
                         cwd=ROOT / 'services/backend', env=env, stdout=log, stderr=log, creationflags=FLAGS))
        await ready('http://127.0.0.1:8081/healthz', processes)
        for app, port, origin in [('doctor-web', 3002, 'http://127.0.0.1:3002'), ('admin-web', 3003, 'http://localhost:3003')]:
            web_env = {**os.environ, **dotenv(ROOT / f'apps/{app}/.env.local'),
                       'BACKEND_URL': 'http://127.0.0.1:8081', 'DOCTOR_WEB_ORIGIN': origin, 'NODE_ENV': 'production'}
            log = (OUT / f'isolated-{app}.log').open('w', encoding='utf-8')
            logs.append(log)
            processes.append(subprocess.Popen(['node', 'node_modules/next/dist/bin/next', 'start', '-H', '127.0.0.1', '-p', str(port)],
                             cwd=ROOT / f'apps/{app}', env=web_env, stdout=log, stderr=log, creationflags=FLAGS))
            await ready(origin + ('/login' if app == 'admin-web' else ''), processes)
        print('Isolated servers ready; shared database not used for writes.', flush=True)
        await walkthrough()
    finally:
        for process in reversed(processes):
            process.terminate()
        for process in processes:
            process.wait(timeout=15)
        for log in logs:
            log.close()
        if created:
            sql(f'DROP DATABASE {DB} WITH (FORCE)')
        if redis_owned:
            command(['docker', 'exec', 'hellodoctor-redis', 'redis-cli', '-n', '14', 'FLUSHDB'])
        print('Owned test database, test processes and test Redis data cleaned up.', flush=True)

if __name__ == '__main__':
    asyncio.run(main())
