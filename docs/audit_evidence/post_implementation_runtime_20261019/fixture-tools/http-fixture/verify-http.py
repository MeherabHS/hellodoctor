"""Dedicated synthetic local fixture supervisor; never use with shared services."""
import argparse
import base64
import ctypes
from ctypes import wintypes as w
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import threading
import time

K = ctypes.WinDLL("kernel32", use_last_error=True)
P = ctypes.WinDLL("psapi", use_last_error=True)
SIZE = ctypes.c_size_t


class BASIC(ctypes.Structure):
    _fields_ = [("process_time", ctypes.c_longlong), ("job_time", ctypes.c_longlong),
                ("flags", w.DWORD), ("min_ws", SIZE), ("max_ws", SIZE),
                ("processes", w.DWORD), ("affinity", SIZE),
                ("priority", w.DWORD), ("scheduling", w.DWORD)]


class IO(ctypes.Structure):
    _fields_ = [(name, ctypes.c_ulonglong) for name in
                ("read_ops", "write_ops", "other_ops", "read_bytes", "write_bytes", "other_bytes")]


class LIMIT(ctypes.Structure):
    _fields_ = [("basic", BASIC), ("io", IO), ("process_memory", SIZE),
                ("job_memory", SIZE), ("peak_process_memory", SIZE), ("peak_job_memory", SIZE)]


class CPU(ctypes.Structure):
    _fields_ = [("flags", w.DWORD), ("rate", w.DWORD)]


class STARTUP(ctypes.Structure):
    _fields_ = [("cb", w.DWORD), ("reserved", w.LPWSTR), ("desktop", w.LPWSTR),
                ("title", w.LPWSTR), ("x", w.DWORD), ("y", w.DWORD),
                ("xsize", w.DWORD), ("ysize", w.DWORD), ("xcount", w.DWORD),
                ("ycount", w.DWORD), ("fill", w.DWORD), ("flags", w.DWORD),
                ("show", w.WORD), ("reserved2count", w.WORD), ("reserved2", ctypes.c_void_p),
                ("stdin", w.HANDLE), ("stdout", w.HANDLE), ("stderr", w.HANDLE)]


class PROCESS(ctypes.Structure):
    _fields_ = [("process", w.HANDLE), ("thread", w.HANDLE), ("pid", w.DWORD), ("tid", w.DWORD)]


class MEMORY(ctypes.Structure):
    _fields_ = [("cb", w.DWORD), ("faults", w.DWORD)] + [(n, SIZE) for n in
               ("peak_ws", "ws", "peak_paged", "paged", "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile", "private")]


K.CreateJobObjectW.restype = w.HANDLE
K.CreateJobObjectW.argtypes = [ctypes.c_void_p, w.LPCWSTR]
K.CreateFileW.restype = w.HANDLE
K.CreateFileW.argtypes = [w.LPCWSTR, w.DWORD, w.DWORD, ctypes.c_void_p, w.DWORD, w.DWORD, w.HANDLE]
K.SetInformationJobObject.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD]
K.AssignProcessToJobObject.argtypes = [w.HANDLE, w.HANDLE]
K.CreateProcessW.argtypes = [w.LPCWSTR, w.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, w.BOOL,
                            w.DWORD, ctypes.c_void_p, w.LPCWSTR, ctypes.POINTER(STARTUP), ctypes.POINTER(PROCESS)]
K.ResumeThread.argtypes = [w.HANDLE]
K.SetHandleInformation.argtypes = [w.HANDLE, w.DWORD, w.DWORD]
K.GetExitCodeProcess.argtypes = [w.HANDLE, ctypes.POINTER(w.DWORD)]
K.GetProcessTimes.argtypes = [w.HANDLE] + [ctypes.POINTER(w.FILETIME)] * 4
K.TerminateJobObject.argtypes = [w.HANDLE, w.UINT]
K.TerminateProcess.argtypes = [w.HANDLE, w.UINT]
K.CloseHandle.argtypes = [w.HANDLE]
K.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
P.GetProcessMemoryInfo.argtypes = [w.HANDLE, ctypes.POINTER(MEMORY), w.DWORD]


def require(ok, operation):
    if not ok:
        raise OSError(ctypes.get_last_error(), operation)


class Backend:
    def __init__(self, binary, env, cwd):
        self.job = K.CreateJobObjectW(None, None)
        require(self.job, "CreateJobObject")
        limits = LIMIT()
        limits.basic.flags = 0x2000 | 0x8 | 0x200
        limits.basic.processes = 32
        limits.job_memory = 512 * 1024 * 1024
        require(K.SetInformationJobObject(self.job, 9, ctypes.byref(limits), ctypes.sizeof(limits)), "Job memory/process limits")
        cpu = CPU(1 | 4, 2000)
        require(K.SetInformationJobObject(self.job, 15, ctypes.byref(cpu), ctypes.sizeof(cpu)), "Job CPU hard cap")
        self.log = K.CreateFileW(str(cwd / "backend.log"), 0x40000000, 1, None, 2, 0x80, None)
        self.stdin = K.CreateFileW("NUL", 0x80000000, 1, None, 3, 0x80, None)
        for handle in (self.log, self.stdin):
            require(handle not in (None, ctypes.c_void_p(-1).value), "Create fixture log handle")
            require(K.SetHandleInformation(handle, 1, 1), "Set fixture handle inheritance")
        startup = STARTUP()
        startup.cb = ctypes.sizeof(startup)
        startup.flags = 0x100 | 1
        startup.show = 0
        startup.stdin, startup.stdout, startup.stderr = self.stdin, self.log, self.log
        self.pi = PROCESS()
        block = ctypes.create_unicode_buffer("\0".join(f"{k}={v}" for k, v in sorted(env.items())) + "\0\0")
        require(K.CreateProcessW(str(binary), ctypes.create_unicode_buffer('"' + str(binary) + '"'),
                                None, None, True, 0x4 | 0x400 | 0x08000000, block,
                                str(cwd), ctypes.byref(startup), ctypes.byref(self.pi)), "Create suspended backend")
        if not K.AssignProcessToJobObject(self.job, self.pi.process):
            K.TerminateProcess(self.pi.process, 1)
            raise OSError(ctypes.get_last_error(), "Assign backend job")
        require(K.ResumeThread(self.pi.thread) != 0xFFFFFFFF, "Resume bounded backend")
        K.CloseHandle(self.pi.thread)

    def alive(self):
        code = w.DWORD()
        require(K.GetExitCodeProcess(self.pi.process, ctypes.byref(code)), "Get backend status")
        return code.value == 259

    def usage(self):
        memory = MEMORY()
        memory.cb = ctypes.sizeof(memory)
        require(P.GetProcessMemoryInfo(self.pi.process, ctypes.byref(memory), memory.cb), "Backend memory observation")
        a, b, kernel, user = w.FILETIME(), w.FILETIME(), w.FILETIME(), w.FILETIME()
        require(K.GetProcessTimes(self.pi.process, ctypes.byref(a), ctypes.byref(b), ctypes.byref(kernel), ctypes.byref(user)), "Backend CPU observation")
        ticks = ((kernel.dwHighDateTime << 32) + kernel.dwLowDateTime + (user.dwHighDateTime << 32) + user.dwLowDateTime)
        return {"rss_bytes": memory.ws, "private_bytes": memory.private, "cpu_seconds": ticks / 10_000_000}

    def close(self):
        K.TerminateJobObject(self.job, 0)
        K.WaitForSingleObject(self.pi.process, 5000)
        for handle in (self.pi.process, self.job, self.log, self.stdin):
            K.CloseHandle(handle)


def request(path, method="GET", body=None, token=None, ip="203.0.113.10"):
    headers = {"X-Forwarded-For": ip, "X-Forwarded-Proto": "https"}
    if token:
        headers["Authorization"] = "Bearer " + token
    encoded = None
    if body is not None:
        encoded = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    connection = http.client.HTTPConnection("127.0.0.1", 18080, timeout=2)
    start = time.monotonic()
    try:
        connection.request(method, path, body=encoded, headers=headers)
        response = connection.getresponse()
        content = response.read(1024 * 1024)
        row = {"path": path, "status": response.status, "latency_ms": (time.monotonic() - start) * 1000,
               "retry_after": response.getheader("Retry-After"), "request_id": response.getheader("x-request-id")}
        return row, content
    except (OSError, http.client.HTTPException):
        return {"path": path, "status": None, "latency_ms": (time.monotonic() - start) * 1000, "transport_error": True}, b""
    finally:
        connection.close()


def metrics_snapshot():
    metrics_snapshot.sequence += 1
    n = metrics_snapshot.sequence
    row, data = request("/metrics", token="synthetic-fixture-monitor-only", ip=f"198.19.{(n >> 8) & 255}.{n & 255}")
    result = {}
    if row["status"] == 200:
        for line in data.decode("utf-8", "replace").splitlines():
            if line.startswith(("admission_", "db_pool_", "db_acquire_", "argon2_", "sanitize", "upload_sanit")) and not line.startswith("#"):
                match = re.match(r"(.+) ([0-9.eE+\-]+)$", line)
                if match:
                    result[match[1]] = float(match[2])
    return row["status"], result


metrics_snapshot.sequence = 0


def docker_read(args):
    result = subprocess.run(["docker", *args], capture_output=True, text=True, timeout=10)
    if result.returncode:
        raise RuntimeError("Owned fixture observer command failed")
    return result.stdout.strip()


def database_snapshot():
    statement = "SELECT json_build_object('users',(SELECT count(*) FROM users),'sessions',(SELECT count(*) FROM auth_sessions),'bound_families',(SELECT count(*) FROM auth_session_device_keys),'revoked_session_rows',(SELECT count(*) FROM auth_sessions WHERE revoked_at IS NOT NULL),'active_family_leaves',(SELECT count(*) FROM auth_sessions WHERE revoked_at IS NULL AND rotated_at IS NULL AND expires_at>now()),'transactions',(SELECT count(*) FROM transactions),'economic_effects',(SELECT count(*) FROM financial_economic_effects),'payment_events',(SELECT count(*) FROM payment_events),'system_errors',(SELECT count(*) FROM system_error_logs),'db_stats',(SELECT row_to_json(s) FROM (SELECT numbackends,xact_commit,xact_rollback,blks_read,blks_hit,tup_returned,tup_fetched FROM pg_stat_database WHERE datname=current_database())s))"
    return json.loads(docker_read(["exec", "hd-acceptance-pg-20261019", "psql", "-X", "-At", "-U", "postgres", "-d", "hd_http_fixture", "-c", statement]))


def redis_snapshot():
    data = docker_read(["exec", "hd-acceptance-redis-20261019", "redis-cli", "--raw", "INFO", "stats"])
    return {k: int(v) for line in data.splitlines() if ":" in line for k, v in [line.split(":", 1)]
            if k in {"total_commands_processed", "rejected_connections", "evicted_keys", "total_error_replies"}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    args = parser.parse_args()
    root, binary = args.repo.resolve(), args.binary.resolve()
    scratch = Path(__file__).resolve().parent
    evidence = root / "docs/audit_evidence/post_implementation_runtime_20261019"
    run_id = time.strftime("http-%Y%m%dT%H%M%S", time.gmtime())
    output = {"run_id": run_id, "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "scope": "Dedicated development-mode loopback fixture; not formal network-isolated certification or provider/device acceptance",
              "configured_job_limits": {"memory_bytes": 512 * 1024 * 1024, "processes": 32, "cpu_host_percent": 20, "wall_seconds": 300},
              "stages": [], "blockers": []}
    for name in ("hd-acceptance-pg-20261019", "hd-acceptance-redis-20261019"):
        inspection = json.loads(docker_read(["inspect", name]))[0]
        if inspection["Config"]["Labels"].get("hellodoctor.acceptance.run") != "runtime-20261019":
            raise RuntimeError("Disposable fixture ownership mismatch")
    source = (root / "services/backend/tests/api_tests.rs").read_text(encoding="utf-8")
    secret = lambda name: re.search(r"const " + name + r': &str\s*=\s*"([^"]+)"', source).group(1)
    env = {"SystemRoot": os.environ["SystemRoot"], "WINDIR": os.environ["SystemRoot"],
           "PATH": os.environ["SystemRoot"] + r"\System32;C:\Users\CUBE\llvm-mingw-20260616-ucrt-x86_64\bin",
           "HOME": str(scratch), "USERPROFILE": str(scratch), "TEMP": str(scratch), "TMP": str(scratch),
           "APPDATA": str(scratch), "LOCALAPPDATA": str(scratch),
           "ENVIRONMENT": "development", "SEED_DEV_DATA": "false", "DEV_AUTH_OTP_ENABLED": "true", "DEV_SIMULATION_ENABLED": "false",
           "DATABASE_URL": "postgres://postgres@127.0.0.1:15432/hd_http_fixture", "APP_DB_HOST": "127.0.0.1", "APP_DB_PORT": "15432", "APP_DB_NAME": "hd_http_fixture",
           "REDIS_URL": "redis://127.0.0.1:16379/0", "HOST": "127.0.0.1", "PORT": "18080",
           "JWT_ED25519_PRIVATE_KEY_B64": secret("TEST_JWT_PRIVATE_KEY_B64"), "JWT_ED25519_PUBLIC_KEY_B64": secret("TEST_JWT_PUBLIC_KEY_B64"),
           "TOTP_ENCRYPTION_KEY_B64": base64.b64encode(bytes([17]) * 32).decode(),
           "AGORA_APP_ID": "1" * 32, "AGORA_APP_CERTIFICATE": "2" * 32, "WEBHOOK_HMAC_SECRET": "synthetic-local-fixture-only",
           "TRUSTED_PROXY_IPS": "127.0.0.1", "METRICS_SCRAPE_TOKEN": "synthetic-fixture-monitor-only",
           "PUBLIC_API_BASE_URL": "http://127.0.0.1:18080", "RUST_LOG": "warn", "ALLOWED_ORIGINS": "http://127.0.0.1:18080"}
    for index, name in enumerate(("DB_API_ROLE_PASSWORD", "DB_AUTH_WORKER_PASSWORD", "DB_PAYMENT_WORKER_PASSWORD", "DB_SYSTEM_WORKER_PASSWORD", "DB_FINANCE_WORKER_PASSWORD"), 1):
        env[name] = f"{index:02x}" * 24
    (scratch / ".env").write_text("# Empty disposable fixture; prevents ancestor dotenv credential loading.\n", encoding="ascii")
    output["child_environment_names"] = sorted(env)
    output["provider_configuration"] = "Azure/email/payment/storage endpoints absent; no provider network calls requested"
    backend = Backend(binary, env, scratch)
    output["backend_pid"] = backend.pi.pid
    start = time.monotonic()
    try:
        for _ in range(60):
            if not backend.alive():
                raise RuntimeError("Fresh backend exited during startup; inspect scratch backend.log")
            readiness, _ = request("/healthz")
            if readiness["status"] == 200:
                break
            time.sleep(0.5)
        else:
            raise RuntimeError("Fresh backend readiness deadline exceeded")
        legal, legal_body = request("/api/v1/legal/documents")
        denied, _ = request("/api/v1/patients/me")
        output["baseline"] = {"health": readiness, "legal": legal, "unauthenticated_patient": denied}
        if legal["status"] != 200 or denied["status"] != 401:
            raise RuntimeError("Baseline positive API or authentication denial failed")
        proof_command = [r"C:\Program Files\nodejs\node.exe", str(scratch / "device-proof-http.cjs")]
        proof_run = subprocess.run(proof_command, cwd=scratch, capture_output=True, text=True, timeout=60)
        try:
            proof_result = json.loads(proof_run.stdout)
        except ValueError:
            proof_result = {"verdict": "INCONCLUSIVE", "error": "Synthetic proof driver did not return JSON"}
        proof_result["command"] = proof_command
        proof_result["exit_code"] = proof_run.returncode
        proof_result["db_after"] = database_snapshot()
        output["device_proof_http"] = proof_result
        (evidence / f"{run_id}-device-proof.json").write_text(json.dumps(proof_result, indent=2), encoding="utf-8")
        if proof_run.returncode:
            output["blockers"].append("Synthetic bound-session HTTP case failed; preserved without application changes")
        token = None
        try:
            documents = json.loads(legal_body)["data"]
            consent = next(doc for doc in documents if doc["document_type"] == "PATIENT_DATA_SHARING_CONSENT")
            otp_row, otp_body = request("/api/v1/auth/register-otp", "POST", {"phone_number": "+254799000099"})
            if otp_row["status"] != 200:
                raise RuntimeError("Synthetic development OTP request failed")
            otp = json.loads(otp_body)["data"]["debug_otp_code"]
            login_row, login_body = request("/api/v1/auth/verify-otp-and-login", "POST", {"phone_number": "+254799000099", "otp_code": otp, "accepted_documents": [{"type": "PATIENT_DATA_SHARING_CONSENT", "version": consent["version"]}]})
            if login_row["status"] != 200:
                raise RuntimeError("Synthetic development OTP verification failed")
            token = json.loads(login_body)["data"]["access_token"]
            authenticated, _ = request("/api/v1/patients/me", token=token)
            output["baseline"]["authenticated_patient"] = authenticated
            if authenticated["status"] != 200:
                token = None
        except (KeyError, StopIteration, ValueError, RuntimeError) as error:
            output["blockers"].append(str(error))
        output["normal_probe_scope"] = "Authenticated synthetic Patient profile" if token else "Public legal documents; authenticated availability NOT_MEASURED"
        producer = root / "services/backend/scripts/resilience/attack-local.py"
        manifest = {"synthetic_fixture": True, "base_url": "http://127.0.0.1:18080", "cases": {
            realm: {"path": f"/api/v1/auth/{realm}/login", "body": {"email": "synthetic-shared@example.invalid", "password": "synthetic-invalid-password"}, "headers": {"X-Forwarded-Proto": "https"}}
            for realm in ("patient", "doctor", "admin")}}
        manifest_path = scratch / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        stages = [("patient", 100, 4, "same"), ("patient", 1000, 8, "different"), ("patient", 30000, 32, "different"), ("doctor", 100, 4, "same"), ("admin", 100, 4, "same")]
        for stage_index, (realm, count, workers, accounts) in enumerate(stages, 1):
            if time.monotonic() - start > 260:
                output["blockers"].append("Overall bounded fixture wall deadline prevents remaining stages")
                break
            stage = {"realm": realm, "count": count, "workers": workers, "accounts": accounts, "metrics_before": metrics_snapshot(), "db_before": database_snapshot(), "redis_before": redis_snapshot(), "probes": [], "samples": []}
            attack_file = evidence / f"{run_id}-stage-{stage_index}-attack.json"
            command = [sys.executable, "-I", "-S", str(producer), "--manifest", str(manifest_path), "--case", realm,
                       "--count", str(count), "--workers", str(workers), "--accounts", accounts, "--keys", "none", "--output", str(attack_file)]
            stage["command"] = command
            pending = subprocess.Popen(command, cwd=scratch, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stage_start = time.monotonic()
            next_probe, next_metrics = stage_start, stage_start
            last_usage = backend.usage()
            last_time = stage_start
            failure = None
            while pending.poll() is None:
                now = time.monotonic()
                usage = backend.usage()
                delta = now - last_time
                usage["cpu_percent_one_core"] = max(0, (usage["cpu_seconds"] - last_usage["cpu_seconds"]) * 100 / delta) if delta > 0 else 0
                usage["elapsed_seconds"] = now - stage_start
                if now >= next_metrics:
                    usage["metrics_status"], usage["metrics"] = metrics_snapshot()
                    next_metrics = now + 0.25
                stage["samples"].append(usage)
                last_usage, last_time = usage, now
                if now >= next_probe:
                    normal, _ = request("/api/v1/patients/me" if token else "/api/v1/legal/documents", token=token)
                    health, _ = request("/healthz")
                    normal["kind"], health["kind"] = "ordinary", "health"
                    normal["elapsed_seconds"] = health["elapsed_seconds"] = now - stage_start
                    stage["probes"].extend((normal, health))
                    next_probe = now + 1
                    if any(row["status"] and row["status"] >= 500 and row["status"] != 503 for row in (normal, health)):
                        failure = "Unexpected 5xx in independent probe"
                if usage["rss_bytes"] > 480 * 1024 * 1024 or time.monotonic() - start > 290:
                    failure = "Fixture memory or wall guard reached"
                if failure:
                    pending.kill()
                    break
                time.sleep(0.05)
            stdout, stderr = pending.communicate(timeout=5)
            stage["producer_exit_code"] = pending.returncode
            stage["wall_seconds"] = time.monotonic() - stage_start
            stage["metrics_after"] = metrics_snapshot()
            stage["db_after"] = database_snapshot()
            stage["redis_after"] = redis_snapshot()
            stage["resource_summary"] = {"rss_peak_bytes": max(row["rss_bytes"] for row in stage["samples"]), "private_peak_bytes": max(row["private_bytes"] for row in stage["samples"]), "cpu_one_core_percent_peak": max(row["cpu_percent_one_core"] for row in stage["samples"])}
            if stderr:
                stage["producer_error"] = stderr.decode("utf-8", "replace")[:2000]
            if attack_file.exists():
                attack = json.loads(attack_file.read_text(encoding="utf-8"))
                stage["attacker_observations"] = {key: value for key, value in attack.items() if key != "requests"}
                unexpected = sum(value for key, value in attack["http_statuses"].items() if key.isdigit() and int(key) >= 500 and int(key) != 503)
                if unexpected:
                    failure = "Unexpected attacker-observed 5xx; remaining stages stopped"
            output["stages"].append(stage)
            (evidence / f"{run_id}-observer.json").write_text(json.dumps(output, indent=2), encoding="utf-8")
            if failure or pending.returncode:
                output["blockers"].append(failure or "Producer command failed")
                break
            time.sleep(1.1)
        output["elapsed_seconds"] = time.monotonic() - start
        output["not_measured"] = ["consultation authorized traffic", "upload sanitizer flood", "signed provider callbacks", "DB waiter count", "provider calls", "production edge", "physical Android"]
    finally:
        backend.close()
        output["backend_stopped"] = True
        (evidence / f"{run_id}-observer.json").write_text(json.dumps(output, indent=2), encoding="utf-8")
    print(json.dumps({"run_id": run_id, "stages_executed": len(output["stages"]), "blockers": output["blockers"], "backend_stopped": True}))


if __name__ == "__main__":
    main()
