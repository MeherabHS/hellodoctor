# Assurance pack

Performance and security evidence for HelloDoctor / AfyaMkononi, re-baselined 2026-11-17 on commit
`eef9ce7` (formerly `6f02b84`; see BASELINE.md). Start with the performance report's summary.

| Document | What it holds |
|---|---|
| [PERFORMANCE_REPORT.md](PERFORMANCE_REPORT.md) | Capacity, overload behaviour, soak, chat, login and upload results; recommendations; defects found and fixed |
| [BASELINE.md](BASELINE.md) | Exactly what was measured: commit, migration-set hash, images, resource caps, dataset, host, and how it differs from production |
| [SECURITY_STATUS.md](SECURITY_STATUS.md) | Re-verified status of every earlier security finding (fixed, partial, open) |
| [HISTORY.md](HISTORY.md) | Catalogue of the earlier, superseded assurance documents |

## Evidence (`evidence/`)

Each folder is one run: the load generator's log and `result.json`, CPU/memory telemetry per
container, database metrics before and after, top SQL statements, peak memory and OOM counts, and
row counts. Summarise any folder with
`python -I services/backend/scripts/assurance/summarize_assurance.py docs/assurance/evidence/<folder>`.

| Folder | Run | Quoted in the report |
|---|---|---|
| `baseline/` | Canonical dataset v2 manifest (the dump itself is gitignored) | Dataset |
| `ramp/`, `spike/`, `soak/`, `chat/`, `uploads/` | Baseline runs, payments deferred | Yes |
| `login_single_ip/`, `login_distributed_10/25/50/` | Login flows from one and from many addresses | Yes |
| `ramp_diag_x10_payments_enabled/` | Diagnostic build, budgets ×10, to find the hardware limit | Yes (labelled) |
| `ramp_fixed/`, `spike_fixed/`, `uploads_fixed/` | First 502 fix (`Connection: close` only) | Before/after table |
| `uploads_fixed2/`, `spike_fixed2/` | Final 502 fix (drain the unread body) | Before/after table |
| `smoke/`, `ramp_v1/`, `ramp_payments_enabled/`, `spike_payments_enabled/` | Exploratory runs (harness check, dataset v1, payments enabled) | No, kept for the record |

Production runs with payments deferred (`FINANCIAL_MODULE_MODE=deferred`); the
`*_payments_enabled` runs predate that decision.
