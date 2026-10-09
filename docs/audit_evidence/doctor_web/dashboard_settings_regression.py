"""Dashboard and settings regression using existing Edge and real APIs.

Existing synthetic Doctor only for reversible display-name/duty edits. Real
seed password/MFA session for empty Dashboard, account menu and logout.
"""
import asyncio
import json
import re
import uuid
from pathlib import Path
from playwright.async_api import async_playwright, expect
from workflow_extra import token

HERE=Path(__file__).parent
ROOT=HERE.parents[2]
OUT=HERE/'dashboard-settings'
BASE='http://127.0.0.1:3001'

async def main():
    OUT.mkdir(exist_ok=True)
    result={'checks':{},'console_errors':[],'network':[]}
    fixture=json.loads((HERE/'workflow/results.json').read_text())
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        ctx=await browser.new_context(viewport={'width':1440,'height':900})
        await ctx.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(fixture['fixture_ids']['doctors'][0])),('hd_doctor_scope',str(uuid.uuid4()))]])
        async def api(context,path,method='GET',body=None):
            headers={'Origin':BASE,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await context.cookies())}
            response=await context.request.fetch(BASE+'/api/proxy/'+path,method=method,data=body,headers=headers)
            return response.status,await response.json()
        page=await ctx.new_page()
        page.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        page.on('response',lambda r:result['network'].append({'path':r.url.split(BASE)[-1].split('?')[0],'method':r.request.method,'status':r.status}) if r.url.startswith(BASE+'/api/') else None)
        await page.goto(BASE,wait_until='networkidle')
        await page.locator('#doctorWebShell').wait_for()
        await expect(page.locator('.web-dashboard-summary .dw-kpi-card')).to_have_count(4)
        await expect(page.locator('.web-dashboard-queue [data-appointment-id]')).to_have_count(1)
        text=await page.locator('#dwPage_dashboard').inner_text()
        assert not re.search(r'Unavailable|Current API|Backend data|Digital Prescriptions|Instant Rx|Modality|Recent Prescription|Resume Telehealth|Requisition',text)
        assert not re.search(r'[\U0001F300-\U0001FAFF]',text)
        assert await page.get_by_text(re.compile('Patient Queue')).count()==0
        assert await page.locator('.web-tools-menu').count()==0
        result['checks']['four_supported_summaries_and_redundant_widgets_removed']='PASS'
        status,rows=await api(ctx,'doctor/appointments')
        assert status==200
        active=[a for a in rows['data'] if a['status'] not in ['COMPLETED','CANCELLED','REFUNDED','NO_SHOW'] and a['doctor_lifecycle']!='DONE']
        ids=await page.locator('.web-dashboard-queue [data-appointment-id]').evaluate_all('(rows)=>rows.map(row=>row.dataset.appointmentId)')
        assert ids==[a['id'] for a in active[:5]]
        _,wallet=await api(ctx,'doctor/wallet/summary')
        assert 'KES' in await page.get_by_role('region',name='Eligible Earnings',exact=True).inner_text()
        result['checks']['operational_queue_uses_backend_order_and_kes_earnings']='PASS'
        await page.screenshot(animations='disabled',path=str(OUT/'dashboard-populated-1440.png'))
        await page.get_by_label('Open account menu',exact=True).click()
        await expect(page.get_by_role('menu',name='Doctor account')).to_be_visible()
        assert not await page.get_by_role('dialog',name='Doctor Profile & Settings').count()
        await page.screenshot(animations='disabled',path=str(OUT/'account-menu-1440.png'))
        await page.keyboard.press('Escape')
        await expect(page.get_by_role('menu',name='Doctor account')).not_to_be_visible()
        await expect(page.get_by_label('Open account menu',exact=True)).to_be_focused()
        await page.keyboard.press('ArrowDown')
        await expect(page.get_by_role('menuitem',name='View profile')).to_be_focused()
        await page.keyboard.press('Escape')
        result['checks']['account_menu_keyboard_escape_focus_and_no_old_modal']='PASS'
        await page.locator('#doctorWebSidebar [data-page="settings"]').click()
        await page.locator('#webSettingsWorkspace').wait_for()
        assert await page.locator('.web-modal').count()==0
        assert await page.locator('.dw-doc-profile-card[role="button"]').count()==0
        result['checks']['settings_full_workspace_sidebar_identity_static']='PASS'
        nav=page.get_by_role('navigation',name='Settings sections')
        _,original=await api(ctx,'doctor/profile')
        original_name=original['data']['full_name']
        changed=False
        try:
            await page.get_by_label('Display name',exact=True).fill(original_name+' UI check')
            await page.get_by_role('button',name='Save changes',exact=True).click()
            await expect(page.get_by_text('Profile updated',exact=True)).to_be_visible()
            _,saved=await api(ctx,'doctor/profile')
            assert saved['data']['full_name']==original_name+' UI check'
            changed=True
            await page.get_by_label('Display name',exact=True).fill(original_name)
            await page.get_by_role('button',name='Save changes',exact=True).click()
            await expect(page.get_by_text('Profile updated',exact=True)).to_be_visible()
            _,restored=await api(ctx,'doctor/profile')
            assert restored['data']['full_name']==original_name
            changed=False
            result['checks']['real_profile_save_local_confirmation_and_restore']='PASS'
        finally:
            if changed:await api(ctx,'doctor/profile','PATCH',{'full_name':original_name})
        await page.get_by_label('Display name',exact=True).fill('   ')
        await page.get_by_role('button',name='Save changes',exact=True).click()
        await expect(page.locator('#settingsNameError')).to_be_visible()
        await page.get_by_label('Display name',exact=True).fill(original_name)
        await page.screenshot(animations='disabled',path=str(OUT/'settings-profile-1440.png'))
        result['checks']['profile_field_validation']='PASS'
        status,_=await api(ctx,'doctor/profile','PATCH',{'is_verified':True,'current_hospital':'unauthorized change'})
        assert status in [400,422]
        _,protected=await api(ctx,'doctor/profile')
        for key in ['license_number','current_hospital','qualifications','is_verified','primary_specialty']:
            assert original['data'][key]==protected['data'][key]
        await nav.get_by_role('button',name='Professional Details',exact=True).click()
        assert await page.locator('#settings-professional input').count()==0
        await page.screenshot(animations='disabled',path=str(OUT/'settings-professional-1440.png'))
        result['checks']['verified_fields_read_only_and_backend_rejects_mass_assignment']='PASS'
        await nav.get_by_role('button',name='Availability',exact=True).click()
        _,before=await api(ctx,'doctor/duty')
        duty=page.get_by_role('switch',name='On duty',exact=True)
        await duty.click()
        await expect(duty).to_have_attribute('aria-checked',str(not before['data']['is_on_duty']).lower())
        _,changed_duty=await api(ctx,'doctor/duty')
        assert changed_duty['data']['is_on_duty']!=before['data']['is_on_duty']
        await duty.click()
        await expect(duty).to_have_attribute('aria-checked',str(before['data']['is_on_duty']).lower())
        await page.screenshot(animations='disabled',path=str(OUT/'settings-availability-1440.png'))
        result['checks']['availability_duty_server_confirmed_and_restored']='PASS'
        await nav.get_by_role('button',name='Payout',exact=True).click()
        await expect(page.locator('#settings-payout').get_by_text('No payout destination connected',exact=True)).to_be_visible()
        assert not await page.locator('#settings-payout input').count()
        await page.screenshot(animations='disabled',path=str(OUT/'settings-payout-1440.png'))
        await page.locator('#settings-payout').get_by_role('button',name='View Wallet').click()
        await page.locator('#webEarningsWorkspace').wait_for()
        result['checks']['masked_payout_state_external_deferral_and_contextual_wallet_link']='PASS'
        await page.locator('#doctorWebSidebar [data-page="settings"]').click()
        await nav.get_by_role('button',name='Security',exact=True).click()
        await page.screenshot(animations='disabled',path=str(OUT/'settings-security-1440.png'))
        assert not await page.locator('#settings-profile').get_by_role('button',name='Log out',exact=True).count()
        result['checks']['security_session_actions_separate_from_profile']='PASS'
        for width,height in [(1920,1080),(1440,900),(1024,768),(390,844)]:
            await page.set_viewport_size({'width':width,'height':height})
            await nav.get_by_role('button',name='Profile',exact=True).click()
            assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth')
            await page.screenshot(animations='disabled',path=str(OUT/f'settings-profile-{width}.png'))
        result['checks']['settings_four_viewports_no_horizontal_overflow']='PASS'
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
        await expect(login.get_by_text('No consultations scheduled today.',exact=True)).to_be_visible()
        await expect(login.get_by_role('region',name="Today's Consultations",exact=True).locator('.dw-kpi-val')).to_have_text('0')
        for width,height in [(1920,1080),(1440,900),(1024,768),(390,844)]:
            await login.set_viewport_size({'width':width,'height':height})
            assert await login.evaluate('document.documentElement.scrollWidth<=innerWidth')
            await login.screenshot(animations='disabled',path=str(OUT/f'dashboard-empty-{width}.png'))
        result['checks']['actual_login_mfa_one_empty_queue_four_dashboard_viewports']='PASS'
        await login.get_by_label('Open account menu',exact=True).click()
        await login.get_by_role('menuitem',name='View profile',exact=True).click()
        await expect(login.locator('#settings-professional')).to_be_visible()
        await login.get_by_label('Open account menu',exact=True).click()
        await login.get_by_role('menuitem',name='Notifications',exact=True).click()
        await login.get_by_role('dialog',name='Notifications',exact=True).wait_for()
        await login.keyboard.press('Escape')
        await login.get_by_label('Open account menu',exact=True).click()
        await login.get_by_role('menuitem',name='Log out',exact=True).click()
        await login.get_by_label('Email address',exact=True).wait_for()
        result['checks']['account_profile_notification_and_real_logout']='PASS'
        assert not result['console_errors']
        assert not [r for r in result['network'] if r['status']>=500]
        result['build_id']=(ROOT/'apps/doctor-web/.next/BUILD_ID').read_text().strip()
        (OUT/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
        await browser.close()
    print(json.dumps({'checks':result['checks'],'console_errors':result['console_errors']}))

if __name__=='__main__':asyncio.run(main())
