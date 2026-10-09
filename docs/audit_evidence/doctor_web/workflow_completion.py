"""Recheck the previously incomplete synthetic encounter on the current binary."""
import asyncio,json,uuid
from pathlib import Path
from playwright.async_api import async_playwright
from workflow_extra import token,OUT,WEB

async def main():
    fixtures=json.loads((OUT/'results.json').read_text())['fixture_ids']
    appointment=fixtures['appointments'][0];doctor=fixtures['doctors'][0]
    result={'fixture':'Existing synthetic Doctor/appointment; development claims, not MFA evidence','checks':{},'console_errors':[]}
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        context=await browser.new_context(viewport={'width':1440,'height':1000})
        await context.add_cookies([{'name':'hd_doctor_access','value':token(doctor),'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'},{'name':'hd_doctor_scope','value':str(uuid.uuid4()),'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'}])
        async def api(path,body=None,key=None):
            headers={'Origin':WEB,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await context.cookies())}
            if key:headers['Idempotency-Key']=key
            response=await context.request.fetch(WEB+'/api/proxy/'+path,method='POST' if body is not None else 'GET',data=body,headers=headers)
            payload=await response.json();assert response.status==200,(path,response.status,payload.get('error',{}).get('code'))
            return payload['data']
        page=await context.new_page();page.on('pageerror',lambda e:result['console_errors'].append(str(e)))
        await page.goto(WEB,wait_until='networkidle')
        await page.locator(f'[data-appointment-id="{appointment}"] .queue-patient-name .web-name-button').click()
        await page.get_by_role('button',name='Complete consultation',exact=True).click()
        await page.get_by_label('Clinical outcome').select_option('COMPLETED_NO_RX')
        reason='Synthetic workflow verification; no clinical treatment provided.'
        await page.get_by_label('Reason for no prescription').fill(reason)
        await page.get_by_role('button',name='Confirm consultation completion').click()
        await page.get_by_text('Consultation completion confirmed by the server').wait_for()
        rows=await api('doctor/appointments');completed=next(row for row in rows if row['id']==appointment)
        assert completed['status']=='COMPLETED' and completed['clinical_outcome']=='COMPLETED_NO_RX'
        result['checks']['ui_success_matches_authoritative_completion']='PASS'
        await page.screenshot(path=str(OUT/'completion_current_build_1440.png'))
        before=await api('doctor/wallet/summary');key=str(uuid.uuid4())
        await api(f'consultations/{appointment}/complete-no-rx',{'reason':reason},key)
        await api(f'consultations/{appointment}/complete-no-rx',{'reason':reason},key)
        assert before==await api('doctor/wallet/summary')
        result['checks']['same_reason_retry_no_double_credit']='PASS'
        await page.keyboard.press('Escape');await page.locator('.bn-item').nth(4).click()
        await page.get_by_role('button',name='Statement',exact=False).click();await page.get_by_label('Statement month').wait_for()
        await page.screenshot(path=str(OUT/'statement_current_build_1440.png'))
        assert await page.get_by_role('dialog').count()==1
        result['checks']['statement_canonical_dialog_current_build']='PASS'
        await page.keyboard.press('Escape');await page.locator('.bn-item').nth(3).click()
        await page.get_by_role('button',name='Add Slot',exact=False).click();await page.get_by_label('Slot date').wait_for()
        await page.screenshot(path=str(OUT/'slot_generator_current_build_1440.png'))
        await page.keyboard.press('Escape');assert await page.get_by_role('dialog').count()==0
        result['checks']['slot_generator_keyboard_dismissal']='PASS'
        await browser.close()
    (OUT/'completion-current-results.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result))

if __name__=='__main__':asyncio.run(main())
