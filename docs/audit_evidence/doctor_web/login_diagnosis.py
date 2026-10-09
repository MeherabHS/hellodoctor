"""Diagnose user-visible login with installed Edge and current development seed."""
import asyncio,json,re
from pathlib import Path
from playwright.async_api import async_playwright
ROOT=Path(__file__).resolve().parents[3]
async def main():
    source=(ROOT/'services/backend/src/seed.rs').read_text(encoding='utf-8')
    def constant(name):return re.search(rf'pub const {name}: &str = "([^"]+)"',source)[1]
    result=[]
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        for base in ['http://127.0.0.1:3001','http://localhost:3001']:
            ctx=await browser.new_context();page=await ctx.new_page();item={'origin':base,'console_errors':[],'responses':[]}
            page.on('pageerror',lambda e,item=item:item['console_errors'].append(str(e)))
            async def response(r,item=item):
                if '/api/auth/' in r.url:
                    body=await r.json();item['responses'].append({'action':r.url.rsplit('/',1)[-1],'status':r.status,'error':body.get('error',{}).get('code')})
            page.on('response',response)
            await page.goto(base,wait_until='networkidle')
            item['effective_origin']=page.url
            item['buttons']=await page.get_by_role('button').all_text_contents()
            await page.get_by_label('Email address',exact=True).fill(constant('SEED_DOCTOR1_EMAIL'))
            await page.get_by_label('Password',exact=True).fill(constant('SEED_DOCTOR1_PASSWORD'))
            await page.get_by_role('button',name='Doctor Secure Sign In').click()
            await page.wait_for_timeout(1000)
            if await page.get_by_label('Verification code').count():
                code=await page.locator('.web-gap strong').inner_text()
                await page.get_by_label('Verification code').fill(code)
                await page.get_by_role('button',name='Verify & sign in').click()
                await page.wait_for_timeout(1000)
                item['signed_in']=await page.locator('#doctorWebShell').is_visible()
                item['cookie_flags']=[{'name':c['name'],'httpOnly':c['httpOnly'],'secure':c['secure'],'sameSite':c['sameSite']} for c in await ctx.cookies()]
            else:item['signed_in']=False
            item['alerts']=await page.get_by_role('alert').all_text_contents()
            result.append(item);await ctx.close()
        await browser.close()
    out=Path(__file__).parent/'login-diagnosis.json';out.write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result))
if __name__=='__main__':asyncio.run(main())
