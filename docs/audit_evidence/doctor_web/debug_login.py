import asyncio, re
from pathlib import Path
from playwright.async_api import async_playwright
ROOT=Path(__file__).resolve().parents[3]
async def main():
    source=(ROOT/'services/backend/src/seed.rs').read_text()
    def c(name):return re.search(rf'pub const {name}: &str = "([^"]+)"',source)[1]
    async with async_playwright() as p:
        browser=await p.chromium.launch(channel='msedge',headless=True)
        page=await browser.new_page()
        await page.goto('http://127.0.0.1:3001',wait_until='networkidle')
        await page.get_by_label('Email address',exact=True).fill(c('SEED_DOCTOR1_EMAIL'))
        await page.get_by_label('Password',exact=True).fill(c('SEED_DOCTOR1_PASSWORD'))
        async with page.expect_response('**/api/auth/login') as r:
            await page.get_by_role('button',name='Doctor Secure Sign In').click()
        res=await r.value
        body=await res.json()
        print({'status':res.status,'error':body.get('error'),'development_code_present':bool(body.get('data',{}).get('development_otp'))})
        print(await page.locator('main').first.inner_text())
        await browser.close()
asyncio.run(main())
