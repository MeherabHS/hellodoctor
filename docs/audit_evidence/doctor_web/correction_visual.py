"""Render actual canonical Web prototype and corrected Web in installed Edge.

Only controlled development accounts are used. No cookies/tokens/PHI saved.
Canonical screenshots remain distinct from viewport-normalized comparisons.
"""
import asyncio,json,re
from pathlib import Path
from playwright.async_api import async_playwright

ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).parent/'correction'
BASE='http://127.0.0.1:3001'
SIZES=[(1920,1080),(1440,900),(1280,720),(1024,768),(390,844)]
PRIMARY=['dashboard','appointments','patients','telehealth','qna']
NORMALIZE='''
.wb,#appToastContainer,#dwToastContainer{display:none!important}
.canvas{padding:0!important;width:100%!important;min-height:100dvh!important;display:block!important}
#doctorWebShell{width:100%!important;max-width:none!important;height:100dvh!important;min-height:100dvh!important;margin:0!important;border:0!important;border-radius:0!important;box-shadow:none!important;transition:none!important}
.dw-station-layout,.dw-meet-container{min-height:0!important}
.dw-main-content-area,.dw-page-view,.dw-meet-video-stage,.dw-meet-side-panel,.dw-swap-view{min-height:0!important;min-width:0!important}
.dw-meet-bottom-bar{flex-wrap:wrap!important;gap:10px!important}
.dw-meet-bar-left,.dw-meet-bar-right{min-width:0!important}
.dw-meet-bar-center{flex-wrap:wrap!important}
.dw-meet-side-panel{width:min(440px,42%)!important}
body{padding:0!important;margin:0!important}
'''
MEASURE='''() => {
 const selectors=['#doctorWebShell','.dw-topbar','.dw-sidebar','#dwMainContentArea','.web-primary-view','.dw-page-view.active','.dw-meet-container','.dw-meet-video-stage','.dw-meet-side-panel','.dw-kpi-card','.dw-section-card'];
 return Object.fromEntries(selectors.map(selector=>[selector,[...document.querySelectorAll(selector)].filter(el=>el.getClientRects().length).map(el=>{const r=el.getBoundingClientRect(),s=getComputedStyle(el);return {x:r.x,y:r.y,width:r.width,height:r.height,padding:s.padding,radius:s.borderRadius,border:s.border,shadow:s.boxShadow,font:s.font,gap:s.gap,maxWidth:s.maxWidth}})]));
}'''

async def main():
    OUT.mkdir(exist_ok=True)
    results={'screens':{},'console_errors':[],'network':[],'checks':{}}
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        reference=await browser.new_page()
        await reference.goto((ROOT/'prototype/index.html').as_uri(),wait_until='networkidle')
        await reference.evaluate('document.fonts.ready')
        await reference.evaluate("switchRole('doctor-web')")
        await reference.add_style_tag(content='.wb,#appToastContainer,#dwToastContainer{visibility:hidden!important}')
        for width,height in SIZES:
            await reference.set_viewport_size({'width':width,'height':height})
            for name in PRIMARY:
                await reference.evaluate('(name)=>switchDoctorWebPage(name)',name)
                await reference.screenshot(animations='disabled',path=str(OUT/f'canonical_{name}_{width}.png'))
        await reference.add_style_tag(content=NORMALIZE)
        for width,height in SIZES:
            await reference.set_viewport_size({'width':width,'height':height})
            for name in PRIMARY:
                await reference.evaluate('(name)=>switchDoctorWebPage(name)',name)
                await reference.screenshot(animations='disabled',path=str(OUT/f'reference_viewport_{name}_{width}.png'))
                results['screens'][f'{name}_{width}']={'reference':await reference.evaluate(MEASURE)}
        source=(ROOT/'services/backend/src/seed.rs').read_text(encoding='utf-8')
        def constant(name):return re.search(r'pub const '+name+r': &str = "([^"]+)"',source).group(1)
        context=await browser.new_context(viewport={'width':1920,'height':1080})
        page=await context.new_page()
        page.on('pageerror',lambda error:results['console_errors'].append(str(error)))
        page.on('response',lambda response:results['network'].append({'path':response.url.split(BASE)[-1].split('?')[0],'method':response.request.method,'status':response.status}) if response.url.startswith(BASE+'/api/') else None)
        await page.goto(BASE,wait_until='networkidle')
        await page.get_by_label('Email address',exact=True).fill(constant('SEED_DOCTOR1_EMAIL'))
        await page.get_by_label('Password',exact=True).fill(constant('SEED_DOCTOR1_PASSWORD'))
        await page.get_by_role('button',name=re.compile('Doctor Secure Sign In')).click()
        await page.get_by_label('Verification code',exact=True).wait_for()
        await page.get_by_label('Verification code',exact=True).fill(await page.locator('.web-gap strong').inner_text())
        await page.get_by_role('button',name=re.compile('Verify & sign in')).click()
        await page.locator('#doctorWebShell').wait_for()
        results['checks']['ui_login_mfa']='PASS'
        async def nav(name):
            if (await page.locator('.web-menu-toggle').is_visible()) and 'web-sidebar-open' not in (await page.locator('#doctorWebShell').get_attribute('class')):
                await page.get_by_role('button',name='Open navigation',exact=True).click()
                await page.wait_for_timeout(250)
            await page.locator(f'.dw-nav-item[data-page="{name}"]').click()
            await page.wait_for_timeout(250)
        async def record(name,width):
            await page.wait_for_timeout(300)
            await page.screenshot(animations='disabled',path=str(OUT/f'web_{name}_{width}.png'))
            measure=await page.evaluate(MEASURE)
            results['screens'].setdefault(f'{name}_{width}',{})['web']=measure
            r=measure['#doctorWebShell'][0]
            assert r['x']==0 and r['y']==0 and round(r['width'])==width
            assert r['maxWidth']=='none' and r['radius']=='0px' and r['shadow']=='none'
            assert await page.locator('#doctorAppShell,.doc-bnav,.bn-item,.web-primary-nav').count()==0
            assert not await page.get_by_text('This screen could not be rendered',exact=False).count()
            assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth'), f'{name} horizontal overflow'
            results['screens'][f'{name}_{width}']['shell_checks']='PASS'
        for width,height in SIZES:
            await page.set_viewport_size({'width':width,'height':height})
            for name in PRIMARY:
                await nav(name);await record(name,width)
            await nav('dashboard')
            await page.get_by_role('button',name='Instant Rx',exact=True).click();await record('rx',width)
            assert (await page.locator('#webRxWorkspace').bounding_box())['width']>width-260
            await nav('appointments')
            await page.get_by_role('button',name='Manage consultation availability').click();await record('schedule',width)
            await nav('dashboard')
            await page.get_by_role('button',name=re.compile('Earnings & Wallet')).click();await record('wallet',width)
            await nav('telehealth')
            await page.locator('#dwMeetSwapTabChat').click()
            await page.get_by_role('button',name='Open Consultation Chats',exact=True).click();await record('chat',width)
        await page.set_viewport_size({'width':1440,'height':900})
        await nav('appointments');await page.get_by_role('button',name='+ Add Schedule Slot',exact=True).click()
        await page.screenshot(animations='disabled',path=str(OUT/'web_slot_dialog_1440.png'))
        await page.keyboard.press('Escape')
        assert await page.locator('[aria-modal="true"]').count()==0
        results['checks']['slot_dialog_escape']='PASS'
        await nav('dashboard')
        await page.get_by_role('button',name='View the monthly earnings statement').click()
        await page.screenshot(animations='disabled',path=str(OUT/'web_statement_dialog_1440.png'))
        await page.keyboard.press('Escape')
        await page.reload(wait_until='networkidle')
        await page.locator('#doctorWebShell').wait_for()
        results['checks']['session_reload']='PASS'
        await page.locator('.dw-user-badge').click()
        await page.screenshot(animations='disabled',path=str(OUT/'web_settings_dialog_1440.png'))
        await page.get_by_role('button',name='Log out',exact=True).click()
        await page.get_by_label('Email address',exact=True).wait_for()
        results['checks']['ui_logout']='PASS'
        assert not results['console_errors']
        (OUT/'visual-browser-results.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
        await browser.close()
    print(json.dumps({'checks':results['checks'],'screen_captures':len(results['screens']),'console_errors':results['console_errors'],'network_responses':len(results['network'])}))

if __name__=='__main__':asyncio.run(main())
