"""Real Block/Unblock, protected cards, conflicts and two-Doctor BFF checks.

Uses installed Edge and existing approved backend/fixture Doctors. Newly created
unbooked fixture slots are removed in finally; no real patient data is changed.
"""
import asyncio
import json
import re
import uuid
from datetime import datetime, timedelta, timezone
from playwright.async_api import async_playwright, expect
from workflow_extra import sql, token
from rx_compression_regression import HERE, ROOT, BASE

OUT=HERE/'schedule'

async def request(ctx,path,method='GET',body=None):
    headers={'Origin':BASE,'Cookie':'; '.join(c['name']+'='+c['value'] for c in await ctx.cookies())}
    response=await ctx.request.fetch(BASE+'/api/proxy/'+path,method=method,data=body,headers=headers)
    return response.status,await response.json()

async def main():
    from live_fixture_guard import require_fixture_opt_in
    require_fixture_opt_in()
    OUT.mkdir(exist_ok=True)
    report={'checks':{},'console_errors':[],'unexpected_network':[]}
    doctors=json.loads((HERE/'workflow/results.json').read_text())['fixture_ids']['doctors']
    made=[]
    async with async_playwright() as pw:
        browser=await pw.chromium.launch(channel='msedge',headless=True)
        contexts=[]
        for doctor in doctors:
            ctx=await browser.new_context(viewport={'width':1440,'height':900})
            await ctx.add_cookies([{'name':name,'value':value,'domain':'127.0.0.1','path':'/api','httpOnly':True,'secure':True,'sameSite':'Strict'} for name,value in [('hd_doctor_access',token(doctor)),('hd_doctor_scope',str(uuid.uuid4()))]])
            contexts.append(ctx)
        owner,foreign=contexts
        doctor=uuid.UUID(doctors[0])
        try:
            start=datetime.now(timezone.utc).replace(second=0,microsecond=0)+timedelta(minutes=80)
            _,all_slots=await request(owner,'doctor/slots?limit=100')
            existing=all_slots['data']
            for attempt in range(20):
                ranges=[(start+timedelta(minutes=i*15),start+timedelta(minutes=i*15+10)) for i in range(5)]
                if all(not any(datetime.fromisoformat(s['start_time'].replace('Z','+00:00'))<end and datetime.fromisoformat(s['end_time'].replace('Z','+00:00'))>begin for s in existing if s['status']!='CANCELLED') for begin,end in ranges):break
                start+=timedelta(minutes=15)
            day=start.astimezone(timezone(timedelta(hours=3))).date().isoformat()
            assert day==datetime.now(timezone(timedelta(hours=3))).date().isoformat(),'Fixture needs room today to verify daily availability count.'
            for begin,end in ranges:
                status,dto=await request(owner,'doctor/slots','POST',{'start_time':begin.isoformat(),'end_time':end.isoformat()})
                assert status==201,(status,dto.get('error',{}).get('code'))
                made.append(str(uuid.UUID(dto['data']['id'])))
            available,blocked,locked,cancelled,conflict=made
            assert (await request(owner,'doctor/slots/'+blocked,'PATCH',{'status':'BLOCKED'}))[0]==200
            assert (await request(owner,'doctor/slots/'+cancelled,'DELETE'))[0]==200
            # Only this newly created, unbooked synthetic row is changed.
            sql(f"UPDATE doctor_schedule_slots SET status='LOCKED_IN_PAYMENT' WHERE id='{uuid.UUID(locked)}' AND doctor_id='{doctor}'")
            page=await owner.new_page()
            page.on('pageerror',lambda e:report['console_errors'].append(str(e)))
            page.on('response',lambda r:report['unexpected_network'].append({'status':r.status,'path':r.url.split(BASE)[-1]}) if r.status>=500 else None)
            await page.goto(BASE,wait_until='networkidle')
            await page.get_by_label('Open consultation schedule',exact=True).click()
            await page.locator('#webScheduleWorkspace').wait_for()
            await page.get_by_label('Date',exact=True).fill(day)
            def card(id):return page.locator(f'[data-slot-id="{id}"]')
            await expect(card(available)).to_be_visible()
            assert '?' not in await card(available).inner_text()
            assert await card(available).locator('.web-slot-icon svg circle').count()==1
            assert await card(available).locator('.dw-doc-avatar-img').count()==0
            await expect(card(available)).to_contain_text('Open for booking')
            report['checks']['available_clock_icon_replaces_question_avatar']='PASS'
            await expect(card(blocked)).to_contain_text('Blocked by you')
            await expect(card(blocked).get_by_role('button',name='Make available',exact=True)).to_be_visible()
            colors=await page.evaluate('''() => ['available','blocked'].map(status=>getComputedStyle(document.querySelector('.web-slot-badge-'+status)).backgroundColor)''')
            assert colors[0]!=colors[1]
            report['checks']['blocked_card_has_neutral_status_and_restore_action']='PASS'
            await expect(card(locked)).to_contain_text('Booking in progress')
            assert await card(locked).get_by_role('button').count()==0
            assert await card(cancelled).get_by_role('button').count()==0
            await expect(card(cancelled)).to_contain_text('Slot cancelled')
            report['checks']['payment_lock_and_cancelled_cards_have_no_destructive_actions']='PASS'
            booked=page.locator('[data-slot-status="BOOKED"]')
            assert await booked.count()>0,'Existing fixture appointments required for booked card checks.'
            assert await booked.first.locator('.dw-doc-avatar-img').count()==1
            assert await booked.get_by_role('button',name=re.compile('Block|Cancel|Make available')).count()==0
            await expect(booked.first.get_by_role('button',name='Open consultation',exact=True)).to_be_visible()
            report['checks']['booked_patient_identity_and_clinical_actions_only']='PASS'
            await page.screenshot(animations='disabled',path=str(OUT/'schedule-all-states-1440.png'))
            async def check_count():
                _,data=await request(owner,'doctor/slots?limit=100')
                shown_date=datetime.fromisoformat(await page.get_by_label('Date',exact=True).input_value()).date()
                count=sum(s['status']=='AVAILABLE' and datetime.fromisoformat(s['start_time'].replace('Z','+00:00')).astimezone(timezone(timedelta(hours=3))).date()==shown_date for s in data['data'])
                await expect(page.get_by_test_id('available-selected-date')).to_have_text(f'{count} available on this date')
                return count
            before=await check_count()
            await card(available).get_by_role('button',name='Block slot',exact=True).click()
            dialog=page.get_by_role('dialog',name='Block this slot?',exact=True)
            await expect(dialog).to_contain_text('until you make it available again')
            await page.screenshot(animations='disabled',path=str(OUT/'block-confirmation-1440.png'))
            async with page.expect_response(lambda r:r.url.endswith('/'+available) and r.request.method=='PATCH') as wait:
                await dialog.get_by_role('button',name='Block slot',exact=True).click()
            response=await wait.value
            assert response.status==200 and json.loads(response.request.post_data)=={'status':'BLOCKED'}
            await expect(card(available)).to_have_attribute('data-slot-status','BLOCKED')
            assert await check_count()==before-1
            report['checks']['real_block_api_and_available_count_excludes_blocked']='PASS'
            await page.locator('.dw-filter-pills').get_by_role('button',name='Blocked',exact=True).click()
            assert await page.locator('[data-slot-status="AVAILABLE"]').count()==0
            assert await page.locator('[data-slot-status="BLOCKED"]').count()==2
            report['checks']['blocked_filter_uses_real_state']='PASS'
            await card(available).get_by_role('button',name='Make available',exact=True).click()
            await page.get_by_role('dialog',name='Make this slot available?',exact=True).get_by_role('button',name='Make available',exact=True).click()
            await expect(card(available)).to_have_count(0)
            await page.locator('.dw-filter-pills').get_by_role('button',name='All',exact=True).click()
            await expect(card(available)).to_have_attribute('data-slot-status','AVAILABLE')
            assert await check_count()==before
            report['checks']['real_unblock_restores_available_and_count']='PASS'
            # Foreign Doctor uses the actual Web BFF; hiding buttons is not proof.
            assert (await request(foreign,'doctor/slots/'+available,'PATCH',{'status':'BLOCKED'}))[0]==409
            assert (await request(foreign,'doctor/slots/'+blocked,'PATCH',{'status':'AVAILABLE'}))[0]==409
            assert (await request(foreign,'doctor/slots/'+blocked,'DELETE'))[0]==409
            _,rows=await request(owner,'doctor/slots?limit=100')
            states={row['id']:row['status'] for row in rows['data']}
            assert states[available]=='AVAILABLE' and states[blocked]=='BLOCKED'
            report['checks']['two_doctor_bff_block_unblock_cancel_denied_without_mutation']='PASS'
            # An actual concurrent status change, not a mocked API response.
            await card(conflict).get_by_role('button',name='Block slot',exact=True).click()
            assert (await request(owner,'doctor/slots/'+conflict,'PATCH',{'status':'BLOCKED'}))[0]==200
            await page.get_by_role('dialog',name='Block this slot?',exact=True).get_by_role('button',name='Block slot',exact=True).click()
            await expect(page.locator('#webScheduleWorkspace').get_by_role('alert')).to_contain_text('no longer available to block')
            await expect(card(conflict)).to_have_attribute('data-slot-status','BLOCKED')
            report['checks']['real_conflict_closes_stale_confirmation_and_refreshes']='PASS'
            await card(blocked).get_by_role('button',name='Cancel slot',exact=True).click()
            dialog=page.get_by_role('dialog',name='Cancel this slot?',exact=True)
            await expect(dialog).to_contain_text('removes the slot from your active schedule')
            await page.screenshot(animations='disabled',path=str(OUT/'cancel-confirmation-1440.png'))
            await dialog.get_by_role('button',name='Confirm cancellation',exact=True).click()
            await expect(card(blocked)).to_have_attribute('data-slot-status','CANCELLED')
            assert (await request(owner,'doctor/slots/'+blocked,'PATCH',{'status':'AVAILABLE'}))[0]==409
            report['checks']['block_and_cancel_distinct_with_no_cancelled_reopening']='PASS'
            await expect(card(available)).to_contain_text(re.compile(r'\d{1,2} [A-Za-z]{3} 2026'))
            await page.get_by_label('Date',exact=True).fill('')
            await expect(page.get_by_label('Date',exact=True)).to_have_value(day)
            report['checks']['unambiguous_eat_dates_and_empty_input_safety']='PASS'
            for width,height in [(1920,1080),(1440,900),(1024,768),(390,844)]:
                await page.set_viewport_size({'width':width,'height':height})
                assert await page.evaluate('document.documentElement.scrollWidth<=innerWidth')
                await page.screenshot(animations='disabled',path=str(OUT/f'schedule-states-{width}.png'))
            report['checks']['four_responsive_widths_without_overflow']='PASS'
            assert not report['console_errors'] and not report['unexpected_network']
            report['build_id']=(ROOT/'apps/doctor-web/.next/BUILD_ID').read_text().strip()
            (OUT/'results.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
        finally:
            if made:
                ids=','.join("'"+str(uuid.UUID(id))+"'" for id in made)
                sql(f"DELETE FROM doctor_schedule_slots WHERE doctor_id='{doctor}' AND id IN ({ids}) AND NOT EXISTS(SELECT 1 FROM appointments WHERE slot_id=doctor_schedule_slots.id)")
            await browser.close()
    print(json.dumps(report))

if __name__=='__main__':asyncio.run(main())
