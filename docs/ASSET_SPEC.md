# Phase 13 — Asset Specification & Resource Inventory

> **Document Version:** 1.0.1  
> **Source Baseline:** `/scratch/images/` (68 PNG files), `/thumbnail.png`, Inline SVGs in `index.html`  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Asset Strategy & Optimization Guidelines

To guarantee high visual fidelity while keeping the Flutter APK and IPA under 35 MB:
1. **Core UI Icons:** Implemented as vector SVGs (`flutter_svg`) or standard Material/Cupertino icons where 1:1 identical.
2. **Branding & Static Graphics:** Bundled inside the application package under `assets/images/` and `assets/icons/`.
3. **Public vs PHI Assets:** 
   - **Public Assets:** Use `cached_network_image` for public assets like doctor portraits (with local disk caching).
   - **PHI Assets:** Medical images (e.g., patient prescriptions, lab reports) must NOT use generic disk cache. Use encrypted lifecycle-controlled storage exclusively.
4. **Target Compression:** All raster assets must be converted to **WebP** format at 85% quality, reducing file sizes by up to 65% compared to raw PNGs.

---

## 2. Comprehensive Asset Inventory Table

| Asset Identifier | Path in Repo | Format | Dimensions | Purpose | Screen Usage | Deployment Mode | Optimization Required |
|---|---|---|---|---|---|---|---|
| `img_app_logo` | Inline SVG in `index.html` | SVG Vector | Dynamic | Primary HelloDoctor medical cross & heart logo | Splash, Header, Auth | Bundled (`assets/icons/logo.svg`) | None (Vector) |
| `avatar_doc_sabrina` | `scratch/images/912082f...png` | PNG | 400x400 | Doctor portrait: Dr. Sabrina Akter | `view-0`, `view-3`, `view-9`, `view-10` | Remote / Bundled Sample | Convert to WebP (40KB) |
| `avatar_doc_anika` | `scratch/images/020cd31...png` | PNG | 400x400 | Doctor portrait: Dr. Anika Rahman | `view-3`, `view-9`, `adminPage_doctors` | Remote / Bundled Sample | Convert to WebP (38KB) |
| `avatar_doc_sadik` | `scratch/images/1220dd6...png` | PNG | 400x400 | Doctor portrait: Dr. Sadik Al-Amin | `view-3`, `view-9`, `adminPage_doctors` | Remote / Bundled Sample | Convert to WebP (42KB) |
| `avatar_doc_farhana` | `scratch/images/a37b4b7...png` | PNG | 400x400 | Doctor portrait: Dr. Farhana Yesmin | `view-3`, `view-9`, `adminPage_doctors` | Remote / Bundled Sample | Convert to WebP (39KB) |
| `avatar_patient_rafiq`| `scratch/images/7ca8502...png` | PNG | 200x200 | Patient portrait: Rafiq Ahmed. This is fictional/mock prototype data only. Real patient images must never be bundled in production application assets. | `view-0`, `view-7`, `doc-view-0` | Remote / Bundled Sample | Convert to WebP (18KB) |
| `icon_service_doctor` | Inline SVG | SVG Vector | 32x32 | Core service icon: Specialist consultation | `view-0`, `view-1` | Bundled SVG | None |
| `icon_service_medicine`| Inline SVG | SVG Vector | 32x32 | Core service icon: Prescription pharmacy | `view-0`, `view-1` | Bundled SVG | None |
| `icon_service_lab` | Inline SVG | SVG Vector | 32x32 | Core service icon: Diagnostic sample tests | `view-0`, `view-1` | Bundled SVG | None |
| `icon_service_vault` | Inline SVG | SVG Vector | 32x32 | Core service icon: Health vault / EMR | `view-0`, `view-1` | Bundled SVG | None |
| `icon_emergency_bell` | Inline SVG | SVG Vector | 24x24 | Emergency contact icon (Ambulance / Cross) | `view-0` (`#patientEmergencyHelplineStrip`) | Bundled SVG | None |
| `badge_bmdc_verified` | Inline SVG | SVG Vector | 16x16 | Green verified credential checkmark badge | `view-3`, `view-9`, `adminPage_bmdc` | Bundled SVG | None |
| `flag_icon_en` | Inline SVG | SVG Vector | 20x15 | UK / International English flag indicator | `#languageModal` | Bundled SVG | None |
| `flag_icon_sw` | Inline SVG | SVG Vector | 20x15 | Kenya national flag for Swahili localization | `#languageModal` | Bundled SVG | None |
| `img_empty_box` | `scratch/images/ba7a0e8...png` | PNG | 300x220 | Empty state illustration for empty queue/vault | `view-5`, `doc-view-0` | Bundled (`assets/images/empty_box.webp`) | Convert to WebP |
| `img_video_placeholder`| `scratch/images/c0548fc...png` | PNG | 1280x720 | Video stream fallback background | `view-10`, `#docWebVideoFeed` | Bundled WebP | Convert to WebP |
| `sound_incoming_chime`| Inferred Audio | MP3 / WAV | Audio (3s) | Consultation incoming call ringtone | `view-10`, `doc-view-0` | Bundled (`assets/audio/chime.mp3`) | 128kbps mono MP3 |

---

## 3. Font Asset Bundling (`pubspec.yaml`)

```yaml
flutter:
  fonts:
    - family: Plus Jakarta Sans
      fonts:
        - asset: assets/fonts/PlusJakartaSans-Regular.ttf
          weight: 400
        - asset: assets/fonts/PlusJakartaSans-Medium.ttf
          weight: 500
        - asset: assets/fonts/PlusJakartaSans-SemiBold.ttf
          weight: 600
        - asset: assets/fonts/PlusJakartaSans-Bold.ttf
          weight: 700
        - asset: assets/fonts/PlusJakartaSans-ExtraBold.ttf
          weight: 800
```
