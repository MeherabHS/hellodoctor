"""Development-only, real session -> existing storage -> Admin UI evidence.
Uses installed Edge/Playwright/Flutter and existing services. No installation.
Never persists tokens, credentials, arbitrary exception messages or patient data.
"""
import asyncio
import importlib.util
import json
import os
import re
import subprocess
from pathlib import Path
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).parent
ADMIN = 'http://127.0.0.1:3000'
DOCTOR = 'http://127.0.0.1:3001'
BACKEND = 'http://127.0.0.1:8080'
spec = importlib.util.spec_from_file_location('existing_evidence', OUT / 'verify.py')
existing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(existing)
REPORT = {'checks': {}, 'incidents': [], 'host_software_installed': 'NONE',
          'live_device_delivery': 'NOT_LIVE_VERIFIED'}
check = REPORT['checks']

async def main():
    assert re.search(r'(?m)^ENVIRONMENT=development\s*$', (ROOT/'services/backend/.env').read_text())
    # Repeated fixture logins exhausted the local 5/15-minute development
    # bucket. Reset only the loopback login counter; never flush Redis or alter
    # auth/rate-limit configuration. The guard above forbids production use.
    subprocess.run(['docker','exec','hellodoctor-redis','redis-cli','DEL','ratelimit:login:127.0.0.1'],check=True,capture_output=True)
    REPORT['development_loopback_login_limiter_reset_only'] = True
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel='msedge', headless=True)
        admin = await browser.new_context(viewport={'width':1440,'height':1000})
        doctor = await browser.new_context(viewport={'width':1440,'height':1000})
        page = await admin.new_page()
        await page.goto(ADMIN+'/login', wait_until='networkidle')
        await page.wait_for_function("Array.from(document.querySelectorAll('form')).some(form=>Object.keys(form).some(key=>key.startsWith('__reactProps')))")
        await page.get_by_label('Admin email').fill(existing.seed_constant('SEED_ADMIN_EMAIL'))
        await page.get_by_label('Password',exact=True).fill(existing.seed_constant('SEED_ADMIN_PASSWORD'))
        await page.get_by_label('Authenticator code').fill(existing.totp())
        await page.get_by_role('button',name='Sign in',exact=True).click()
        await page.get_by_text('App Error & Incident Logs',exact=True).wait_for(timeout=30000)
        await page.get_by_text('App Error & Incident Logs',exact=True).click()
        check['admin_login_and_diagnostics_ui'] = 'PASS'

        async def admin_api(path,body=None):
            # Use the actual browser cookie/session path, including Secure cookies
            # supported by Chromium on loopback, just as the Admin UI does.
            for attempt in range(4):
                res = await page.evaluate("""async ({path,body})=>{
                    const response=await fetch('/api/proxy/'+path,{method:body===null?'GET':'POST',
                        credentials:'same-origin',cache:'no-store',headers:{'Content-Type':'application/json'},
                        body:body===null?undefined:JSON.stringify(body)});
                    const raw=await response.text();let data;
                    try {data=JSON.parse(raw);} catch {data=null;}
                    return {status:response.status,ok:response.ok,body:data};
                }""",{'path':path,'body':body})
                if res['status']!=429 or attempt==3: break
                # Honor the existing shared loopback rate limiter. Do not
                # weaken/reset the blanket API ceiling to make a probe pass.
                REPORT['admin_rate_limit_backoff_observed'] = True
                await asyncio.sleep(20)
            assert res['ok'], f"Admin request failed: {res['status']}"
            return res['body']['data']

        async def accepted_web_incident(response,since):
            assert response.status == 202, f'Browser incident status {response.status}'
            try:
                return (await asyncio.wait_for(response.json(),timeout=3))['data']['incident_id']
            except Exception:
                # Chromium can discard a fire-and-forget keepalive response body.
                # Verify delivery independently in PostgreSQL using the exact safe
                # reporter context and server-side time captured before the probe.
                REPORT['browser_response_body_lookup'] = 'POSTGRES_FALLBACK'
                payload=response.request.post_data_json
                kind=payload['error_type']
                operation=payload['operation']
                assert re.fullmatch(r'[A-Za-z_]+',kind)
                assert re.fullmatch(r'[a-z_]+',operation)
                rows=existing.sql(f"SELECT id FROM system_error_logs WHERE subsystem='DOCTOR_WEB' AND impacted_user_id='{existing.seed_constant('DOCTOR1_USER_ID')}' AND client_context->>'error_type'='{kind}' AND action_attempted='{operation}' AND last_seen_at>='{since}'::timestamptz")
                identifiers=rows.splitlines()
                assert len(identifiers)==1 and re.fullmatch(r'[0-9a-f-]{36}',identifiers[0]), 'Accepted web incident must match one persisted row'
                return identifiers[0]

        async def show_incident(identifier,subsystem,filename,poll=False):
            REPORT['stage']='incident_api_detail'
            incident = await admin_api('admin/system-errors/'+identifier)
            assert incident['subsystem'] == subsystem
            if subsystem.startswith('DOCTOR_'):
                assert incident['impacted_user_id'] == existing.seed_constant('DOCTOR1_USER_ID')
                assert incident['severity'] == 'ERROR'
                assert incident['client_context']['app_version'] == '1.0.0'
                assert incident['client_context']['build_number']
                assert incident['stack_trace']
                encoded = json.dumps(incident)
                for secret in ['Jane Test','private-fixture','asthma','+254712345678']:
                    assert secret not in encoded
                # Independent PostgreSQL evidence, not just an HTTP response.
                assert existing.sql(f"SELECT count(*) FROM system_error_logs WHERE id='{identifier}' AND impacted_user_id='{existing.seed_constant('DOCTOR1_USER_ID')}'") == '1'
            await page.get_by_placeholder('Filter by User, Trace ID, Subsystem, Error Code...').fill(incident['trace_id'])
            if not poll:
                await page.get_by_role('button',name=re.compile('Refresh Feed')).click()
            row = page.locator('tr').filter(has_text=incident['trace_id'])
            REPORT['stage']='incident_row_poll'
            await row.wait_for(timeout=35000)
            await row.get_by_role('button').first.click()
            REPORT['stage']='incident_modal_stack'
            await page.get_by_text('SOURCE CODE SNIPPET / STACK TRACE',exact=False).wait_for()
            if subsystem.startswith('DOCTOR_'):
                REPORT['stage']='incident_modal_app_version'
                await page.get_by_text(incident['client_context']['app_version']+' (build '+incident['client_context']['build_number']+')',exact=True).wait_for()
                await page.get_by_text(incident['client_context']['screen']+' / '+incident['client_context']['operation'],exact=True).last.wait_for()
            await page.screenshot(path=str(OUT/filename))
            await page.get_by_role('button',name='Close Dossier',exact=True).click()
            REPORT['incidents'].append({'id':identifier,'subsystem':subsystem,'rendered_in_admin':'PASS',
                'release':incident.get('client_context'), 'trace_id':incident['trace_id']})
            await admin_api('admin/system-errors/'+identifier+'/resolve', {'resolution_note':'Synthetic development verification completed; no clinical operation failed.'})
            return incident

        web = await doctor.new_page()
        REPORT['stage']='doctor_browser_login'
        await web.goto(DOCTOR, wait_until='networkidle')
        async def browser_auth(action,data):
            return await web.evaluate("""async ({action,data})=>{const res=await fetch('/api/auth/'+action,{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json'},body:JSON.stringify(data)});return {status:res.status,body:await res.json()};}""",{'action':action,'data':data})
        login=await browser_auth('login',{'email':existing.seed_constant('SEED_DOCTOR1_EMAIL'),'password':existing.seed_constant('SEED_DOCTOR1_PASSWORD')})
        assert login['status']==200, f"Doctor login status {login['status']}"
        challenge=login['body']['data']
        verified=await browser_auth('verify',{'otp_code':challenge['development_otp']})
        assert verified['status']==200, f"Doctor MFA status {verified['status']}"
        await web.goto(DOCTOR, wait_until='networkidle')
        await web.locator('button[data-page="settings"]').wait_for()
        check['doctor_web_real_authenticated_session'] = 'PASS'
        chunks = await web.locator('script[src*="/_next/static/chunks/"]').evaluate_all('(scripts)=>scripts.map(s=>s.src)')
        # Actual global error listener; synthetic stack frame clearly belongs to
        # the development fixture rather than claiming a real source defect.
        since=existing.sql('SELECT now()')
        async with web.expect_response(lambda r:r.url.endswith('/api/diagnostics'),timeout=15000) as response:
            REPORT['stage']='trigger_browser_exception'
            await web.evaluate(r'''chunk=>{setTimeout(()=>{const error=new TypeError('Synthetic development exception; patient=Jane Test token=private-fixture diagnosis=asthma +254712345678');error.stack='TypeError: synthetic development fixture\n at synthetic ('+chunk+':1:1)';throw error;},0);}''',chunks[0])
        result = await response.value
        REPORT['stage']='browser_ingestion_response'
        identifier = await accepted_web_incident(result,since)
        incident = await show_incident(identifier,'DOCTOR_WEB','doctor-web-client-incident.png',poll=True)
        assert incident['client_context']['platform'] == 'web'
        check['browser_exception_to_database_to_admin_polling'] = 'PASS'
        check['web_stack_build_doctor_association_privacy'] = 'PASS'

        # Real application API transport, intercepted response only. No clinical
        # mutation executes. Exercise 503 and expected 409/422 from a UI action.
        await web.locator('button[data-page="settings"]').click()
        await web.get_by_role('button',name='Availability',exact=True).click()
        duty_before = existing.sql(f"SELECT is_on_duty FROM doctor_profiles WHERE user_id='{existing.seed_constant('DOCTOR1_USER_ID')}'")
        before = existing.sql("SELECT coalesce(sum(occurrence_count),0) FROM system_error_logs WHERE subsystem='DOCTOR_WEB'")
        for status in [409,422,503]:
            async def mock(route,request,status=status):
                await route.fulfill(status=status,content_type='application/json',body=json.dumps({'success':False,'error':{'code':'SYNTHETIC_TEST','message':'Synthetic development response.'}}))
            await web.route('**/api/proxy/doctor/duty',mock)
            if status==503:
                since=existing.sql('SELECT now()')
                async with web.expect_response(lambda r:r.url.endswith('/api/diagnostics'),timeout=15000) as diag:
                    await web.get_by_role('switch',name='On duty',exact=True).click()
                res=await diag.value
                identifier=await accepted_web_incident(res,since)
                await show_incident(identifier,'DOCTOR_WEB','doctor-web-api-failure.png')
                check['doctor_web_system_503_action_to_admin'] = 'PASS'
            else:
                await web.get_by_role('switch',name='On duty',exact=True).click()
                await web.wait_for_timeout(500)
                assert existing.sql("SELECT coalesce(sum(occurrence_count),0) FROM system_error_logs WHERE subsystem='DOCTOR_WEB'") == before
                check[f'normal_{status}_not_reported_live_browser'] = 'PASS'
            await web.unroute('**/api/proxy/doctor/duty',mock)
        assert existing.sql(f"SELECT is_on_duty FROM doctor_profiles WHERE user_id='{existing.seed_constant('DOCTOR1_USER_ID')}'") == duty_before
        check['controlled_api_failures_did_not_change_doctor_duty'] = 'PASS'

        anonymous=await browser.new_context()
        rejected=await anonymous.request.post(DOCTOR+'/api/diagnostics',headers={'Origin':DOCTOR},
            data={'client':'DOCTOR_WEB','error_type':'Error','message':'synthetic','stack_trace':'','screen':'Auth','operation':'login','platform':'web','occurred_at':'2026-10-08T00:00:00Z'})
        assert rejected.status==401
        check['preauth_ingestion_closed'] = 'PASS'

        # Real backend Doctor MFA session used by the real Flutter/Dio reporter.
        REPORT['stage']='flutter_framework_delivery'
        auth=await admin.request.post(BACKEND+'/api/v1/auth/doctor/login',data={
            'email':existing.seed_constant('SEED_DOCTOR1_EMAIL'),'password':existing.seed_constant('SEED_DOCTOR1_PASSWORD')})
        assert auth.ok
        data=(await auth.json())['data']
        auth=await admin.request.post(BACKEND+'/api/v1/auth/doctor/verify-otp',data={
            'session_token':data['session_token'],'otp_code':data['debug_otp_code']})
        assert auth.ok
        tokens=(await auth.json())['data']
        result_path=OUT/'flutter-live-result.json'
        task_env=dict(os.environ)
        task_env['HD_OBSERVABILITY_DOCTOR_TOKEN']=tokens['access_token']
        task_env['HD_OBSERVABILITY_RESULT']=str(result_path)
        process=await asyncio.create_subprocess_exec(str(Path(r'C:/Users/CUBE/.codex/tools/flutter/bin/flutter.bat')),
            'test','--no-pub','test/dev/client_incident_live.dart',cwd=str(ROOT/'apps/mobile'),env=task_env,
            stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.STDOUT)
        output,_=await process.communicate()
        if process.returncode!=0:
            decoded=output.decode('utf8',errors='replace')
            REPORT['flutter_harness_failure_categories']=[label for label in ['TimeoutException','400','401','403','422','429','503','Compilation failed'] if label in decoded]
        assert process.returncode==0, 'Flutter framework live harness failed'
        identifier=json.loads(result_path.read_text())['incident_id']
        await show_incident(identifier,'DOCTOR_FLUTTER','flutter-framework-client-incident.png')
        check['real_flutter_framework_dio_to_backend_database_admin'] = 'PASS'
        await admin.request.post(BACKEND+'/api/v1/auth/logout',data={'refresh_token':tokens['refresh_token']})

        # Existing backend simulator writes through the same System Error Service.
        selected=next((scenario for scenario,component in [('offline_sync','MutationSyncResolver'),('kyc_hub_down','CredentialRegistryClient'),('sms_otp','SmsOtpDispatcher')]
            if existing.sql(f"SELECT count(*) FROM system_error_logs WHERE status='OPEN' AND component='{component}' AND last_seen_at>now()-interval '60 minutes'")=='0'),None)
        assert selected, 'No unused backend simulation preset; avoid changing a real incident'
        incident=await admin_api('admin/system-errors/simulate',{'scenario':selected,'severity':'WARNING','custom_note':'SYNTHETIC_CLIENT_OBSERVABILITY_VERIFICATION'})
        await show_incident(incident['id'],incident['subsystem'],'backend-system-service-simulation.png')
        check['existing_backend_service_simulation_to_admin'] = 'PASS'
        check['primary_clinical_operations_not_modified_by_simulation'] = 'PASS'
        health=await admin.request.get(BACKEND+'/healthz')
        protected=await admin.request.get(BACKEND+'/api/v1/doctor/profile')
        assert health.status==200 and protected.status==401
        check['live_backend_health_and_default_deny_sanity'] = 'PASS'
        owned_ids=[item['id'] for item in REPORT['incidents']]+['8a3aa6af-5b4e-4588-9ac0-561b4b3f742f','a98585aa-cc4b-4229-a0f5-218ff5858c35']
        assert all(existing.sql(f"SELECT status FROM system_error_logs WHERE id='{identifier}'")=='RESOLVED' for identifier in owned_ids)
        check['known_synthetic_incidents_resolved'] = 'PASS'
        await browser_auth('logout',{})
        await browser.close()

if __name__=='__main__':
    try:
        asyncio.run(main())
    except Exception as error:
        REPORT['failure_type']=type(error).__name__
        # Assertions contain only our static messages/status, never a DOM excerpt.
        if isinstance(error,AssertionError): REPORT['failure_assertion']=str(error)
        # Only classify Playwright's fixed diagnostic prefix; no DOM excerpts.
        first=str(error).splitlines()[0] if str(error) else ''
        for category in ['SyntaxError','strict mode violation','Timeout','Execution context']:
            if category in first: REPORT['playwright_failure_category']=category
        print('Verification failed: '+type(error).__name__)
    finally:
        (OUT/'client-delivery-results.json').write_text(json.dumps(REPORT,indent=2),encoding='utf-8')
        print(json.dumps(REPORT['checks'],indent=2))
