"""Exercise real Web slot creation on existing synthetic Doctor data, installed Edge.

Only slots created by this run are cancelled afterward using the existing API.
No tokens, new accounts, PHI, dependencies or infrastructure are saved/created.
"""
import asyncio
import json
import re
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from playwright.async_api import async_playwright
from workflow_extra import token

HERE = Path(__file__).parent
OUT = HERE / 'slot-fix'
BASE = 'http://127.0.0.1:3001'
EAT = timezone(timedelta(hours=3))

async def main():
    from live_fixture_guard import require_fixture_opt_in
    require_fixture_opt_in()
    OUT.mkdir(exist_ok=True)
    fixture = json.loads((HERE / 'workflow/results.json').read_text())
    doctor = fixture['fixture_ids']['doctors'][0]
    result = {'checks': {}, 'console_errors': [], 'network': [], 'created_slot_ids': [], 'cleanup': [],
              'auth_scope': 'Existing synthetic Doctor signed development claims; not a new MFA test.'}
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel='msedge', headless=True)
        context = await browser.new_context(viewport={'width':1440, 'height':900})
        await context.add_cookies([{'name':name, 'value':value, 'domain':'127.0.0.1', 'path':'/api',
                                   'httpOnly':True, 'secure':True, 'sameSite':'Strict'}
                                  for name,value in [('hd_doctor_access',token(doctor)), ('hd_doctor_scope',str(uuid.uuid4()))]])
        page = await context.new_page()
        page.on('pageerror', lambda error: result['console_errors'].append(str(error)))
        page.on('response', lambda response: result['network'].append({'path':response.url.split(BASE)[-1].split('?')[0],
            'method':response.request.method, 'status':response.status}) if response.url.startswith(BASE+'/api/') else None)
        async def api(path, method='GET'):
            headers = {'Origin':BASE, 'Cookie':'; '.join(c['name']+'='+c['value'] for c in await context.cookies())}
            return await context.request.fetch(BASE+'/api/proxy/'+path, method=method, headers=headers)
        async def fill(date, start, end):
            await page.get_by_label('Slot date', exact=True).click()
            assert await page.get_by_role('dialog').count() == 1
            await page.get_by_label('Slot date', exact=True).fill(date)
            await page.get_by_label('Start time', exact=True).click()
            await page.get_by_label('Start time', exact=True).fill(start)
            await page.get_by_label('End time', exact=True).fill(end)
        async def publish(expected=201):
            async with page.expect_response(lambda r:r.url.endswith('/api/proxy/doctor/slots') and r.request.method=='POST') as pending:
                await page.get_by_role('button', name=re.compile('Generate & Publish')).click()
            response = await pending.value
            assert response.status == expected, (response.status, await response.text())
            return await response.json()
        try:
            await page.goto(BASE, wait_until='networkidle')
            assert await page.get_by_role('button',name='Resume Telehealth Call',exact=True).count() == 0
            result['checks']['dashboard_resume_call_removed'] = 'PASS'
            await page.locator('.dw-nav-item[data-page="appointments"]').click()
            await page.get_by_role('button',name='+ Add Schedule Slot',exact=True).click()
            dialog = page.get_by_role('dialog')
            await page.get_by_label('Slot date',exact=True).click()
            await dialog.locator('button.toast-dismiss-btn').click()
            assert await page.get_by_role('dialog').count() == 0
            result['checks']['inside_click_keeps_dialog_open_and_close_button_works'] = 'PASS'
            await page.get_by_role('button',name='+ Add Schedule Slot',exact=True).click()
            existing = await api('doctor/slots?limit=100&offset=0')
            assert existing.status == 200
            rows = (await existing.json())['data']
            occupied = {datetime.fromisoformat(row['start_time'].replace('Z','+00:00')).astimezone(EAT).date().isoformat()
                        for row in rows if row['status'] != 'CANCELLED'}
            candidate = datetime.now(EAT).date()+timedelta(days=2)
            while candidate.isoformat() in occupied:
                candidate += timedelta(days=1)
            date = candidate.isoformat()
            await fill(date,'13:00','13:10')
            await page.get_by_label('Generate sequential 10-minute slots',exact=True).uncheck()
            data = await publish()
            slot_id = data['data']['id']
            result['created_slot_ids'].append(slot_id)
            assert data['data']['start_time'] == date+'T10:00:00Z'
            assert data['data']['end_time'] == date+'T10:10:00Z'
            await page.get_by_text('1 slot(s) created by the server',exact=True).wait_for()
            await page.keyboard.press('Escape')
            await page.locator(f'[data-slot-id="{slot_id}"]').wait_for()
            assert await page.locator('#webScheduleWorkspace input[type="date"]').input_value() == date
            await page.screenshot(animations='disabled',path=str(OUT/'single-slot-created.png'))
            result['checks']['single_slot_post_201_correct_eat_time_and_visible_schedule'] = 'PASS'
            await page.get_by_role('button',name='+ Add Schedule Slot',exact=True).click()
            await fill(date,'13:00','13:10')
            await publish(expected=409)
            await page.get_by_role('alert').filter(has_text='overlap').wait_for()
            assert await page.get_by_label('Start time',exact=True).is_enabled()
            result['checks']['server_overlap_conflict_keeps_form_editable'] = 'PASS'
            await fill(date,'13:20','13:40')
            posts_before = sum(r['method']=='POST' and r['path']=='/api/proxy/doctor/slots' for r in result['network'])
            await publish()
            await page.get_by_text('2 slot(s) created by the server',exact=True).wait_for()
            rows_after = (await (await api('doctor/slots?limit=100&offset=0')).json())['data']
            batch = [r for r in rows_after if r['status']=='AVAILABLE' and r['start_time'] in [date+'T10:20:00Z',date+'T10:30:00Z']]
            assert len(batch)==2
            result['created_slot_ids'].extend(r['id'] for r in batch)
            posts_after = sum(r['method']=='POST' and r['path']=='/api/proxy/doctor/slots' for r in result['network'])
            assert posts_after-posts_before==2
            await page.keyboard.press('Escape')
            for row in batch:
                await page.locator(f'[data-slot-id="{row["id"]}"]').wait_for()
            result['checks']['batch_two_real_slots_exactly_two_posts'] = 'PASS'
            await page.reload(wait_until='networkidle')
            await page.locator('.dw-nav-item[data-page="appointments"]').click()
            await page.get_by_role('button',name='Manage consultation availability',exact=True).click()
            await page.locator('#webScheduleWorkspace input[type="date"]').fill(date)
            for ident in result['created_slot_ids']:
                await page.locator(f'[data-slot-id="{ident}"]').wait_for()
            await page.screenshot(animations='disabled',path=str(OUT/'slots-persist-after-reload.png'))
            result['checks']['all_created_slots_persist_after_browser_reload'] = 'PASS'
            await page.get_by_role('button',name='+ Add Schedule Slot',exact=True).click()
            past=(datetime.now(EAT).date()-timedelta(days=1)).isoformat()
            await fill(past,'13:00','13:10')
            posts_before=sum(r['method']=='POST' and r['path']=='/api/proxy/doctor/slots' for r in result['network'])
            await page.get_by_role('button',name=re.compile('Generate & Publish')).click()
            await page.get_by_role('alert').filter(has_text='future start time').wait_for()
            assert await page.get_by_label('Slot date',exact=True).is_enabled()
            assert posts_before==sum(r['method']=='POST' and r['path']=='/api/proxy/doctor/slots' for r in result['network'])
            await page.keyboard.press('Escape')
            result['checks']['past_time_validation_without_freezing_or_request'] = 'PASS'
            assert not result['console_errors']
        finally:
            for ident in result['created_slot_ids']:
                response=await api('doctor/slots/'+ident,method='DELETE')
                result['cleanup'].append({'slot_id':ident,'status':response.status})
            result['build_id']=(HERE.parents[2]/'apps/doctor-web/.next/BUILD_ID').read_text().strip()
            (OUT/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
            await browser.close()
    assert all(r['status']==200 for r in result['cleanup'])
    print(json.dumps({'checks':result['checks'],'created_slots':len(result['created_slot_ids']),
                      'cleanup':'PASS','console_errors':result['console_errors']}))

if __name__=='__main__':
    asyncio.run(main())
