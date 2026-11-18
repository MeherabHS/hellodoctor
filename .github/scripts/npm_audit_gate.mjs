#!/usr/bin/env node
// CI dependency gate: `npm audit` at moderate or above, except advisories listed in
// .github/npm-audit-allowlist.json. Run from an app directory (apps/admin-web, apps/doctor-web).
//
// Fails when:
//   - any moderate/high/critical advisory is not allowlisted;
//   - an allowlist entry has expired or lacks a reason;
//   - an allowlisted advisory appears in production dependencies (npm audit --omit=dev).
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const LEVELS = new Set(['moderate', 'high', 'critical']);
const here = dirname(fileURLToPath(import.meta.url));
const allowlist = JSON.parse(readFileSync(join(here, '..', 'npm-audit-allowlist.json'), 'utf8'));
const today = new Date().toISOString().slice(0, 10);

function audit(extraArgs) {
  let out;
  try {
    out = execFileSync('npm', ['audit', '--json', ...extraArgs], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'inherit'],
      shell: process.platform === 'win32',
      maxBuffer: 64 * 1024 * 1024,
    });
  } catch (error) {
    // npm audit exits non-zero when it finds anything; the JSON is still on stdout.
    out = error.stdout;
  }
  return JSON.parse(out || '{}');
}

/** Root advisories (the objects in `via`) at moderate or above, keyed by GHSA id. */
function advisories(report) {
  const found = new Map();
  for (const [name, vuln] of Object.entries(report.vulnerabilities ?? {})) {
    for (const via of vuln.via ?? []) {
      if (typeof via !== 'object' || !LEVELS.has(via.severity)) continue;
      const id = (via.url ?? '').split('/').pop() || `${name}:${via.source}`;
      found.set(id, { id, package: via.name ?? name, severity: via.severity, title: via.title });
    }
  }
  return found;
}

const problems = [];
const allowed = new Map();
for (const entry of allowlist.advisories ?? []) {
  if (!entry.id || !entry.reason || !entry.expires) {
    problems.push(`allowlist entry ${entry.id ?? '(no id)'} needs id, reason and expires`);
  } else if (entry.expires < today) {
    problems.push(`allowlist entry ${entry.id} expired on ${entry.expires}; re-check it`);
  } else {
    allowed.set(entry.id, entry);
  }
}

const all = advisories(audit([]));
const production = advisories(audit(['--omit=dev']));

for (const advisory of all.values()) {
  const label = `${advisory.id} ${advisory.severity} ${advisory.package}: ${advisory.title}`;
  if (!allowed.has(advisory.id)) {
    problems.push(`not allowlisted: ${label}`);
  } else if (production.has(advisory.id)) {
    problems.push(`allowlisted but present in production dependencies: ${label}`);
  } else {
    console.log(`allowlisted until ${allowed.get(advisory.id).expires} (dev only): ${label}`);
  }
}

if (problems.length) {
  console.error('npm audit gate failed:');
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}
console.log(`npm audit gate passed (${all.size} advisories at moderate or above, all allowlisted dev-only).`);
