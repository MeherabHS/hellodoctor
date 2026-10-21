# Local synthetic resilience harness

`Invoke-IsolatedSql.ps1` uses the already cached `postgres:16` image, no pull or
installation, no external network, no published port or shared database, a read-only
root and source mounts, empty allowlisted child environment, 512 MiB scratch and
RAM, no swap, two CPU quota, 64 processes, file/open-file limits and 180 second
deadline. Only the scratch tmpfs is a writable target filesystem. It creates a
new cluster, applies every up migration in its own transaction, then runs six
100-operation phases with eight clients maximum. `observe.sql` grades committed
rows separately from the workers. Fixtures have `.invalid` identities, no PHI or
provider credentials. SQLx metadata rows are never fabricated; this is raw SQL
validation, not a SQLx startup/readiness test. It never restarts the shared dev
backend or applies migrations to the shared dev DB.

Run from the workspace: `./services/backend/scripts/resilience/Invoke-IsolatedSql.ps1`.
The new economic migration down/up cycle is checked only in the disposable fixture.

`attack-local.py` is an HTTP producer for a separate **isolated** disposable full
HelloDoctor stack. It has a fixed 32-worker maximum, 30,000 logical-identity maximum,
two-second sockets, 128 KiB request and 64 KiB response bounds, and a bounded
producer queue. It only accepts literal loopback origins and an explicitly
synthetic manifest. Run it in the fixture namespace, never against a shared dev
instance, tunnel, production, external provider, or third-party URL. The actual
backend is the defender. A fixture-owned loopback ingress may model logical IPs
through its approved proxy configuration; do not add test headers/bypass logic to
production code. All requests still traverse actual application admission.

Example manifest (supply other cases with synthetic fixture credentials):

```json
{"synthetic_fixture":true,"base_url":"http://127.0.0.1:18080","cases":{"distributed_login":{"path":"/api/v1/auth/patient/login","body":{"email":"synthetic@example.invalid","password":"synthetic-invalid-password"}}}}
```

Example command: `python attack-local.py --manifest fixture.json --case distributed_login --count 30000 --workers 32 --output attack.json`.
For payment storms use `--keys same`, `--keys different`, and `--keys none` on
separate fresh obligations. For many-account login use `--accounts different`;
the generator substitutes a different `synthetic-N@example.invalid` target for
each request. Test Patient, Doctor and Admin surfaces separately (Admin requires
the fixture's signed BFF path, not a production challenge bypass).

`observe-local.py` is a separate observer. Its before/after snapshots must come
from independent fixture DB reads and telemetry collection. It does not accept
an attacker success assertion. Missing DB/worker/ordinary-traffic evidence gives
INCONCLUSIVE. The SQL observer is executable in the supplied cached-image sandbox;
full HTTP, Python, Rust, browser and Android harness execution need suitable
already-provisioned isolated toolchains. No results from these unexecuted paths
are claimed in the report.

The HTTP producer does not yet inject commit-response loss/provider timeouts,
generate Android proofs or mock provider signatures. Use the dedicated backend
race tests and a provider fixture for those acceptance cases. Do not label absent
provider checkout behavior as an idempotency pass.
