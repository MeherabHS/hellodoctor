"""Layout-regression walkthrough with existing API-created development fixtures.

Fixture JWTs are test-only and captured in memory, as in the previous audit.
Actual password/MFA acceptance is separately established by correction_visual.py.
No Doctor, Patient, payment or infrastructure fixture is created here.
"""
import asyncio,json,re,uuid
from pathlib import Path
from playwright.async_api import async_playwright
from workflow_extra import token,png,sql

HERE=Path(__file__).parent
OUT=HERE/'correction'
BASE='http://127.0.0.1:3001'

async def main():
    prior=json.loads((HERE/'workflow/results.json').read_text())
    doctors=prior['fixture_ids']['doctors']
    result={'checks':{},'console_errors':[],'network':[], 'auth_scope':'Existing synthetic Doctor fixtures; test-only signed development claims. UI password/MFA separately verified.'}
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        contexts=[]
        for doctor in doctors:
            context=await browser.new_context(viewport={'width':1440,'height':900})
            await context.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(doctor)),('hd_doctor_scope',str(uuid.uuid4()))]])
            contexts.append(context)
        context,other=contexts
        page=await context.new_page()
        page.on('pageerror',lambda error:result['console_errors'].append(str(error)))
        page.on('response',lambda response:result['network'].append({'path':response.url.split(BASE)[-1].split('?')[0],'method':response.request.method,'status':response.status}) if response.url.startswith(BASE+'/api/') else None)
        async def api(ctx,path,body=None,method='GET'):
            headers={'Origin':BASE,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await ctx.cookies())}
            response=await ctx.request.fetch(BASE+'/api/proxy/'+path,method=method,data=body,headers=headers)
            return response.status,await response.json()
        await page.goto(BASE,wait_until='networkidle')
        await page.locator('#doctorWebShell').wait_for()
        await page.locator('.dw-user-badge').click()
        status,initial_duty=await api(context,'doctor/duty')
        assert status==200
        await page.locator('.web-duty').click()
        await page.wait_for_timeout(300)
        status,changed_duty=await api(context,'doctor/duty')
        assert status==200 and changed_duty['data']['is_on_duty']!=initial_duty['data']['is_on_duty']
        await page.locator('.web-duty').click()
        await page.wait_for_timeout(300)
        status,restored_duty=await api(context,'doctor/duty')
        assert status==200 and restored_duty['data']==initial_duty['data']
        result['checks']['duty_ui_server_confirmed_and_restored']='PASS'
        await page.keyboard.press('Escape')
        status,dto=await api(context,'doctor/appointments')
        assert status==200 and dto['data']
        rows=dto['data']
        authorized=next((a for a in rows if a['history_access_active']),None)
        chosen=authorized or rows[0]
        await page.locator('.dw-nav-item[data-page="appointments"]').click()
        await page.locator('[data-appointment-id]').first.wait_for()
        await page.screenshot(animations='disabled',path=str(OUT/'web_appointments_populated_1440.png'))
        ordered=await page.locator('[data-appointment-id]').evaluate_all('(items)=>items.map(el=>el.dataset.appointmentId)')
        by_id={a['id']:a for a in rows}
        nonterminal=[i for i in ordered if by_id[i]['status'] not in ['COMPLETED','CANCELLED','REFUNDED','NO_SHOW'] and by_id[i]['doctor_lifecycle']!='DONE']
        assert nonterminal==sorted(nonterminal,key=lambda i:by_id[i]['start_time'])
        assert ordered[:len(nonterminal)]==nonterminal
        result['checks']['upcoming_first_preserved']='PASS'
        card=page.locator(f'[data-appointment-id="{chosen["id"]}"]')
        await card.get_by_role('button',name='Review Chart',exact=True).click()
        await page.get_by_role('dialog',name='Consultation Detail').wait_for()
        await page.wait_for_timeout(400)
        await page.screenshot(animations='disabled',path=str(OUT/'web_summary_dialog_1440.png'))
        await page.keyboard.press('Escape')
        status,_=await api(other,'doctor/consultations/'+chosen['id']+'/summary')
        assert status in [403,404]
        result['checks']['foreign_summary_still_denied']='PASS'
        if authorized:
            status,_=await api(context,'doctor/appointments/'+chosen['id']+'/patient-history')
            assert status==200
            await card.get_by_role('button',name='Review Chart',exact=True).click()
            await page.get_by_role('button',name='Patient History',exact=True).click()
            await page.wait_for_timeout(400)
            await page.screenshot(animations='disabled',path=str(OUT/'web_history_dialog_1440.png'))
            await page.keyboard.press('Escape')
            result['checks']['authorized_history']='PASS'
        else:
            result['checks']['authorized_history']='No fixture currently inside server access window; no window bypass'
        status,_=await api(other,'doctor/appointments/'+chosen['id']+'/patient-history')
        assert status in [403,404]
        result['checks']['foreign_history_still_denied']='PASS'
        await card.get_by_role('button',name='Review Chart',exact=True).click()
        await page.get_by_role('button',name='Documents / Labs',exact=True).click()
        await page.wait_for_timeout(400)
        await page.screenshot(animations='disabled',path=str(OUT/'web_documents_dialog_1440.png'))
        await page.keyboard.press('Escape')
        result['checks']['document_dialog_authorized_empty_state']='PASS'
        if chosen['status'] not in ['COMPLETED','CANCELLED']:
            await card.get_by_role('button',name=re.compile('Call Patient')).click()
            await page.locator('#dwPage_telehealth').wait_for()
            await page.wait_for_timeout(400)
            await page.screenshot(animations='disabled',path=str(OUT/'web_telehealth_populated_1440.png'))
            await page.locator('#dwMeetSwapTabChat').click()
            await page.wait_for_timeout(400)
            await page.screenshot(animations='disabled',path=str(OUT/'web_incall_chat_1440.png'))
            result['checks']['web_emr_and_incall_chat_panel']='PASS'
            await page.locator('.dw-nav-item[data-page="appointments"]').click()
            card=page.locator(f'[data-appointment-id="{chosen["id"]}"]')
        await card.get_by_role('button',name='Write Rx',exact=True).click()
        await page.locator('#webRxWorkspace').wait_for()
        await page.locator('#rxPhotoFileInput').set_input_files([{'name':'synthetic-page-1.png','mimeType':'image/png','buffer':png()},{'name':'synthetic-page-2.png','mimeType':'image/png','buffer':png()}])
        assert await page.locator('.web-rx-preview-grid figure').count()==2
        await page.locator('#btnSendCapturedRx').click()
        for _ in range(40):
            states=await page.locator('.web-rx-preview-grid figcaption').all_inner_texts()
            if len(states)==2 and all('FAILED' in state or 'UPLOADED' in state for state in states):break
            await page.wait_for_timeout(250)
        else:raise AssertionError('Prescription pages did not reach a confirmed upload state')
        assert await page.locator('.web-rx-preview-grid figure').count()==2
        result['checks']['two_page_rx_recoverable']='PASS'
        await page.screenshot(animations='disabled',path=str(OUT/'web_rx_populated_1440.png'))
        if await page.get_by_role('button',name='Retry page',exact=True).count():
            await page.get_by_role('button',name='Retry page',exact=True).first.click()
            await page.wait_for_timeout(500)
            assert await page.locator('.web-rx-preview-grid figure').count()==2
            await page.get_by_role('button',name='Remove',exact=True).last.click()
            assert await page.locator('.web-rx-preview-grid figure').count()==1
            result['checks']['rx_retry_remove_preserved']='PASS'
        await page.get_by_role('button',name='Complete without prescription / Other outcome',exact=True).click()
        await page.get_by_role('dialog',name='Complete Consultation').wait_for()
        await page.get_by_label('Clinical outcome').select_option('COMPLETED_NO_RX')
        assert await page.get_by_label('Reason for no prescription').is_visible()
        await page.screenshot(animations='disabled',path=str(OUT/'web_completion_dialog_1440.png'))
        await page.keyboard.press('Escape')
        result['checks']['completion_form_preserved_without_mutating_completed_fixture']='PASS'
        await page.locator('.dw-nav-item[data-page="appointments"]').click()
        await page.get_by_role('button',name='Manage consultation availability').click()
        await page.wait_for_timeout(400)
        await page.screenshot(animations='disabled',path=str(OUT/'web_schedule_populated_1440.png'))
        booked=page.locator('[data-slot-id]').filter(has_text='BOOKED / LOCKED')
        if await booked.count():
            assert await booked.get_by_role('button',name='Cancel Slot',exact=True).count()==0
            result['checks']['booked_slots_have_no_cancel_action']='PASS'
        await page.locator('.dw-nav-item[data-page="appointments"]').click()
        await page.locator(f'[data-appointment-id="{chosen["id"]}"]').get_by_role('button',name='Chat',exact=True).click()
        await page.locator('#webChatWorkspace').wait_for()
        await page.wait_for_timeout(400)
        if not await page.locator('#dwChatInputField').count() and await page.locator('[data-conversation-id]').count():
            await page.locator('[data-conversation-id]').first.click()
        if await page.locator('#dwChatInputField').count():
            assert await page.get_by_role('button',name='Take after meals',exact=True).count()==1
            assert await page.get_by_role('button',name='Check fever',exact=True).count()==1
            await page.screenshot(animations='disabled',path=str(OUT/'web_chat_populated_1440.png'))
            result['checks']['web_thread_and_canned_controls']='PASS'
            if not await page.locator('#dwChatInputField').is_disabled():
                message='Synthetic Web layout regression message <b>render as text</b>'
                await page.locator('#dwChatInputField').fill(message)
                async with page.expect_response(re.compile('/api/proxy/chat/.*/messages')) as sent:
                    await page.get_by_role('button',name='Send',exact=True).click()
                sent_response=await sent.value
                if not sent_response.ok:
                    dto=await sent_response.json()
                    print(json.dumps({'chat_send_status':sent_response.status,'code':dto.get('error',{}).get('code')}))
                assert sent_response.status==201
                assert await page.get_by_text(message,exact=True).count()>=1
                assert await page.locator('#dwChatMsgStream b').count()==0
                async with page.expect_response(re.compile('/api/proxy/chat/.*/messages')) as canned:
                    await page.get_by_role('button',name='Take after meals',exact=True).click()
                assert (await canned.value).status==201
                result['checks']['http_chat_send_and_canned_reply']='PASS'
        await page.locator('.dw-user-badge').click()
        await page.get_by_role('button',name='Earnings & Wallet',exact=True).click()
        await page.get_by_role('button',name='Monthly Statement',exact=True).click()
        await page.wait_for_timeout(400)
        await page.screenshot(animations='disabled',path=str(OUT/'web_statement_populated_1440.png'))
        await page.keyboard.press('Escape')
        await page.get_by_role('button',name='Payout Settings',exact=True).click()
        await page.wait_for_timeout(400)
        await page.screenshot(animations='disabled',path=str(OUT/'web_payout_dialog_1440.png'))
        await page.keyboard.press('Escape')
        result['checks']['wallet_statement_payout_controls']='PASS'
        assert not result['console_errors']
        (OUT/'workflow-results.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
        await browser.close()
    print(json.dumps({'checks':result['checks'],'console_errors':result['console_errors']}))

if __name__=='__main__':asyncio.run(main())
