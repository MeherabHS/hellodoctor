"""Actual Edge login/MFA/logout with controlled development credentials.

No password, OTP, token, patient response or cookie value is saved or logged.
"""
import asyncio,json,re
from pathlib import Path
from playwright.async_api import async_playwright

HERE=Path(__file__).parent
ROOT=HERE.resolve().parents[2]
BASE='http://127.0.0.1:3001'

async def main():
    source=(ROOT/'services/backend/src/seed.rs').read_text(encoding='utf-8')
    def constant(name):
        return re.search(r'pub const '+name+r': &str = "([^"]+)"',source).group(1)
    result={'checks':{},'console_errors':[],'auth_responses':[], 'fixture':'Controlled development Doctor 2; no credentials recorded'}
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        context=await browser.new_context()
        page=await context.new_page()
        page.on('pageerror',lambda error:result['console_errors'].append(str(error)))
        page.on('response',lambda response: result['auth_responses'].append({'action':response.url.rsplit('/',1)[-1],'status':response.status}) if '/api/auth/' in response.url else None)
        await page.goto('http://localhost:3001/',wait_until='networkidle')
        assert page.url==BASE+'/', 'Canonical origin redirect failed'
        result['checks']['localhost_redirect']='PASS'
        await page.get_by_label('Email address',exact=True).fill(constant('SEED_DOCTOR2_EMAIL'))
        await page.get_by_label('Password',exact=True).fill(constant('SEED_DOCTOR1_PASSWORD'))
        await page.get_by_role('button',name=re.compile('Doctor Secure Sign In')).click()
        await page.get_by_label('Verification code',exact=True).wait_for(timeout=20000)
        result['checks']['ui_password_login']='PASS'
        code=await page.locator('.web-gap strong').inner_text()
        await page.get_by_label('Verification code',exact=True).fill(code)
        await page.get_by_role('button',name=re.compile('Verify & sign in')).click()
        await page.locator('#doctorWebShell').wait_for(timeout=20000)
        result['checks']['ui_mfa']='PASS'
        assert not await page.evaluate('document.cookie'), 'Session is readable by JavaScript'
        result['checks']['tokens_not_js_readable']='PASS'
        await page.reload(wait_until='networkidle')
        await page.locator('#doctorWebShell').wait_for()
        result['checks']['reload_session_restored']='PASS'
        denied=await context.request.post(BASE+'/api/auth/login',data={},headers={'Origin':'https://untrusted.example'})
        assert denied.status==403
        result['checks']['foreign_origin_denied']='PASS'
        await page.locator('.dw-user-badge').click()
        await page.get_by_role('button',name='Log out',exact=True).click()
        await page.get_by_label('Email address',exact=True).wait_for()
        result['checks']['logout']='PASS'
        assert not result['console_errors'], 'Browser console exception'
        await browser.close()
    (HERE/'login-ui-recovery.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result))

if __name__=='__main__':asyncio.run(main())
