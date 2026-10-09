"""Dedicated Patient Chats navigation, current APIs and two-Doctor ownership.

Existing Edge only. Expiry is tested by temporarily expiring an existing
synthetic Doctor conversation; its exact original state is restored in finally.
One synthetic UI message is removed by exact returned id after verification.
"""
import asyncio
import datetime
import json
import re
import uuid
from pathlib import Path
from playwright.async_api import async_playwright, expect
from workflow_extra import token, sql

HERE=Path(__file__).parent
ROOT=HERE.parents[2]
OUT=HERE/'patient-chats'
BASE='http://127.0.0.1:3001'

async def main():
    OUT.mkdir(exist_ok=True)
    result={'checks':{},'console_errors':[],'network':[],'unread_count':'NOT_SUPPORTED','last_message_preview':'NOT_SUPPORTED','classification_filters':'Removed: no authoritative backend classifications'}
    fixture=json.loads((HERE/'workflow/results.json').read_text())
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        contexts=[]
        for doctor in fixture['fixture_ids']['doctors']:
            ctx=await browser.new_context(viewport={'width':1440,'height':900})
            await ctx.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(doctor)),('hd_doctor_scope',str(uuid.uuid4()))]])
            contexts.append(ctx)
        owner,attacker=contexts
        async def api(context,path,method='GET',body=None):
            headers={'Origin':BASE,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await context.cookies())}
            if method=='POST':headers['Idempotency-Key']=str(uuid.uuid4())
            response=await context.request.fetch(BASE+'/api/proxy/'+path,method=method,data=body,headers=headers)
            return response.status,await response.json()
        _,own=await api(owner,'doctor/chats?limit=100&offset=0')
        status,other=await api(attacker,'doctor/chats?limit=100&offset=0')
        assert status==200 and own['data']
        ids={c['id'] for c in own['data']}
        assert not ids.intersection(c['id'] for c in other['data'])
        conversation=next(c for c in own['data'] if not c['is_locked'])
        cid=str(uuid.UUID(conversation['id']))
        status,_=await api(attacker,'chat/'+cid+'/messages')
        assert status in [403,404]
        before=sql(f"SELECT COUNT(*) FROM chat_messages WHERE conversation_id='{cid}'")
        status,_=await api(attacker,'chat/'+cid+'/messages','POST',{'content':'Synthetic foreign Doctor denial check'})
        assert status in [403,404]
        after=sql(f"SELECT COUNT(*) FROM chat_messages WHERE conversation_id='{cid}'")
        assert before==after
        result['checks']['two_doctor_list_read_and_send_ownership']='PASS'
        page=await owner.new_page()
        page.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        page.on('response',lambda r:result['network'].append({'path':r.url.split(BASE)[-1].split('?')[0],'method':r.request.method,'status':r.status}) if r.url.startswith(BASE+'/api/') else None)
        await page.goto(BASE,wait_until='networkidle')
        await page.locator('#doctorWebShell').wait_for()
        await page.locator('#doctorWebSidebar [data-page="chat"]').click()
        await page.locator('#webChatWorkspace').wait_for()
        assert page.url==BASE+'/doctor/chats'
        await expect(page.locator('#doctorWebSidebar [data-page="chat"]')).to_have_attribute('aria-current','page')
        assert await page.locator('#doctorWebSidebar [data-page="telehealth"]').get_attribute('aria-current') is None
        assert await page.locator('.web-tools-menu').count()==0
        assert await page.get_by_role('button',name=re.compile('^Urgent$|^Follow-up$|^Refill$')).count()==0
        await expect(page.get_by_role('heading',name='Select a conversation',exact=True)).to_be_visible()
        result['checks']['primary_sidebar_chat_selected_separate_from_telehealth']='PASS'
        await page.screenshot(animations='disabled',path=str(OUT/'inbox-no-selection-1440.png'))
        await page.locator(f'[data-conversation-id="{cid}"]').click()
        await page.locator('#dwChatMsgStream').wait_for()
        _,existing=await api(owner,'chat/'+cid+'/messages')
        for message in existing['data']:
            assert await page.locator('#dwChatMsgStream .web-clinical-text').filter(has_text=message['content']).count()>0
        marker='Synthetic Patient Chats navigation verification.'
        message_id=None
        try:
            await page.get_by_label('Chat message',exact=True).fill(marker)
            async with page.expect_response(lambda r:r.url.endswith('/messages') and r.request.method=='POST') as pending:
                await page.get_by_role('button',name='Send',exact=True).click()
            response=await pending.value
            assert response.status==201
            message_id=str(uuid.UUID((await response.json())['data']['id']))
            await expect(page.locator('#dwChatMsgStream').get_by_text(marker,exact=True)).to_be_visible()
            result['checks']['real_backend_message_history_and_ui_send']='PASS'
            await page.screenshot(animations='disabled',path=str(OUT/'thread-active-1440.png'))
        finally:
            if message_id:sql(f"DELETE FROM chat_messages WHERE id='{message_id}' AND conversation_id='{cid}' AND content='{marker}'")
        await page.get_by_role('button',name='All chats',exact=True).click()
        await page.get_by_label('Search conversations',exact=True).fill('does-not-match-synthetic-patient')
        await expect(page.get_by_text('No matching conversations',exact=True)).to_be_visible()
        await page.get_by_label('Search conversations',exact=True).fill('')
        await page.get_by_role('button',name='Active',exact=True).click()
        assert await page.locator('[data-conversation-id]').count()==len([c for c in own['data'] if not c['is_locked']])
        result['checks']['conversation_search_and_authoritative_filters']='PASS'
        # Exact test-fixture guard; never touch ordinary Doctors or clinical data.
        original_expiry=datetime.datetime.fromisoformat(conversation['expires_at'].replace('Z','+00:00')).isoformat()
        original_locked='TRUE' if conversation['is_locked'] else 'FALSE'
        guard=f"id='{cid}' AND doctor_id IN (SELECT id FROM doctor_profiles WHERE full_name LIKE 'Web Fixture Doctor%')"
        try:
            sql(f"UPDATE chat_conversations SET expires_at=NOW()-INTERVAL '1 minute' WHERE {guard}")
            status,_=await api(owner,'chat/'+cid+'/messages','POST',{'content':'Synthetic expiry denial check'})
            assert status==403
            await page.get_by_role('button',name='Refresh inbox',exact=True).click()
            await page.get_by_role('button',name='Read-only',exact=True).click()
            await page.locator(f'[data-conversation-id="{cid}"]').click()
            await expect(page.get_by_label('Chat message',exact=True)).to_be_disabled()
            await expect(page.get_by_role('button',name='Send',exact=True)).to_be_disabled()
            await expect(page.get_by_text('Consultation window closed. This conversation is read-only.',exact=True)).to_be_visible()
            await expect(page.locator('#dwChatMsgStream .web-state')).to_have_count(0)
            await page.screenshot(animations='disabled',path=str(OUT/'thread-readonly-1440.png'))
            result['checks']['real_backend_expiry_denial_and_read_only_composer']='PASS'
        finally:
            sql(f"UPDATE chat_conversations SET expires_at='{original_expiry}'::timestamptz,is_locked={original_locked} WHERE {guard}")
        await page.get_by_role('button',name='Refresh inbox',exact=True).click()
        await page.get_by_role('button',name='All',exact=True).click()
        await page.locator(f'[data-conversation-id="{cid}"]').wait_for()
        await page.locator(f'[data-conversation-id="{cid}"]').click()
        for width,height in [(1920,1080),(1440,900),(1280,720),(1024,768),(390,844)]:
            await page.set_viewport_size({'width':width,'height':height})
            assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth')
            if width>=1024:
                bounds=await page.locator('#webChatWorkspace').bounding_box()
                thread=await page.locator('.web-chat-thread').bounding_box()
                assert bounds['width']>width-260 and thread['width']>width-620
            await page.screenshot(animations='disabled',path=str(OUT/f'patient-chat-{width}.png'))
        result['checks']['fluid_two_pane_desktop_five_viewports']='PASS'
        await page.set_viewport_size({'width':1440,'height':900})
        await page.locator('#doctorWebSidebar [data-page="telehealth"]').click()
        await page.locator('#dwPage_telehealth').wait_for()
        await page.locator('#dwMeetSwapTabChat').click()
        await page.locator('#dwChatMsgStream').wait_for()
        assert await page.locator('.web-chat-list').count()==0
        await page.screenshot(animations='disabled',path=str(OUT/'telehealth-contextual-chat-1440.png'))
        result['checks']['in_call_chat_retains_same_thread_contract']='PASS'
        await page.go_back()
        await page.locator('#webChatWorkspace').wait_for()
        assert page.url==BASE+'/doctor/chats'
        await page.go_forward()
        await page.locator('#dwPage_telehealth').wait_for()
        await page.go_back()
        await page.reload(wait_until='networkidle')
        await page.locator('#webChatWorkspace').wait_for()
        await expect(page.locator('#doctorWebSidebar [data-page="chat"]')).to_have_attribute('aria-current','page')
        result['checks']['dedicated_url_back_forward_reload_session_restore']='PASS'
        await page.get_by_label('Search owned appointments',exact=False).fill('synthetic')
        await page.locator('#dwPage_appointments').wait_for()
        assert page.url==BASE+'/'
        result['checks']['global_appointment_search_distinct_from_chat_search']='PASS'
        fresh=await browser.new_context(viewport={'width':1440,'height':900})
        login=await fresh.new_page()
        login.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        source=(ROOT/'services/backend/src/seed.rs').read_text(encoding='utf-8')
        def constant(name):return re.search(r'pub const '+name+r': &str = "([^"]+)"',source).group(1)
        await login.goto(BASE+'/doctor/chats',wait_until='networkidle')
        await login.get_by_label('Email address',exact=True).fill(constant('SEED_DOCTOR1_EMAIL'))
        await login.get_by_label('Password',exact=True).fill(constant('SEED_DOCTOR1_PASSWORD'))
        await login.get_by_role('button',name=re.compile('Doctor Secure Sign In')).click()
        await login.get_by_label('Verification code',exact=True).wait_for()
        await login.get_by_label('Verification code',exact=True).fill(await login.locator('.web-gap strong').inner_text())
        await login.get_by_role('button',name=re.compile('Verify & sign in')).click()
        await login.locator('#webChatWorkspace').wait_for()
        await expect(login.get_by_text('No conversations yet',exact=True)).to_be_visible()
        await login.screenshot(animations='disabled',path=str(OUT/'empty-chat-real-login-1440.png'))
        result['checks']['cold_chat_url_real_login_mfa_empty_inbox']='PASS'
        assert not result['console_errors']
        assert not [r for r in result['network'] if r['status']>=500]
        result['build_id']=(ROOT/'apps/doctor-web/.next/BUILD_ID').read_text().strip()
        (OUT/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
        await browser.close()
    print(json.dumps({'checks':result['checks'],'console_errors':result['console_errors']}))

if __name__=='__main__':asyncio.run(main())
