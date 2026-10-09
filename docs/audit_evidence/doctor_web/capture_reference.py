import asyncio
import json
from pathlib import Path
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).parent / 'reference'

async def main():
    OUT.mkdir(parents=True, exist_ok=True)
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel='msedge', headless=True)
        page = await browser.new_page(viewport={'width':1440,'height':1000})
        fonts = {}
        async def font_response(response):
            if 'fonts.gstatic.com' in response.url and '.woff2' in response.url:
                fonts[response.url] = await response.body()
        page.on('response', font_response)
        await page.goto((ROOT/'prototype/index.html').as_uri(), wait_until='networkidle')
        await page.evaluate('document.fonts.ready')
        # Demo workbench/toasts are outside the Doctor experience. Hide without
        # changing shell geometry or modifying the canonical source.
        await page.add_style_tag(content='.wb,#appToastContainer,#dwToastContainer{visibility:hidden!important}')
        styles = {}
        for width in [1440,1280,1024,768,390]:
            await page.set_viewport_size({'width':width,'height':1000})
            await page.evaluate("switchRole('doctor-web')")
            for name in ['dashboard','appointments','patients','telehealth','qna']:
                await page.evaluate('(name) => switchDoctorWebPage(name)',name)
                await page.locator('#doctorWebShell').screenshot(path=str(OUT/f'desktop_{name}_{width}.png'))
                styles[f'{name}_{width}'] = await page.locator('#doctorWebShell').evaluate('''root => [...root.querySelectorAll('.dw-topbar,.dw-sidebar,.dw-page-header,.dw-pt-card,.dw-kpi-card,.dw-section-card')].map(el=>{const r=el.getBoundingClientRect(),s=getComputedStyle(el);return {class:el.className,x:r.x,y:r.y,width:r.width,height:r.height,font:s.font,padding:s.padding,gap:s.gap,radius:s.borderRadius,shadow:s.boxShadow}})''')
            await page.evaluate("switchRole('doctor')")
            # Responsive Doctor views without the demo handset bezel (documented platform difference).
            await page.add_style_tag(content=f'.phone{{width:{width if width<600 else 390}px!important;padding:0!important;border-radius:0!important}} .screen-shell{{height:800px!important;border-radius:0!important}}')
            for name,tab in [('queue',0),('chat',4),('rx',1),('schedule',2),('wallet',3),('qna',5)]:
                await page.evaluate('(tab)=>switchDoctorTab(tab)',tab)
                await page.locator('#appShell').screenshot(path=str(OUT/f'doctor_{name}_{width}.png'))
        # Font bytes are project assets, not software installation. Latin subset CSS URLs.
        fontdir = ROOT/'apps/doctor-web/public/fonts'
        fontdir.mkdir(parents=True,exist_ok=True)
        css = await page.request.get('https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=Inter:wght@400;500;600;700&display=swap',headers={'User-Agent':'Mozilla/5.0'})
        import re
        text = await css.text()
        (OUT/'font-source.css').write_text(text,encoding='utf-8')
        for name,file in [('Plus Jakarta Sans','plus-jakarta-sans.woff2'),('Inter','inter.woff2')]:
            blocks = re.findall(r'@font-face\s*\{([^}]+)\}',text)
            matches = [b for b in blocks if f"font-family: '{name}'" in b and 'U+0000-00FF' in b]
            if matches:
                url = re.search(r'url\(([^)]+)\)',matches[-1])[1]
                response = await page.request.get(url)
                (fontdir/file).write_bytes(await response.body())
        (OUT/'computed_styles.json').write_text(json.dumps(styles,indent=2),encoding='utf-8')
        await browser.close()
    print('Captured 55 current canonical references using installed Edge. Font assets saved locally.')

if __name__=='__main__': asyncio.run(main())
