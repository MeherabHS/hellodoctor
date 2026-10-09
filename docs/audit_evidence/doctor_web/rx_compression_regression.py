"""Native image compression and real XHR progress, installed Edge only.

Generated photos and existing synthetic appointment fixtures; real backend
storage failure must remain failure. No browser/package/storage installation.
"""
import asyncio
import base64
import hashlib
import json
import re
import uuid
from pathlib import Path
from playwright.async_api import async_playwright, expect
from workflow_extra import token, sql

HERE=Path(__file__).parent
ROOT=HERE.parents[2]
OUT=HERE/'rx-compression'
BASE='http://127.0.0.1:3001'

async def main():
    OUT.mkdir(exist_ok=True)
    result={'checks':{},'console_errors':[],'uploads':[],'progress_samples':[],'backend_responses':[]}
    fixtures=json.loads((HERE/'workflow/results.json').read_text())
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        selected=None
        for doctor in fixtures['fixture_ids']['doctors']:
            ctx=await browser.new_context(viewport={'width':1440,'height':900})
            await ctx.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(doctor)),('hd_doctor_scope',str(uuid.uuid4()))]])
            headers={'Origin':BASE,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await ctx.cookies())}
            response=await ctx.request.get(BASE+'/api/proxy/doctor/appointments',headers=headers)
            assert response.status==200
            for row in (await response.json())['data']:
                appointment=str(uuid.UUID(row['id']))
                if row['history_access_active'] and sql(f"SELECT COUNT(*) FROM prescriptions WHERE appointment_id='{appointment}'")=='0':
                    selected=(ctx,row);break
            if selected:break
            await ctx.close()
        assert selected,'An existing authorized synthetic fixture is required; do not bypass authorization.'
        ctx,appointment=selected
        page=await ctx.new_page()
        page.on('pageerror',lambda error:result['console_errors'].append(str(error)))
        await page.goto(BASE,wait_until='networkidle')
        await page.locator('#doctorWebSidebar [data-page="appointments"]').click()
        await page.locator(f'[data-appointment-id="{appointment["id"]}"]').get_by_role('button',name='Write Rx',exact=True).click()
        await page.locator('#webRxWorkspace').wait_for()
        # Only synthetic image bytes leave this canvas. No real patient content.
        assets=await page.evaluate('''async () => {
          const canvas=document.createElement('canvas');canvas.width=1600;canvas.height=1200;
          const context=canvas.getContext('2d');const data=context.createImageData(1600,1200);
          let seed=123456789;
          for(let i=0;i<data.data.length;i+=4){for(let c=0;c<3;c++){seed=(Math.imul(seed,1664525)+1013904223)>>>0;data.data[i+c]=seed>>>24;}data.data[i+3]=255;}
          context.putImageData(data,0,0);
          const result=[];
          for(const mime of ['image/jpeg','image/png','image/webp']){
            const blob=await new Promise(resolve=>canvas.toBlob(resolve,mime,0.98));
            const encoded=await new Promise(resolve=>{const r=new FileReader();r.onload=()=>resolve(r.result.split(',')[1]);r.readAsDataURL(blob);});
            result.push({mime,bytes:blob.size,encoded});
          }return result;
        }''')
        assert all(409600 < item['bytes'] <= 10*1024*1024 for item in assets)
        await page.locator('#rxPhotoFileInput').set_input_files([{'name':f'synthetic-noise-{index}.{item["mime"].split("/")[1]}','mimeType':item['mime'],'buffer':base64.b64decode(item['encoded'])} for index,item in enumerate(assets)])
        await expect(page.locator('.web-rx-preview-grid figure')).to_have_count(3)
        await expect(page.locator('#btnSendCapturedRx')).to_be_enabled()
        # CSP deliberately disallows fetch(blob:). Inspect the rendered image
        # dimensions here; verify exact bytes/MIME from the real POST below.
        previews=await page.evaluate("() => Array.from(document.querySelectorAll('.web-rx-preview-grid img'),img=>({width:img.naturalWidth,height:img.naturalHeight}))")
        assert len(previews)==3 and all(item['width']>0 for item in previews)
        assert all(max(item['width'],item['height']) <= 2000 for item in previews)
        result['source_images']=[{'mime':item['mime'],'bytes':item['bytes']} for item in assets]
        result['preview_dimensions']=previews
        await page.screenshot(path=str(OUT/'compressed-photos-1440.png'))
        await page.locator('#rxPhotoFileInput').set_input_files({'name':'corrupt.png','mimeType':'image/png','buffer':b'not a photo'})
        await expect(page.locator('#webRxWorkspace').get_by_role('alert')).to_contain_text('could not be opened')
        assert await page.locator('.web-rx-preview-grid figure').count()==3
        result['checks']['corrupt_photo_rejected_without_discarding_valid_pages']='PASS'
        await page.locator('#rxPhotoFileInput').set_input_files({'name':'too-large.jpg','mimeType':'image/jpeg','buffer':bytes(10*1024*1024+1)})
        await expect(page.locator('#webRxWorkspace').get_by_role('alert')).to_contain_text('10 MB')
        await expect(page.locator('#btnSendCapturedRx')).to_be_enabled()
        result['checks']['source_10mb_limit_prevents_decode_without_rejecting_compressible_photos']='PASS'
        def inspect_request(request):
            if request.method!='POST' or not request.url.endswith('/prescription'):return
            dto=json.loads(request.post_data)
            content=base64.b64decode(dto['base64_photo'])
            assert 0<len(content)<=409600
            expected=b'\x89PNG' if dto['mime_type']=='image/png' else b'\xff\xd8\xff'
            assert content.startswith(expected)
            result['uploads'].append({'bytes':len(content),'mime':dto['mime_type'],'hash':hashlib.sha256(content).hexdigest(),'idempotency_key':request.headers.get('idempotency-key'),'notes':dto['doctor_notes']})
        page.on('request',inspect_request)
        async def inspect_response(response):
            if response.request.method=='POST' and response.url.endswith('/prescription'):
                dto=await response.json()
                result['backend_responses'].append({'status':response.status,'code':dto.get('error',{}).get('code')})
        page.on('response',inspect_response)
        await page.locator('#docAdviceInput').fill('Synthetic compression regression advice.')
        cdp=await ctx.new_cdp_session(page)
        await cdp.send('Network.enable')
        await cdp.send('Network.emulateNetworkConditions',{'offline':False,'latency':100,'downloadThroughput':1024*1024,'uploadThroughput':64*1024})
        await page.locator('#btnSendCapturedRx').click()
        await page.wait_for_function('''() => {
          const p=document.querySelector('.web-rx-upload-progress progress');
          return p&&p.hasAttribute('value')&&p.value>0&&p.value<100;
        }''')
        for _ in range(3):
            value=await page.locator('.web-rx-upload-progress progress').first.get_attribute('value')
            if value is not None:result['progress_samples'].append(float(value))
            await asyncio.sleep(0.15)
        assert any(0<value<100 for value in result['progress_samples'])
        assert await page.get_by_text('Server confirmed',exact=True).count()==0
        assert await page.locator('.web-rx-preview-grid figure').first.get_by_text('UPLOADED',exact=False).count()==0
        await page.screenshot(animations='disabled',path=str(OUT/'actual-upload-progress-1440.png'))
        for width,height in [(1024,768),(390,844)]:
            await page.set_viewport_size({'width':width,'height':height})
            assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth')
            await page.locator('.web-rx-upload-progress progress').first.scroll_into_view_if_needed()
            await expect(page.locator('.web-rx-upload-progress progress').first).to_be_visible()
            await page.screenshot(animations='disabled',path=str(OUT/f'actual-upload-progress-{width}.png'))
        result['checks']['progress_visible_at_desktop_tablet_narrow_widths_without_overflow']='PASS'
        await page.set_viewport_size({'width':1440,'height':900})
        for caption in await page.locator('.web-rx-preview-grid figcaption').all():
            await expect(caption).to_contain_text('FAILED',timeout=60000)
            # The API's genuine upload rate limit is expected after repeated
            # regression runs. Never reset Redis or bypass its protection.
            await expect(caption).to_contain_text(re.compile('Document storage is not configured|Too many requests'))
        assert len(result['uploads'])==3
        result['compressed_images']=[{'bytes':item['bytes'],'mime':item['mime']} for item in result['uploads']]
        assert [item['mime'] for item in result['compressed_images']]==['image/jpeg','image/png','image/jpeg']
        result['checks']['real_jpeg_png_webp_automatically_compress_to_400kb_hard_cap']='PASS'
        assert await page.get_by_text('Server confirmed',exact=True).count()==0
        assert all(item['status'] in [409,429] for item in result['backend_responses'])
        result['checks']['real_progress_and_actual_backend_failure_never_claim_persistence']='PASS'
        await cdp.send('Network.emulateNetworkConditions',{'offline':False,'latency':0,'downloadThroughput':-1,'uploadThroughput':-1})
        await page.locator('#docAdviceInput').fill('Edited advice must not change retry payload.')
        await page.get_by_role('button',name='Retry page',exact=True).first.click()
        await expect(page.locator('#btnSendCapturedRx')).to_be_enabled()
        assert len(result['uploads'])==4 and result['uploads'][0]==result['uploads'][3]
        result['checks']['retry_preserves_compressed_bytes_key_notes_and_other_pages']='PASS'
        # Cancel a real pending transfer at the existing account/session boundary.
        await cdp.send('Network.emulateNetworkConditions',{'offline':False,'latency':100,'downloadThroughput':1024*1024,'uploadThroughput':16*1024})
        await page.get_by_role('button',name='Retry page',exact=True).first.click()
        await page.wait_for_function('''() => {const p=document.querySelector('.web-rx-upload-progress progress');return p&&p.hasAttribute('value')&&p.value>0&&p.value<100;}''')
        await page.evaluate("() => window.dispatchEvent(new Event('doctor-session-expired'))")
        await page.get_by_label('Email address',exact=True).wait_for()
        assert await page.locator('.web-rx-preview-grid figure').count()==0
        assert await page.get_by_text('Server confirmed',exact=True).count()==0
        result['checks']['account_boundary_aborts_pending_xhr_and_clears_photos']='PASS'
        assert sql(f"SELECT COUNT(*) FROM prescriptions WHERE appointment_id='{uuid.UUID(appointment['id'])}'")=='0'
        result['checks']['failed_or_aborted_transfers_create_no_prescription_record']='PASS'
        assert not result['console_errors']
        result['build_id']=(ROOT/'apps/doctor-web/.next/BUILD_ID').read_text().strip()
        (OUT/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
        await browser.close()
    print(json.dumps({'checks':result['checks'],'compressed_images':result['compressed_images'],'progress_samples':result['progress_samples'],'console_errors':result['console_errors']}))

if __name__=='__main__':asyncio.run(main())

