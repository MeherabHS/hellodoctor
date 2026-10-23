"""Real local APIs and existing Edge. Development payments only; no secrets logged.
Uses existing synthetic accounts, leaves two identified dev bookings for review.
"""
import asyncio, hashlib, hmac, json, os, subprocess, uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from playwright.async_api import async_playwright, expect

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).parent
BACK='http://127.0.0.1:8080/api/v1/'
WEB='http://127.0.0.1:3001'

async def main():
    # Login limits remain intact. Wait for the existing local shared IP window.
    while True:
        ttl=int(subprocess.check_output(['docker','exec','hellodoctor-redis','redis-cli','TTL','ratelimit:login:127.0.0.1'],text=True).strip())
        used=int(subprocess.check_output(['docker','exec','hellodoctor-redis','redis-cli','GET','ratelimit:login:127.0.0.1'],text=True).strip() or '0')
        if ttl<=0 or used<4: break
        print(f'Waiting for existing login rate window: {ttl}s remaining',flush=True)
        await asyncio.sleep(min(ttl+1,45))
    env={}
    for line in (ROOT/'services/backend/.env').read_text().splitlines():
        if '=' in line and not line.startswith('#'):
            k,v=line.split('=',1);env[k]=v.strip('"\'')
    assert env.get('ENVIRONMENT')=='development'
    report={'payment':'Signed development MPESA webhook; no live provider or money', 'checks':{},'console_errors':[]}
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        context=await browser.new_context(viewport={'width':1440,'height':1000})
        page=await context.new_page()
        page.on('pageerror',lambda e:report['console_errors'].append(str(e)))
        async def backend(path, body=None, token=None, extra=None):
            headers={**({'Authorization':'Bearer '+token} if token else {}),**(extra or {})}
            r=await context.request.fetch(BACK+path, method='POST' if body is not None else 'GET', data=body, headers=headers)
            result=await r.json()
            assert r.status<300,(path,r.status,result.get('error',{}).get('code'))
            return result['data']
        p=await backend('auth/patient/login',{'email':'patient.test3@hellodoctor.local','password':'PatientTest!2026'})
        p=await backend('auth/patient/verify-otp',{'challenge_token':p['challenge_token'],'otp_code':p['debug_otp_code']})
        patient=p['access_token']
        profile=await backend('patients/me',token=patient)
        await page.goto(WEB,wait_until='networkidle')
        await page.get_by_label('Email address',exact=True).fill('sabrina.akter@hellodoctor.health')
        await page.get_by_label('Password',exact=True).fill('Password123!')
        async with page.expect_response('**/api/auth/login') as login:
            await page.get_by_role('button',name='Doctor Secure Sign In').click()
        login_response=await login.value
        login=await login_response.json()
        assert login_response.status==200,('doctor_login',login_response.status,login.get('error',{}).get('code'))
        code=login['data']['development_otp']
        await page.get_by_label('Verification code',exact=True).fill(code)
        await page.get_by_role('button',name='Verify & sign in').click()
        await page.locator('#doctorWebShell').wait_for()
        async def own(path,body=None):
            r=await context.request.fetch(WEB+'/api/proxy/'+path,method='POST' if body is not None else 'GET',data=body,
              headers={'Origin':WEB,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await context.cookies()),**({'Idempotency-Key':str(uuid.uuid4())} if body is not None else {})})
            result=await r.json();assert r.status<300,(path,r.status,result.get('error',{}).get('code'))
            return result['data']
        doctor_profile=await own('doctor/profile')
        report['checks']['doctor_password_and_development_mfa']='PASS'
        appointments=[]
        existing=json.loads((OUT/'live_bookings.json').read_text()) if (OUT/'live_bookings.json').exists() else []
        for index,modality in enumerate(['CHAT','VIDEO']):
            if existing:
                appointments.append(existing[index])
                continue
            start=datetime.now(timezone.utc)+(timedelta(hours=2) if modality=='CHAT' else timedelta(days=40,hours=index))
            slot=await own('doctor/slots',{'start_time':start.isoformat(),'end_time':(start+timedelta(minutes=10)).isoformat()})
            booking=await backend('appointments/book',{'doctor_id':doctor_profile['id'],'slot_id':slot['id'],'modality':modality,'gateway':'MPESA'},patient,{'Idempotency-Key':str(uuid.uuid4())})
            before=await backend('appointments/'+booking['appointment_id'],token=patient)
            assert before['status']=='PENDING_PAYMENT' and before['payment_status']=='INITIATED'
            body=json.dumps({'payment_session_id':booking['payment_session_id'],'transaction_reference':'DEV-UI-'+str(uuid.uuid4()),
              'status':'SUCCESS','event_timestamp':datetime.now(timezone.utc).isoformat(),'provider_event_id':str(uuid.uuid4())},separators=(',',':'))
            secret=env.get('WEBHOOK_HMAC_SECRET')
            assert secret,'Missing configured webhook signing secret'
            signature=hmac.new(secret.encode(),body.encode(),hashlib.sha256).hexdigest()
            r=await context.request.post(BACK+'webhooks/payment/mpesa',data=body,headers={'Content-Type':'application/json','X-Signature':signature})
            assert r.status==200,('signed_dev_webhook',r.status)
            after=await backend('appointments/'+booking['appointment_id'],token=patient)
            assert after['status']=='CONFIRMED' and after['payment_status']=='PAYMENT_HELD'
            conversation=await backend('appointments/'+booking['appointment_id']+'/conversation',token=patient)
            appointments.append({**booking,'modality':modality,'conversation_id':conversation['id']})
        chat,video=appointments
        (OUT/'live_bookings.json').write_text(json.dumps(appointments,indent=2),encoding='utf-8')
        assert chat['conversation_id']!=video['conversation_id']
        report['appointments']=appointments
        queue=await own('doctor/appointments?limit=100&offset=0')
        for a in appointments:
            item=next(q for q in queue if q['id']==a['appointment_id'])
            assert item['patient_display_name']==profile['full_name'] and item['modality']==a['modality']
            assert item['conversation_id']==a['conversation_id']
        report['checks']['paid_patient_specific_chat_and_video_mapping']='PASS'
        await page.reload(wait_until='networkidle')
        await page.locator('#doctorWebSidebar [data-page="appointments"]').click()
        for a in appointments:
            card=page.locator(f'[data-appointment-id="{a["appointment_id"]}"]')
            while await card.count()==0 and await page.get_by_role('button',name='Load more consultations').count():
                await page.get_by_role('button',name='Load more consultations').click()
            await expect(card).to_contain_text(profile['full_name'])
            await expect(card).to_contain_text('Start chat consultation' if a['modality']=='CHAT' else 'Start video consultation')
        await page.screenshot(path=str(OUT/'doctor_web_queue.png'),full_page=True)
        existing_messages=await backend('chat/'+chat['conversation_id']+'/messages',token=patient)
        if not existing_messages:
            await backend('chat/'+chat['conversation_id']+'/messages',{'content':'Hello doctor. Consultation verification message.'},patient,{'Idempotency-Key':str(uuid.uuid4())})
        await page.locator(f'[data-appointment-id="{chat["appointment_id"]}"]').get_by_role('button',name='Start chat consultation',exact=True).click()
        await page.locator('#dwChatInputField').wait_for()
        await expect(page.locator('#dwChatMsgStream')).to_contain_text('Hello doctor. Consultation verification message.')
        assert await page.get_by_role('button',name='Start video consultation',exact=True).count()==0
        if not any(m['content']=='Doctor Web reply in the same consultation.' for m in existing_messages):
            await page.get_by_label('Chat message',exact=True).fill('Doctor Web reply in the same consultation.')
            await page.get_by_role('button',name='Send',exact=True).click()
        await expect(page.locator('#dwChatMsgStream')).to_contain_text('Doctor Web reply in the same consultation.')
        patient_messages=await backend('chat/'+chat['conversation_id']+'/messages',token=patient)
        assert [m['content'] for m in patient_messages][:2]==['Hello doctor. Consultation verification message.','Doctor Web reply in the same consultation.']
        await page.screenshot(path=str(OUT/'doctor_web_shared_chat.png'),full_page=True)
        report['checks']['patient_send_web_receive_web_reply_patient_receive']='PASS'
        report['checks']['chat_has_no_video_action']='PASS'
        doctor_token=next(c['value'] for c in await context.cookies() if c['name']=='hd_doctor_access')
        runtime={**os.environ,'HD_PATIENT_TOKEN':patient,'HD_DOCTOR_TOKEN':doctor_token,
          'HD_CONVERSATION_ID':chat['conversation_id'],'HD_APPOINTMENT_ID':chat['appointment_id'],
          'HD_PATIENT_NAME':profile['full_name'],'HD_DOCTOR_NAME':doctor_profile['full_name']}
        # Temporary local test-session transport, inside ignored build output.
        (ROOT/'services/backend/target/patient_consultation_runtime.json').write_text(json.dumps({k:v for k,v in runtime.items() if k.startswith('HD_')}),encoding='utf-8')
        (OUT/'live_results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
        for app, runner in [('mobile','live_doctor_flutter.dart'),('mobile_patient','live_patient_flutter.dart')]:
            command=['flutter.bat','test',str(OUT/runner)]
            completed=await asyncio.to_thread(subprocess.run,command,cwd=ROOT/'apps'/app,env=runtime,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,encoding='utf-8')
            (OUT/(app+'_live.log')).write_text(completed.stdout,encoding='utf-8')
            assert completed.returncode==0,(app,'live_flutter_failed')
            report['checks'][app+'_real_backend_render']='PASS'
        history=await backend('chat/'+chat['conversation_id']+'/messages',token=patient)
        assert len(history)==3 and history[-1]['content']=='Doctor Flutter reply in the same consultation.'
        report['checks']['all_three_clients_shared_history']='PASS'
        assert not report['console_errors'],report['console_errors']
        (OUT/'live_results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
        await browser.close()
        (ROOT/'services/backend/target/patient_consultation_runtime.json').unlink()
        print(json.dumps(report['checks']))

if __name__=='__main__': asyncio.run(main())
