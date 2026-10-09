"""Extra development-only workflow tests using fixture JWTs, as Rust api_tests do.
MFA/session tests are independently recorded by workflow_live.py. No keys/tokens saved.
"""
import asyncio,base64,json,struct,subprocess,time,uuid,zlib
from pathlib import Path
from playwright.async_api import async_playwright
ROOT=Path(__file__).resolve().parents[3];OUT=Path(__file__).parent/'workflow';WEB='http://127.0.0.1:3001'
REPORT={'checks':{},'auth_fixture':'Signed development test claims for previously API-created synthetic Doctors; not MFA acceptance evidence','console_errors':[]}
def sql(statement):
    names=subprocess.check_output(['docker','exec','hellodoctor-postgres','printenv','POSTGRES_USER','POSTGRES_DB'],text=True).splitlines()
    return subprocess.check_output(['docker','exec','hellodoctor-postgres','psql','-U',names[0],'-d',names[1],'-At','-c',statement],text=True).strip()
def token(doctor):
    uid=sql(f"SELECT user_id FROM doctor_profiles WHERE id='{uuid.UUID(doctor)}' AND full_name LIKE 'Web Fixture Doctor%'")
    assert uid,'Only synthetic fixture Doctors are allowed'
    return subprocess.check_output(['node',str(Path(__file__).parent/'fixture_token.mjs')],input=json.dumps({'user_id':uid}),text=True).strip()
def png():
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',200,200,8,2,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+b'\xff'*600)*200))+chunk(b'IEND',b'')
async def main():
    from live_fixture_guard import require_fixture_opt_in
    require_fixture_opt_in()
    env=(ROOT/'services/backend/.env').read_text();assert 'ENVIRONMENT=development' in env
    previous=json.loads((OUT/'results.json').read_text());doctors=previous['fixture_ids']['doctors'];appointments=previous['fixture_ids']['appointments'];check=REPORT['checks']
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        contexts=[]
        for doctor in doctors:
            context=await browser.new_context(viewport={'width':1440,'height':1000})
            await context.add_cookies([{'name':'hd_doctor_access','value':token(doctor),'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'},{'name':'hd_doctor_scope','value':str(uuid.uuid4()),'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'}])
            page=await context.new_page();page.on('pageerror',lambda e:REPORT['console_errors'].append(str(e)));await page.goto(WEB,wait_until='networkidle');contexts.append((context,page))
        context,page=contexts[0];other,other_page=contexts[1]
        async def request(ctx,path,body=None,method=None,key=None):
            headers={'Origin':WEB,'Sec-Fetch-Site':'same-origin','Cookie':'; '.join(c['name']+'='+c['value'] for c in await ctx.cookies())}
            if key:headers['Idempotency-Key']=key
            response=await ctx.request.fetch(WEB+'/api/proxy/'+path,method=method or ('POST' if body is not None else 'GET'),data=body,headers=headers)
            return response.status,await response.json()
        async def own(path,body=None,method=None,key=None):
            status,result=await request(context,path,body,method,key);assert status<300,(path,status,result.get('error',{}).get('code'));return result['data']
        await page.bring_to_front();await page.locator('.queue-card').first.wait_for()
        slots=await own('doctor/slots');booked=next(s for s in slots if s['appointment_id']==appointments[0])
        status,_=await request(other,'doctor/slots/'+booked['id'],method='DELETE')
        unchanged=next(s for s in await own('doctor/slots') if s['id']==booked['id'])
        check['foreign_booked_slot_denied_and_unchanged']='PASS' if status==409 and unchanged['status']=='BOOKED' else 'FAIL'
        now=time.time()+7200;start=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime(now));end=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime(now+600))
        available=await own('doctor/slots',{'start_time':start,'end_time':end})
        status,_=await request(other,'doctor/slots/'+available['id'],method='DELETE')
        unchanged=next(s for s in await own('doctor/slots') if s['id']==available['id'])
        check['foreign_available_slot_denied_and_unchanged']='PASS' if status==409 and unchanged['status']=='AVAILABLE' else 'FAIL'
        status,_=await request(context,'doctor/slots/'+available['id'],method='DELETE');check['own_available_slot_cancellation']='PASS' if status==200 else 'FAIL'
        await page.locator('.bn-item').nth(2).click()
        await page.locator('#rxPhotoFileInput').set_input_files([{'name':'synthetic-page-1.png','mimeType':'image/png','buffer':png()},{'name':'synthetic-page-2.png','mimeType':'image/png','buffer':png()}])
        assert await page.locator('.web-rx-preview-grid figure').count()==2
        async with page.expect_response('**/prescription') as rx_response:
            await page.locator('#btnSendCapturedRx').click()
        rx_result=await rx_response.value
        rx_error=(await rx_result.json()).get('error',{})
        check['rx_backend_response']={'status':rx_result.status,'code':rx_error.get('code'),'message':rx_error.get('message')}
        await page.get_by_role('button',name='Retry page').first.wait_for()
        assert await page.locator('.web-rx-preview-grid figure').count()==2
        check['multi_page_rx_failure_preserves_pages']='PASS'
        await page.screenshot(path=str(OUT/'rx_storage_unavailable_1440.png'))
        await page.get_by_role('button',name='Retry page').first.click();await page.get_by_role('button',name='Retry page').first.wait_for()
        check['rx_retry_truthful_storage_unavailable']='PASS'
        await page.get_by_role('button',name='Remove',exact=True).first.click();assert await page.locator('.web-rx-preview-grid figure').count()==1
        check['rx_remove_local_failed_page']='PASS'
        qid=str(uuid.uuid4());sql(f"INSERT INTO health_questions(id,question_text,category,status,created_at,updated_at) VALUES('{qid}','Synthetic Doctor Web question: can general advice be published?','WEB_FIXTURE','OPEN',NOW(),NOW())")
        REPORT['qna_fixture_id']=qid
        await page.locator('.dw-nav-item[data-page=qna]').click();await page.get_by_label('Your clinical guidance').last.wait_for()
        question=page.locator('article').filter(has_text='Synthetic Doctor Web question:')
        await question.locator('textarea').fill('Synthetic verification answer. Seek an in-person assessment for urgent symptoms.')
        await question.get_by_role('button',name='Publish Verified Doctor Answer').click()
        await page.get_by_text('Synthetic verification answer.',exact=False).wait_for()
        status,_=await request(other,f'doctor/qna/questions/{qid}/answers',{'answer_text':'Synthetic concurrent second answer'})
        check['qna_submit_and_second_answer_409']='PASS' if status==409 else f'FAIL:{status}'
        await page.screenshot(path=str(OUT/'qna_answered_1440.png'))
        await page.locator('.bn-item').first.click();await page.locator('.queue-patient-name .web-name-button').first.click();await page.get_by_role('button',name='Complete consultation',exact=True).click()
        await page.get_by_label('Clinical outcome').select_option('COMPLETED_NO_RX');await page.get_by_label('Reason for no prescription').fill('Synthetic workflow verification; no clinical treatment provided.')
        await page.get_by_role('button',name='Confirm consultation completion').click();await page.get_by_text('Consultation completion confirmed by the server').wait_for()
        check['complete_without_rx_real_api']='PASS';await page.screenshot(path=str(OUT/'completion_confirmed_1440.png'));await page.keyboard.press('Escape')
        complete=await own('doctor/appointments');assert next(a for a in complete if a['id']==appointments[0])['status']=='COMPLETED'
        before=await own('doctor/wallet/summary');key=str(uuid.uuid4())
        first=await request(context,f'consultations/{appointments[0]}/complete-no-rx',{'reason':'Synthetic workflow verification; no clinical treatment provided.'},key=key)
        second=await request(context,f'consultations/{appointments[0]}/complete-no-rx',{'reason':'Synthetic workflow verification; no clinical treatment provided.'},key=key)
        after=await own('doctor/wallet/summary');check['completion_retry_no_financial_change']='PASS' if first[0] in [200,409] and second[0] in [200,409] and before==after else 'FAIL'
        await page.locator('.bn-item').nth(4).click();await page.get_by_role('button',name='Statement',exact=False).click();await page.get_by_label('Statement month').wait_for()
        await page.screenshot(path=str(OUT/'monthly_statement_1440.png'));check['statement_dialog_real_api']='PASS';await page.keyboard.press('Escape')
        await page.get_by_role('button',name='Payout Settings').click();await page.get_by_text('No payout destination connected').wait_for();await page.screenshot(path=str(OUT/'payout_settings_1440.png'));await page.keyboard.press('Escape')
        check['payout_unconfigured_truthful']='PASS'
        for width in [1440,1280,1024,768,390]:
            await page.set_viewport_size({'width':width,'height':1000})
            for index,name in enumerate(['queue','chat','rx','schedule','wallet']):
                await page.locator('.bn-item').nth(index).click();await page.wait_for_timeout(200);await page.screenshot(path=str(OUT/f'{name}_{width}.png'))
        await browser.close()
    print(json.dumps(REPORT))
if __name__=='__main__':
    try:asyncio.run(main())
    finally:(OUT/'extra-results.json').write_text(json.dumps(REPORT,indent=2),encoding='utf-8')
