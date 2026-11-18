-- Peer Navigator fixtures for the performance-assurance runs (2026-11-17).
-- Adds 40 approved synthetic Peer Navigators (phone prefix +25476, the
-- convention examples/capacity_test.rs loads by) with 300 open 30-minute
-- slots each, starting tomorrow. Synthetic data only; no PHI.
--
--   docker exec -i hellodoctor-perf-baseline-postgres-1 psql -U postgres -d hellodoctor \
--     -v ON_ERROR_STOP=1 < scripts/seed_capacity_peers.sql
--
-- Applied once while building the canonical baseline (see
-- docs/assurance/evidence/baseline/CANONICAL_BASELINE_MANIFEST.json); not
-- meant to be re-run per scenario.

BEGIN;

WITH p AS (
    SELECT gs AS seq, gen_random_uuid() AS user_id FROM generate_series(1, 40) AS gs
)
INSERT INTO users (id, phone_number, email, password_hash, role, preferred_language,
                   is_active, is_verified, created_at, updated_at)
SELECT user_id,
       '+25476' || lpad(seq::text, 7, '0'),
       'capacity.peer.' || seq || '@example.invalid',
       NULL,
       'PEER',
       CASE WHEN seq % 3 = 0 THEN 'sw' ELSE 'en' END,
       TRUE, TRUE, now() - interval '60 days', now()
FROM p;

INSERT INTO peer_profiles (user_id, full_name, county, organization, bio,
                           approval_status, reviewed_at, created_at, updated_at)
SELECT u.id,
       'Capacity Test Peer ' || substring(u.phone_number from 7),
       (ARRAY['Nairobi','Mombasa','Kisumu','Nakuru','Eldoret'])[1 + (substring(u.phone_number from 7)::int % 5)],
       'Synthetic Peer Support Group',
       'Synthetic peer navigator used only for load testing.',
       'APPROVED', now() - interval '59 days', now() - interval '60 days', now()
FROM users u
WHERE u.phone_number LIKE '+25476%';

-- 300 non-overlapping 30-minute slots per peer, 45 minutes apart, from
-- tomorrow 06:00 UTC onward.
INSERT INTO peer_slots (peer_id, start_time, end_time, status)
SELECT pp.id,
       date_trunc('day', now()) + interval '1 day 6 hours' + (i * interval '45 minutes'),
       date_trunc('day', now()) + interval '1 day 6 hours' + (i * interval '45 minutes') + interval '30 minutes',
       'AVAILABLE'
FROM peer_profiles pp
JOIN users u ON u.id = pp.user_id AND u.phone_number LIKE '+25476%'
CROSS JOIN generate_series(0, 299) AS i;

SELECT 'peers' AS metric, count(*) AS n FROM peer_profiles WHERE approval_status = 'APPROVED'
UNION ALL
SELECT 'open_peer_slots', count(*) FROM peer_slots WHERE status = 'AVAILABLE';

COMMIT;
