# Doctor bottom navigation — 2026-10-15

The custom bottom bar occupied Android's system-navigation area in the
edge-to-edge release. `DoctorShell` now wraps its shared bottom navigation in
`SafeArea(top: false)`. The system bottom inset is reserved across all six tabs,
while retaining the existing 76dp bar, labels, tap targets and tab behavior.

Nine new widget cases mount the actual Doctor shell at 320/360/412dp widths with
0/24/48dp bottom insets. They assert the bar's exact bottom position, visibility
of all six labels, at least 48dp tap-target height, successful Schedule
navigation and no overflow/uncaught error. Existing test files and assertions
are unchanged; their pre-edit hashes are recorded with the evidence.

Validation:

- Formatting: clean.
- Analysis: no issues.
- Full Doctor suite: 189 passed, 0 failed, 0 skipped; previously 180.
- The previously added WorkManager/Room startup preservation rules remain.
- Actual signed-in device screenshot shows all six tab labels above Android's
  system buttons. The user independently confirmed the positioning is correct.
- Release/device process exit codes and artifact identity are recorded with the
  [device results](audit_evidence/doctor_navigation_safe_area/device_results.json).

Physical evidence:

- [Before: Android buttons overlap Doctor tabs](audit_evidence/doctor_navigation_safe_area/before_device.png)
- [After: Doctor tabs above Android buttons](audit_evidence/doctor_navigation_safe_area/after_device.png)

Updated development release: [Doctor 1.0.1+4](../releases/hellodoctor-doctor-arm64-navigation-fix-20261015.apk).
Installed in place on the connected Moto g96 5G (Android 16), preserving app data.
This record covers the shared navigation bar and startup, not full acceptance
of unrelated consultation/payment/media functionality.
