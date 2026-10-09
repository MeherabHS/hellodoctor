"""Real credential/MFA and installed Edge session recovery; no saved tokens."""
import asyncio,json,subprocess,uuid
from pathlib import Path
from playwright.async_api import async_playwright
HERE=Path(__file__).parent;BASE='http://127.0.0.1:3001'
async def main():
    # Separate loopback source prevents verification consuming the user's
    # remaining shared-IP attempts. No rate-limit keys or policies are changed.
    process=subprocess.run(['node',str(HERE/'check_login_recovery.mjs')],capture_output=True,text=True)
    data=json.loads(process.stdout)
    if process.returncode:
        print(json.dumps({key:value for key,value in data.items() if key in ['login','mfa','error']}));return
    result={'backend_password_login':data['login'],'backend_mfa':data['mfa'],'console_errors':[],'checks':{},'scope':'Real development credentials/MFA, then HttpOnly cookies supplied by test automation; no UI password submit in this check'}
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        context=await browser.new_context();page=await context.new_page()
        await page.goto('http://localhost:3001/',wait_until='networkidle')
        assert page.url==BASE+'/'
        result['checks']['localhost_redirect']='PASS'
        headers={'Origin':'https://untrusted.example'}
        denied=await context.request.post(BASE+'/api/auth/login',data={},headers=headers)
        assert denied.status==403;result['checks']['foreign_origin_still_denied']='PASS'
        await context.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',data['access']),('hd_doctor_refresh',data['refresh']),('hd_doctor_scope',str(uuid.uuid4()))]])
        page.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        await page.reload(wait_until='networkidle');await page.locator('#doctorWebShell').wait_for()
        assert not await page.evaluate('document.cookie')
        result['checks']['browser_session_restored']='PASS';result['checks']['tokens_not_js_readable']='PASS'
        await page.locator('.dw-user-badge').click();await page.get_by_role('button',name='Log out',exact=True).click()
        await page.get_by_label('Email address',exact=True).wait_for()
        result['checks']['logout']='PASS'
        await browser.close()
    (HERE/'login-recovery.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result))
if __name__=='__main__':asyncio.run(main())
