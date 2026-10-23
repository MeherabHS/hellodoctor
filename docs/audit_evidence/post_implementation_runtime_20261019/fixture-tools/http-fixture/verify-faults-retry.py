"""Owner-authorized disposable dependency-fault checks; original source unchanged."""
import concurrent.futures
import http.client
import importlib.util
import json
import os
from pathlib import Path
import re
import base64
import subprocess
import sys
import time

scratch = Path(__file__).resolve().parent
root = scratch.parents[2]
spec = importlib.util.spec_from_file_location("fixture", scratch / "verify-http.py")
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
evidence = root / "docs/audit_evidence/post_implementation_runtime_20261019"
run_id = time.strftime("faults-%Y%m%dT%H%M%S", time.gmtime())
out = {"run_id": run_id, "scope": "Disposable loopback development fixture, real backend; no provider calls", "cases": []}

def sql(statement):
    return m.docker_read(["exec", "hd-acceptance-pg-20261019", "psql", "-X", "-At", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", "hd_fault_retry_fixture", "-c", statement])

def snapshot():
    return json.loads(sql("SELECT json_build_object('system_errors',(SELECT count(*) FROM system_error_logs),'error_occurrences',(SELECT COALESCE(sum(occurrence_count),0) FROM system_error_logs),'transactions',(SELECT count(*) FROM transactions),'economic_effects',(SELECT count(*) FROM financial_economic_effects),'blocked_auth_connections',(SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND usename='hellodoctor_auth_worker' AND wait_event_type='Lock'))"))

def long_request(path, token, ip):
    connection = http.client.HTTPConnection("127.0.0.1", 18080, timeout=13)
    start = time.monotonic()
    try:
        connection.request("GET", path, headers={"Authorization": "Bearer " + token, "X-Forwarded-Proto": "https", "X-Forwarded-For": ip})
        response = connection.getresponse()
        response.read(65536)
        return {"status": response.status, "latency_ms": (time.monotonic()-start)*1000, "retry_after": response.getheader("Retry-After"), "request_id": response.getheader("x-request-id")}
    except (OSError, http.client.HTTPException) as error:
        return {"status": None, "error_class": type(error).__name__, "latency_ms": (time.monotonic()-start)*1000}
    finally:
        connection.close()

backend = None
paused = False
try:
    for name in ("hd-acceptance-pg-20261019", "hd-acceptance-redis-20261019"):
        record = json.loads(m.docker_read(["inspect", name]))[0]
        if record["Config"]["Labels"].get("hellodoctor.acceptance.run") != "runtime-20261019":
            raise RuntimeError("Owned fixture label mismatch")
    # New database only. The parent has verified that this name does not exist.
    m.docker_read(["exec", "hd-acceptance-pg-20261019", "createdb", "-U", "postgres", "hd_fault_retry_fixture"])
    setup = (scratch / "verify-http.py").read_text(encoding="utf-8")
    setup = setup[setup.index("    source = (root"):setup.index('    (scratch / ".env")')]
    context = {"root": root, "scratch": scratch, "re": re, "os": os, "base64": base64}
    exec(compile("\n".join(line[4:] for line in setup.splitlines()), "fixture-env-setup", "exec"), context)
    env = context["env"]
    env["DATABASE_URL"] = "postgres://postgres@127.0.0.1:15432/hd_fault_retry_fixture"
    env["APP_DB_NAME"] = "hd_fault_retry_fixture"
    env["REDIS_URL"] = "redis://127.0.0.1:16379/3"
    backend = m.Backend(root / "scratch/acceptance-runtime-20261019/cargo-target/debug/hellodoctor-backend.exe", env, scratch)
    for _ in range(60):
        row, _ = m.request("/healthz")
        if row["status"] == 200:
            break
        if not backend.alive():
            raise RuntimeError("Backend exited during fresh fixture startup")
        time.sleep(.5)
    else:
        raise RuntimeError("Fixture readiness deadline")
    _, legal = m.request("/api/v1/legal/documents")
    consent = next(x for x in json.loads(legal)["data"] if x["document_type"] == "PATIENT_DATA_SHARING_CONSENT")
    row, data = m.request("/api/v1/auth/register-otp", "POST", {"phone_number": "+254799000098"})
    if row["status"] != 200:
        raise RuntimeError("Synthetic OTP creation failed")
    otp = json.loads(data)["data"]["debug_otp_code"]
    row, data = m.request("/api/v1/auth/verify-otp-and-login", "POST", {"phone_number": "+254799000098", "otp_code": otp, "accepted_documents": [{"type": "PATIENT_DATA_SHARING_CONSENT", "version": consent["version"]}]})
    if row["status"] != 200:
        raise RuntimeError("Synthetic sign-in failed")
    token = json.loads(data)["data"]["access_token"]
    baseline, _ = m.request("/api/v1/patients/me", token=token)
    if baseline["status"] != 200:
        raise RuntimeError("Authenticated baseline failed")
    out["baseline"] = baseline
    before = snapshot()
    m.docker_read(["pause", "hd-acceptance-redis-20261019"])
    paused = True
    health, _ = m.request("/healthz")
    start = time.monotonic()
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        responses = list(pool.map(lambda i: m.request("/api/v1/patients/me", token=token, ip=f"198.20.0.{i}")[0], range(100)))
    after = snapshot()
    out["cases"].append({"case_id": "redis_dependency_loss", "requests": responses, "health": health, "before": before, "after": after, "elapsed_seconds": time.monotonic()-start, "metrics": m.metrics_snapshot(), "all_fail_closed": all(x["status"] in (429,503) for x in responses), "incident_row_delta": after["system_errors"]-before["system_errors"]})
    m.docker_read(["unpause", "hd-acceptance-redis-20261019"])
    paused = False
    for _ in range(20):
        recovered, _ = m.request("/healthz")
        if recovered["status"] == 200:
            break
        time.sleep(.5)
    out["redis_recovery"] = recovered
    time.sleep(1.1)
    # A bounded fixture-owned table lock creates real auth pool pressure.
    lock_command = ["docker", "exec", "hd-acceptance-pg-20261019", "psql", "-X", "-At", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", "hd_fault_retry_fixture", "-c", "SET statement_timeout='15s'; BEGIN; LOCK TABLE auth_sessions IN ACCESS EXCLUSIVE MODE; SELECT pg_sleep(12); ROLLBACK;"]
    locker = subprocess.Popen(lock_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    time.sleep(.6)
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
        pending = [pool.submit(long_request, "/api/v1/patients/me" if i < 8 else "/api/v1/doctor/chats", token, f"198.21.0.{i}") for i in range(12)]
        samples = []
        for _ in range(8):
            samples.append({"db": snapshot(), "metrics": m.metrics_snapshot(), "process": backend.usage()})
            time.sleep(.2)
        pressure, _ = m.request("/api/v1/patients/me", token=token, ip="198.21.1.1")
        health, _ = m.request("/healthz", ip="198.21.1.2")
        results = [future.result(timeout=15) for future in pending]
    locker.communicate(timeout=5)
    out["cases"].append({"case_id": "auth_pool_pressure", "lock_seconds": 12, "offered_parallel_requests": 12, "lock_process_exit": locker.returncode, "samples": samples, "new_request_during_pressure": pressure, "health_during_pressure": health, "requests": results, "db_waiter_count": "NOT_MEASURED", "sql_lock_waits_observed": any(x["db"]["blocked_auth_connections"] > 0 for x in samples)})
    time.sleep(1.1)
    out["after_recovery"] = m.request("/api/v1/patients/me", token=token, ip="198.21.2.1")[0]
    out["final_db"] = snapshot()
    out["final_metrics"] = m.metrics_snapshot()
    out["credentials_logged"] = False
except Exception as error:
    out["failure"] = {"type": type(error).__name__, "message": str(error)[:300]}
finally:
    if paused:
        m.docker_read(["unpause", "hd-acceptance-redis-20261019"])
    if backend:
        backend.close()
    out["backend_stopped"] = True
    (evidence / (run_id + "-observer.json")).write_text(json.dumps(out, indent=2), encoding="utf-8")
    print(json.dumps({"run_id": run_id, "cases": len(out["cases"]), "failure": out.get("failure"), "backend_stopped": True}))
sys.exit(1 if out.get("failure") else 0)
