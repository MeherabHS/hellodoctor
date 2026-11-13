# Facilities & Services — implementation report

Date 2026-11-13. Plan: `FACILITIES_IMPLEMENTATION_PLAN.md`. Reference: `docs/design/facilities/facility-services-flow.png`. Product-owner changes applied: no distance anywhere; Admin-managed opening hours with server-computed status.

Nothing was deployed, no production configuration or migration was touched, and no release APK was built.

## 1. Database — migration `20261212000001_health_facilities` (with down)

| Table | Purpose |
|---|---|
| `facility_service_categories` | `code`, EN/SW names, `icon_key`, order, active. Seeded TESTING, PREP, ART, COUNSELLING. A new category is a row, not an app release (unknown icon keys get a generic icon) |
| `health_facilities` | name, description, county (one of 47), location text, contact phone (E.164 `+254…`), Google Maps URL, services (codes), youth friendly, timezone (`Africa/Nairobi`), opening hours (JSONB), photo id, publication status DRAFT/PUBLISHED/ARCHIVED, version, published/archived/created/updated timestamps, created_by/updated_by |
| `facility_media` | photo metadata (object key `facility-media/<uuid>.jpg`, content hash, size, ready). Bytes live in object storage, encrypted at rest |

Constraints: a PUBLISHED row must have county, location, phone, Maps link, a photo and ≥1 service; ARCHIVED ⇔ `archived_at`; archived rows cannot change; version must advance by one. RLS: Admin full access; Patient/Doctor read PUBLISHED only; photo rows readable only while they are a published facility's current photo.

## 2. Backend endpoints

| Route | Who | Notes |
|---|---|---|
| `GET /facilities` | any signed-in user | Published only. `search` (name, location, county, service names), `county`, `services` (any of), `youth_friendly`, `limit`/`offset`; ordered by name then id; returns `total`, `next_offset`, `server_time` |
| `GET /facilities/counties` | any signed-in user | Counties that have published facilities (for "Nearby") |
| `GET /facilities/:id` | any signed-in user | 404 unless published |
| `GET /facilities/:id/image` | any signed-in user | Published facility's current photo; `Cache-Control: private, max-age=86400`, ETag; URL carries the photo id so a new photo is a new URL |
| `GET/POST /admin/facilities`, `GET/PATCH /admin/facilities/:id` | Admin | Optimistic version check; drafts need only a name |
| `POST /admin/facilities/:id/{publish,unpublish,archive}` | Admin | Publish refuses incomplete facilities with the list of what is missing |
| `GET/POST/DELETE /admin/facilities/:id/image` | Admin | JPEG/PNG ≤ 5 MB, re-encoded JPEG ≤ 500 KB (same sanitiser as article covers); DELETE only on drafts |
| `GET /admin/facility-services` | Admin | Service categories and the 47 counties |

Patient responses carry display fields only: no storage keys, audit fields, versions or status of unpublished records. Status object: `state` (`OPEN`, `CLOSED`, `OPEN_24_HOURS`, `HOURS_NOT_AVAILABLE`), `next_change_at` (UTC), `next_change_local_time`, `next_change_day_offset`, `next_change_weekday`, `today` — computed from server time in the facility's time zone (opening inclusive, closing exclusive; overnight periods; 24 h; missing hours).

Validation (server, mirrored in Admin Web): Maps links must be HTTPS on `google.com/maps`, `www.google.com/maps`, `google.co.ke/maps`, `maps.google.com`, `maps.google.co.ke` or `maps.app.goo.gl` (no credentials, ports, look-alike hosts, other paths or generic shorteners); phones accept Kenyan `+254`/`254`/`0` forms; opening periods must not overlap (within a day or across midnight, including Sunday→Monday) and must close after opening unless marked overnight.

## 3. Admin Web

Nav **Facilities & Services** (`components/facilities/`): list with name search, county and status filters, pagination; editor with facility information, services, contact and directions, opening hours (Closed / Open with up to three periods and "closes next day" / Open 24 hours, "Copy Monday hours to weekdays", remove hours), photo upload/preview/replace/remove (drafts), Save draft / Update published facility / Publish / Unpublish / Archive, and the list of what blocks publication. Photos go through `app/api/facility-media/[id]` (authenticated, size-bounded, like article covers); no storage URL or key reaches the browser.

## 4. Patient app

`lib/features/patient/facilities/`:

- **Directory** (`facility_directory_page.dart`): app header, "Facility Services", subtitle, search (350 ms debounce), chips Testing / PrEP/ART / Youth Friendly (no Nearby chip: product owner, 2026-11-13), cards (photo, name, services, Youth Friendly, status, location, Get Directions with the Admin's Google Maps link shown underneath, chevron), skeleton while loading, error with Try again, "No facilities listed yet" vs "No matching facilities", infinite scroll, pull to refresh, refresh on resume and when the earliest open/close time passes. No map picture (removed by the product owner, 2026-11-13).
- **Details** (`facility_details_page.dart`): photo, name, Youth Friendly, location, status, Services Available (only the Admin's selection), Call Facility (dialer with the number; never calls), Get Directions with the Admin's link shown underneath; tapping opens it in Google Maps (external app, browser fallback), Opening Hours (seven days, today highlighted in facility time), About.
- Photos: authenticated API bytes, in-memory cache keyed by the versioned path; placeholder instead of a broken image.

## 5. Differences from the reference (deliberate)

| Reference | App | Reason |
|---|---|---|
| "2.3 km", "Approx. 6 min" | Location text | Product owner: no distance |
| "Open · Closes 5:00 PM" sample | Real status from Admin hours, or "Opening hours not available" | No invented hours |
| "View on Map" button | Not shown; the decorative map strip was also removed (product owner, 2026-11-13) | No multi-facility map; each facility has its own directions link |
| Bookmark / Share on details | Not shown | No bookmark store; sign-in required |
| Nearby chip | Removed | Product owner, 2026-11-13: no GPS or location filter. The backend still accepts a `county` filter and serves `/facilities/counties`; the app does not use them |
| Get Directions subtitle (distance) | The Admin's Google Maps link (Admin field "Location URL (Google Maps)") | Product owner, 2026-11-13 |
| Service icons (illustrated) | Material icons in the same colours | Reference icons are ~55 px; too small to extract cleanly |
| Empty-state illustration | Interim crop (338 × 198 px) from the flow image | **Needs a full-size image from the product owner**; it looks soft on a phone |

## 6. Verification

### Automated

| Suite | Result |
|---|---|
| Backend unit (`domain::facilities`) | 9/9: phones, Maps links, counties, hours validation, status (boundaries, weekend, split, overnight, 24 h, missing, all-closed, time zone), wire names |
| Backend integration (`facilities::`) | 4/4: lifecycle + patient visibility (drafts/unpublished/archived hidden by list, id and photo; no audit/storage fields), Admin-only writes (Patient and Doctor get 403 on create/edit/publish/upload/list/image; unauthenticated 401; article permissions unchanged), validation (bad links incl. `javascript:`, `http:`, look-alike host, credentials; bad phones; unknown county/service; reversed/overlapping hours; wrong MIME; non-image bytes; stale version), search/filters/pagination/counties |
| Backend clippy `-D warnings`, `cargo fmt --check` | clean |
| Backend full suite | **Not completed.** The run was stopped when the machine ran low on memory; only the facility, admin-article, quiz, password-reset and unit groups have been run since these changes |
| Admin Web `tsc`, `eslint`, `next build` | clean (one pre-existing lint warning in campaigns) |
| Admin Web tests | 105/106; the one failure is `refresh.test.ts`, which needs a separately provisioned Redis fixture (unrelated, unchanged) |
| Patient `flutter analyze` / `flutter test` | clean / 513 + facility tests all passing (12 facility tests: parsing, link and image-path safety, every status phrase, empty vs no-match, chips → filters, county picker, error/retry, details content, hours not provided, 360 dp × 1.3 × Swahili with no overflow) |

### End to end (local backend, Azurite storage, Pixel emulator)

Synthetic facilities only (names marked "(synthetic)", placeholder numbers, Google Maps search links, photos cropped from the AI-generated reference).

| Test | Result |
|---|---|
| A — Admin creates facilities, uploads photos, a draft stays invisible | Pass. Drafts never appeared in the app; an incomplete draft's publish was refused (422); an `https://evil.example/maps` link was refused (422) |
| B — Publish | Pass. Published facilities appeared in the running app without a rebuild |
| C — Details | Pass. Photo, name, Youth Friendly, location, status, only the chosen services, phone, directions, all seven days (today highlighted in facility time), About |
| D — Actions | Pass. Call Facility opened the dialer with the number and placed no call; Get Directions opened Google Maps with the Admin's link |
| E — Empty state | Pass. After unpublishing all, "No facilities listed yet" with the approved message |
| F — Search and filters | Pass. Testing → 3, PrEP/ART → 2, Youth Friendly → 2, (county Kisumu → 1, before Nearby was removed), search "counselling" → 2, search "nowhere" → "No matching facilities" |
| Status | Friday 18:2x EAT: weekday 8–5 facility "Closed · Opens tomorrow at 8:00 AM"; 24 h clinic "Open 24 hours"; Friday-night service "Closed · Opens today at 8:00 PM" |
| Loading / error | Skeleton rows while loading; with the backend stopped, "We couldn't load facilities…" and Try again |

Screenshots: `docs/facilities/screenshots/`.

## 7. Known limitations

- Time zones: only `Africa/Nairobi` is accepted (Kenya-only deployment; fixed UTC+3, exact). Other zones need a time-zone database dependency and tests.
- Replaced photos leave their old encrypted object in storage (unreferenced, not served). A cleanup job like the article-media one is a follow-up.
- Swahili strings are machine-quality and need native review.
- No location-based filtering in the app (product owner decision). Proximity would need coordinates and a separately verified feature.
- Screenshots 01, 04, 05 and 08 predate the 2026-11-13 change (Nearby chip, link under Get Directions); retake when the app is next run.
