# Real Edge + local backend/BFF. Synthetic development accounts; no saved tokens.
import asyncio, importlib.util, json, time
from collections import Counter
from pathlib import Path
from urllib.parse import urlparse
from urllib.error import URLError
from urllib.request import urlopen
from playwright.async_api import async_playwright, expect
ROOT=Path(__file__).resolve().parents[3];OUT=Path(__file__).parent
spec=importlib.util.spec_from_file_location('helpers',ROOT/'docs/audit_evidence/lifecycle_milestone/verify_live_web.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
def path(url):
 p=urlparse(url);return p.path+('?' + p.query if p.query else '')
async def main():
 report={'status':'RUNNING','windows':{},'errors':[],'http_failures':[],'physical_device':False,'visibility_mode':'controlled DOM visibilityState, not OS background','clock':'real wall time, no timer acceleration','backend':'real development backend and HttpOnly-cookie BFF'}
 try:
  for origin in ['http://127.0.0.1:3001','http://127.0.0.1:3000/login']:
   ready=False
   for attempt in range(30):
    try:
     with urlopen(origin,timeout=2) as response:ready=response.status==200
     if ready:break
    except (URLError,TimeoutError):pass
    await asyncio.sleep(0.3)
   assert ready,'Web service must be ready before browser assertions: '+origin
  async with async_playwright() as pw:
   browser=await pw.chromium.launch(channel='msedge',headless=True)
   try:
    dc=await browser.new_context(viewport={'width':1440,'height':1000});page=await dc.new_page();traffic=[]
    page.on('request',lambda r:traffic.append((r.method,path(r.url))) if '/api/' in r.url else None)
    authenticated=False
    page.on('pageerror',lambda e:report['errors'].append(str(e)))
    page.on('response',lambda r:report['http_failures'].append({'status':r.status,'path':path(r.url)}) if authenticated and '/api/proxy/' in r.url and r.status>=400 else None)
    await page.goto('http://127.0.0.1:3001',wait_until='networkidle')
    await page.get_by_label('Email address',exact=True).fill(h.seed('SEED_DOCTOR1_EMAIL'));await page.get_by_label('Password',exact=True).fill(h.seed('SEED_DOCTOR1_PASSWORD'))
    async with page.expect_response('**/api/auth/login') as response:await page.get_by_role('button',name='Doctor Secure Sign In').click()
    result=await response.value;assert result.status==200;challenge=await result.json()
    await page.get_by_label('Verification code',exact=True).fill(challenge['data']['development_otp']);await page.get_by_role('button',name='Verify & sign in').click();await page.locator('#doctorWebShell').wait_for();authenticated=True
    async def measure(name,seconds=35,target=None,requests=None):
     target=target or page;requests=traffic if requests is None else requests
     await target.wait_for_timeout(1500);index=len(requests);start=time.monotonic();await target.wait_for_timeout(seconds*1000);rows=requests[index:]
     report['windows'][name]={'seconds':round(time.monotonic()-start,2),'requests':len(rows),'counts':dict(Counter(method+' '+url for method,url in rows))};return rows
    async def navigate(name):
     await page.locator(f'[data-page="{name}"]').first.click();await page.wait_for_timeout(1800)
    await navigate('wallet');rows=await measure('doctor_wallet_idle')
    assert not any(any(s in url for s in ['/appointments','/slots','/messages','/chats']) for _,url in rows)
    await navigate('appointments');rows=await measure('doctor_queue_idle');queue=[url for method,url in rows if method=='GET' and '/doctor/appointments' in url]
    assert 1<=len(queue)<=3,queue;assert all('view=upcoming' in url for url in queue)
    await navigate('schedule');index=len(traffic)
    async with page.expect_response('**/api/proxy/doctor/slots?**day=2030-10-17') as response:await page.locator('input[type="date"]').fill('2030-10-17')
    result=await response.value;assert result.status==200,'Selected-day BFF request must succeed';await page.wait_for_timeout(2000)
    assert any('/doctor/slots?' in url and 'day=2030-10-17' in url for _,url in traffic[index:])
    rows=await measure('doctor_schedule_selected_day');assert not any('/doctor/appointments' in url for _,url in rows)
    assert all('day=2030-10-17' in url for method,url in rows if method=='GET' and '/doctor/slots?' in url)
    await navigate('chat')
    response=await page.evaluate("async () => {const r=await fetch('/api/proxy/doctor/chats?limit=100&offset=0',{credentials:'same-origin',cache:'no-store'});return {status:r.status,body:await r.json()};}");assert response['status']==200
    conversations=response['body']['data']
    active=[c for c in conversations if not c['is_locked'] and h.datetime.fromisoformat(c['expires_at'].replace('Z','+00:00'))>h.datetime.now(h.timezone.utc)]
    assert active,'Real active synthetic conversation required for Chat traffic measurement'
    conversation=page.locator('[data-conversation-id="'+active[0]['id']+'"]');await conversation.wait_for();await conversation.click();await page.locator('#dwChatMsgStream').wait_for()
    rows=await measure('doctor_chat_open',32);reads=[url for method,url in rows if method=='GET' and '/messages' in url];assert 2<=len(reads)<=4,reads
    assert not any('/doctor/appointments' in url or '/doctor/slots' in url for _,url in rows)
    await page.screenshot(path=str(OUT/'doctor_chat_traffic.png'),full_page=True)
    await page.evaluate("() => {window.__trafficVisibility='hidden';Object.defineProperty(document,'visibilityState',{configurable:true,get:()=>window.__trafficVisibility});document.dispatchEvent(new Event('visibilitychange'));}")
    rows=await measure('doctor_chat_dom_hidden',22);assert not any('/messages' in url or '/notifications' in url for _,url in rows)
    await page.evaluate("() => {window.__trafficVisibility='visible';document.dispatchEvent(new Event('visibilitychange'));}");await page.wait_for_timeout(1500)
    await dc.set_offline(True);rows=await measure('doctor_chat_browser_offline',22);assert not any('/messages' in url or '/notifications' in url for _,url in rows)
    await dc.set_offline(False);await page.wait_for_timeout(1500);await navigate('wallet');rows=await measure('doctor_chat_closed',22);assert not any('/messages' in url for _,url in rows)
    ac=await browser.new_context(viewport={'width':1440,'height':1000});admin=await ac.new_page();at=[]
    admin.on('request',lambda r:at.append((r.method,path(r.url))) if '/api/' in r.url else None);admin.on('pageerror',lambda e:report['errors'].append(str(e)))
    admin.on('response',lambda r:report['http_failures'].append({'status':r.status,'path':path(r.url)}) if '/api/proxy/' in r.url and r.status>=400 else None)
    await admin.goto('http://127.0.0.1:3000/login',wait_until='networkidle');await admin.locator('#email').fill(h.seed('SEED_ADMIN_EMAIL'));await admin.locator('#password').fill(h.seed('SEED_ADMIN_PASSWORD'));await admin.locator('#totp').fill(h.totp())
    async with admin.expect_response('**/api/login') as response:await admin.get_by_role('button',name='Sign in',exact=True).click()
    result=await response.value;assert result.status==200;await admin.locator('#adminNav-grievances').click();await admin.wait_for_timeout(2000)
    rows=await measure('admin_grievances_idle',35,admin,at);assert 1<=sum('/admin/grievances' in url for _,url in rows)<=2
    assert not any(any(s in url for s in ['/doctors','/patients','/slots','/finance']) for _,url in rows)
    await admin.locator('tbody tr').first.click();section=admin.get_by_role('region',name='Consultation investigation timeline');await expect(section).to_be_visible();await expect(section).to_contain_text('recorded trace and audit events loaded')
    await admin.screenshot(path=str(OUT/'admin_paginated_grievance.png'),full_page=True)
    await admin.evaluate("() => {Object.defineProperty(document,'visibilityState',{configurable:true,get:()=> 'hidden'});document.dispatchEvent(new Event('visibilitychange'));}")
    rows=await measure('admin_dom_hidden',35,admin,at);assert not rows,rows;assert not report['errors'],report['errors'];assert not report['http_failures'],report['http_failures'];report['status']='PASS'
   finally:await browser.close()
 except Exception as error:report['status']='FAIL';report['failure_type']=type(error).__name__;raise
 finally:(OUT/'live_web_traffic.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
 print(json.dumps(report,indent=2))
if __name__=='__main__':asyncio.run(main())
