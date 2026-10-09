"""Read-only probes; prints only status codes and safe infrastructure facts."""
import asyncio
import json
import urllib.error
import urllib.request
from verify import OUT, REPORT, seed_constant, sql, totp
from playwright.async_api import async_playwright


async def main():
    report = {"checks": {}, "scope": "read-only; no incidents injected"}
    try:
        response = urllib.request.urlopen("http://127.0.0.1:3000/login")
        body = response.read().decode()
        report["admin_login_http_status"] = response.status
    except urllib.error.HTTPError as error:
        body = error.read().decode()
        report["admin_login_http_status"] = error.code
    report["admin_page_error_markers"] = [value for value in [
        "Cannot find module", "ENOENT", "MODULE_NOT_FOUND", "webpack-runtime", "middleware-manifest",
        "Internal Server Error", "Failed to load chunk", "SyntaxError",
    ] if value in body]
    report["database"] = json.loads(sql("SELECT json_build_object('incidents', (SELECT count(*) FROM system_error_logs), 'telemetry_aggregates', (SELECT count(*) FROM consultation_telemetry), 'telemetry_events', (SELECT count(*) FROM consultation_telemetry_events))"))
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(channel="msedge", headless=True)
        context = await browser.new_context()
        login = await context.request.post("http://127.0.0.1:8080/api/v1/auth/admin/login", data={
            "email": seed_constant("SEED_ADMIN_EMAIL"),
            "password": seed_constant("SEED_ADMIN_PASSWORD"), "totp_code": totp(),
        })
        report["checks"]["backend_admin_login_mfa"] = "PASS" if login.ok else f"HTTP_{login.status}"
        if login.ok:
            token = (await login.json())["data"]["access_token"]
            for name, path in [("system_error_feed", "admin/system-errors?page_size=1"),
                               ("command_telemetry", "admin/command/telemetry")]:
                res = await context.request.get("http://127.0.0.1:8080/api/v1/" + path,
                    headers={"Authorization": "Bearer " + token})
                report["checks"][name] = "PASS" if res.ok else f"HTTP_{res.status}"
                if name == "system_error_feed" and res.ok:
                    data = (await res.json())["data"]
                    report["error_feed_totals"] = {key: data[key] for key in ["total", "open_count", "open_critical_count"]}
            # Log out the test login rather than leave an extra live session.
            await context.request.post("http://127.0.0.1:8080/api/v1/auth/logout", headers={"Authorization": "Bearer " + token})
        before = sql("SELECT count(*) FROM system_error_logs")
        web = await context.request.post("http://127.0.0.1:3001/api/diagnostics",
            headers={"Origin": "http://127.0.0.1:3001"}, data={"category": "browser-error"})
        report["checks"]["doctor_web_diagnostic_local_endpoint"] = "PASS" if web.ok else f"HTTP_{web.status}"
        after = sql("SELECT count(*) FROM system_error_logs")
        report["checks"]["doctor_web_diagnostic_reached_admin_database"] = "NO_ROW_CREATED" if before == after else "INVESTIGATE_CONCURRENT_WRITE"
        metrics = await context.request.get("http://127.0.0.1:8080/metrics")
        metrics_text = await metrics.text()
        report["checks"]["backend_prometheus_request_metrics"] = "PASS" if metrics.ok and "http_requests_total" in metrics_text and "http_request_duration_seconds" in metrics_text else "FAIL"
        await browser.close()
    (OUT / "live-probes.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
