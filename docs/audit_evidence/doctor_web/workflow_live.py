"""Controlled development fixtures through existing APIs; never real provider payment.
Secrets/cookies stay in this process. Report contains only assertions and fixture IDs.
Uses existing Edge and existing approved Postgres solely for a synthetic public question.
"""
import asyncio,base64,hashlib,hmac,json,re,secrets,struct,time,uuid,subprocess
from datetime import datetime,timedelta,timezone
from pathlib import Path
from playwright.async_api import async_playwright
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).parent/'workflow'
WEB='http://127.0.0.1:3001'; BACK='http://127.0.0.1:8080/api/v1/'
REPORT={}
def iso(value):return value.isoformat().replace('+00:00','Z')

async def main():
    from live_fixture_guard import require_fixture_opt_in
    require_fixture_opt_in()
    OUT.mkdir(exist_ok=True)
    remaining=int(subprocess.check_output(['docker','exec','hellodoctor-redis','redis-cli','TTL','ratelimit:login:127.0.0.1'],text=True).strip())
    if remaining>0:
        print(f'Waiting for the existing login-rate window ({remaining}s); no controls modified.',flush=True)
        while remaining>0:
            wait=min(remaining+1,50);await asyncio.sleep(wait);remaining-=wait
    report=REPORT
    report.update({'checks':{},'fixture_ids':{},'console_errors':[],'simulation':'Development webhook only; zero-value fees; no real payment'})
    check=report['checks']
    source=(ROOT/'services/backend/src/seed.rs').read_text()
    def constant(name):return re.search(rf'pub const {name}: &str = "([^"]+)"',source)[1]
    config=dict(line.split('=',1) for line in (ROOT/'services/backend/.env').read_text().splitlines() if '=' in line and not line.startswith('#'))
    assert config['ENVIRONMENT'].strip('"\'')=='development','Development fixtures only'
    nonce=uuid.uuid4().hex[:10]
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        raw=await pw.request.new_context()
        async def backend(path,body=None,token=None,method=None):
            response=await raw.fetch(BACK+path,method=method or ('POST' if body is not None else 'GET'),data=body,headers={'Authorization':'Bearer '+token} if token else {})
            dto=await response.json()
            if not response.ok:raise RuntimeError(f'{path}: {response.status} {dto.get("error",{}).get("code")}')
            return dto['data']
        key=base64.b32decode(constant('SEED_ADMIN_TOTP_SECRET_BASE32'))
        digest=hmac.new(key,struct.pack('>Q',int(time.time())//30),hashlib.sha1).digest();offset=digest[-1]&15
        otp=str((struct.unpack('>I',digest[offset:offset+4])[0]&0x7fffffff)%1000000).zfill(6)
        admin=await backend('auth/admin/login',{'email':constant('SEED_ADMIN_EMAIL'),'password':constant('SEED_ADMIN_PASSWORD'),'totp_code':otp})
        doctors=[]
        for label in ['A','B']:
            email=f'doctor-web-{nonce}-{label.lower()}@example.test'
            created=await backend('admin/doctors',{'email':email,'full_name':f'Web Fixture Doctor {label}','license_number':f'WEB-{nonce}-{label}','primary_specialty':'General Medicine','experience_years':5,'current_hospital':'Synthetic Test Clinic','verified_phone':'+2547'+str(secrets.randbelow(100000000)).zfill(8),'consultation_fee_video':'0','consultation_fee_chat':'0','qualifications':['Test MBBS'],'residential_address':'Synthetic test location'},admin['access_token'])
            await backend('admin/doctors/'+created['doctor']['id']+'/verify', {'note':'Synthetic fixture manual review'}, admin['access_token'])
            doctors.append({'email':email,'password':created['temporary_password'],'id':created['doctor']['id']})
        report['fixture_ids']['doctors']=[d['id'] for d in doctors]
        async def login(doctor):
            context=await browser.new_context(viewport={'width':1440,'height':1000})
            page=await context.new_page();page.on('pageerror',lambda e:report['console_errors'].append(str(e)))
            await page.goto(WEB,wait_until='networkidle')
            await page.get_by_label('Email address',exact=True).fill(doctor['email'])
            await page.get_by_label('Password',exact=True).fill(doctor['password'])
            for attempt in range(3):
                async with page.expect_response('**/api/auth/login') as login_response:
                    await page.get_by_role('button',name='Doctor Secure Sign In').click()
                login_result=await login_response.value
                if login_result.ok:break
                error=(await login_result.json()).get('error',{})
                if login_result.status==429 and attempt<2:
                    await asyncio.sleep(61);continue
                raise RuntimeError(f'Doctor fixture login {login_result.status}: {error.get("code")}')
            await page.get_by_label('Verification code').wait_for()
            await page.get_by_label('Verification code').fill(await page.locator('.web-gap strong').inner_text())
            await page.get_by_role('button',name='Verify & sign in').click()
            await page.locator('#doctorWebShell').wait_for()
            return context,page
        contextA,pageA=await login(doctors[0]); contextB,pageB=await login(doctors[1])
        check['two_independent_doctor_mfa_sessions']='PASS'
        async def web(context,path,body=None,method=None,key=None):
            headers={'Origin':WEB,'Sec-Fetch-Site':'same-origin','Cookie':'; '.join(c['name']+'='+c['value'] for c in await context.cookies())}
            if key:headers['Idempotency-Key']=key
            response=await context.request.fetch(WEB+'/api/proxy/'+path,method=method or ('POST' if body is not None else 'GET'),data=body,headers=headers)
            return response.status,await response.json()
        async def own(path,body=None,method=None,key=None):
            status,dto=await web(contextA,path,body,method,key)
            assert status<300,(path,status,dto.get('error'))
            return dto['data']
        initial=await own('doctor/duty');await own('doctor/duty',{'is_on_duty':not initial['is_on_duty']},'PUT');await own('doctor/duty',initial,'PUT')
        check['duty_server_roundtrip']='PASS'
        phone='+2547'+str(secrets.randbelow(100000000)).zfill(8)
        challenge=await backend('auth/register-otp',{'phone_number':phone})
        patient=await backend('auth/verify-otp-and-login',{'phone_number':phone,'otp_code':challenge['debug_otp_code']})
        now=datetime.now(timezone.utc).replace(second=0,microsecond=0)
        appointments=[];slots=[]
        for mins in [3,45]:
            start=now+timedelta(minutes=mins);end=start+timedelta(minutes=10)
            slot=await own('doctor/slots',{'start_time':iso(start),'end_time':iso(end)})
            slots.append(slot)
            booking=await backend('appointments/book',{'doctor_id':doctors[0]['id'],'slot_id':slot['id'],'modality':'CHAT' if mins==3 else 'VIDEO','gateway':'MPESA','chief_complaint':'Synthetic Doctor Web verification intake. No clinical patient data.'},patient['access_token'])
            payload=json.dumps({'payment_session_id':booking['payment_session_id'],'transaction_reference':'DEV-WEB-'+uuid.uuid4().hex,'status':'SUCCESS','event_timestamp':iso(datetime.now(timezone.utc)),'provider_event_id':'DEV-WEB-'+uuid.uuid4().hex},separators=(',',':'))
            signature=hmac.new(config['WEBHOOK_HMAC_SECRET'].strip('"\'').encode(),payload.encode(),hashlib.sha256).hexdigest()
            response=await raw.post(BACK+'webhooks/payment/mpesa',data=payload,headers={'Content-Type':'application/json','X-Signature':signature})
            assert response.ok,('development webhook',response.status)
            appointments.append(booking['appointment_id'])
        report['fixture_ids']['appointments']=appointments
        check['schedule_create_and_booked_fixture']='PASS'
        status,_=await web(contextA,'doctor/slots/'+slots[0]['id'],method='DELETE');check['booked_slot_protected']='PASS' if status==409 else f'FAIL:{status}'
        status,_=await web(contextA,'doctor/slots',{'start_time':slots[0]['start_time'],'end_time':slots[0]['end_time']});check['overlap_conflict']='PASS' if status==409 else f'FAIL:{status}'
        queue=await own('doctor/appointments');assert len(queue)==2
        await pageA.reload(wait_until='networkidle');await pageA.locator('[data-appointment-id]').first.wait_for()
        assert await pageA.locator('.queue-card[data-appointment-id]').first.get_attribute('data-appointment-id')==appointments[0]
        check['queue_upcoming_first_real_desc_backend']='PASS'
        await pageA.screenshot(path=str(OUT/'queue_populated_1440.png'))
        summary=await own(f'doctor/consultations/{appointments[0]}/summary');await own(f'doctor/appointments/{appointments[0]}/patient-history')
        check['summary_and_history_authorized']='PASS'
        status,_=await web(contextA,f'doctor/appointments/{appointments[1]}/patient-history');check['history_before_T_minus_15']='PASS' if status==404 else f'FAIL:{status}'
        chats=await own('doctor/chats');chat=next(c for c in chats if c['appointment_id']==appointments[0])
        await pageA.locator('.bn-item').nth(1).click();await pageA.locator(f'[data-conversation-id="{chat["id"]}"]').click()
        await pageA.locator('#docChatInputField').fill('Synthetic text safety check: <img src=x onerror=alert(1)>')
        await pageA.locator('#docChatSendBtn').click();await pageA.locator('.chat-bubble').wait_for()
        assert await pageA.locator('.chat-bubble img').count()==0
        check['chat_send_text_escaped']='PASS'
        await pageA.get_by_text('Take after meals',exact=False).click();await pageA.wait_for_timeout(300)
        assert len(await own(f'chat/{chat["id"]}/messages'))>=2
        check['chat_canned_reply']='PASS';await pageA.screenshot(path=str(OUT/'chat_thread_1440.png'))
        for name,path,method in [('foreign_summary',f'doctor/consultations/{appointments[0]}/summary','GET'),('foreign_history',f'doctor/appointments/{appointments[0]}/patient-history','GET'),('foreign_slot',f'doctor/slots/{slots[0]["id"]}','DELETE'),('foreign_chat',f'chat/{chat["id"]}/messages','GET')]:
            status,_=await web(contextB,path,method=method);check[name]='PASS' if status in [403,404] else f'FAIL:{status}'
        foreign=await web(contextB,f'doctor/wallet/summary?doctor_id={doctors[0]["id"]}');check['foreign_wallet_parameter_rejected']='PASS' if foreign[0]==422 else f'FAIL:{foreign[0]}'
        for path in ['doctor/wallet/summary','doctor/wallet/statements?month='+now.strftime('%Y-%m'),'doctor/wallet/payout-methods']:
            await own(path)
        check['wallet_statement_destinations_real_api']='PASS'
        csrf=await contextA.request.put(WEB+'/api/proxy/doctor/duty',data={'is_on_duty':True},headers={'Origin':'https://untrusted.example'})
        check['csrf_foreign_origin']='PASS' if csrf.status==403 else 'FAIL'
        cookies=await contextA.cookies();assert all(c['httpOnly'] and c['secure'] and c['sameSite']=='Strict' for c in cookies)
        check['cookie_flags']='PASS'
        await contextA.clear_cookies(name='hd_doctor_access')
        refreshes=await asyncio.gather(web(contextA,'doctor/profile'),web(contextA,'doctor/duty'),web(contextA,'doctor/wallet/summary'))
        check['concurrent_refresh']='PASS' if all(r[0]==200 for r in refreshes) else 'FAIL'
        check['document_bytes']='STORAGE_CONFIGURATION_REQUIRED'
        check['rx_server_persistence']='STORAGE_CONFIGURATION_REQUIRED'
        check['qna_answer_conflict']='NOT_YET_TESTED'
        await pageA.locator('.bn-item').nth(0).click();await pageA.get_by_role('button',name='Web Fixture Doctor A',exact=False).count()
        await pageA.locator('.queue-patient-name .web-name-button').first.click();await pageA.locator('[aria-modal=true]').wait_for();await pageA.screenshot(path=str(OUT/'consultation_summary_1440.png'));await pageA.keyboard.press('Escape')
        await pageA.locator('.bn-item').nth(2).click();await pageA.screenshot(path=str(OUT/'rx_empty_1440.png'))
        await pageA.locator('.bn-item').nth(3).click();await pageA.screenshot(path=str(OUT/'schedule_booked_1440.png'))
        await pageA.locator('.bn-item').nth(4).click();await pageA.screenshot(path=str(OUT/'wallet_1440.png'))
        check['completion']='NOT_YET_TESTED'
        await browser.close();await raw.dispose()
    (OUT/'results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps({'checks':check,'console_errors':report['console_errors']}))

if __name__=='__main__':
    try:asyncio.run(main())
    finally:
        if REPORT:(OUT/'results.json').write_text(json.dumps(REPORT,indent=2),encoding='utf-8')
