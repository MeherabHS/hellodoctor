"""Real notification/API and requested UI regressions using installed Edge only."""
import asyncio
import json
import re
import uuid
from pathlib import Path
from playwright.async_api import async_playwright
from workflow_extra import token

HERE=Path(__file__).parent
ROOT=HERE.parents[2]
OUT=HERE/'notification-tools'
BASE='http://127.0.0.1:3001'

async def main():
    OUT.mkdir(exist_ok=True)
    result={'checks':{},'console_errors':[],'network':[],
            'fixture_scope':'Previously API-created synthetic Doctors; separate real UI login/MFA below.'}
    fixture=json.loads((HERE/'workflow/results.json').read_text())
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        contexts=[]
        for doctor in fixture['fixture_ids']['doctors']:
            ctx=await browser.new_context(viewport={'width':1440,'height':900})
            await ctx.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(doctor)),('hd_doctor_scope',str(uuid.uuid4()))]])
            contexts.append(ctx)
        ctx,other=contexts[::-1]
        async def api(context,path,method='GET'):
            headers={'Origin':BASE,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await context.cookies())}
            response=await context.request.fetch(BASE+'/api/proxy/'+path,method=method,headers=headers)
            return response.status,await response.json()
        status,inbox=await api(ctx,'notifications?limit=25&offset=0')
        other_status,theirs=await api(other,'notifications?limit=25&offset=0')
        assert status==other_status==200
        mine=inbox['data'];foreign=theirs['data']
        assert mine and foreign, 'Expected existing backend-generated development Doctor notifications'
        assert not set(n['id'] for n in mine)&set(n['id'] for n in foreign)
        assert all(n['user_id']==mine[0]['user_id'] for n in mine)
        assert mine[0]['user_id']!=foreign[0]['user_id']
        result['checks']['real_server_notification_inboxes_are_separate']='PASS'
        status,body=await api(ctx,'notifications/'+foreign[0]['id']+'/read',method='POST')
        assert status==200 and body['data']['marked'] is False
        _,after=await api(other,'notifications?limit=25&offset=0')
        assert next(n['read_at'] for n in after['data'] if n['id']==foreign[0]['id'])==foreign[0]['read_at']
        result['checks']['foreign_notification_mark_is_noop_and_owner_state_unchanged']='PASS'
        status,_=await api(ctx,'notifications?user_id='+foreign[0]['user_id'])
        assert status==422
        result['checks']['recipient_query_injection_rejected']='PASS'
        page=await ctx.new_page()
        page.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        page.on('response',lambda r:result['network'].append({'path':r.url.split(BASE)[-1].split('?')[0],'method':r.request.method,'status':r.status}) if r.url.startswith(BASE+'/api/') else None)
        await page.goto(BASE,wait_until='networkidle')
        assert await page.get_by_role('button',name=re.compile('Requisition Lab|Resume Telehealth Call')).count()==0
        await page.get_by_role('button',name='Open notifications',exact=True).click()
        await page.get_by_role('dialog',name='Notifications',exact=True).wait_for()
        await page.locator('[data-notification-id]').first.wait_for()
        assert await page.locator('#dwPage_dashboard').is_visible()
        await page.screenshot(animations='disabled',path=str(OUT/'notification-inbox-1440.png'))
        result['checks']['bell_opens_real_inbox_without_schedule_navigation']='PASS'
        unread=[n for n in mine if not n['read_at']]
        if unread:
            async with page.expect_response(lambda r:r.url.endswith('/notifications/'+unread[0]['id']+'/read') and r.request.method=='POST') as pending:
                await page.locator(f'[data-notification-id="{unread[0]["id"]}"]').get_by_role('button',name='Mark read',exact=True).click()
            response=await pending.value
            assert response.status==200 and (await response.json())['data']['marked'] is True
            await page.locator(f'[data-notification-id="{unread[0]["id"]}"]').get_by_role('button',name='Mark read',exact=True).wait_for(state='detached')
            _,current=await api(ctx,'notifications?limit=25&offset=0')
            assert next(n['read_at'] for n in current['data'] if n['id']==unread[0]['id'])
            result['checks']['ui_mark_read_confirmed_by_backend']='PASS'
        mark_all=page.get_by_role('button',name='Mark all read',exact=True)
        if await mark_all.is_enabled():
            async with page.expect_response(lambda r:r.url.endswith('/notifications/read-all') and r.request.method=='POST') as pending:
                await mark_all.click()
            assert (await pending.value).status==200
            await page.get_by_role('button',name='Mark read',exact=True).first.wait_for(state='detached')
        else:
            status,_=await api(ctx,'notifications/read-all',method='POST')
            assert status==200
        status,count=await api(ctx,'notifications/unread-count')
        assert status==200 and count['data']['unread_count']==0
        result['checks']['read_all_and_authoritative_unread_count']='PASS'
        daily=next((n for n in mine if n['notification_type']=='DAILY_CONSULTATION_SUMMARY'),None)
        if daily:
            await page.locator(f'[data-notification-id="{daily["id"]}"]').get_by_role('button',name='View details',exact=True).click()
            await page.locator('#dwPage_appointments').wait_for()
            assert await page.get_by_role('dialog').count()==0
            result['checks']['daily_summary_deep_link_opens_queue']='PASS'
        else:
            await page.keyboard.press('Escape')
        async def tool(label):
            await page.locator('.web-tools-menu summary').click()
            await page.locator('#doctorToolsList').get_by_role('button',name=label,exact=True).click()
        await tool('Patient history')
        await page.locator('#dwPage_patients').wait_for()
        assert await page.get_by_role('button',name=re.compile('register.*patient|patient registration',re.I)).count()==0
        result['checks']['patient_registration_control_removed']='PASS'
        await tool('Write prescription')
        await page.locator('#webRxWorkspace').wait_for()
        text=await page.locator('#webRxWorkspace').inner_text()
        assert not re.search(r'SpO2|BP mmHg|Pulse|Vitals and risk|Attached Lab',text)
        await page.screenshot(animations='disabled',path=str(OUT/'rx-without-vitals-1440.png'))
        result['checks']['rx_vitals_and_lab_controls_removed']='PASS'
        await tool('Patient chats')
        await page.locator('#webChatWorkspace').wait_for()
        assert await page.get_by_role('button',name=re.compile('Lab Report')).count()==0
        await page.screenshot(animations='disabled',path=str(OUT/'chat-from-tools-1440.png'))
        result['checks']['separate_chat_list_from_feature_menu']='PASS'
        await tool('Schedule slots')
        await page.locator('#webScheduleWorkspace').wait_for()
        result['checks']['separate_schedule_from_feature_menu']='PASS'
        await tool('Consultation queue')
        await page.locator('#dwPage_appointments').wait_for()
        await page.locator('.dw-nav-item[data-page="telehealth"]').click()
        await page.locator('#dwPage_telehealth').wait_for()
        assert await page.get_by_role('button',name=re.compile('Order Lab|Lab Report|Diagnostics & Labs')).count()==0
        result['checks']['telehealth_lab_actions_removed']='PASS'
        for width,height in [(1920,1080),(1440,900),(1024,768),(390,844)]:
            await page.set_viewport_size({'width':width,'height':height})
            await page.locator('.web-tools-menu summary').click()
            assert await page.locator('#doctorToolsList').is_visible()
            bounds=await page.locator('#doctorToolsList').bounding_box()
            assert bounds and bounds['x']>=0 and bounds['x']+bounds['width']<=width
            assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth')
            await page.screenshot(animations='disabled',path=str(OUT/f'feature-menu-{width}.png'))
            await page.keyboard.press('Escape')
            assert not await page.locator('#doctorToolsList').is_visible()
        result['checks']['feature_menu_four_viewports_and_escape']='PASS'
        await page.emulate_media(reduced_motion='reduce')
        animation=await page.evaluate('''() => { const ring=document.createElement('span'); ring.className='loading-spinner-ring web-state-spinner'; document.body.append(ring); const name=getComputedStyle(ring).animationName; ring.remove(); return name; }''')
        assert animation=='none'
        result['checks']['loading_ring_respects_reduced_motion']='PASS'
        for asset in ['calendar','bell']:
            response=await ctx.request.get(BASE+'/icons/'+asset+'.svg')
            assert response.status==200 and '<svg' in await response.text()
        result['checks']['prototype_icons_served_locally']='PASS'
        # Actual UI login/MFA with the controlled seed account, independent of fixture claims.
        source=(ROOT/'services/backend/src/seed.rs').read_text(encoding='utf-8')
        def constant(name):return re.search(r'pub const '+name+r': &str = "([^"]+)"',source).group(1)
        fresh=await browser.new_context(viewport={'width':1440,'height':900})
        login=await fresh.new_page()
        login.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        await login.goto(BASE,wait_until='networkidle')
        await login.get_by_label('Email address',exact=True).fill(constant('SEED_DOCTOR1_EMAIL'))
        await login.get_by_label('Password',exact=True).fill(constant('SEED_DOCTOR1_PASSWORD'))
        await login.get_by_role('button',name=re.compile('Doctor Secure Sign In')).click()
        await login.get_by_label('Verification code',exact=True).fill(await login.locator('.web-gap strong').inner_text())
        await login.get_by_role('button',name=re.compile('Verify & sign in')).click()
        await login.locator('#doctorWebShell').wait_for()
        await login.get_by_role('button',name='Open notifications',exact=True).click()
        await login.get_by_role('dialog',name='Notifications',exact=True).wait_for()
        await login.get_by_role('button',name='Refresh notifications',exact=True).click()
        await login.locator('[data-notification-id]').first.wait_for()
        if await login.locator('.web-state[role="alert"]').count():
            print(json.dumps({'seed_doctor_notification_alerts':await login.locator('.web-state[role="alert"]').all_text_contents()},ensure_ascii=True))
            await login.screenshot(animations='disabled',path=str(OUT/'notification-login-error.png'))
        assert not await login.locator('.web-state[role="alert"]').count()
        await login.screenshot(animations='disabled',path=str(OUT/'seed-doctor-real-login-notifications.png'))
        mark_read=login.get_by_role('button',name='Mark read',exact=True)
        if await mark_read.count():
            async with login.expect_response(lambda r:r.url.endswith('/read') and r.request.method=='POST') as pending:
                await mark_read.first.click()
            response=await pending.value
            assert response.status==200 and (await response.json())['data']['marked'] is True
            status,count=await api(fresh,'notifications/unread-count')
            assert status==200 and count['data']['unread_count']==0
            result['checks']['ui_mark_read_confirmed_by_backend']='PASS'
        await login.keyboard.press('Escape')
        await login.reload(wait_until='networkidle')
        await login.locator('#doctorWebShell').wait_for()
        await login.get_by_role('button',name='Open Doctor profile',exact=True).click()
        assert await login.get_by_role('button',name='My Articles',exact=True).count()==0
        await login.get_by_role('button',name='Log out',exact=True).click()
        await login.get_by_label('Email address',exact=True).wait_for()
        result['checks']['actual_login_mfa_notification_reload_logout']='PASS'
        assert not result['console_errors']
        assert not [r for r in result['network'] if r['status']>=500]
        result['build_id']=(ROOT/'apps/doctor-web/.next/BUILD_ID').read_text().strip()
        (OUT/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
        await browser.close()
    print(json.dumps({'checks':result['checks'],'console_errors':result['console_errors']}))

if __name__=='__main__':asyncio.run(main())
