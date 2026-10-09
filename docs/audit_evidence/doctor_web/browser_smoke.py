"""Installed Edge, real backend; no browser download, saved cookies or tokens."""
import asyncio, json, re
from pathlib import Path
from playwright.async_api import async_playwright
ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).parent/'web'
BASE = 'http://127.0.0.1:3001'

async def main():
    OUT.mkdir(exist_ok=True)
    source = (ROOT/'services/backend/src/seed.rs').read_text()
    def constant(name): return re.search(rf'pub const {name}: &str = "([^"]+)"',source)[1]
    results={'console_errors':[], 'unexpected_requests':[], 'checks':{}}
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        context=await browser.new_context(viewport={'width':1440,'height':1000})
        page=await context.new_page()
        page.on('pageerror',lambda e:results['console_errors'].append(str(e)))
        await page.goto(BASE,wait_until='networkidle')
        await page.screenshot(path=str(OUT/'auth_1440.png'))
        await page.get_by_label('Email address',exact=True).fill(constant('SEED_DOCTOR1_EMAIL'))
        await page.get_by_label('Password',exact=True).fill(constant('SEED_DOCTOR1_PASSWORD'))
        await page.get_by_role('button',name='Doctor Secure Sign In').click()
        await page.get_by_label('Verification code').wait_for()
        code=await page.locator('.web-gap strong').inner_text()
        await page.get_by_label('Verification code').fill(code)
        await page.get_by_role('button',name='Verify & sign in').click()
        await page.locator('#doctorWebShell').wait_for()
        await page.wait_for_timeout(600)
        results['checks']['login_mfa']='PASS'
        results['checks']['tokens_not_js_cookies']='PASS' if not await page.evaluate('document.cookie') else 'FAIL'
        responses=[]
        def response(r):
            if '/api/' in r.url: responses.append({'path':r.url.split(BASE)[-1].split('?')[0],'status':r.status,'method':r.request.method})
        page.on('response',response)
        for width in [1440,1280,1024,768,390]:
            await page.set_viewport_size({'width':width,'height':1000})
            for name,selector in [('queue','.bn-item:nth-child(1)'),('chat','.bn-item:nth-child(2)'),('rx','.bn-item:nth-child(3)'),('schedule','.bn-item:nth-child(4)'),('wallet','.bn-item:nth-child(5)')]:
                await page.locator(selector).click()
                await page.wait_for_timeout(400)
                await page.screenshot(path=str(OUT/f'{name}_{width}.png'))
                text=await page.locator('#dwMainContentArea').inner_text()
                results['checks'][f'{name}_{width}']='FAIL' if 'This screen could not be rendered' in text else 'RENDERED'
            if width>=768:
                for name in ['dashboard','appointments','patients','telehealth','qna']:
                    await page.locator(f'.dw-nav-item[data-page="{name}"]').click()
                    await page.wait_for_timeout(400)
                    await page.screenshot(path=str(OUT/f'desktop_{name}_{width}.png'))
            else:
                await page.locator('.bn-item').first.click()
                await page.get_by_text('Open Patient Q&A',exact=False).first.click()
                await page.wait_for_timeout(400)
                await page.screenshot(path=str(OUT/f'qna_{width}.png'))
        results['network']=responses
        results['checks']['session_reload']='PENDING'
        await page.reload(wait_until='networkidle')
        await page.locator('#doctorWebShell').wait_for()
        results['checks']['session_reload']='PASS'
        await page.locator('.dw-user-badge').click()
        await page.screenshot(path=str(OUT/'profile_390.png'))
        await page.get_by_role('button',name='Log out',exact=True).click()
        await page.get_by_label('Email address',exact=True).wait_for()
        results['checks']['logout']='PASS'
        await browser.close()
    (OUT/'smoke.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
    print(json.dumps({'checks':results['checks'],'console_errors':results['console_errors'],'responses':len(responses)}))

if __name__=='__main__': asyncio.run(main())
