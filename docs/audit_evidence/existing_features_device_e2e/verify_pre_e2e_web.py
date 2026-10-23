"""Targeted real backend/BFF/Edge checks. No payment, RTC, or mock responses."""
import asyncio
import json
from pathlib import Path
from playwright.async_api import async_playwright, expect
from backend_probe import seed

OUT = Path(__file__).parent
WEB = 'http://127.0.0.1:3001'

async def main():
    report = {'checks': {}, 'page_errors': [], 'physical_device': False}
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel='msedge', headless=True)
        context = await browser.new_context(viewport={'width':1440,'height':1000})
        page = await context.new_page()
        page.on('pageerror',lambda e: report['page_errors'].append(str(e)))
        await page.goto(WEB,wait_until='networkidle')
        await page.get_by_label('Email address',exact=True).fill(seed('SEED_DOCTOR1_EMAIL'))
        await page.get_by_label('Password',exact=True).fill(seed('SEED_DOCTOR1_PASSWORD'))
        async with page.expect_response('**/api/auth/login') as pending:
            await page.get_by_role('button',name='Doctor Secure Sign In').click()
        login = await pending.value
        assert login.status == 200, ('login',login.status)
        payload = await login.json()
        await page.get_by_label('Verification code',exact=True).fill(payload['data']['development_otp'])
        await page.get_by_role('button',name='Verify & sign in').click()
        await page.locator('#doctorWebShell').wait_for()
        async def own(path):
            # Use the actual browser transport, including its secure-cookie
            # localhost rules, rather than Playwright's separate HTTP jar.
            response = await page.evaluate('''async path => { const r=await fetch('/api/proxy/'+path,{credentials:'same-origin',cache:'no-store'}); return {status:r.status,body:await r.json()}; }''',path)
            assert response['status'] == 200, (path,response['status'])
            return response['body']['data']
        appointments = await own('doctor/appointments?limit=100')
        upcoming = await own('doctor/appointments?view=upcoming&limit=100')
        assert [a['id'] for a in upcoming] == [a['id'] for a in appointments if a['queue_actionable']]
        assert all(a['status'] not in ['COMPLETED','CANCELLED','EXPIRED','REFUNDED'] for a in upcoming)
        report['checks']['bff_queue_matches_backend_eligibility_and_order'] = True
        await page.locator('[data-page="appointments"]').first.click()
        await expect(page.get_by_role('button',name='Upcoming',exact=True)).to_have_class('dw-filter-pill active')
        await expect(page.locator('#dwApptCardsGrid [data-appointment-id]')).to_have_count(len(upcoming))
        assert await page.locator('#dwApptCardsGrid [data-appointment-id]').evaluate_all('(nodes)=>nodes.map(n=>n.dataset.appointmentId)') == [a['id'] for a in upcoming]
        await page.screenshot(path=str(OUT/'correction_doctor_web_queue.png'),full_page=True)
        report['checks']['desktop_default_is_upcoming'] = True
        await page.get_by_role('button',name='Completed',exact=True).click()
        await page.screenshot(path=str(OUT/'correction_doctor_web_completed.png'),full_page=True)
        report['checks']['completed_filter_remains_available'] = True
        booking=json.loads((OUT/'chat_booking.json').read_text())
        await page.locator('[data-page="chat"]').first.click()
        await page.locator('[data-conversation-id="'+booking['conversation_id']+'"]').click()
        await page.locator('#dwChatInputField').wait_for()
        await expect(page.locator('#dwChatMsgStream')).to_contain_text('Hello, I received your test message.')
        await expect(page.locator('#dwChatMsgStream')).to_contain_text('Hello Doctor, this is the E2E test.')
        own_bubble=page.locator('[data-sender-side="doctor"] .dw-chat-bubble-content').filter(has_text='Hello, I received your test message.')
        patient_bubble=page.locator('[data-sender-side="patient"] .dw-chat-bubble-content').filter(has_text='Hello Doctor, this is the E2E test.')
        assert await own_bubble.evaluate('(n)=>getComputedStyle(n).color') == 'rgb(255, 255, 255)'
        assert await patient_bubble.evaluate('(n)=>getComputedStyle(n).backgroundColor') == 'rgb(255, 255, 255)'
        assert await own_bubble.evaluate('(n)=>getComputedStyle(n.parentElement).flexDirection') == 'row-reverse'
        await expect(page.get_by_role('button',name='Attach clinical image',exact=True)).to_be_visible()
        await expect(page.get_by_role('button',name='Take after meals',exact=True)).to_have_count(0)
        await expect(page.get_by_role('button',name='Check fever',exact=True)).to_have_count(0)
        await page.screenshot(path=str(OUT/'correction_doctor_web_chat.png'),full_page=True)
        report['checks']['real_shared_history_ownership_colors'] = True
        report['checks']['paperclip_present_canned_medical_advice_removed'] = True
        await page.reload(wait_until='networkidle')
        await page.locator('[data-conversation-id="'+booking['conversation_id']+'"]').click()
        await expect(page.locator('[data-sender-side="doctor"]').filter(has_text='Hello, I received your test message.')).to_have_count(1)
        report['checks']['reload_preserves_sender_identity'] = True
        assert not report['page_errors'], report['page_errors']
        await browser.close()
    (OUT/'correction_live_web.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2))

if __name__ == '__main__':
    asyncio.run(main())
