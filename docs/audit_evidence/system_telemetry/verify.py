"""Development-only verification using installed Edge and existing services.

No tokens, login credentials, patient records, or raw server errors are saved.
The simulated incident is explicitly synthetic and is resolved after the check.
"""
import asyncio
import base64
import hashlib
import hmac
import json
import re
import struct
import subprocess
import time
import uuid
from pathlib import Path

from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).parent
ADMIN = "http://127.0.0.1:3000"
REPORT = {"checks": {}, "browser_exceptions": 0, "unexpected_5xx": 0}


def sql(statement):
    names = subprocess.check_output(
        ["docker", "exec", "hellodoctor-postgres", "printenv", "POSTGRES_USER", "POSTGRES_DB"],
        text=True,
    ).splitlines()
    return subprocess.check_output(
        ["docker", "exec", "hellodoctor-postgres", "psql", "-U", names[0], "-d", names[1], "-At", "-c", statement],
        text=True,
    ).strip()


def seed_constant(name):
    source = (ROOT / "services/backend/src/seed.rs").read_text(encoding="utf-8")
    return re.search(rf'{name}: &str = "([^"]+)"', source)[1]


def totp():
    key = base64.b32decode(seed_constant("SEED_ADMIN_TOTP_SECRET_BASE32"))
    digest = hmac.new(key, struct.pack(">Q", int(time.time()) // 30), hashlib.sha1).digest()
    offset = digest[-1] & 15
    return str((struct.unpack(">I", digest[offset:offset + 4])[0] & 0x7fffffff) % 1000000).zfill(6)


async def main():
    assert re.search(r"(?m)^ENVIRONMENT=development\s*$", (ROOT / "services/backend/.env").read_text())
    check = REPORT["checks"]
    marker = "SYNTHETIC_TELEMETRY_CHECK_" + uuid.uuid4().hex[:12]
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel="msedge", headless=True)
        context = await browser.new_context(viewport={"width": 1440, "height": 1000})
        page = await context.new_page()
        page.on("pageerror", lambda _: REPORT.update(browser_exceptions=REPORT["browser_exceptions"] + 1))
        page.on("response", lambda r: REPORT.update(unexpected_5xx=REPORT["unexpected_5xx"] + 1) if r.status >= 500 else None)
        await page.goto(ADMIN + "/login", wait_until="networkidle")
        await page.get_by_label("Admin email").fill(seed_constant("SEED_ADMIN_EMAIL"))
        await page.get_by_label("Password", exact=True).fill(seed_constant("SEED_ADMIN_PASSWORD"))
        await page.get_by_label("Authenticator code").fill(totp())
        await page.get_by_role("button", name="Sign in", exact=True).click()
        await page.get_by_text("App Error & Incident Logs", exact=True).wait_for(timeout=30000)
        check["admin_login_mfa"] = "PASS"
        await page.get_by_text("App Error & Incident Logs", exact=True).click()
        await page.get_by_role("heading", name="App Incident & Failure Diagnostics").wait_for()
        search = page.get_by_placeholder("Filter by User, Trace ID, Subsystem, Error Code...")
        await search.fill(marker)

        async def api(path, body=None):
            res = await context.request.fetch(ADMIN + "/api/proxy/" + path,
                method="POST" if body is not None else "GET", data=body,
                headers={"Origin": ADMIN})
            assert res.ok, f"Admin API status {res.status}"
            return (await res.json())["data"]

        initial = await api("admin/system-errors?page_size=200")
        check["admin_system_error_list_live"] = "PASS"
        REPORT["initial_incident_total"] = initial["total"]
        # Avoid deduping into or changing any pre-existing incident.
        presets = [("sms_otp", "AUTH_OTP_GATEWAY", "SmsOtpDispatcher"),
                   ("kyc_hub_down", "PROFESSIONAL_KYC_HUB", "CredentialRegistryClient"),
                   ("offline_sync", "SYNC_OFFLINE_CACHE", "MutationSyncResolver")]
        chosen = next((p for p in presets if sql(
            f"SELECT count(*) FROM system_error_logs WHERE status='OPEN' AND component='{p[2]}' AND last_seen_at > now()-interval '60 minutes'"
        ) == "0"), None)
        incident = None
        try:
            if chosen:
                incident = await api("admin/system-errors/simulate", {
                    "scenario": chosen[0], "severity": "WARNING", "custom_note": marker,
                })
                assert marker in incident["error_message"] and incident["trace_id"].startswith("SIM-")
                identifier = str(uuid.UUID(incident["id"]))
                assert sql(f"SELECT count(*) FROM system_error_logs WHERE id='{identifier}'") == "1"
                check["synthetic_ingestion_database_persistence"] = "PASS"
                await page.get_by_role("button", name=re.compile("Refresh Feed")).click()
                row = page.locator("tr").filter(has_text=incident["trace_id"])
                await row.wait_for(timeout=30000)
                check["admin_feed_renders_persisted_incident"] = "PASS"
                await page.screenshot(path=str(OUT / "admin-synthetic-incident.png"))
                await row.get_by_role("button").first.click()
                await page.get_by_text("SOURCE CODE SNIPPET / STACK TRACE", exact=False).wait_for()
                check["admin_incident_detail"] = "PASS"
                await page.get_by_role("button", name=re.compile("Acknowledge / Investigate")).click()
                assert sql(f"SELECT status FROM system_error_logs WHERE id='{identifier}'") == "ACKNOWLEDGED"
                check["admin_acknowledge_persists"] = "PASS"
                await page.get_by_role("button", name=re.compile("Mark as Resolved|Resolve Incident")).click()
                assert sql(f"SELECT status FROM system_error_logs WHERE id='{identifier}'") == "RESOLVED"
                check["admin_resolve_persists"] = "PASS"
                await page.reload(wait_until="networkidle")
                await page.get_by_text("App Error & Incident Logs", exact=True).click()
                await page.get_by_placeholder("Filter by User, Trace ID, Subsystem, Error Code...").fill(marker)
                await page.locator("tr").filter(has_text=incident["trace_id"]).wait_for(timeout=30000)
                check["admin_incident_survives_reload"] = "PASS"
            else:
                check["synthetic_ingestion_database_persistence"] = "NOT_RUN_NO_UNUSED_PRESET"

            # A Web diagnostic has only a local Next log sink; prove it does not create an Admin incident.
            before = sql("SELECT count(*) FROM system_error_logs WHERE subsystem='DOCTOR_WEB'")
            web = await context.request.post("http://127.0.0.1:3001/api/diagnostics",
                headers={"Origin": "http://127.0.0.1:3001"}, data={"category": "browser-error"})
            assert web.ok
            after = sql("SELECT count(*) FROM system_error_logs WHERE subsystem='DOCTOR_WEB'")
            check["doctor_web_diagnostic_local_endpoint"] = "PASS"
            check["doctor_web_diagnostic_admin_ingestion"] = "MISSING" if before == after else "INVESTIGATE"
            metrics = await context.request.get("http://127.0.0.1:8080/metrics")
            text = await metrics.text()
            assert metrics.ok and "http_requests_total" in text and "http_request_duration_seconds" in text
            check["backend_request_metrics_live"] = "PASS"
        finally:
            if incident and sql(f"SELECT status FROM system_error_logs WHERE id='{uuid.UUID(incident['id'])}'") != "RESOLVED":
                await api(f"admin/system-errors/{incident['id']}/resolve", {
                    "resolution_note": "Synthetic verification complete; no real outage or clinical record involved."
                })
            await browser.close()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as error:
        # Exception text may contain a DOM excerpt; persist only the type.
        REPORT["verification_failure"] = type(error).__name__
        print("Verification failed: " + type(error).__name__)
    finally:
        (OUT / "results.json").write_text(json.dumps(REPORT, indent=2), encoding="utf-8")
