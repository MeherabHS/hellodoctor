"""Actual Next BFF + rendered Admin UI; isolated synthetic backend, no real accounts."""
import asyncio, copy, json, os, subprocess, sys, threading, time, uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen
from playwright.async_api import async_playwright

ROOT=Path(__file__).resolve().parents[4]
OUT=ROOT/'scratch/remediation-b'
ORIGIN='http://127.0.0.1:3011'
ARTICLE='e0f20dc7-61b7-432e-805c-ded77ab2ee78'
CATEGORY={'id':'c0849d9e-2ef8-4418-861c-0b8051c50e99','name_en':'Education','name_sw':'Elimu','sensitive':False}
BASE_ARTICLE=dict(id=ARTICLE,author_id='synthetic-author',author_name='Fixture Editorial',title_en='Saved article',title_sw='',body_en='Saved article body.',body_sw='',category_id=CATEGORY['id'],tags=[],featured=False,sensitive=False,status='DRAFT',version=1,content_version=1,approved_version=None,approved_by=None,approved_at=None,requires_independent_review=False,can_approve=False,approval_block_reason=None,cover_image_id=None,created_at='2026-10-26T08:00:00Z',updated_at='2026-10-26T08:00:00Z',published_at=None,scheduled_for=None)
def question(text='Saved question'):
    options=[dict(id=str(uuid.uuid4()),text_en=t,text_sw='') for t in ['First','Second']]
    return dict(id=str(uuid.uuid4()),text_en=text,text_sw='',explanation_en='First is correct.',explanation_sw='',options=options,correct_option_id=options[0]['id'])

class Fixture:
    def __init__(self):
        self.lock=threading.Lock();self.sessions={};self.access={};self.families={};self.access_families={};self.failures={};self.rotations=0;self.logout_calls=0;self.protected=0;self.writes=0;self.import_gate=None;self.import_started=threading.Event();self.article=copy.deepcopy(BASE_ARTICLE);self.quiz=dict(schema_version=1,draft=dict(id=str(uuid.uuid4()),article_id=ARTICLE,status='DRAFT',version=1,content_version=1,article_content_version=1,can_approve=False,requires_independent_review=False,approved_by=None,include_quiz=False,questions=[question()]),published=None)
    def seed(self,revoked=False,family=None):
        access='fixture-access-'+str(uuid.uuid4());refresh='fixture-refresh-'+str(uuid.uuid4())
        with self.lock:
            self.sessions[refresh]=not revoked;self.access[access]=not revoked;self.families[refresh]=family or str(uuid.uuid4());self.access_families[access]=self.families[refresh]
        return access,refresh

STATE=Fixture()
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def reply(self,data,status=200,content_type='application/json'):
        raw=json.dumps(data).encode() if content_type=='application/json' else data
        self.send_response(status);self.send_header('Content-Type',content_type);self.send_header('Content-Length',str(len(raw)));self.end_headers();self.wfile.write(raw)
    def handle_request(self):
        path=urlparse(self.path).path
        raw=self.rfile.read(int(self.headers.get('Content-Length','0')))
        body=json.loads(raw or b'{}') if self.headers.get('Content-Type','').startswith('application/json') else None
        if path=='/api/v1/auth/refresh':
            token=(body or {}).get('refresh_token')
            failure=STATE.failures.get(token)
            if failure=='unavailable':return self.reply(dict(success=False,error='Synthetic dependency failure'),503)
            if failure=='malformed':return self.reply(dict(success=True,data=dict(access_token=1)))
            if failure=='disconnect':self.connection.close();return
            with STATE.lock:
                STATE.rotations+=1
                allowed=STATE.sessions.get(token,False)
                if allowed:STATE.sessions[token]=False
            if not allowed:return self.reply(dict(success=False,error='Refresh session rejected.'),401)
            access,refresh=STATE.seed(family=STATE.families[token])
            if failure=='revoked_after_rotation':STATE.access[access]=False
            return self.reply(dict(success=True,data=dict(access_token=access,refresh_token=refresh)))
        if path=='/api/v1/auth/logout':
            with STATE.lock:
                family=STATE.families.get((body or {}).get('refresh_token'))
                if family:STATE.logout_calls+=1
                for key in STATE.sessions:
                    if STATE.families.get(key)==family:STATE.sessions[key]=False
                for key in STATE.access:
                    if STATE.access_families.get(key)==family:STATE.access[key]=False
            return self.reply(dict(success=True))
        token=self.headers.get('Authorization','').removeprefix('Bearer ')
        if not STATE.access.get(token,False):return self.reply(dict(success=False,error='Session rejected.'),401)
        STATE.protected+=1
        if path.endswith('/cover'):
            if self.command=='POST':
                STATE.cover_body=raw;STATE.writes+=1;STATE.article['cover_image_id']=str(uuid.uuid4());STATE.article['version']+=1
                return self.reply(dict(success=True,data=STATE.article))
            return self.reply(b'synthetic-cover',content_type='image/png')
        if path.startswith('/api/v1/appointments/') and '/documents/' in path:return self.reply(dict(success=True,data=dict(download_url='/api/v1/documents/download?token=synthetic-grant')))
        if path=='/api/v1/documents/download':return self.reply(b'synthetic-private-document',content_type='application/pdf')
        if path=='/api/v1/admin/overview':return self.reply(dict(success=True,data={}))
        if path=='/api/v1/client-errors':return self.reply(dict(success=True,data={}))
        if path=='/api/v1/admin/article-categories':return self.reply(dict(success=True,data=[CATEGORY]))
        if path=='/api/v1/admin/articles':return self.reply(dict(success=True,data=dict(items=[STATE.article],total=1)))
        if path==f'/api/v1/admin/articles/{ARTICLE}':
            if self.command=='PATCH':
                if body['version']!=STATE.article['version']:return self.reply(dict(success=False,error='Version conflict'),409)
                STATE.mutation_body=body;STATE.mutation_key=self.headers.get('Idempotency-Key');STATE.article.update(body['content']);STATE.article['version']+=1;STATE.writes+=1
            return self.reply(dict(success=True,data=STATE.article))
        if path==f'/api/v1/admin/articles/{ARTICLE}/quiz':
            if self.command=='PATCH':STATE.quiz['draft'].update(body['content']);STATE.quiz['draft']['version']+=1;STATE.writes+=1
            return self.reply(dict(success=True,data=STATE.quiz))
        if path==f'/api/v1/admin/articles/{ARTICLE}/quiz/import-preview':return self.reply(dict(success=True,data=dict(questions=[question('Imported question')],errors=[])))
        if path==f'/api/v1/admin/articles/{ARTICLE}/quiz/import':
            STATE.import_started.set()
            if STATE.import_gate and not STATE.import_gate.wait(12):return self.reply(dict(success=False,error='Synthetic import deadline'),503)
            if body['version']!=STATE.quiz['draft']['version']:return self.reply(dict(success=False,error='Version conflict'),409)
            STATE.quiz['draft']['questions'].append(question('Imported question'));STATE.quiz['draft']['version']+=1;STATE.writes+=1
            return self.reply(dict(success=True,data=STATE.quiz))
        return self.reply(dict(success=False,error='Unrelated synthetic dashboard transport.'),503)
    do_GET=handle_request
    do_POST=handle_request
    do_PATCH=handle_request

async def main():
    phase=sys.argv[1] if len(sys.argv)>1 else 'after';OUT.mkdir(parents=True,exist_ok=True)
    server=ThreadingHTTPServer(('127.0.0.1',3012),Handler);threading.Thread(target=server.serve_forever,daemon=True).start()
    env=os.environ.copy();env.update(ENVIRONMENT='test',ADMIN_WEB_ORIGIN=ORIGIN,BACKEND_URL='http://127.0.0.1:3012',REDIS_URL='',NEXT_TELEMETRY_DISABLED='1')
    log=(OUT/f'browser-{phase}-server.log').open('wb')
    process=subprocess.Popen(['node','node_modules/next/dist/bin/next','start','-p','3011','--hostname','127.0.0.1'],cwd=ROOT/'apps/admin-web',env=env,stdout=log,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW)
    checks=[]
    try:
        for _ in range(120):
            if process.poll() is not None:raise RuntimeError('Owned Next test server exited; inspect its log.')
            try:
                with urlopen(ORIGIN+'/api/runtime',timeout=1):break
            except Exception:await asyncio.sleep(.5)
        else:raise RuntimeError('Owned Next test server did not become ready.')
        async with async_playwright() as pw:
            browser=await pw.chromium.launch(channel='msedge',headless=True)
            async def check(name,fn):
                if len(sys.argv)>2 and sys.argv[2] not in name:return
                try:await asyncio.wait_for(fn(),timeout=45);checks.append(dict(name=name,passed=True))
                except Exception as e:checks.append(dict(name=name,passed=False,error=str(e)[:500]))
                print(json.dumps(checks[-1]),flush=True)
            async def session(path):
                access,refresh=STATE.seed();context=await browser.new_context();await context.add_cookies([dict(name='hd_admin_refresh',value=refresh,domain='127.0.0.1',path='/api',httpOnly=True)])
                try:
                    before=STATE.rotations;response=await context.request.get(ORIGIN+path);assert response.status==200,f'Expected refresh recovery, got {response.status}';assert STATE.rotations==before+1
                    cookies=await context.cookies();assert any(c['name']=='hd_admin_access' and c['httpOnly'] and c['sameSite']=='Strict' for c in cookies)
                finally:await context.close()
            for name,path in [('missing access JSON BFF refresh','/api/proxy/admin/overview'),('missing access private cover refresh',f'/api/article-media/{ARTICLE}'),('missing access clinical grant refresh','/api/documents/download?appointment_id=a&document_id=b')]:await check(name,lambda p=path:session(p))
            async def cookie_context(access=None,refresh=None,marker=True):
                context=await browser.new_context();cookies=[]
                if marker:cookies.append(dict(name='hd_admin_session',value='1',domain='127.0.0.1',path='/',httpOnly=True))
                for name,value in [('hd_admin_access',access),('hd_admin_refresh',refresh)]:
                    if value:cookies.append(dict(name=name,value=value,domain='127.0.0.1',path='/api',httpOnly=True))
                await context.add_cookies(cookies);return context
            async def mutation_retry():
                _,refresh=STATE.seed();context=await cookie_context(access='expired-mutation-access',refresh=refresh)
                try:
                    before=STATE.writes;rotations=STATE.rotations;key='synthetic-idempotency-'+str(uuid.uuid4());content={k:STATE.article[k] for k in ['title_en','title_sw','body_en','body_sw','category_id','tags','featured','sensitive']};content['title_en']='Recovered mutation';payload=dict(version=STATE.article['version'],content=content)
                    r=await context.request.patch(ORIGIN+f'/api/proxy/admin/articles/{ARTICLE}',data=payload,headers={'Origin':ORIGIN,'Idempotency-Key':key});assert r.status==200
                    assert STATE.writes==before+1 and STATE.rotations==rotations+1;assert STATE.mutation_body==payload and STATE.mutation_key==key
                finally:await context.close()
            await check('mutation retry preserves body, version and idempotency key with one authorized write',mutation_retry)
            async def cover_mutation():
                _,refresh=STATE.seed();context=await cookie_context(refresh=refresh)
                try:
                    before=STATE.writes;rotations=STATE.rotations;raw=b'synthetic-cover-transport'
                    r=await context.request.post(ORIGIN+f'/api/article-media/{ARTICLE}?version={STATE.article["version"]}',data=raw,headers={'Origin':ORIGIN,'Content-Type':'image/png'});assert r.status==200
                    assert STATE.writes==before+1 and STATE.rotations==rotations+1;assert STATE.cover_body==raw
                finally:await context.close()
            await check('cover mutation recovers access and preserves bounded binary payload',cover_mutation)
            async def rejected(kind):
                _,refresh=STATE.seed(revoked=kind=='revoked');refresh=refresh if kind=='revoked' else ('synthetic-invalid' if kind=='invalid' else None)
                context=await cookie_context(refresh=refresh)
                try:
                    before=STATE.protected;r=await context.request.get(ORIGIN+'/api/proxy/admin/overview');assert r.status==401
                    assert STATE.protected==before;assert not any(c['name'].startswith('hd_admin_') for c in await context.cookies())
                finally:await context.close()
            for kind in ['missing','invalid','revoked']:await check(f'{kind} refresh fails closed and clears credentials',lambda k=kind:rejected(k))
            async def transient(kind):
                _,refresh=STATE.seed();STATE.failures[refresh]=kind;context=await cookie_context(refresh=refresh)
                try:
                    r=await context.request.get(ORIGIN+'/api/proxy/admin/overview');assert r.status==502,f'Expected dependency failure, got {r.status}';assert any(c['name']=='hd_admin_refresh' for c in await context.cookies())
                    del STATE.failures[refresh]
                    # The existing five-second single-flight grace also keeps a
                    # rejected result, preventing immediate uncertain retries.
                    await asyncio.sleep(5.2)
                    r=await context.request.get(ORIGIN+'/api/proxy/admin/overview');assert r.status==200,f'Expected recovery after grace, got {r.status}'
                finally:await context.close()
            for kind in ['unavailable','malformed','disconnect']:await check(f'{kind} refresh returns truthful failure and can recover',lambda k=kind:transient(k))
            async def concurrent():
                _,refresh=STATE.seed();context=await cookie_context(refresh=refresh)
                try:
                    before=STATE.rotations;results=await asyncio.gather(*[context.request.get(ORIGIN+'/api/proxy/admin/overview') for _ in range(5)])
                    assert [r.status for r in results]==[200]*5;assert STATE.rotations==before+1
                    for r in results:assert 'access_token' not in await r.text() and 'refresh_token' not in await r.text()
                finally:await context.close()
            await check('five concurrent missing-access BFF requests rotate once',concurrent)
            async def expired():
                _,refresh=STATE.seed();context=await cookie_context(access='expired-fixture-access',refresh=refresh)
                try:
                    r=await context.request.get(ORIGIN+'/api/proxy/admin/overview');assert r.status==200
                    cookies={c['name']:c for c in await context.cookies()}
                    for c in cookies.values():assert c['httpOnly'] and c['secure'] and c['sameSite']=='Strict'
                    assert cookies['hd_admin_access']['path']=='/api';assert cookies['hd_admin_refresh']['path']=='/api'
                    assert 850<cookies['hd_admin_access']['expires']-time.time()<=901
                    assert cookies['hd_admin_session']['expires']-time.time()>6*86400
                    assert cookies['hd_admin_refresh']['expires']-time.time()>6*86400
                    page=await context.new_page();await page.goto(ORIGIN);assert '/login' not in page.url
                finally:await context.close()
            await check('expired access recovers; production cookies and recoverable shell lifetimes are preserved',expired)
            async def actual_cookie_expiry():
                access,refresh=STATE.seed();context=await cookie_context(refresh=refresh)
                try:
                    await context.add_cookies([dict(name='hd_admin_access',value=access,domain='127.0.0.1',path='/api',httpOnly=True,expires=time.time()+1)])
                    await asyncio.sleep(1.2);assert not any(c['name']=='hd_admin_access' for c in await context.cookies())
                    before=STATE.rotations;r=await context.request.get(ORIGIN+'/api/proxy/admin/overview');assert r.status==200 and STATE.rotations==before+1
                finally:await context.close()
            await check('real browser access-cookie expiry recovers through its surviving refresh cookie',actual_cookie_expiry)
            async def logout():
                _,refresh=STATE.seed();context=await cookie_context(refresh=refresh)
                try:
                    assert (await context.request.get(ORIGIN+'/api/proxy/admin/overview')).status==200
                    # Use the actual browser fetch for Secure cookies on the
                    # trusted loopback origin; APIRequestContext follows stricter
                    # HTTPS-only cookie sending and would omit them on HTTP.
                    page=await context.new_page();await page.goto(ORIGIN);before=STATE.logout_calls
                    status=await page.evaluate("fetch('/api/logout',{method:'POST'}).then(r=>r.status)")
                    assert status==200,f'Logout returned {status}';assert STATE.logout_calls==before+1,'Backend logout did not receive its refresh cookie'
                    assert not any(c['name'].startswith('hd_admin_') for c in await context.cookies()),'Logout retained a session cookie'
                    await context.add_cookies([dict(name='hd_admin_refresh',value=refresh,domain='127.0.0.1',path='/api',httpOnly=True)])
                    denied=await context.request.get(ORIGIN+'/api/proxy/admin/overview');assert denied.status==401,f'Cached revoked session returned {denied.status}'
                    assert not any(c['name'].startswith('hd_admin_') for c in await context.cookies()),'Revocation retained a session cookie'
                finally:await context.close()
            await check('logout clears credentials and cached rotation cannot restore a revoked family',logout)
            async def legacy_marker():
                _,refresh=STATE.seed();context=await cookie_context(refresh=refresh,marker=False)
                try:
                    before=STATE.rotations;page=await context.new_page();await page.goto(ORIGIN);assert '/login' not in page.url;assert STATE.rotations==before+1
                    assert any(c['name']=='hd_admin_session' for c in await context.cookies())
                finally:await context.close()
            await check('legacy expired shell marker recovers before login redirect',legacy_marker)
            async def final_revocation():
                _,refresh=STATE.seed();STATE.failures[refresh]='revoked_after_rotation';context=await cookie_context(refresh=refresh)
                try:
                    r=await context.request.get(ORIGIN+'/api/proxy/admin/overview');assert r.status==401;assert not any(c['name'].startswith('hd_admin_') for c in await context.cookies())
                finally:await context.close()
            await check('backend revocation after rotation remains authoritative',final_revocation)
            async def csrf():
                _,refresh=STATE.seed();context=await cookie_context(refresh=refresh)
                try:
                    before=STATE.rotations;r=await context.request.post(ORIGIN+'/api/proxy/admin/articles',data={},headers={'Origin':'https://untrusted.invalid'});assert r.status==403 and STATE.rotations==before
                    r=await context.request.post(ORIGIN+'/api/diagnostics',data=dict(error_type='NETWORK',message='Synthetic incident',stack_trace='',screen='blogs',operation='load',occurred_at='2026-10-26T08:00:00Z'),headers={'Origin':ORIGIN});assert r.status==200
                finally:await context.close()
            await check('CSRF denial precedes refresh and diagnostics uses the common resolver',csrf)
            async def sidebar():
                access,refresh=STATE.seed();context=await browser.new_context();await context.add_cookies([dict(name=n,value=v,url=ORIGIN,httpOnly=True) for n,v in [('hd_admin_session','1'),('hd_admin_access',access),('hd_admin_refresh',refresh)]])
                try:
                    page=await context.new_page();await page.goto(ORIGIN);await page.locator('#adminNav-blogs').click();await page.get_by_role('button',name='Open / preview').click();await page.get_by_label('English title',exact=True).fill('Unsaved local title');await page.locator('#adminNav-finance').click()
                    await page.get_by_role('button',name='Stay',exact=True).wait_for(timeout=1800);await page.get_by_role('button',name='Stay',exact=True).click();assert await page.get_by_label('English title',exact=True).input_value()=='Unsaved local title'
                    await page.locator('#adminNav-finance').click();await page.get_by_role('button',name='Discard and Leave',exact=True).click();assert await page.locator('#adminPage_finance').count()==1
                finally:await context.close()
            await check('actual sidebar protects unsaved article and allows explicit discard',sidebar)
            async def editor_page():
                access,refresh=STATE.seed();context=await cookie_context(access=access,refresh=refresh);page=await context.new_page();await page.goto(ORIGIN+'/login');await page.goto(ORIGIN);await page.locator('#adminNav-blogs').click();await page.get_by_role('button',name='Open / preview').click();return context,page
            async def traversal():
                context,page=await editor_page()
                try:
                    await page.get_by_label('English title',exact=True).fill('Keep on browser Back');await page.go_back();await page.get_by_role('button',name='Stay',exact=True).click();assert await page.get_by_label('English title',exact=True).input_value()=='Keep on browser Back'
                    assert not await page.evaluate("JSON.stringify(history.state).includes('Keep on browser Back') || JSON.stringify(localStorage).includes('Keep on browser Back') || JSON.stringify(sessionStorage).includes('Keep on browser Back')")
                    await page.go_back();await page.get_by_role('button',name='Discard and Leave',exact=True).click();await page.wait_for_url('**/login')
                finally:await context.close()
            await check('browser Back can Stay or discard with opaque history metadata only',traversal)
            async def unload():
                context,page=await editor_page()
                try:
                    await page.get_by_label('English title',exact=True).fill('Keep when closing tab')
                    async with page.expect_event('dialog') as pending:await page.close(run_before_unload=True)
                    dialog=await pending.value;assert dialog.type=='beforeunload';await dialog.dismiss();assert not page.is_closed()
                    assert await page.get_by_label('English title',exact=True).input_value()=='Keep when closing tab'
                finally:await context.close()
            await check('native tab-close warning can retain the unsaved editor',unload)
            async def quiz_dirty():
                context,page=await editor_page()
                try:
                    await page.get_by_role('button',name='Open MCQ Builder',exact=True).click();toggle=page.get_by_label('Include MCQs in this article',exact=True);await toggle.check()
                    await page.locator('#adminNav-finance').click();await page.get_by_role('button',name='Stay',exact=True).click();assert await toggle.is_checked()
                    await toggle.uncheck();await page.get_by_role('button',name='Add question',exact=True).click();await page.get_by_role('button',name='Back to articles',exact=True).click();await page.get_by_role('button',name='Stay',exact=True).click();assert await page.locator('.cms-question').count()==2
                    await page.get_by_role('button',name='Back to articles',exact=True).click();await page.get_by_role('button',name='Discard and Leave',exact=True).click();await page.get_by_role('button',name='Create article',exact=True).wait_for()
                finally:await context.close()
            await check('optional quiz inclusion and manual questions guard sidebar and Editor Back',quiz_dirty)
            async def cover_dirty():
                context,page=await editor_page()
                try:
                    await page.get_by_label('Cover image (JPEG/PNG, up to 5 MB)',exact=True).set_input_files(dict(name='cover.png',mimeType='image/png',buffer=b'synthetic-preview-image'))
                    await page.locator('#adminNav-finance').click();await page.get_by_role('button',name='Stay',exact=True).click();await page.get_by_role('button',name='Remove selected cover',exact=True).click()
                    await page.locator('#adminNav-finance').click();assert await page.get_by_role('alertdialog').count()==0
                finally:await context.close()
            await check('selected cover remains guarded and clearing it restores clean navigation',cover_dirty)
            async def pending_import():
                context,page=await editor_page();STATE.import_started.clear();STATE.import_gate=threading.Event()
                try:
                    await page.get_by_role('button',name='Open MCQ Builder',exact=True).click();await page.get_by_text('Import CSV / JSON questions',exact=True).click()
                    await page.get_by_label('Question import file').set_input_files(dict(name='quiz.json',mimeType='application/json',buffer=b'{"schema_version":1,"questions":[]}'))
                    await page.get_by_role('button',name='Preview import',exact=True).click();await page.get_by_role('button',name='Confirm import to draft',exact=True).click()
                    for _ in range(100):
                        if STATE.import_started.is_set():break
                        await asyncio.sleep(.02)
                    assert STATE.import_started.is_set();assert await page.locator('.cms-question textarea').first.is_disabled()
                    await page.locator('#adminNav-finance').click();assert await page.get_by_role('button',name='Discard and Leave',exact=True).is_disabled()
                    STATE.import_gate.set();await page.get_by_text('Questions imported to draft.',exact=False).wait_for();await page.get_by_role('button',name='Stay',exact=True).click()
                    assert await page.locator('.cms-question').count()==2
                    await page.locator('#adminNav-finance').click();assert await page.get_by_role('alertdialog').count()==0
                except Exception:
                    (OUT/f'pending-import-{phase}.html').write_text(await page.content(),encoding='utf-8');raise
                finally:STATE.import_gate.set();STATE.import_gate=None;await context.close()
            await check('pending import locks editing/navigation and applies the intended persisted draft',pending_import)
            await browser.close()
    finally:
        subprocess.run(['taskkill','/PID',str(process.pid),'/T','/F'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);process.wait(timeout=20);log.close();server.shutdown();server.server_close();(OUT/f'browser-{phase}.json').write_text(json.dumps(checks,indent=2))
    if any(not c['passed'] for c in checks):raise SystemExit(1)

if __name__=='__main__':asyncio.run(main())
