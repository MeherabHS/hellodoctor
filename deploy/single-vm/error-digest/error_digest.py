#!/usr/bin/env python3
"""Nightly error digest for the HelloDoctor VM.

Collects everything that went wrong in the last 24 hours and emails one
report (by default at 03:00 Africa/Nairobi via the systemd timer next to this
file). Sources:

  * Backend and app errors   - `system_error_logs` (server faults and the
                               crash reports the apps send to /client-errors)
  * Email delivery           - `email_outbox` rows that FAILED or EXPIRED
  * Push/in-app delivery     - `notifications` rows that FAILED
  * Background workers       - erasure, expiry-cleanup and document-removal
                               jobs that are retrying with an error
  * Containers               - stopped/unhealthy/restarting services, and
                               ERROR/panic lines in each service's logs
  * Database                 - reachable, connections, long-running queries
  * VM                       - disk, memory, load, recent reboot

Safety:
  * Read-only. Database queries run with default_transaction_read_only=on
    through `docker exec ... psql`, so no database password is stored here.
  * No patient data. Only counts, components and error text are reported,
    never user ids or recipients; emails, phone numbers, tokens and long
    numbers are masked before anything leaves the VM (CLAUDE.md: no PHI in
    logs or notifications).
  * The SMTP password is read from a file (DIGEST_SMTP_PASSWORD_FILE), never
    from the command line.

Usage:
  error_digest.py            collect and send
  error_digest.py --dry-run  collect and print the report, send nothing

Python 3.10+, standard library only.
"""
from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import os
import re
import shutil
import smtplib
import socket
import ssl
import subprocess
import sys
from dataclasses import dataclass, field
from email.message import EmailMessage

WINDOW_HOURS = int(os.environ.get("DIGEST_WINDOW_HOURS", "24"))
PG_CONTAINER = os.environ.get("DIGEST_PG_CONTAINER", "hellodoctor-vm-postgres-1")
PG_USER = os.environ.get("DIGEST_PG_USER", "hellodoctor")
PG_DB = os.environ.get("DIGEST_PG_DB", "hellodoctor")
COMPOSE_PROJECT = os.environ.get("DIGEST_COMPOSE_PROJECT", "hellodoctor-vm")
DISK_WARN_PERCENT = int(os.environ.get("DIGEST_DISK_WARN_PERCENT", "80"))
MAX_LOG_SAMPLES = 3
MAX_TEXT = 220

# ---------------------------------------------------------------- masking --

_MASKS = [
    (re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"), "[email]"),
    (re.compile(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"), "[token]"),
    (re.compile(r"(?i)\b(bearer|token|password|secret|key)\b([\"'=: ]+)\S+"), r"\1\2[hidden]"),
    (re.compile(r"\+?254[\s-]?\d{3}[\s-]?\d{3}[\s-]?\d{3}\b"), "[phone]"),
    (re.compile(r"\b0[17]\d{2}[\s-]?\d{3}[\s-]?\d{3}\b"), "[phone]"),
    (re.compile(r"\b\d{9,}\b"), "[number]"),
]


def mask(text: str, limit: int = MAX_TEXT) -> str:
    """Hide personal data and secrets, collapse whitespace, cap length."""
    out = " ".join((text or "").split())
    for pattern, replacement in _MASKS:
        out = pattern.sub(replacement, out)
    return out if len(out) <= limit else out[: limit - 1] + "…"


# ----------------------------------------------------------------- report --


@dataclass
class Section:
    title: str
    status: str = "ok"  # ok | warn | error | unavailable
    lines: list[str] = field(default_factory=list)

    def flag(self, status: str, line: str) -> None:
        order = ["ok", "unavailable", "warn", "error"]
        if order.index(status) > order.index(self.status):
            self.status = status
        self.lines.append(line)


def run(cmd: list[str], timeout: int = 60, env: dict | None = None) -> tuple[int, str]:
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, env=env
        )
        return proc.returncode, (proc.stdout or "") + (proc.stderr if proc.returncode else "")
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 1, str(exc)


def psql(sql: str) -> list[list[str]] | None:
    """Rows of a read-only query, or None if the database is unreachable."""
    code, out = run(
        [
            "docker", "exec", "-e", "PGOPTIONS=-c default_transaction_read_only=on",
            PG_CONTAINER, "psql", "-X", "-U", PG_USER, "-d", PG_DB,
            "-At", "-F", "\t", "-v", "ON_ERROR_STOP=1", "-c", sql,
        ]
    )
    if code != 0:
        return None
    return [line.split("\t") for line in out.splitlines() if line.strip()]


SINCE = f"now() - interval '{WINDOW_HOURS} hours'"

# ---------------------------------------------------------------- sources --


def app_errors() -> Section:
    s = Section("Backend and app errors")
    rows = psql(
        "SELECT subsystem, component, severity, count(*), "
        "max(logged_at)::timestamp(0), "
        "(array_agg(error_message ORDER BY logged_at DESC))[1] "
        f"FROM system_error_logs WHERE logged_at > {SINCE} "
        "GROUP BY 1,2,3 ORDER BY 4 DESC LIMIT 25"
    )
    if rows is None:
        s.flag("unavailable", "Could not read system_error_logs (database unreachable).")
        return s
    total = sum(int(r[3]) for r in rows)
    if not rows:
        s.lines.append("No errors recorded.")
    for sub, comp, sev, count, last, message in rows:
        status = "error" if sev.upper() in ("CRITICAL", "ERROR", "FATAL") else "warn"
        s.flag(status, f"{count}× [{sev}] {sub}/{comp} (last {last} UTC): {mask(message)}")
    if total:
        s.lines.insert(0, f"{total} error(s) in the last {WINDOW_HOURS} h.")
    return s


def email_delivery() -> Section:
    s = Section("Email delivery")
    rows = psql(
        "SELECT email_type, delivery_status, count(*), "
        "(array_agg(last_delivery_error ORDER BY created_at DESC))[1] "
        f"FROM email_outbox WHERE created_at > {SINCE} "
        "AND delivery_status IN ('FAILED','EXPIRED') GROUP BY 1,2 ORDER BY 3 DESC"
    )
    if rows is None:
        s.flag("unavailable", "Could not read email_outbox.")
        return s
    stuck = psql(
        "SELECT count(*) FROM email_outbox WHERE delivery_status = 'CREATED' "
        "AND created_at < now() - interval '30 minutes'"
    )
    if not rows:
        s.lines.append("No failed emails.")
    for etype, status, count, reason in rows:
        s.flag("error", f"{count}× {etype} {status}" + (f": {mask(reason)}" if reason else ""))
    if stuck and int(stuck[0][0]):
        s.flag("warn", f"{stuck[0][0]} email(s) queued for over 30 minutes and not sent.")
    return s


def notification_delivery() -> Section:
    s = Section("Push and in-app notifications")
    rows = psql(
        "SELECT type, count(*), (array_agg(last_delivery_error ORDER BY created_at DESC))[1] "
        f"FROM notifications WHERE created_at > {SINCE} AND delivery_status = 'FAILED' "
        "GROUP BY 1 ORDER BY 2 DESC LIMIT 15"
    )
    if rows is None:
        s.flag("unavailable", "Could not read notifications.")
        return s
    if not rows:
        s.lines.append("No failed notifications.")
    for ntype, count, reason in rows:
        s.flag("warn", f"{count}× {ntype} failed" + (f": {mask(reason)}" if reason else ""))
    return s


def worker_jobs() -> Section:
    s = Section("Background workers")
    checks = [
        ("Account erasure", "account_erasure_jobs",
         "last_error_code IS NOT NULL AND updated_at > " + SINCE, "last_error_code"),
        ("Booking expiry cleanup", "appointment_expiry_cleanup_jobs",
         "completed_at IS NULL AND last_error IS NOT NULL", "last_error"),
        ("Intake document removal", "intake_document_removal_jobs",
         "completed_at IS NULL AND last_error IS NOT NULL", "last_error"),
        ("Erasure journal sync", "durable_erasure_journal_outbox",
         "journal_synced_at IS NULL AND last_sync_error IS NOT NULL", "last_sync_error"),
    ]
    reachable = False
    for label, table, where, err_col in checks:
        rows = psql(
            f"SELECT count(*), (array_agg({err_col} ORDER BY created_at DESC))[1] "
            f"FROM {table} WHERE {where}"
        )
        if rows is None:
            continue
        reachable = True
        count, last = rows[0][0], rows[0][1] if len(rows[0]) > 1 else ""
        if int(count):
            s.flag("error", f"{label}: {count} job(s) failing" + (f": {mask(last)}" if last else ""))
    if not reachable:
        s.flag("unavailable", "Could not read the worker job tables.")
    elif not s.lines:
        s.lines.append("All background jobs healthy.")
    return s


def database() -> Section:
    s = Section("Database")
    rows = psql(
        "SELECT (SELECT count(*) FROM pg_stat_activity), "
        "current_setting('max_connections'), "
        "pg_size_pretty(pg_database_size(current_database())), "
        "(SELECT count(*) FROM pg_stat_activity WHERE state = 'active' "
        " AND now() - query_start > interval '5 minutes')"
    )
    if rows is None:
        s.flag("error", f"Database not reachable in container {PG_CONTAINER}.")
        return s
    conns, max_conns, size, long_running = rows[0]
    s.lines.append(f"Reachable. {conns}/{max_conns} connections, size {size}.")
    if int(conns) > 0.8 * int(max_conns):
        s.flag("warn", "Connections above 80% of the limit.")
    if int(long_running):
        s.flag("warn", f"{long_running} query(ies) running for more than 5 minutes.")
    return s


def containers() -> Section:
    s = Section("Containers")
    if not shutil.which("docker"):
        s.flag("unavailable", "docker not found on this host.")
        return s
    code, out = run(
        ["docker", "ps", "-a", "--filter", f"label=com.docker.compose.project={COMPOSE_PROJECT}",
         "--format", "{{.Names}}"]
    )
    names = [n for n in out.split() if n] if code == 0 else []
    if not names:
        s.flag("warn", f"No containers found for compose project {COMPOSE_PROJECT}.")
        return s
    for name in names:
        code, raw = run(["docker", "inspect", name])
        if code != 0:
            s.flag("warn", f"{name}: could not inspect.")
            continue
        info = json.loads(raw)[0]
        state = info.get("State", {})
        health = (state.get("Health") or {}).get("Status", "")
        restarts = info.get("RestartCount", 0)
        if state.get("Status") != "running":
            s.flag("error", f"{name}: {state.get('Status')} (exit {state.get('ExitCode')}).")
        elif health == "unhealthy":
            s.flag("error", f"{name}: unhealthy.")
        if restarts:
            s.flag("warn", f"{name}: restarted {restarts} time(s) since created.")
        code, logs = run(["docker", "logs", "--since", f"{WINDOW_HOURS}h", name], timeout=120)
        bad = [
            line for line in logs.splitlines()
            if re.search(r"\b(ERROR|FATAL|PANIC|panicked|Traceback|OOMKilled)\b", line)
        ]
        if bad:
            s.flag("warn", f"{name}: {len(bad)} error line(s) in logs. Latest:")
            for line in bad[-MAX_LOG_SAMPLES:]:
                s.lines.append(f"    {mask(line)}")
    if not s.lines:
        s.lines.append(f"All {len(names)} containers running, no error lines in logs.")
    return s


def vm_health() -> Section:
    s = Section("VM")
    for mount in ("/", "/var/lib/docker"):
        if os.path.exists(mount):
            usage = shutil.disk_usage(mount)
            pct = round(100 * usage.used / usage.total)
            line = f"Disk {mount}: {pct}% used ({usage.free // 2**30} GB free)."
            if pct >= DISK_WARN_PERCENT:
                s.flag("error" if pct >= 90 else "warn", line)
            else:
                s.lines.append(line)
    try:
        meminfo = dict(
            (k, int(v.split()[0])) for k, v in
            (l.split(":", 1) for l in open("/proc/meminfo", encoding="ascii"))
        )
        avail = round(100 * meminfo["MemAvailable"] / meminfo["MemTotal"])
        line = f"Memory: {avail}% available."
        s.flag("warn", line) if avail < 10 else s.lines.append(line)
        load1, _, _ = os.getloadavg()
        cpus = os.cpu_count() or 1
        line = f"Load (1 min): {load1:.2f} on {cpus} CPU(s)."
        s.flag("warn", line) if load1 > 2 * cpus else s.lines.append(line)
        uptime = float(open("/proc/uptime", encoding="ascii").read().split()[0])
        if uptime < WINDOW_HOURS * 3600:
            s.flag("warn", f"VM rebooted {uptime / 3600:.1f} h ago.")
    except (OSError, KeyError, ValueError):
        s.flag("unavailable", "Memory/load/uptime not readable (not a Linux host?).")
    return s


# ------------------------------------------------------------------ email --

ICON = {"ok": "✅", "warn": "⚠️", "error": "❌", "unavailable": "❔"}


def build_report(sections: list[Section]) -> tuple[str, str, str]:
    host = socket.gethostname()
    now = dt.datetime.now(dt.timezone(dt.timedelta(hours=3)))
    worst = max(
        (s.status for s in sections),
        key=["ok", "unavailable", "warn", "error"].index,
    )
    problems = sum(1 for s in sections if s.status in ("warn", "error"))
    headline = "all clear" if problems == 0 else f"{problems} area(s) need attention"
    subject = f"{ICON[worst]} HelloDoctor nightly report ({host}): {headline}"
    when = now.strftime("%A %d %B %Y, %H:%M EAT")
    text = [f"HelloDoctor nightly report - {when}", f"Host: {host}", f"Window: last {WINDOW_HOURS} hours", ""]
    rows_html = []
    for s in sections:
        text.append(f"{ICON[s.status]} {s.title}")
        text.extend(f"  {line}" for line in s.lines)
        text.append("")
        items = "".join(f"<li>{html.escape(line)}</li>" for line in s.lines)
        rows_html.append(
            f"<h3 style=\"margin:18px 0 6px\">{ICON[s.status]} {html.escape(s.title)}</h3>"
            f"<ul style=\"margin:0;padding-left:20px\">{items}</ul>"
        )
    text.append("Read-only report. Personal data is masked.")
    body_html = (
        "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:14px;color:#15232c\">"
        f"<h2 style=\"margin:0 0 4px\">HelloDoctor nightly report</h2>"
        f"<p style=\"margin:0;color:#5b6a74\">{html.escape(when)} · {html.escape(host)} · "
        f"last {WINDOW_HOURS} hours</p>{''.join(rows_html)}"
        "<p style=\"margin-top:20px;color:#5b6a74;font-size:12px\">"
        "Read-only report. Personal data is masked.</p></div>"
    )
    return subject, "\n".join(text), body_html


def send(subject: str, text: str, body_html: str) -> None:
    host = os.environ["DIGEST_SMTP_HOST"]
    port = int(os.environ.get("DIGEST_SMTP_PORT", "587"))
    user = os.environ.get("DIGEST_SMTP_USER", "")
    password = ""
    pw_file = os.environ.get("DIGEST_SMTP_PASSWORD_FILE", "")
    if pw_file:
        with open(pw_file, encoding="utf-8") as fh:
            password = fh.read().strip()
    sender = os.environ.get("DIGEST_FROM", user)
    recipients = [r.strip() for r in os.environ["DIGEST_TO"].split(",") if r.strip()]

    msg = EmailMessage()
    msg["Subject"] = subject
    msg["From"] = sender
    msg["To"] = ", ".join(recipients)
    msg.set_content(text)
    msg.add_alternative(body_html, subtype="html")

    context = ssl.create_default_context()
    if port == 465:
        with smtplib.SMTP_SSL(host, port, context=context, timeout=60) as smtp:
            if user:
                smtp.login(user, password)
            smtp.send_message(msg)
    else:
        with smtplib.SMTP(host, port, timeout=60) as smtp:
            smtp.starttls(context=context)
            if user:
                smtp.login(user, password)
            smtp.send_message(msg)


def collect() -> list[Section]:
    sections = []
    for source in (app_errors, email_delivery, notification_delivery, worker_jobs,
                   database, containers, vm_health):
        try:
            sections.append(source())
        except Exception as exc:  # one broken source must not stop the report
            failed = Section(source.__name__.replace("_", " ").capitalize())
            failed.flag("unavailable", f"Check failed: {mask(str(exc))}")
            sections.append(failed)
    return sections


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dry-run", action="store_true", help="print the report, send nothing")
    args = parser.parse_args()
    subject, text, body_html = build_report(collect())
    if args.dry_run:
        if hasattr(sys.stdout, "reconfigure"):
            sys.stdout.reconfigure(encoding="utf-8")
        print(subject)
        print()
        print(text)
        return 0
    try:
        send(subject, text, body_html)
    except (KeyError, OSError, smtplib.SMTPException) as exc:
        print(f"error-digest: sending failed: {mask(str(exc))}", file=sys.stderr)
        return 1
    print(f"error-digest: sent '{subject}'")
    return 0


if __name__ == "__main__":
    sys.exit(main())
