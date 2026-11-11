# Milestone 8 APK Size Investigation

Date: 2026-11-01

Status: **MEASURED — no APK or application code changed**

## Artifacts measured

The latest arm64 APK artifacts were inspected directly as ZIP/APK files:

| Application | Artifact | Bytes | Decimal size | ABI contents |
|---|---|---:|---:|---|
| Doctor | `apps/mobile/build/app/outputs/flutter-apk/app-arm64-v8a-release.apk` | 66,324,794 | 66.32 MB | `arm64-v8a` only |
| Patient | `apps/mobile_patient/build/app/outputs/flutter-apk/app-arm64-v8a-release.apk` | 66,850,274 | 66.85 MB | `arm64-v8a` only |

Both `flutter-apk` files have the same SHA-256 as their corresponding
`build/app/outputs/apk/fieldtest/*` files, so their filenames do not prove that
they were produced by the ordinary minified release task. Their four-DEX
layout also matches the non-minified field-test configuration. They are still
valid arm64 APK measurements and are the artifacts currently available for
physical-device testing.

The Patient minified ordinary-release artifact is also present at
`apps/mobile_patient/build/app/outputs/apk/release/app-arm64-v8a-release.apk`
and measures 64,019,508 bytes (64.02 MB). The Doctor release configuration
does not currently enable minification; its split arm64 artifact is therefore
not present under `outputs/apk/release`; the ordinary Doctor release artifact
is the 137.23 MB universal APK described below. The 66.32 MB arm64 file is the
field-test-equivalent artifact.

The Doctor universal artifact,
`apps/mobile/build/app/outputs/apk/release/app-release.apk`, is 137,233,983
bytes because it contains `arm64-v8a`, `armeabi-v7a` and `x86_64` native
libraries. That universal artifact is not the physical arm64 delivery target.

## Compressed APK breakdown

ZIP compressed sizes for the arm64 artifacts are:

| Component | Doctor | Patient |
|---|---:|---:|
| Native libraries | 60.776 MB | 61.300 MB |
| DEX | 4.852 MB | 4.851 MB |
| Flutter assets | 0.211 MB | 0.212 MB |
| Other packaged files | 0.218 MB | 0.218 MB |
| Resources | 0.054 MB | 0.054 MB |
| Kotlin metadata | 0.012 MB | 0.012 MB |
| Manifest, META-INF and small assets | 0.010 MB | 0.010 MB |

The native `.so` files are stored uncompressed in the APK, so their compressed
and uncompressed sizes are identical. This is normal Android JNI packaging and
means ZIP compression cannot explain or materially reduce this APK size.

## Largest packaged files

The largest files are the same in both applications, apart from the compiled
Dart application library:

| File | Doctor compressed/uncompressed | Patient compressed/uncompressed |
|---|---:|---:|
| `lib/arm64-v8a/libagora-rtc-sdk.so` | 28.127 / 28.127 MB | 28.127 / 28.127 MB |
| `lib/arm64-v8a/libflutter.so` | 11.748 / 11.748 MB | 11.748 / 11.748 MB |
| `lib/arm64-v8a/libapp.so` | 8.783 / 8.783 MB | 9.307 / 9.307 MB |
| `classes.dex` | 3.160 / 8.617 MB | 3.159 / 8.616 MB |
| `lib/arm64-v8a/libagora-ffmpeg.so` | 5.859 / 5.859 MB | 5.859 / 5.859 MB |
| `classes4.dex` | 1.580 / 4.745 MB | 1.580 / 4.745 MB |
| `lib/arm64-v8a/libAgoraRtcWrapper.so` | 1.575 / 1.575 MB | 1.575 / 1.575 MB |
| `lib/arm64-v8a/libvideo_dec.so` | 1.212 / 1.212 MB | 1.212 / 1.212 MB |
| `lib/arm64-v8a/libagora_video_encoder_extension.so` | 1.084 / 1.084 MB | 1.084 / 1.084 MB |
| `lib/arm64-v8a/libagora-fdkaac.so` | 0.690 / 0.690 MB | 0.690 / 0.690 MB |
| `lib/arm64-v8a/libaosl.so` | 0.594 / 0.594 MB | 0.594 / 0.594 MB |
| `lib/arm64-v8a/libiris_method_channel.so` | 0.393 / 0.393 MB | 0.393 / 0.393 MB |
| `lib/arm64-v8a/libiris_rendering_android.so` | 0.381 / 0.381 MB | 0.381 / 0.381 MB |
| `lib/arm64-v8a/libagora-soundtouch.so` | 0.198 / 0.198 MB | 0.198 / 0.198 MB |
| `assets/flutter_assets/assets/fonts/PlusJakartaSans.ttf` | 0.079 / 0.176 MB | 0.079 / 0.176 MB |
| `lib/arm64-v8a/libdartjni.so` | 0.131 / 0.131 MB | 0.131 / 0.131 MB |
| `classes3.dex` | 0.111 / 0.254 MB | 0.111 / 0.254 MB |
| `resources.arsc` | 0.213 / 0.213 MB | 0.213 / 0.213 MB |
| `assets/flutter_assets/NOTICES.Z` | 0.117 / 0.118 MB | 0.117 / 0.118 MB |
| `assets/flutter_assets/shaders/ink_sparkle.frag` | 0.005 / 0.022 MB | 0.005 / 0.022 MB |

## Native ownership of the size

For both arm64 artifacts:

- Agora RTC core: **28.127 MB** (`libagora-rtc-sdk.so`).
- Agora FFmpeg/media processing: **5.859 MB** (`libagora-ffmpeg.so`).
- Remaining Agora wrapper, codec, rendering, method-channel and required
  extension libraries: approximately **6.128 MB**.
- Total Agora native footprint: approximately **40.114 MB**.
- Flutter engine: **11.748 MB** (`libflutter.so`).
- Compiled Dart application: **8.783 MB Doctor**, **9.307 MB Patient**.
- Dart JNI bridge: **0.131 MB**.

The remaining native total is therefore accounted for as required Agora media
runtime plus Flutter/Dart runtime. Assets, fonts and Android resources are
well under 1 MB combined.

## Agora extension verification

The following excluded libraries are absent from both arm64 APKs:

`libagora_lip_sync_extension.so`, `libagora_clear_vision_extension.so`,
`libagora_spatial_audio_extension.so`, `libagora_ai_noise_suppression_extension.so`,
`libagora_face_capture_extension.so`, `libagora_segmentation_extension.so`,
`libagora_ai_echo_cancellation_extension.so`, `libagora_audio_beauty_extension.so`,
`libagora_content_inspect_extension.so`,
`libagora_video_quality_analyzer_extension.so`,
`libagora_video_av1_encoder_extension.so`, `libagora_face_detection_extension.so`
and `libagora_screen_capture_extension.so`.

`libagora_video_encoder_extension.so` remains packaged. It is not one of the
excluded optional libraries and has not been removed because the current
two-way consultation path has not established that it is optional.

No duplicate arm64 native libraries, accidental x86/armeabi-v7a payloads,
debug-symbol files or large unreferenced assets were found in the measured
arm64 artifacts. The universal Doctor APK does contain the additional ABIs,
which explains its 137.23 MB size.

## Why the APK grew from the earlier approximately 18 MB build

The preserved Doctor arm64 artifact from the pre-Agora period,
`releases/hellodoctor-doctor-arm64-startup-fix-20261015.apk`, is 20,456,432
bytes. Its largest native files are only `libflutter.so` (11.748 MB),
`libapp.so` (7.341 MB) and `libdartjni.so`; it contains no Agora libraries.

The current arm64 artifact adds approximately 40.114 MB of required Agora
native media runtime. The compiled Dart library and DEX also grew as chat,
notifications, bookings, authentication and consultation features were added.
The increase is therefore primarily the real Agora RTC/media payload, with a
smaller contribution from application growth. The old 18–20 MB artifact and
the current consultation-enabled artifact are not equivalent feature sets.

## Safe recommendations

1. **Use Android App Bundles for store delivery — low risk, high practical
   value.** AAB delivery lets Google Play serve only the device ABI and
   configuration. This will not materially shrink the standalone arm64 APK,
   but it avoids universal-APK downloads and is the preferred distribution
   format.
2. **Measure a Doctor release build with R8/resource shrinking — medium risk,
   potentially 2–3 MB.** Doctor release currently leaves minification and
   resource shrinking disabled, while Patient's optimized release is 64.02 MB
   and has only 2.125 MB compressed DEX versus Doctor's 4.852 MB. This should
   be tested as a separate artifact with the consultation, WorkManager and
   notification smoke checks before any configuration change.
3. **Keep the current Agora exclusions — already realized savings, low risk.
   ** The excluded extension catalogue is absent from both APKs. Further native
   removals are not safe without a real two-device audio/video regression.
4. **Do not remove `libagora-rtc-sdk.so`, `libagora-ffmpeg.so`, codecs,
   `libvideo_dec.so` or the wrapper libraries based on size alone.** They are
   part of the active consultation runtime.

Potential savings from fonts, shaders and small resources are measured in
kilobytes. Splitting or compressing native libraries further would trade
installation/runtime behavior for little or unproven download benefit and was
not justified by this investigation.

No APK was rebuilt, no library was removed, and no Flutter, Agora, chat,
authentication, booking or consultation behavior was changed.
