"""Bounded synthetic HTTP producer. No defense verdicts are produced here.

Run ONLY inside an isolated fixture namespace with a disposable HelloDoctor
stack. A manifest must explicitly identify synthetic data and a loopback origin.
No dependencies are installed. Tokens/bodies/authorization are never recorded.
"""
import argparse
import concurrent.futures
import errno
import http.client
import ipaddress
import json
import math
import ssl
import time
import uuid
from collections import Counter
from pathlib import Path
from urllib.parse import urlsplit


def percentile(values, fraction):
    return sorted(values)[min(len(values) - 1, math.ceil(len(values) * fraction) - 1)] if values else None


def transport_failure(error, phase):
    """Keep only bounded structural diagnostics, never credential-bearing text.

    These are client observations, not server fault attribution. In particular,
    a reset or timeout alone cannot establish SERVER_RESOURCE_ERROR.
    """
    code = getattr(error, "errno", None)
    winerror = getattr(error, "winerror", None)
    codes = {code, winerror}
    if isinstance(error, ssl.SSLError):
        category = "TLS_ERROR_IF_APPLICABLE"
    elif isinstance(error, (http.client.RemoteDisconnected, http.client.IncompleteRead, EOFError)):
        # RemoteDisconnected also subclasses ConnectionResetError; retain the
        # stronger HTTP parser observation before the generic OS classification.
        category = "EOF_BEFORE_RESPONSE" if phase == "response_headers" else "EOF_DURING_RESPONSE"
    elif isinstance(error, TimeoutError) or codes.intersection({errno.ETIMEDOUT, 10060}):
        category = "CONNECT_TIMEOUT" if phase == "connect" else "READ_TIMEOUT" if phase.startswith("response_") else "WRITE_TIMEOUT"
    elif isinstance(error, ConnectionRefusedError) or codes.intersection({errno.ECONNREFUSED, 10061}):
        category = "CONNECTION_REFUSED"
    elif isinstance(error, ConnectionResetError) or codes.intersection({errno.ECONNRESET, 10054}):
        category = "CONNECTION_RESET"
    elif isinstance(error, BrokenPipeError) or errno.EPIPE in codes:
        category = "BROKEN_PIPE"
    elif codes.intersection({errno.EMFILE, errno.ENFILE, errno.ENOBUFS, errno.ENOMEM,
                             errno.EADDRNOTAVAIL, errno.EADDRINUSE, 10024, 10048, 10049, 10055}):
        category = "CLIENT_RESOURCE_ERROR"
    else:
        category = "UNKNOWN_TRANSPORT_ERROR"
    return {"transport_error": True, "transport_error_type": category,
            "transport_phase": phase, "exception_class": type(error).__name__[:64],
            "os_errno": code if isinstance(code, int) else None,
            "os_winerror": winerror if isinstance(winerror, int) else None}


def bounded_results(send, count, workers):
    """Submit at most one outstanding future per worker, including completions."""
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
        indices = iter(range(count))
        pending = {executor.submit(send, index) for index in range(min(workers, count))}
        # Consume the exact initial batch once; do not enqueue the remaining load.
        for _ in range(min(workers, count)):
            next(indices)
        while pending:
            done, pending = concurrent.futures.wait(pending, return_when=concurrent.futures.FIRST_COMPLETED)
            for future in done:
                yield future.result()
                index = next(indices, None)
                if index is not None:
                    pending.add(executor.submit(send, index))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--count", type=int, default=100)
    parser.add_argument("--workers", type=int, default=16)
    parser.add_argument("--keys", choices=["same", "different", "none"], default="different")
    parser.add_argument("--accounts", choices=["same", "different"], default="same")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not 1 <= args.count <= 30000 or not 1 <= args.workers <= 32:
        parser.error("Use at most 30,000 requests and 32 workers.")
    if args.manifest.stat().st_size > 128 * 1024:
        parser.error("Synthetic manifest is too large.")
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    target = urlsplit(manifest.get("base_url", ""))
    try:
        loopback = ipaddress.ip_address(target.hostname).is_loopback
    except (ValueError, TypeError):
        loopback = False
    if (manifest.get("synthetic_fixture") is not True or not loopback or target.scheme != "http"
            or target.username or target.password or target.query or target.fragment or target.path not in ("", "/")):
        parser.error("Only explicit synthetic HTTP fixtures on literal loopback addresses are allowed.")
    case = manifest["cases"][args.case]
    path = case["path"]
    if not path.startswith("/api/v1/") or "?" in path or "#" in path or "\r" in path or "\n" in path:
        parser.error("Use a fixed API path without URL credentials or query strings.")
    body = json.dumps(case.get("body", {}), separators=(",", ":")).encode()
    if args.accounts == "different" and (not path.endswith("/login") or "email" not in case.get("body", {})):
        parser.error("Distinct synthetic account targets are supported only for email login cases.")
    if len(body) > 128 * 1024:
        parser.error("Use small synthetic bodies, at most 128 KiB.")
    common_headers = {"Content-Type": "application/json", **case.get("headers", {})}
    same_key = str(uuid.uuid4())

    def send(index):
        request_body = body if args.accounts == "same" else json.dumps({
            **case["body"], "email": f"synthetic-{index}@example.invalid"
        }, separators=(",", ":")).encode()
        logical_ip = str(ipaddress.ip_address(int(ipaddress.ip_address("198.18.0.0")) + index))
        # This is an isolated trusted-ingress simulation. Production never trusts
        # this header from arbitrary peers. No real IP interfaces are provisioned.
        headers = {**common_headers, "X-Forwarded-For": logical_ip}
        if args.keys != "none":
            headers["Idempotency-Key"] = same_key if args.keys == "same" else str(uuid.uuid4())
        connection = http.client.HTTPConnection(target.hostname, target.port or 80, timeout=2)
        request_started = time.monotonic()
        row = {"index": index, "logical_ip": logical_ip, "status": None, "request_id": None, "trace_id": None,
               "started_ms": round((request_started - started) * 1000, 3)}
        phase = "connect"
        try:
            # Explicit connect distinguishes connect timeout from a read/write
            # timeout; the same per-socket two-second bound remains in force.
            connection.connect()
            phase = "request_write"
            connection.request(case.get("method", "POST"), path, body=request_body, headers=headers)
            phase = "response_headers"
            response = connection.getresponse()
            row.update(status=response.status,
                       request_id=(response.getheader("x-request-id") or "")[:128],
                       trace_id=(response.getheader("traceparent") or "")[:128])
            phase = "response_body"
            response.read(64 * 1024)  # Response size and connection lifetime bounded.
        except (OSError, http.client.HTTPException) as error:
            row.update(transport_failure(error, phase))
        finally:
            connection.close()
        row["latency_ms"] = round((time.monotonic() - request_started) * 1000, 3)
        return row

    started = time.monotonic()
    # Do not put 30,000 Futures into an executor's unbounded work queue.
    rows = list(bounded_results(send, args.count, args.workers))
    elapsed = time.monotonic() - started
    latencies = [row["latency_ms"] for row in rows]
    statuses = Counter(str(row["status"]) for row in rows)
    http_rows = [row for row in rows if row["status"] is not None]
    completed_http_rows = [row for row in http_rows if not row.get("transport_error")]
    transport_errors = [row for row in rows if row.get("transport_error")]
    http_latencies = [row["latency_ms"] for row in completed_http_rows]
    rejected = sum(statuses[str(code)] for code in (429, 503))
    result = {"case_id": args.case, "request_count": len(rows), "logical_identities": args.count,
              "elapsed_seconds": elapsed, "worker_concurrency": args.workers,
              "workload_model": "bounded_closed_loop", "offered_rps": None,
              "http_statuses": dict(statuses),
              "completed_attempts_per_second": len(rows) / elapsed,
              "http_responses_per_second": len(http_rows) / elapsed,
              "completed_http_responses_per_second": len(completed_http_rows) / elapsed,
              "http_response_count": len(http_rows), "completed_http_response_count": len(completed_http_rows),
              "transport_error_count": len(transport_errors),
              "transport_error_types": dict(Counter(row["transport_error_type"] for row in transport_errors)),
              "transport_error_phases": dict(Counter(row["transport_phase"] for row in transport_errors)),
              "http_p50_ms": percentile(http_latencies, .50),
              "http_p95_ms": percentile(http_latencies, .95), "http_p99_ms": percentile(http_latencies, .99),
              # Kept for existing consumers; explicitly identified as attempts.
              "responses_per_second_semantics": "legacy_completed_attempts_including_transport_errors",
              "responses_per_second": len(rows) / elapsed, "rejections_per_second": rejected / elapsed,
              "p50_ms": percentile(latencies, .50), "p95_ms": percentile(latencies, .95), "p99_ms": percentile(latencies, .99),
              "requests": rows, "verdict": "OBSERVER_REQUIRED"}
    with args.output.open("x", encoding="utf-8") as output:
        json.dump(result, output, indent=2)


if __name__ == "__main__":
    main()
