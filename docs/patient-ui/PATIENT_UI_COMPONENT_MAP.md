# Patient UI — Component Map

Discovery date: 2026-11-12. Scope: `apps/mobile_patient/lib`. Nothing here has been implemented; this maps what exists to what the reference needs.

Evidence labels: **[V]** verified by reading the source or sampling the image; **[A]** assumption that needs approval.

---

## 1. Current theme (`lib/core/theme/tokens.dart`) [V]

One file, `PatientTokens`, holds every token and builds the `ThemeData`. Material 3, light only. There is no `darkTheme`.

| Group | Tokens |
|---|---|
| Surfaces | `bg #F7FAF9`, `surface #FFFFFF`, `surfaceSubtle #F1F5F3`, `surfaceAlt #EEF4F1` |
| Brand | `primary #087A5B`, `primaryDark #075E49`, `primaryLight #DDF3E9` |
| Text | `text #172321`, `textSub #687773`, `muted #527169` |
| Lines | `border #DDE6E2` |
| Status | `warning #B7791F` / `warningLight #FFF6DD`, `info #2563EB` / `infoLight #EFF6FF`, `danger #B42318` / `dangerLight #FFF1F0`, `rose #F9738F` / `roseLight #FFE7EB`, `amberText #996515`, `roseText #9F1239` |
| Shape | `radius 12`, `cardRadius 16`, `chipRadius 9` |
| Size | `navHeight 68` (unused: the bottom nav hardcodes 76), `inputHeight 52`, `pad 20` |
| Type | Plus Jakarta Sans (bundled variable TTF). `headlineSmall 27/700`, `titleLarge 20/700`, `titleMedium 16/600`, `bodyLarge 16`, `bodyMedium 14`, `labelLarge 15/600` |
| Component themes | AppBar (flat, `bg`, left title 20/700), bottom sheet, card (flat), divider, input (filled white, 12 radius, 1 px border), filled button (50 high, 12 radius), outlined button |

Gaps in the token set:

- No spacing scale. Screens use literal paddings (18, 20 and 24 all appear as page gutters).
- No elevation or shadow tokens. Cards are flat with a 1 px border.
- No icon-tile, pill or segmented-control tokens, all of which the reference uses heavily.
- 35 colour literals sit outside the token file: `waiting_room_page.dart` (11), `patient_chat_page.dart` (9), `patient_home_page.dart` (6), `patient_booking_page.dart` (4), `emergency_contact_banner.dart` (2), `grievance_modal.dart` (2), `post_consultation_page.dart` (1).
- Most text styles are inline `TextStyle(fontSize: …)` literals; the `TextTheme` is used in only a few screens.

Assets: the only bundled asset is the font. There are no images, illustrations, SVGs or icon sets. Every icon is a Material `Icons.*` glyph.

---

## 2. Shared widgets today (`lib/core/widgets/`) [V]

| Widget | What it is | Used by |
|---|---|---|
| `PatientCard` | Flat white card, 16 radius, 1 px border, 16 padding | 15 files |
| `StatusChip` | Small tinted label, 9 radius | home, waiting room, intake, vault |
| `PatientPageHeader` | White header bar: title 20/800, subtitle 12, optional actions and refresh. Has **no back button** | services, blogs, vault, notification center, post-consultation |
| `NotificationBellAction` | Bell `IconButton` with unread `Badge` | home |
| `ErrorPanel` | Centered cloud-off icon, message, Retry | 9 files |
| `InlineError` | Red-tinted row with optional Retry | 10 files |
| `EmptyState` | Centered icon, title, message | blogs, directory, Q&A |
| `SkeletonBlock` | Pulsing grey block | 9 files |
| `CacheStatusBar` | Offline strip, "last updated" line, refresh progress | vault, notification center |
| `PatientField` | Label above a themed `TextField` | the five auth pages |
| `PatientSectionTitle` | Title row with optional text action | email OTP only |
| `PatientFilterButton` | Outlined filter button | **unused** |
| `showOfflineActionGuard` | "Needs a connection" bottom sheet | **unused** |
| `StartupErrorPage` | Standalone error screen with its own `MaterialApp` | `main.dart`, `app.dart` |

Feature-level widgets reused across screens:

| Widget | File | Reused by |
|---|---|---|
| `PatientBottomNav` | `app.dart` | shell only |
| `DoctorPhotoAvatar` | `directory/doctor_photo_avatar.dart` | home card, doctor card, doctor profile |
| `ProfilePhotoAvatar` | `profile/profile_photo_avatar.dart` | account tab |
| `FamilyProfileChipRow` | `profile/family_profile_chip_row.dart` | home switcher sheet, account tab |
| `PatientDoctorCard`, `DoctorDirectoryFilters`, `ConsultationChoices` | `directory/consultation_actions.dart` | directory, doctor profile |
| `EmergencyContactBanner` | `home/emergency_contact_banner.dart` | home |
| `ArticleCover`, `ArticleMarkdown` | `blogs/article_content.dart` | blogs list, article reader |
| `QuizQuestionCard`, `QuizAnswerOption`, `QuizFeedback`, `QuizCompletionSummary` | `blogs/quiz_widgets.dart` | article quiz section |
| `ClinicalPdfViewer`, `ClinicalImageViewer` | `vault/`, `chat/` | vault, chat thread |

---

## 3. Duplicated patterns [V]

| Pattern | Where it is hand-built | Consolidate into |
|---|---|---|
| Back-arrow header (`Row` of `IconButton` + title) | directory, booking, waiting room, doctor profile, blog detail; other pushed pages use a stock `AppBar` (15 files) | one `AfyaAppBar` with back arrow and title |
| "Icon + title + subtitle + chevron" row inside a card | services list, account `_SettingsTile`, notification tiles | one `AfyaListRow` |
| Tabs as `ChoiceChip`s | vault `_KindTabs`, chat list Active/Past, doctor profile `_TabButton` | one `AfyaSegmentedTabs` |
| Loading / error / empty triad | each page reimplements the `if loading … else if error … else if empty` chain | one `AsyncBody` helper around the existing three widgets |
| Bottom sheets with keyboard padding | profile edit, add dependent, add payment method, grievance, intake | one sheet scaffold |
| Initials avatar | home greeting (plain icon), blog author, doctor avatar fallback | one `AfyaAvatar` |
| Status-to-label `switch` | home `_ConsultCard`, waiting room, chat list (raw lowercase status) | one shared mapper |
| "Open directory / vault / Q&A" route builders | copied in `patient_home_page.dart` and `patient_services_page.dart` | one `PatientRoutes` helper so each module has a single entry point |

Hardcoded, non-localized English strings (at least 70 literals by a single-line pattern count) are concentrated in the auth pages, chat list, consultation status, data rights and the directory's empty state. Section G of the discovery report lists this as a risk because the product requires English and Swahili.

---

## 4. Tokens read from the reference image

The reference is one 1214 × 1295 px JPEG that holds eleven phone panels. Panel 1's phone frame spans x 11–231 and y 110–647, so **one phone screen is about 216 × 535 px**. Measured against a 360 dp wide device that is about 0.6 image pixels per dp: one image pixel is roughly 1.7 dp, and 10 sp body text is about 6 px tall. JPEG compression also shifts colours by a few units per channel.

The practical consequence is that colours can be sampled to within a few RGB units, while spacing, radii and font sizes can only be estimated to about ±2 dp. The values below are dominant-pixel samples and are proposals for approval. Dimensions are not listed because they cannot be read reliably at this resolution.

| Proposed token | Sampled value | Sampled from | Current nearest |
|---|---|---|---|
| Primary action green | `#009A73` | Start Chat, Start Video Call, View on Map buttons; active tab pills in panels 2, 5, 9 | `primary #087A5B` (darker) |
| Deep green (selected chip, active nav) | `#00765A` to `#07674B` | "Nearby" chip in panel 7, active Home icon | `primaryDark #075E49` |
| Teal CTA | `#007F8C` | "Ask a question (anonymous)" | none |
| Hero CTA blue-teal | `#036C8A` | "Explore HIV prevention" | none |
| Mint surface (greeting card, tiles) | `#DEF5ED` / `#DBF5EA` | Home greeting, Mobile App tile | `primaryLight #DDF3E9` (close) |
| Pale mint card | `#E5F8F2` | Peer Navigator hero card | none |
| Hero sky | `#85DCF0` | Home hero banner | none |
| Info blue surface | `#E9F6FC` / `#E2F3FD` | privacy cards in panels 3 and 5 | `infoLight #EFF6FF` |
| Soft lilac-grey surface | `#F8F9FD` | Support Chat intro banner | none |
| Urgent surface | `#FCF0F2`, icon red `#DB3B43` | "Need urgent help?" card | `dangerLight #FFF1F0` |
| Amber surface | `#FEF0D6` | SMS/USSD tile | `warningLight #FFF6DD` |
| Screen background | `#FEFEFC` (white) | phone body, service cards | `bg #F7FAF9` (tinted) |
| Search field fill | `#F7FFFF` | Home search | white |

`#E2F0FB` (panel caption boxes) and `#D8F1EB` (footer band) belong to the presentation collage, not to the app.

Typography in the image is a rounded geometric sans. Whether it is Plus Jakarta Sans cannot be determined from the raster. **[A]** Keep Plus Jakarta Sans unless a font is specified.

---

## 5. Components the reference needs

Status: **exists** (restyle), **partial** (extend), **new**.

| Reference component | Seen in panels | Status | Notes |
|---|---|---|---|
| Brand header: logo, wordmark, tagline, bell, initials avatar | 1 | new | The logo artwork is not in the repo. The bell exists (`NotificationBellAction`). Initials need a small helper |
| Greeting card on mint | 1 | new | Name comes from `ProfileCubit` |
| Search field | 1 | conflict | The v2.1 brief says no Home search field. See report section I |
| Hero banner: illustration, three-line headline, CTA | 1 | new | Illustration asset required |
| Service tile (3 × 3 grid): tinted icon tile, two-line label | 1 | new | Replaces `_CoreServicesGrid` (2 × 2 with badge and description) |
| Bottom navigation: Home, Services, Messages, Profile | 1–8, 11 | partial | `PatientBottomNav` exists with Home, Services, Blogs, Account |
| Back-arrow app bar | 2–8, 11 | partial | See duplicated patterns |
| Segmented pill tabs | 2, 5, 6, 9 | partial | Replaces `ChoiceChip` rows |
| Topic card: square illustration, title, description, duration, chevron | 2 | new | The article model has no duration field (to be confirmed in the API inventory) |
| Category row: coloured rounded icon tile, title, subtitle, chevron | 3, 6 | partial | Same family as `AfyaListRow` |
| Full-width pill CTA with leading icon | 3, 4, 7, 8 | partial | `FilledButton` theme, new radius and colours |
| Info / privacy card: icon, bold title, body on tinted surface | 3, 4, 5, 6, 8, 10, 11 | new | One widget with a tone parameter (info, mint, urgent, amber) |
| Illustration hero card | 4, 8 | new | Illustration assets required |
| Feature row: outlined icon, title, subtitle | 4 | partial | `AfyaListRow` without chevron |
| Reminder card: icon, title, description, date and time, chevron | 5 | new | No reminder feature exists |
| Filter chip row | 7 | partial | `DoctorDirectoryFilters` is a different pattern |
| Facility card: photo, name, services, distance, "Get Directions" | 7 | new | No facility feature exists |
| Map preview with "View on Map" | 7 | conflict | The v2.1 brief rules out an in-app Maps SDK and GPS |
| Checklist row: green check and text | 8, 11 | new | Trivial |
| Urgent-help card with phone icon | 8 | partial | Related to `EmergencyContactBanner`. Number differs: image shows 11663, app config is 999 |
| KPI tile grid, progress bar | 9 | new | GHI dashboard. Not a patient screen; see report section I |
| Channel tile: large icon, title, subtitle on tinted card | 10 | new | Informational only |
| Loading, error, empty states | not shown | exists | Restyle `SkeletonBlock`, `ErrorPanel`, `InlineError`, `EmptyState` |
| Conversation card, message bubble, composer | not shown | exists | `_ChatBubble`, composer in `patient_chat_page.dart`. **No reference panel exists for Messages** |
| Profile header, settings rows, logout | not shown | exists | `_Hero`, `_SettingsTile`. **No reference panel exists for Profile** |

---

## 6. Proposed design-system layout (for approval, not built)

```
lib/core/theme/
  tokens.dart            existing file: colours, radii, spacing, type scale, theme()
lib/core/widgets/
  patient_widgets.dart   existing widgets, restyled in place so call sites keep compiling
  afya_app_bar.dart      back-arrow bar and brand header
  afya_bottom_nav.dart   moved out of app.dart
  afya_tiles.dart        ServiceTile, TopicCard, CategoryRow, ListRow, ChannelTile
  afya_cards.dart        InfoCard (tones), HeroCard, GreetingCard
  afya_tabs.dart         SegmentedTabs, FilterChipRow
  afya_states.dart       AsyncBody around loading, error, empty
assets/
  brand/                 logo, wordmark (to be supplied)
  illustrations/         hero, topics, peer navigator, doctor (to be supplied, or marked placeholders)
```

Rules carried from the brief: one token location, no per-screen colour literals, existing widget names kept where tests reference them, strings through `AppLocalizations`, and no new package unless a component cannot be built without one (`flutter_svg` would be the only likely candidate, if assets arrive as SVG).
