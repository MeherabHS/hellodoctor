# Facilities & Services — implementation plan

Date 2026-11-13. Branch `fix/p0-consultation-repair` (uncommitted Patient education, password-reset and Doctor-app work is present and preserved). Reference: `docs/design/facilities/facility-services-flow.png`. Product-owner changes: no distance anywhere; Admin-managed opening hours.

## Decisions

| Topic | Decision | Why |
|---|---|---|
| Image storage | Reuse the existing article-media object store (`AppState.article_media_storage`), key prefix `facility-media/<uuid>.jpg`, same sanitising (JPEG/PNG → re-encoded JPEG ≤ 500 KB), same encryption at rest | Non-clinical, Admin-authored media; already a private container separate from clinical blobs. No new config |
| Local storage | Azurite (Azure Blob emulator) for the dev backend, `AZURE_STORAGE_AUTH_MODE=azurite` (already supported) | Local storage is `disabled`, so uploads failed before |
| County | One of Kenya's 47 counties, validated server-side; Admin picks from a list | Consistent county filter ("Nearby") without GPS |
| "Nearby" chip | Opens a county picker; filters the directory to that county. Held in memory for the session only | No GPS, no fabricated proximity, no stored location |
| Services | `facility_service_categories` table (code, EN/SW names, icon key, order); facilities hold an array of codes | New categories need a migration row, not a Flutter release; unknown icon keys fall back to a generic icon |
| Opening hours | JSONB on the facility, validated in Rust: per weekday `closed`, `all_day`, or up to 3 periods (`HH:MM`–`HH:MM`, `overnight` flag for after-midnight closing) | One aggregate, saved atomically with the facility |
| Time zone | Stored per facility; only `Africa/Nairobi` (UTC+3, no DST) accepted now | Kenya-only deployment; avoids a new time-zone dependency. Other zones are a later, separately tested change |
| Status | Server computes `OPEN`, `CLOSED`, `OPEN_24_HOURS`, `HOURS_NOT_AVAILABLE` with the next change (UTC instant, facility-local time, day offset) from server time | Phone clock and phone time zone never decide open/closed |
| Distance | Not shown, not computed, no location permission | Product-owner decision |
| "View on Map" button | Not shown. The decorative map strip was removed too (product owner, 2026-11-13) | No multi-facility map exists; Get Directions opens each facility's own Admin link |
| Bookmark / Share (details) | Hidden | Same as articles: no store, sign-in required |
| Publication | DRAFT → PUBLISHED → (unpublish) DRAFT; any → ARCHIVED (final). Publishing requires every required field, ≥1 service and a photo. Edits to a published facility stay published but must remain publishable | Matches the brief; no review step requested |

## Database (migration `20261212000001_health_facilities`, with down)

`facility_service_categories`, `health_facilities`, `facility_media`; RLS: Admin full access; Patient/Doctor read published only; media readable only while it is a published facility's current photo.

## API

Admin (role `ADMIN`): `GET/POST /admin/facilities`, `GET/PATCH /admin/facilities/:id`, `POST /admin/facilities/:id/{publish,unpublish,archive}`, `POST/GET/DELETE /admin/facilities/:id/image`, `GET /admin/facility-services`.
Patient (any signed-in user): `GET /facilities` (search, county, services, youth_friendly, limit/offset), `GET /facilities/counties`, `GET /facilities/:id`, `GET /facilities/:id/image`.

## Clients

- Admin Web: nav "Facilities & Services"; list with search/county/status filters and pagination; editor with all fields, services, opening-hours grid (copy Monday to weekdays, 24 h, periods, overnight), photo upload/preview/replace/remove; publish/unpublish/archive. Upload through a dedicated Next route like article covers.
- Patient Flutter: directory (app header, search with debounce, chips, cards, empty and no-match states, skeleton, error/retry, pagination, refresh), details (photo, Youth Friendly, location, status, services, call, directions, opening hours with today highlighted, about).

## Tests

Backend unit (URL, phone, hours validation, status computation incl. boundaries, overnight, 24 h, missing) and integration (RLS/visibility, admin-only writes, publish rules, image upload auth). Admin Web unit tests. Patient widget tests. Emulator E2E against the local backend with Azurite.
