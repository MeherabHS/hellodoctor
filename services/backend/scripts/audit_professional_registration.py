"""Read-only source inventory; uses installed rg, never scans credentials/builds."""
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'services/backend/target/kenya-professional-verification'
OUT.mkdir(parents=True, exist_ok=True)
PATTERN = re.compile(r'BMDC|Bangladesh|\bBD\b|\+880|KMPDC|license_number|license_authority|license_country|registration_number|professional.verification|is_verified|৳|\bBDT\b|bkash|nagad', re.I)
LEGACY = re.compile(r'BMDC|Bangladesh|\bBD\b|\+880|৳|\bBDT\b|bkash|nagad', re.I)
EVIDENCE_PREFIX = 'docs/audit_evidence/kenya-professional-verification-residue'

def classify(path):
    if '/migrations/' in path:
        return 'MIGRATION_HISTORY'
    if path.startswith(('scratch/', 'prototype/')) or path in ('index.html', 'fix.py', 'update_docs.py'):
        return 'HISTORICAL_ONLY'
    if '/tests/' in path or '/test/' in path or '/audit_evidence/' in path or '/scripts/' in path or path.startswith('.github/'):
        return 'ACTIVE_TEST'
    if path.endswith('src/seed.rs'):
        return 'ACTIVE_SEED_DATA'
    if path.startswith('services/backend/src/'):
        return 'ACTIVE_API' if '/api/' in path else 'ACTIVE_PRODUCTION_CODE'
    if path.startswith('apps/'):
        return 'ACTIVE_UI'
    return 'STALE_DOC'

rows = []
paths = subprocess.check_output(['rg', '--files', '--hidden', '-g', '!.git/**', '-g', '!node_modules/**'], cwd=ROOT, text=True).splitlines()
for raw in paths:
    path = raw.replace('\\', '/')
    p = ROOT / path
    if path.startswith(EVIDENCE_PREFIX):
        continue  # Generated results are evidence, not additional source inputs.
    if p.suffix.lower() == '.json' and any(term in p.name.lower() for term in ('credential', 'token', 'secret')):
        continue
    if p.suffix.lower() not in ('.rs', '.sql', '.ts', '.tsx', '.dart', '.py', '.ps1', '.mjs', '.js', '.json', '.css', '.html', '.md', '.yaml', '.yml', '.toml'):
        continue
    if any(x in path.split('/') for x in ('node_modules', '.next', 'target', 'build', '.dart_tool', '.git')) or p.name in ('package-lock.json', 'Cargo.lock', 'pubspec.lock'):
        continue
    for number, line in enumerate(p.read_text(encoding='utf-8-sig', errors='replace').splitlines(), 1):
        matches = sorted(set(m.group() for m in PATTERN.finditer(line)))
        if matches:
            rows.append({'path': path, 'line': number, 'classification': classify(path), 'terms': matches, 'text': line[:450]})
phase = sys.argv[1] if len(sys.argv) > 1 else 'after'
(OUT / (phase + '-inventory.json')).write_text(json.dumps(rows, indent=2, ensure_ascii=False), encoding='utf-8')
if phase == 'before':
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (ROOT / 'services/backend/migrations').glob('*.sql')}
    (OUT / 'original-migration-hashes.json').write_text(json.dumps(hashes, indent=2), encoding='utf-8')
else:
    hashes = json.loads((OUT / 'original-migration-hashes.json').read_text())
    assert all(hashlib.sha256((ROOT / 'services/backend/migrations' / p).read_bytes()).hexdigest() == digest for p, digest in hashes.items()), 'Historical migration changed'
    residues = []
    non_country_matches = []
    for row in rows:
        terms = [term for term in row['terms'] if LEGACY.fullmatch(term)]
        if not terms:
            continue
        path = row['path']
        if all(term.lower() == 'bd' for term in terms) and not re.search(r'country|Bangladesh|\bBD\b', row['text']):
            non_country_matches.append({**row, 'reason': 'Case-insensitive BD search matched an unrelated identifier/style; not a country or regulator.'})
            continue
        if path.startswith('scratch/') or path in ('fix.py', 'update_docs.py'):
            category, reason = 'DEAD_CODE', 'Unshipped prototype editing/test helper; not an application entry point.'
        elif '/migrations/' in path:
            category, reason = 'HISTORICAL', 'Immutable migration history or explicit legacy audit-action backfill in the new migration.'
        elif path.startswith('prototype/') or path == 'index.html' or path.endswith(('prototype_dom.json', 'coverage-current.json')):
            category, reason = 'HISTORICAL', 'Original prototype or captured prototype/audit evidence; not shipped as the active application.'
        elif path == 'apps/doctor-web/src/app/prototype.css':
            category, reason = 'DEAD_CODE', 'Unused legacy financial CSS selector; no registration concept or rendered provider text.'
        elif '/tests/' in path or '/test/' in path or path.endswith('src/domain/validation.rs'):
            category, reason = 'TEST_LEGACY', 'Historical audit/payment compatibility fixture or rejected foreign-phone input; not current professional-registration data.'
        elif '/scripts/' in path or '/examples/' in path or path.endswith(('check-source.mjs', 'generate_templates.py', 'kenya_professional_verification_live.py')):
            category, reason = 'TEST_LEGACY', 'Search/rejection safeguard, guarded fixture repair, or synthetic legacy financial test harness.'
        elif path.startswith('services/backend/src/') or path.startswith('apps/admin-web/src/'):
            category, reason = 'HISTORICAL', 'Compiled compatibility for historical financial records/style identifiers; no active professional-registration dependency. Financial architecture is outside this correction.'
        elif path.endswith('.md'):
            category, reason = 'HISTORICAL', 'Superseded design/financial prose, dated audit history, or explicit historical compatibility documentation; current Kenya review contract supersedes it.'
        else:
            category, reason = 'ACTIVE_BUG', 'Unclassified legacy occurrence requires manual review.'
        residues.append({**row, 'legacy_terms': terms, 'residue_classification': category, 'reason': reason})
    active_roots = {
        'BACKEND': 'services/backend/src/',
        'DOCTOR_WEB': 'apps/doctor-web/src/',
        'FLUTTER': 'apps/mobile/lib/',
        'ADMIN': 'apps/admin-web/src/',
    }
    active_counts = {name: sum(any('bmdc' in term.lower() for term in row['terms']) for row in rows if row['path'].startswith(prefix)) for name, prefix in active_roots.items()}
    evidence = {
        'active_bmdc_matching_lines': active_counts,
        'classification_counts': dict(Counter(row['residue_classification'] for row in residues)),
        'original_migrations_unchanged': True,
        'non_country_search_matches': non_country_matches,
        'remaining_occurrences': residues,
    }
    (OUT / 'legacy-residue-inventory.json').write_text(json.dumps(evidence, indent=2, ensure_ascii=False), encoding='utf-8')
    # Keep a compact, reviewable artifact in docs. Every matching source line is
    # still identified; avoid duplicating long prototype snippets in the repo.
    groups = defaultdict(list)
    for row in residues:
        groups[(row['path'], row['residue_classification'], row['reason'])].append(row)
    evidence['remaining_matching_lines'] = len(residues)
    evidence['remaining_occurrences'] = [
        {'path': path, 'classification': category, 'reason': reason,
         'lines': [row['line'] for row in group],
         'terms': sorted({term for row in group for term in row['legacy_terms']})}
        for (path, category, reason), group in sorted(groups.items())
    ]
    (ROOT / (EVIDENCE_PREFIX + '.json')).write_text(json.dumps(evidence, indent=2, ensure_ascii=False), encoding='utf-8')
    assert not any(active_counts.values()), 'Active professional-registration legacy regulator remains'
    assert not any(row['residue_classification'] == 'ACTIVE_BUG' for row in residues), 'Unclassified legacy residue remains'
    print(json.dumps({'residue_counts': evidence['classification_counts'], 'active_bmdc_matching_lines': active_counts, 'non_country_matches': len(non_country_matches)}))
print(json.dumps({'phase': phase, 'matching_lines': len(rows), 'classification_counts': dict(Counter(r['classification'] for r in rows)), 'original_migrations_unchanged': phase != 'before'}))
