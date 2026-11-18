# Screenshots

Captures of the shipping apps, linked from the main [README](../../README.md#screenshots). Replace
the file names below with real captures; the README picks them up by name.

## Rules

- Capture from a **current build** against a test or design-preview environment. Never use
  production data: no real patient names, phone numbers, records or messages.
- Phones: full-screen JPEG or PNG, light mode, English, status bar clean (full battery, no notifications).
- Web: 1440 × 900 PNG, browser chrome cropped.
- Do not add screenshots of features that are not live (for example payments while they are
  deferred), and never show star ratings or reviews.
- Optimise before committing (`pngquant` or WebP); keep each file under 500 KB.

## Files

| File | Screen | Status |
|---|---|---|
| `patient-home.jpg` | Patient Home | ✅ |
| `patient-prevention.jpg` | HIV Prevention & Sexual Health (Learn) | ✅ |
| `patient-find-doctor.jpg` | Find a Doctor | ✅ |
| `patient-health-vault.jpg` | Health Vault (from your doctor) | ✅ |
| `care-home.jpg` | Care app Home dashboard | ✅ |
| `care-schedule.jpg` | Consultation schedule (10-minute slots) | ✅ |
| `care-appointments.jpg` | Appointments (today) | ✅ |
| `care-chats.jpg` | Consultation chats | ✅ |
| `doctor-web-dashboard.jpg` | Doctor Web dashboard (design-preview fixtures) | ✅ |
| `doctor-web-chat.jpg` | Doctor Web patient chat (design-preview fixtures) | ✅ |
| `admin-overview.jpg` | Admin overview (no patient conditions are ever shown) | ✅ |
| `admin-verification.jpg` | Admin doctor verification | ✅ |

Phone captures are shown 180 px wide and web captures 400 px wide inside the README tables, each linked to the full-size file.
Re-encode before committing (this strips photo metadata), for example with Pillow:
`Image.open(src).convert("RGB").save(dst, "JPEG", quality=85, optimize=True)`.
