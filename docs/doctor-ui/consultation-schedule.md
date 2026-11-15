# Consultation Schedule (Doctor app)

Source of truth (product owner, 2026-11-13):

- Empty state: `docs/design/afyamkononi-doctor-schedule-empty-reference.png`
- Populated state: `docs/design/afyamkononi-doctor-schedule-populated-reference.png`

The screen carries the Home header (menu, AfyaMkononi logo, bell, profile)
via the shared `DoctorTopBar` widget. Colours are sampled from the
references.

## Extracted imagery

Both references are AI-generated for this project; the art is extracted, not
redrawn.

| Asset | Source region (empty reference) | Notes |
|---|---|---|
| `assets/doctor/schedule_empty_illustration.jpg` | x 140–810, y 790–1145 (native 670×355) | Near-white background set to pure white so it sits flush on the white card. |
| `assets/doctor/schedule_header_stethoscope.png` | x 692–941, y 0–190 (native 249×190) | Page background keyed to transparent; left and bottom edges faded because the reference blob runs behind the title text. |

Neither is upscaled.

## Deliberate differences

- **Summary line:** the empty reference reads "0 Booked / Consultations" and
  the populated one "N Left To See (M Completed)". The screen uses the first
  when today has no slots and the second otherwise. On narrow phones
  "(M Completed)" drops under the count instead of being cut off.
- **Slot cards not in the references** (open and blocked slots) use the same
  card design: amber accent and "Available" pill for open slots, grey for
  blocked. Their Cancel / Block / Unblock actions are unchanged.
- **Completed vs. past:** a booked slot shows "Completed" when its
  consultation is completed, or once its time has passed (as before).
- **"Last updated" line:** hidden unless the doctor is offline, as on Home.
