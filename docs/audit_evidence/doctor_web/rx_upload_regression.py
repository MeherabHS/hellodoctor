"""Prescription upload diagnosis and camera removal with installed Edge.

Uses existing synthetic Doctor/appointment fixtures, then actual seed login/MFA.
No storage configuration, database mutation, browser installation or token output.
"""
import asyncio
import json
import re
import uuid
from pathlib import Path
from playwright.async_api import async_playwright, expect
from workflow_extra import png, token, sql

HERE=Path(__file__).parent
ROOT=HERE.parents[2]
OUT=HERE/'rx-upload'
BASE='http://127.0.0.1:3001'

async def main():
    OUT.mkdir(exist_ok=True)
    result={'checks':{},'console_errors':[],'upload_responses':[],
            'scope':'Existing synthetic fixtures for file upload; actual seed login/MFA for empty consultation state.'}
    fixtures=json.loads((HERE/'workflow/results.json').read_text())
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        selected=None
        for doctor in fixtures['fixture_ids']['doctors']:
            ctx=await browser.new_context(viewport={'width':1440,'height':900})
            await ctx.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(doctor)),('hd_doctor_scope',str(uuid.uuid4()))]])
            headers={'Origin':BASE,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await ctx.cookies())}
            response=await ctx.request.get(BASE+'/api/proxy/doctor/appointments',headers=headers)
            assert response.status==200
            rows=(await response.json())['data']
            for row in rows:
                if not row['history_access_active']:continue
                appointment=str(uuid.UUID(row['id']))
                count=sql(f"SELECT COUNT(*) FROM prescriptions WHERE appointment_id='{appointment}'")
                if count=='0':selected=(ctx,row);break
            if selected:break
            await ctx.close()
        assert selected,'No existing authorized fixture without a prescription; do not bypass authorization.'
        ctx,appointment=selected
        before=sql(f"SELECT COUNT(*) FROM prescriptions WHERE appointment_id='{uuid.UUID(appointment['id'])}'")
        page=await ctx.new_page()
        page.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        await page.goto(BASE,wait_until='networkidle')
        await page.locator('#doctorWebShell').wait_for()
        await page.locator('#doctorWebSidebar [data-page="appointments"]').click()
        await page.locator(f'[data-appointment-id="{appointment["id"]}"]').get_by_role('button',name='Write Rx',exact=True).click()
        await page.locator('#webRxWorkspace').wait_for()
        await page.get_by_label('Prescription consultation').select_option(appointment['id'])
        assert await page.get_by_role('button',name=re.compile('Camera|Capture|Take prescription')).count()==0
        result['checks']['doctor_camera_capture_removed']='PASS'
        async with page.expect_file_chooser() as chooser:
            await page.get_by_role('button',name='Upload Rx Photo',exact=True).click()
        await (await chooser.value).set_files([{'name':f'synthetic-page-{i}.png','mimeType':'image/png','buffer':png()} for i in [1,2]])
        await expect(page.locator('.web-rx-preview-grid figure')).to_have_count(2)
        await expect(page.locator('#btnSendCapturedRx')).to_be_enabled()
        assert all('LOCAL' in text for text in await page.locator('.web-rx-preview-grid figcaption').all_inner_texts())
        result['checks']['file_button_opens_picker_and_two_local_previews']='PASS'
        await page.screenshot(animations='disabled',path=str(OUT/'local-photo-preview-1440.png'))
        async with page.expect_response(lambda r:r.url.endswith('/prescription') and r.request.method=='POST') as pending:
            await page.locator('#btnSendCapturedRx').click()
        response=await pending.value
        dto=await response.json()
        error=dto.get('error',{})
        result['upload_responses'].append({'status':response.status,'code':error.get('code'),'message':error.get('message')})
        assert response.status==409 and 'Document storage is not configured' in error.get('message','')
        for caption in await page.locator('.web-rx-preview-grid figcaption').all():
            await expect(caption).to_contain_text('FAILED')
        assert await page.get_by_text('Server confirmed',exact=True).count()==0
        result['checks']['real_storage_rejection_visible_without_false_success']='PASS'
        await page.screenshot(animations='disabled',path=str(OUT/'storage-unconfigured-1440.png'))
        async with page.expect_response(lambda r:r.url.endswith('/prescription') and r.request.method=='POST') as pending:
            await page.get_by_role('button',name='Retry page',exact=True).first.click()
        retry=await pending.value
        assert retry.status==409
        await page.get_by_role('button',name='Retry page',exact=True).first.wait_for(state='visible')
        await expect(page.locator('#btnSendCapturedRx')).to_be_enabled()
        assert await page.locator('.web-rx-preview-grid figure').count()==2
        await page.get_by_role('button',name='Remove',exact=True).last.click()
        assert await page.locator('.web-rx-preview-grid figure').count()==1
        result['checks']['failed_page_retry_and_removal_preserve_other_page']='PASS'
        after=sql(f"SELECT COUNT(*) FROM prescriptions WHERE appointment_id='{uuid.UUID(appointment['id'])}'")
        assert before==after=='0'
        result['checks']['failed_upload_creates_no_prescription_record']='PASS'
        for width,height in [(1920,1080),(1024,768),(390,844)]:
            await page.set_viewport_size({'width':width,'height':height})
            assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth')
            assert await page.get_by_role('button',name='Upload Rx Photo',exact=True).is_visible()
            assert await page.get_by_role('button',name=re.compile('Camera|Capture|Take prescription')).count()==0
            await page.screenshot(animations='disabled',path=str(OUT/f'upload-workspace-{width}.png'))
        result['checks']['file_only_workspace_three_responsive_widths']='PASS'
        fresh=await browser.new_context(viewport={'width':1440,'height':900})
        login=await fresh.new_page()
        login.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        source=(ROOT/'services/backend/src/seed.rs').read_text(encoding='utf-8')
        def constant(name):return re.search(r'pub const '+name+r': &str = "([^"]+)"',source).group(1)
        await login.goto(BASE,wait_until='networkidle')
        await login.get_by_label('Email address',exact=True).fill(constant('SEED_DOCTOR1_EMAIL'))
        await login.get_by_label('Password',exact=True).fill(constant('SEED_DOCTOR1_PASSWORD'))
        await login.get_by_role('button',name=re.compile('Doctor Secure Sign In')).click()
        await login.get_by_label('Verification code',exact=True).wait_for()
        await login.get_by_label('Verification code',exact=True).fill(await login.locator('.web-gap strong').inner_text())
        await login.get_by_role('button',name=re.compile('Verify & sign in')).click()
        await login.locator('#doctorWebShell').wait_for()
        await login.get_by_text('No consultations scheduled today.',exact=True).wait_for()
        assert await login.get_by_role('button',name='Instant Rx',exact=True).count()==0
        assert await login.get_by_role('button',name='Write prescription',exact=True).count()==0
        assert await login.get_by_role('button',name=re.compile('Camera|Capture|Take prescription')).count()==0
        await login.screenshot(animations='disabled',path=str(OUT/'no-consultation-explanation-1440.png'))
        result['checks']['actual_login_mfa_no_context_free_prescription_action']='PASS'
        assert not result['console_errors']
        result['build_id']=(ROOT/'apps/doctor-web/.next/BUILD_ID').read_text().strip()
        (OUT/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
        await browser.close()
    print(json.dumps(result))

if __name__=='__main__':asyncio.run(main())
