"""Run the required Rust checks strictly sequentially, using installed Cargo."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

BACKEND = Path(__file__).resolve().parents[1]
OUT = BACKEND / 'target/kenya-professional-verification'
OUT.mkdir(parents=True, exist_ok=True)
env = {**os.environ, 'RUST_LOG': 'error', 'CARGO_TERM_COLOR': 'never'}
checks = [
    ('fmt', ['cargo','fmt','--check']),
    ('clippy', ['cargo','clippy','--all-targets','--','-D','warnings']),
    ('lib-tests', ['cargo','test','--lib']),
    ('api-tests', ['cargo','test','--test','api_tests','--','--test-threads=1']),
]
results = {}
for label, command in checks:
    print('Running ' + ' '.join(command), flush=True)
    start = time.monotonic()
    with (OUT / (label + '.log')).open('w', encoding='utf-8') as log:
        code = subprocess.run(command, cwd=BACKEND, env=env, stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0).returncode
    output = (OUT / (label + '.log')).read_text(encoding='utf-8', errors='replace')
    result = {'command':command, 'exit_code':code, 'seconds':round(time.monotonic()-start, 2)}
    match = re.search(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out', output)
    if match:
        result.update(dict(zip(['passed','failed','ignored','measured','filtered'], map(int,match.groups()[1:]))))
    results[label] = result
    (OUT / 'rust-verification.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
    print(json.dumps({label:result}), flush=True)
    if code:
        print(output[-5000:], flush=True)
        sys.exit(code)
print('All four sequential Rust checks passed.', flush=True)
