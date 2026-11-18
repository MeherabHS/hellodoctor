# Screenshots

Captures of the shipping apps, linked from the main [README](../../README.md#screenshots). Replace
the file names below with real captures; the README picks them up by name.

## Rules

- Capture from a **current build** against a test or design-preview environment. Never use
  production data: no real patient names, phone numbers, records or messages.
- Phones: 1080 × 2400 PNG, light mode, English, status bar clean (full battery, no notifications).
- Web: 1440 × 900 PNG, browser chrome cropped.
- Do not add screenshots of features that are not live (for example payments while they are
  deferred), and never show star ratings or reviews.
- Optimise before committing (`pngquant` or WebP); keep each file under 500 KB.

## Expected files

| File | Screen | How to open it |
|---|---|---|
| `patient-home.png` | Patient Home | Patient app, signed-in test account |
| `patient-video.png` | Video consultation | Waiting room or call with a test appointment |
| `patient-records.png` | Health records | Home, Health Records module |
| `patient-chat.png` | Messages / chat thread | Messages tab, test conversation |
| `care-home.png` | Care app Home dashboard | Care app, test doctor account |
| `care-queue.png` | Appointment queue | Appointments tab |
| `care-consultation.png` | Consultation | Join a test consultation |
| `care-notes.png` | Clinical notes and outcome | Complete a test consultation |
| `doctor-web-dashboard.png` | Doctor Web dashboard | Doctor Web, test doctor |
| `doctor-web-chat.png` | Doctor Web chat | Messages with a test patient |
| `admin-overview.png` | Admin overview | Admin Web, test admin (no patient conditions are ever shown) |
| `admin-verification.png` | Doctor verification | Admin Web, Doctors, pending verification |

When a file is added, change its README cell from the file name to an image, for example
`<img src="docs/screenshots/patient-home.png" alt="Patient Home" width="200">`.
