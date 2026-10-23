"""Real local API + existing Edge/BFF acceptance. No device, cloud or RTC claim.

Uses synthetic development accounts and the existing signed development payment
callback. Tokens/OTP/secrets are never written into the evidence output.
"""
import asyncio, base64, hashlib, hmac, json, re, struct, time, uuid
from datetime import datetime, timedelta, timezone
from decimal import Decimal
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from playwright.async_api import async_playwright, expect

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).parent
BASE='http://127.0.0.1:8080/api/v1/'

def seed(name):
    source=(ROOT/'services/backend/src/seed.rs').read_text(encoding='utf-8')
    return re.search(rf'{name}: &str = "([^"]+)"',source)[1]

def call(path,body=None,token=None,expected=200,key=None,raw=None,headers=None,method=None):
    supplied={'Content-Type':'application/json','X-Request-Id':str(uuid.uuid4()),**(headers or {})}
    if token:supplied['Authorization']='Bearer '+token
    if key:supplied['Idempotency-Key']=key
    data=raw if raw is not None else json.dumps(body,separators=(',',':')).encode() if body is not None else None
    request=Request(BASE+path,data=data,headers=supplied,method=method or ('POST' if data is not None else 'GET'))
    try:
        with urlopen(request,timeout=30) as response:status,payload=response.status,json.load(response)
    except HTTPError as error:status,payload=error.code,json.load(error)
    assert status==expected,f'{path}: HTTP {status}; expected {expected}'
    return payload.get('data') if status<300 else payload

def patient_session():
    guide=(ROOT/'releases/PATIENT_CONSULTATION_TESTING_20261014.md').read_text(encoding='utf-8')
    email,password=re.search(r'\| Patient \| `([^`]+)` \| `([^`]+)`',guide).groups()
    challenge=call('auth/patient/login',{'email':email,'password':password})
    return call('auth/patient/verify-otp',{'challenge_token':challenge['challenge_token'],'otp_code':challenge['debug_otp_code']})

def totp():
    secret=base64.b32decode(seed('SEED_ADMIN_TOTP_SECRET_BASE32'))
    digest=hmac.new(secret,struct.pack('>Q',int(time.time())//30),hashlib.sha1).digest()
    offset=digest[-1]&15
    return str((struct.unpack('>I',digest[offset:offset+4])[0]&0x7fffffff)%1000000).zfill(6)

async def main():
    report={'checks':{},'page_errors':[],'physical_device':False,'real_external_payment':False,'real_azure':False,'rtc_media':False,'payment_evidence':'Existing HMAC-signed DEVELOPMENT callback only'}
    try:
        env={}
        for line in (ROOT/'services/backend/.env').read_text(encoding='utf-8').splitlines():
            if '=' in line and not line.startswith('#'):
                name,value=line.split('=',1);env[name]=value.strip().strip('"')
        assert env.get('ENVIRONMENT')=='development','Synthetic acceptance is development-only'
        patient=patient_session();pt=patient['access_token']
        profile=call('patients/me',token=pt)
        challenge=call('auth/doctor/login',{'email':seed('SEED_DOCTOR1_EMAIL'),'password':seed('SEED_DOCTOR1_PASSWORD')})
        doctor=call('auth/doctor/verify-otp',{'session_token':challenge['session_token'],'otp_code':challenge['debug_otp_code']})
        dt=doctor['access_token'];dp=call('doctor/profile',token=dt)
        slots=call(f"doctors/{dp['id']}/slots",token=pt)
        available=[]
        for attempt in range(12):
            start=datetime.now(timezone.utc)+timedelta(minutes=30+attempt*25)
            try:
                created=call('doctor/slots',{'start_time':start.isoformat(),'end_time':(start+timedelta(minutes=20)).isoformat()},dt,201)
                available=[created];break
            except AssertionError as error:
                if 'HTTP 409' not in str(error):raise
        assert available,'No valid future synthetic slot could be published'
        slots=call(f"doctors/{dp['id']}/slots",token=pt)
        assert all(s['status']=='AVAILABLE' and datetime.fromisoformat(s['start_time'].replace('Z','+00:00'))>datetime.now(timezone.utc) for s in slots)
        slot=available[0]['id'];assert any(s['id']==slot for s in slots)
        report['checks']['doctor_slot_publication_and_patient_future_only_sync']=True
        booking=call('appointments/book',{'doctor_id':dp['id'],'slot_id':slot,'modality':'CHAT','gateway':'MPESA','chief_complaint':'Synthetic lifecycle verification'},pt,201,str(uuid.uuid4()))
        appointment=booking['appointment_id']
        assert Decimal(str(booking['amount']))==Decimal(str(dp['consultation_fee_chat']))
        assert booking['status']=='PENDING_PAYMENT'
        call(f'appointments/{appointment}/payment',{},pt,409)
        pending=call(f'appointments/{appointment}',token=pt)
        assert pending['status']=='PENDING_PAYMENT' and pending['payment_status']=='INITIATED'
        report['checks']['provider_unavailable_does_not_fake_payment']=True
        payload={'payment_session_id':booking['payment_session_id'],'transaction_reference':'DEV-LIFECYCLE-'+str(uuid.uuid4()),'status':'SUCCESS','event_timestamp':(datetime.now(timezone.utc)-timedelta(seconds=1)).isoformat(),'provider_event_id':str(uuid.uuid4())}
        raw=json.dumps(payload,separators=(',',':')).encode()
        signature=hmac.new(env['WEBHOOK_HMAC_SECRET'].encode(),raw,hashlib.sha256).hexdigest()
        call('webhooks/payment/mpesa',raw=raw,headers={'X-Signature':signature})
        conv=call(f'appointments/{appointment}/conversation',token=pt)
        assert conv['appointment_id']==appointment
        stamp=str(uuid.uuid4())[:8]
        text='Lifecycle patient test message '+stamp
        message=call(f"chat/{conv['id']}/messages",{'content':text},pt,201,str(uuid.uuid4()))
        notices=call('notifications',token=dt)
        notice=next(n for n in notices if (n.get('metadata') or {}).get('message_id')==message['id'])
        assert text not in json.dumps(notice)
        report['checks']['committed_patient_message_has_privacy_safe_doctor_notice']=True
        summary=call(f'doctor/consultations/{appointment}/summary',token=dt)
        case=call('grievances',{'consultation_id':summary['consultation_id'],'target':'DOCTOR','category':'Synthetic lifecycle investigation','claim_summary':'Synthetic grievance acceptance for authoritative event reconstruction'},pt,201)
        grievance=case['grievance_id'];case_number=case['grievance_number']
        report['references']={'appointment_id':appointment,'conversation_id':conv['id'],'grievance_id':grievance,'grievance_number':case_number}
        async with async_playwright() as pw:
            browser=await pw.chromium.launch(channel='msedge',headless=True)
            try:
                doctor_context=await browser.new_context(viewport={'width':1440,'height':1000})
                page=await doctor_context.new_page();page.on('pageerror',lambda e:report['page_errors'].append(str(e)))
                await page.goto('http://127.0.0.1:3001',wait_until='networkidle')
                await page.get_by_label('Email address',exact=True).fill(seed('SEED_DOCTOR1_EMAIL'))
                await page.get_by_label('Password',exact=True).fill(seed('SEED_DOCTOR1_PASSWORD'))
                async with page.expect_response('**/api/auth/login') as pending:
                    await page.get_by_role('button',name='Doctor Secure Sign In').click()
                login=await pending.value;assert login.status==200
                login_body=await login.json()
                await page.get_by_label('Verification code',exact=True).fill(login_body['data']['development_otp'])
                await page.get_by_role('button',name='Verify & sign in').click()
                await page.locator('#doctorWebShell').wait_for()
                await page.locator('[data-page="chat"]').first.click()
                await page.locator(f'[data-conversation-id="{conv["id"]}"]').click()
                await expect(page.locator('#dwChatMsgStream')).to_contain_text(text)
                await expect(page.locator('.web-chat-header')).to_contain_text(profile['full_name'])
                reply='Lifecycle doctor test reply '+stamp
                await page.locator('#dwChatInputField').fill(reply)
                await page.locator('form').get_by_role('button',name='Send',exact=True).click()
                await expect(page.locator('#dwChatMsgStream')).to_contain_text(reply)
                history=call(f"chat/{conv['id']}/messages",token=pt)
                assert sum(m['content']==text for m in history)==1
                assert sum(m['content']==reply for m in history)==1
                replied=next(m for m in history if m['content']==reply)
                patient_notices=call('notifications',token=pt)
                reply_notices=[n for n in patient_notices if (n.get('metadata') or {}).get('message_id')==replied['id']]
                assert len(reply_notices)==1
                assert reply not in json.dumps(reply_notices[0])
                report['checks']['doctor_reply_has_one_privacy_safe_patient_notification']=True
                incoming=page.locator(f'[data-message-id="{message["id"]}"]')
                outgoing=page.locator('[data-message-id]').filter(has_text=reply)
                await expect(incoming).to_have_attribute('data-sender-side','patient')
                await expect(outgoing).to_have_attribute('data-sender-side','doctor')
                await expect(incoming.locator('.dw-chat-bubble-content')).to_have_css('background-color','rgb(255, 255, 255)')
                original_theme=await page.locator('body').get_attribute('data-theme')
                for theme in ['sage','nordic','lavender']:
                    await page.locator('body').evaluate('(node,theme)=>node.setAttribute("data-theme",theme)',theme)
                    await expect(outgoing.locator('.dw-chat-bubble-content')).to_have_css('background-color','rgb(8, 122, 91)')
                    await expect(incoming.locator('.dw-chat-bubble-content')).to_have_css('background-color','rgb(255, 255, 255)')
                await page.locator('body').evaluate('(node,theme)=>theme ? node.setAttribute("data-theme",theme) : node.removeAttribute("data-theme")',original_theme)
                report['checks']['doctor_own_green_incoming_white_across_all_three_rendered_themes']=True
                report['checks']['persisted_sender_identity_controls_doctor_ownership_styling']=True
                await page.screenshot(path=str(OUT/'live_doctor_chat.png'),full_page=True)
                await page.reload(wait_until='networkidle')
                await page.locator(f'[data-conversation-id="{conv["id"]}"]').click()
                await expect(page.locator('#dwChatMsgStream')).to_contain_text(reply)
                report['checks']['real_doctor_web_reply_patient_api_same_persisted_history']=True
                admin_context=await browser.new_context(viewport={'width':1440,'height':1000})
                admin=await admin_context.new_page();admin.on('pageerror',lambda e:report['page_errors'].append(str(e)))
                await admin.goto('http://127.0.0.1:3000/login',wait_until='networkidle')
                await admin.locator('#email').fill(seed('SEED_ADMIN_EMAIL'))
                await admin.locator('#password').fill(seed('SEED_ADMIN_PASSWORD'))
                await admin.locator('#totp').fill(totp())
                async with admin.expect_response('**/api/login') as pending:
                    await admin.get_by_role('button',name='Sign in',exact=True).click()
                login=await pending.value;assert login.status==200
                actor=(await login.json())['data']['user'];assert actor['role']=='ADMIN'
                await admin.locator('#adminNav-grievances').click()
                row=admin.locator('tr').filter(has_text=case_number)
                async with admin.expect_response(f'**/api/proxy/admin/grievances/{grievance}/timeline') as pending:
                    await row.click()
                response=await pending.value;assert response.status==200
                data=(await response.json())['data']
                section=admin.get_by_role('region',name='Consultation investigation timeline')
                await expect(section.locator('ol > li')).to_have_count(len(data['trace_events'])+len(data['audit_events']))
                for event in data['trace_events']+data['audit_events']:
                    event_row=section.locator('ol > li').filter(has_text='Event ID: '+event['id'])
                    detail=event_row.locator('details')
                    await detail.locator('summary').click()
                    visible=json.loads(await detail.locator('pre').inner_text())
                    assert all(visible.get(key)==value for key,value in event.items()),event['id']
                    await detail.locator('summary').click()
                report['checks']['every_safe_event_field_is_explicitly_available_in_grievance_ui']=True
                assert data['facts']['modality']=='CHAT'
                assert any(not hold['release_at'] for hold in data['preservation_holds'])
                assert any(e['event_type']=='PAYMENT_INITIATION_REJECTED' for e in data['trace_events'])
                assert any(e['event_type']=='CHAT_OPENED' and e['actor_role']=='DOCTOR' for e in data['trace_events'])
                assert any(e['event_type']=='CHAT_READ_ACKNOWLEDGED' and e['actor_role']=='DOCTOR' for e in data['trace_events'])
                assert any(e['action']=='ADMIN_GRIEVANCE_OPENED' and e['actor_user_id']==actor['id'] and e['auth_session_id'] and e['request_id'] for e in data['audit_events'])
                await expect(admin.locator('.admin-user-role')).to_have_text('ADMIN')
                await expect(admin.locator('.admin-user-name')).to_have_text('Admin account')
                await expect(admin.get_by_role('button',name='Record refund decision',exact=True)).to_be_visible()
                await expect(admin.get_by_role('button',name='Record Doctor warning',exact=True)).to_be_visible()
                await expect(admin.get_by_role('button',name='Dismiss grievance',exact=True)).to_be_visible()
                report['checks']['admin_header_has_canonical_role_and_no_fake_persona']=True
                await admin.screenshot(path=str(OUT/'live_admin_grievance_records.png'),full_page=True)
                await section.evaluate('(node)=>node.scrollIntoView({block:"start"})')
                await admin.screenshot(path=str(OUT/'live_admin_grievance_timeline.png'),full_page=True)
                (OUT/'live_investigation_evidence.json').write_text(json.dumps(data,indent=2),encoding='utf-8')
                report['checks']['admin_grievance_explicitly_renders_every_recorded_trace_and_audit_event']=True
                report['checks']['canonical_admin_account_attribution_and_preservation_hold']=True
                assert not report['page_errors'],report['page_errors']
            finally:await browser.close()
        report['status']='PASS'
    except Exception as error:
        report['status']='FAIL';report['failure_type']=type(error).__name__
        raise
    finally:
        (OUT/'live_web_verification.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2))

if __name__=='__main__':asyncio.run(main())
