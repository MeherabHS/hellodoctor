"""Narrow viewport evidence after disabling responsive transition animation."""
import asyncio
import base64
import json
import uuid
from playwright.async_api import async_playwright, expect
from rx_compression_regression import HERE, ROOT, OUT, BASE
from workflow_extra import token

async def main():
    doctor=json.loads((HERE/'workflow/results.json').read_text())['fixture_ids']['doctors'][0]
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        ctx=await browser.new_context(viewport={'width':390,'height':844})
        await ctx.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(doctor)),('hd_doctor_scope',str(uuid.uuid4()))]])
        page=await ctx.new_page()
        errors=[]
        page.on('pageerror',lambda error:errors.append(str(error)))
        await page.goto(BASE,wait_until='networkidle')
        await page.get_by_label('Open navigation',exact=True).click()
        await page.locator('#doctorWebSidebar [data-page="appointments"]').click()
        await page.get_by_role('button',name='Write Rx',exact=True).last.click()
        await page.locator('#webRxWorkspace').wait_for()
        photo=await page.evaluate('''() => {
          const canvas=document.createElement('canvas');canvas.width=1600;canvas.height=1200;
          const c=canvas.getContext('2d');c.fillStyle='white';c.fillRect(0,0,1600,1200);
          c.fillStyle='#163f36';c.font='48px sans-serif';
          c.fillText('SYNTHETIC PRESCRIPTION',70,100);c.fillText('Browser compression test only',70,180);
          c.font='36px sans-serif';c.fillText('Sample handwritten photo workflow',70,330);
          c.fillText('No clinical advice or patient information',70,420);
          const noise=c.createImageData(1600,300);let seed=123;
          for(let i=0;i<noise.data.length;i+=4){for(let j=0;j<3;j++){seed=(Math.imul(seed,1664525)+1013904223)>>>0;noise.data[i+j]=seed>>>24;}noise.data[i+3]=255;}
          c.putImageData(noise,0,900);return canvas.toDataURL('image/png').split(',')[1];
        }''')
        await page.locator('#rxPhotoFileInput').set_input_files({'name':'synthetic-prescription.png','mimeType':'image/png','buffer':base64.b64decode(photo)})
        await expect(page.locator('.web-rx-preview-grid figure')).to_have_count(1)
        await expect(page.locator('#btnSendCapturedRx')).to_be_enabled()
        cdp=await ctx.new_cdp_session(page)
        await cdp.send('Network.enable')
        await cdp.send('Network.emulateNetworkConditions',{'offline':False,'latency':100,'downloadThroughput':1024*1024,'uploadThroughput':32*1024})
        await page.locator('#btnSendCapturedRx').click()
        await page.wait_for_function('''() => {const p=document.querySelector('.web-rx-upload-progress progress');return p&&p.hasAttribute('value')&&p.value>0&&p.value<100;}''')
        await page.locator('.web-rx-upload-progress').scroll_into_view_if_needed()
        assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth')
        await expect(page.locator('#doctorWebSidebar')).not_to_be_visible()
        await page.screenshot(animations='disabled',path=str(OUT/'actual-upload-progress-390.png'))
        await expect(page.locator('.web-rx-preview-grid figcaption')).to_contain_text('FAILED',timeout=60000)
        assert not errors
        (OUT/'narrow-visual.json').write_text(json.dumps({'build_id':(ROOT/'apps/doctor-web/.next/BUILD_ID').read_text().strip(),'narrow_progress_visible':'PASS','sidebar_closed':'PASS','horizontal_overflow':False,'console_errors':errors})+'\n')
        await browser.close()
        print('Narrow upload progress / closed Web drawer / no overflow / no exceptions: PASS')

if __name__=='__main__':asyncio.run(main())
