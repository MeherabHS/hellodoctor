# Doctor Web UI assets - 2026-10-07

No external assets were downloaded in this pass. The canonical repository
prototype already contains the exact icons and a lightweight loading animation.
No third-party package, font, runtime or browser was installed/downloaded.

| Asset name | Source | Source file/path | License record | Original project | Date reused | Local project path | Modified |
|---|---|---|---|---|---|---|---|
| Calendar icon | Local repository | prototype/index.html:9005 | Existing repository asset; no external download | HelloDoctor prototype | 2026-10-07 | apps/doctor-web/public/icons/calendar.svg | SVG namespace added; geometry unchanged |
| Notification bell | Local repository | prototype/index.html:9008 | Existing repository asset; no external download | HelloDoctor prototype | 2026-10-07 | apps/doctor-web/public/icons/bell.svg | SVG namespace added; geometry unchanged |
| Loading ring | Local repository | prototype/index.html:4382 | Existing repository CSS; no external download | HelloDoctor prototype | 2026-10-07 | apps/doctor-web/src/app/prototype.css; Common.tsx/web.css | Smaller size for short requests; reduced-motion static equivalent |

The two local SVGs contain only geometric elements, with no script, event
attributes, foreignObject, linked/embedded images, external URLs or tracking.
They are served locally as CSS masks, preserving prototype stroke and hover
color. The namespace declaration identifies SVG syntax, not a remote resource.
The loading ring is decorative with accessible status text, and never replaces
an error/empty state. Existing font files were retained without downloads.

```text
LOCAL_ASSETS_REUSED = 3
OPEN_SOURCE_ASSETS_DOWNLOADED = 0
DOWNLOADED_ASSET_SOURCES = NONE
LICENSES_VERIFIED = YES (no external licensing dependency introduced)
UNLICENSED_ASSETS_USED = 0 (third-party additions)
REMOTE_HOTLINKED_ASSETS = 0
SVG_SECURITY_CHECK = PASS
HOST_SOFTWARE_INSTALLED = NONE
GLOBAL_PACKAGES_INSTALLED = NONE
BROWSER_INSTALLED = NO
PLAYWRIGHT_BROWSER_DOWNLOADED = NO
DOCKER_INFRASTRUCTURE_ADDED = NONE
MINIO_REINTRODUCED = NO
```

## Dashboard / Settings / Patient Chats reuse follow-up

Additional existing local asset reused: prototype chat-bubble SVG geometry (`prototype/index.html:4670`), used inline in `apps/doctor-web/src/components/Chat.tsx` and the Patient Chats sidebar entry in DoctorApplication.tsx. Original project: HelloDoctor prototype; reused 2026-10-07. Scale/stroke adapted for the decorative empty state; arc values simplified slightly. No external download or new license dependency. Geometry-only SVGs; no script, event attributes, foreignObject, external content or tracking. The simple error triangle is authored project-local SVG geometry, not a third-party asset.

Current total LOCAL_ASSETS_REUSED = 4 (calendar, bell, loading ring, chat bubble). OPEN_SOURCE_ASSETS_DOWNLOADED = 0; DOWNLOADED_ASSET_SOURCES = NONE; LICENSES_VERIFIED = YES; UNLICENSED_ASSETS_USED = 0; REMOTE_HOTLINKED_ASSETS = 0; SVG_SECURITY_CHECK = PASS. No font/package/browser/software download or installation. The earlier total of 3 above is the historical notification follow-up.


## 2026-10-07 ? Real slot blocking and semantic cards

Schedule correction: reused one additional local prototype asset, the clock SVG at `prototype/index.html:7099`, in `Schedule.tsx`. Lock, hourglass and cancelled-calendar paths are simple project-authored static artwork. External assets downloaded = 0 for this pass; unlicensed/remote-hotlinked additions = 0; SVG security review PASS (decorative JSX paths, no script/remote/event content). No packages/software installed.

Contract, tests, mobile exception, telemetry findings and full report: [DOCTOR_WEB_SCHEDULE_CORRECTION.md](DOCTOR_WEB_SCHEDULE_CORRECTION.md).
