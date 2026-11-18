"""Summarise one performance-assurance scenario into Markdown tables.

Reads docs/assurance/evidence/<scenario>/{result.json, telemetry.csv,
metrics_after.txt, container_memory.json} written by
Invoke-AssuranceScenario.ps1 and prints the numbers the report quotes, so
every figure in docs/assurance/PERFORMANCE_REPORT.md is regenerable:

    python -I summarize_assurance.py docs/assurance/evidence/ramp [--window START END]

Standard library only.
"""

import csv
import json
import re
import statistics
import sys
from pathlib import Path

# Service levels a load step must meet (docs/assurance/PERFORMANCE_REPORT.md §2).
SLO_P95_MS = 1000.0
SLO_SERVER_ERROR_PCT = 0.1
SLO_CONTROLLED_REJECT_PCT = 1.0
POOL_MAX_PER_ROLE = 10  # DB_POOL_MAX_CONNECTIONS_PER_ROLE in compose.perf-baseline.yml


def slo_failures(s):
    out = []
    if s["p95_ms"] > SLO_P95_MS:
        out.append(f"p95 {s['p95_ms']:.0f} ms")
    if s["server_error_rate_pct"] > SLO_SERVER_ERROR_PCT:
        out.append(f"server errors {s['server_error_rate_pct']:.2f}%")
    if s["controlled_rejection_rate_pct"] > SLO_CONTROLLED_REJECT_PCT:
        out.append(f"429/503 {s['controlled_rejection_rate_pct']:.2f}%")
    return out


def pct(values, p):
    if not values:
        return float("nan")
    v = sorted(values)
    return v[min(len(v) - 1, int(len(v) * p / 100))]


def steps_table(result):
    steps = result.get("steps") or []
    if not steps:
        return
    print("| Step | Offered RPS | Achieved RPS | Successful RPS | p50 ms | p95 ms | p99 ms | 429 | 503 | 5xx | No response | Meets SLO |")
    print("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|")
    last_ok, first_bad = None, None
    for s in steps:
        fails = slo_failures(s)
        if not fails and first_bad is None:
            last_ok = s
        if fails and first_bad is None:
            first_bad = (s, fails)
        print(
            f"| {s['step']} | {s['offered_rps']:.0f} | {s['achieved_rps']:.1f} | {s['success_rps']:.1f} | "
            f"{s['p50_ms']:.0f} | {s['p95_ms']:.0f} | {s['p99_ms']:.0f} | {s['rate_limited_429']} | "
            f"{s['shed_503']} | {s['server_error_5xx']} | {s['transport_no_response']} | "
            f"{'yes' if not fails else 'no: ' + ', '.join(fails)} |"
        )
    print()
    if last_ok:
        print(f"MAX_SUSTAINABLE_OFFERED_RPS={last_ok['offered_rps']:.0f} (successful {last_ok['success_rps']:.1f}/s, p95 {last_ok['p95_ms']:.0f} ms)")
    if first_bad:
        s, fails = first_bad
        print(f"FIRST_FAILING_STEP offered={s['offered_rps']:.0f}: {', '.join(fails)}")
    print()


def overall(result):
    o = result["overall"]
    print("| Requests | Achieved RPS | Successful RPS | p50 ms | p95 ms | p99 ms | max ms | Server-error rate | 429/503 rate | Retry-After on 429/503 |")
    print("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    print(
        f"| {o['requests']} | {o['achieved_rps']:.1f} | {o['success_rps']:.1f} | {o['p50_ms']:.0f} | {o['p95_ms']:.0f} | "
        f"{o['p99_ms']:.0f} | {o['max_ms']:.0f} | {o['server_error_rate_pct']:.3f}% | "
        f"{o['controlled_rejection_rate_pct']:.3f}% | {o['rejections_with_retry_after']}/{o['rejections_total']} |"
    )
    print()
    print("| Endpoint | Requests | Success | Business 4xx | 401/403 | 429 | 503 | 5xx | No response | p50 | p95 | p99 |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for e in sorted(result["endpoints"], key=lambda e: -e["requests"]):
        print(
            f"| {e['endpoint']} | {e['requests']} | {e['success']} | {e['business_reject']} | {e['auth_reject']} | {e['rate_limited_429']} | "
            f"{e['shed_503']} | {e['server_error_5xx']} | {e['transport_no_response']} | {e['p50_ms']:.0f} | "
            f"{e['p95_ms']:.0f} | {e['p99_ms']:.0f} |"
        )
    b = result.get("end_to_end_booking")
    if b:
        print(f"\nEnd-to-end bookings (booked and payment confirmed): {b['completed']} of {b['attempts']}")
    print()


def windowed(result, start, end):
    rows = [b for b in result["time_series_10s"] if start <= b["t_start_s"] < end]
    n = sum(b["requests"] for b in rows)
    if not n:
        return
    w = lambda k: sum(b[k] for b in rows)
    p95s = [b["p95_ms"] for b in rows if b["requests"]]
    print(f"Window {start}-{end}s: {n} requests, {n / (end - start):.1f} RPS, "
          f"server errors {(w('server_error_5xx') + w('transport_no_response')) / n * 100:.3f}%, "
          f"429/503 {(w('rate_limited_429') + w('shed_503')) / n * 100:.3f}%, "
          f"10s-bucket p95 median {statistics.median(p95s):.0f} ms / worst {max(p95s):.0f} ms, "
          f"sessions active {rows[-1]['sessions_active']}")
    print()


def telemetry(path):
    if not path.exists():
        return
    with path.open(encoding="utf-8-sig") as f:
        rows = list(csv.DictReader(f))
    if not rows:
        return
    num = lambda r, k: float(r[k]) if r.get(k) not in (None, "", "NA") else None
    cols = rows[0].keys()
    print("| Container | CPU avg % | CPU p95 % | CPU max % | Memory max MiB |")
    print("|---|---:|---:|---:|---:|")
    for c in [c[len("cpu_pct_"):] for c in cols if c.startswith("cpu_pct_")]:
        cpu = [v for r in rows if (v := num(r, f"cpu_pct_{c}")) is not None]
        mem = [v for r in rows if (v := num(r, f"mem_bytes_{c}")) is not None]
        short = c.replace("hellodoctor-perf-baseline-", "").replace("-1", "")
        print(f"| {short} | {statistics.mean(cpu) if cpu else 0:.1f} | {pct(cpu, 95):.1f} | {max(cpu, default=0):.1f} | {max(mem, default=0) / 2**20:.0f} |")
    print("\nCPU % is of the container's visible CPUs (all four pinned cores).\n")
    print("| DB pool | Max in use | Saturation (max in use / 10) | Samples at 100 % |")
    print("|---|---:|---:|---:|")
    for role in ("api", "auth", "payment", "finance"):
        used = []
        for r in rows:
            size, idle = num(r, f"db_pool_size_{role}"), num(r, f"db_pool_idle_{role}")
            if size is not None and idle is not None:
                used.append(size - idle)
        full = sum(1 for u in used if u >= POOL_MAX_PER_ROLE)
        print(f"| {role} | {max(used, default=0):.0f} | {max(used, default=0) / POOL_MAX_PER_ROLE * 100:.0f}% | {full} of {len(used)} |")
    pga = [v for r in rows if (v := num(r, "pg_active_connections")) is not None]
    pgl = [v for r in rows if (v := num(r, "pg_waiting_locks")) is not None]
    print(f"\nPostgres active connections max {max(pga, default=0):.0f}; ungranted lock waits max {max(pgl, default=0):.0f}\n")


def metrics(path):
    if not path.exists():
        return
    text = path.read_text(encoding="utf-8-sig")
    rej = re.findall(r'^admission_rejected_total\{class="([^"]+)",reason="([^"]+)"\} (\S+)', text, re.M)
    if rej:
        print("| Admission rejections: class | reason | count |")
        print("|---|---|---:|")
        for cls, reason, v in sorted(rej, key=lambda x: -float(x[2])):
            print(f"| {cls} | {reason} | {float(v):.0f} |")
        print()
    # Every connection checkout is recorded (src/telemetry/mod.rs); the exporter
    # publishes a summary, whose quantiles cover the exporter's recent window.
    cnt = re.search(r"^sqlx_pool_acquire_wait_seconds_count (\S+)", text, re.M)
    tot = re.search(r"^sqlx_pool_acquire_wait_seconds_sum (\S+)", text, re.M)
    q = dict(re.findall(r'^sqlx_pool_acquire_wait_seconds\{quantile="([^"]+)"\} (\S+)', text, re.M))
    if cnt and float(cnt.group(1)):
        ms = lambda k: float(q[k]) * 1000 if k in q else float("nan")
        print(f"DB connection checkouts: {float(cnt.group(1)):.0f}; wait mean "
              f"{float(tot.group(1)) / float(cnt.group(1)) * 1000:.2f} ms, p50 {ms('0.5'):.2f} ms, "
              f"p99 {ms('0.99'):.2f} ms, max (recent window) {ms('1'):.2f} ms\n")


def memory(path):
    if not path.exists():
        return
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    data = data if isinstance(data, list) else [data]
    print("| Container | Peak memory MiB | Limit MiB | OOM kills | Restarts |")
    print("|---|---:|---:|---:|---:|")
    for d in data:
        peak = int(d["memory_peak_bytes"]) / 2**20 if str(d.get("memory_peak_bytes", "")).isdigit() else float("nan")
        lim = d.get("memory_max_bytes") or ""
        lim = f"{int(lim) / 2**20:.0f}" if str(lim).isdigit() else str(lim)
        oom = (d.get("oom") or "oom_kill 0").split()[-1]
        print(f"| {d['container'].replace('hellodoctor-perf-baseline-', '').replace('-1', '')} | {peak:.0f} | {lim} | {oom} | {d['restart_count']} |")
    print()


def main():
    ev = Path(sys.argv[1])
    result = json.loads((ev / "result.json").read_text(encoding="utf-8"))
    print(f"## {ev.name}\n")
    print(f"Config: `{json.dumps(result['config'])}`\n")
    overall(result)
    steps_table(result)
    if "--window" in sys.argv:
        i = sys.argv.index("--window")
        windowed(result, int(sys.argv[i + 1]), int(sys.argv[i + 2]))
    telemetry(ev / "telemetry.csv")
    metrics(ev / "metrics_after.txt")
    memory(ev / "container_memory.json")


if __name__ == "__main__":
    main()
