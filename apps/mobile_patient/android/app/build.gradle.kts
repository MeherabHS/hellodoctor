plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// Google Services (FCM) native config is a per-deployment secret, never
// committed (see .gitignore and docs/FCM_SETUP.md) — so this plugin is
// applied imperatively, only when the real file has actually been placed,
// rather than from the declarative `plugins {}` block above (which would
// fail every build in an environment that hasn't supplied it yet, including
// this repository as checked in).
if (file("google-services.json").exists()) {
    apply(plugin = "com.google.gms.google-services")
}

android {
    namespace = "com.hellodoctor.patient"
    // Matches apps/mobile's working configuration. The file_picker-driven
    // compileSdk=36 override that was here previously is obsolete: file_picker
    // is now pinned to ^8.1.2 (see pubspec.yaml), which no longer needs it, and
    // the override was actively breaking the agora_rtc_engine AAR-metadata
    // check (forces androidx deps requiring compileSdk 34+ into the same graph
    // as an AAR published against API 31).
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
        // Required by flutter_local_notifications (java.time usage) on API
        // levels below 26 — without this the release build fails with a
        // "missing java.time classes" desugaring error, not an app-level bug.
        isCoreLibraryDesugaringEnabled = true
    }

    defaultConfig {
        applicationId = "com.hellodoctor.patient"
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // Signing with the debug keys for now, so `flutter run --release` works.
            // Replace with a real signing config before any real distribution.
            signingConfig = signingConfigs.getByName("debug")
            // Shrink/minify the release build to help keep the per-device download
            // under the project's ~20MB size budget. flutter_local_notifications and
            // workmanager both rely on reflection for Android BroadcastReceiver/Worker
            // registration, so the matching keep rules in proguard-rules.pro are
            // required — verify with a real release build + manual notification
            // smoke test, not just a clean `flutter build appbundle`.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
        // Additive, independent build type for physical-device LAN testing
        // during local development - NOT a flavor (flavors would force every
        // `flutter build apk`/`assembleRelease` invocation, including the
        // ordinary production one, to require an explicit --flavor choice).
        // This only exists as its own separate `assembleFieldtest` Gradle
        // task; the real `release` build type above, and every existing
        // `flutter build apk --release` invocation, is completely untouched.
        // Behaves exactly like `release` (same signing/minify/proguard via
        // initWith) except for src/fieldtest/res/xml/network_security_config.xml,
        // which trusts only this test session's specific LAN IP over
        // cleartext HTTP - see that file for why this is safe and scoped.
        create("fieldtest") {
            initWith(getByName("release"))
            // Lets Gradle resolve native/AAR dependencies (e.g. agora_rtc_engine)
            // that only publish debug/release variant-aware artifacts, by
            // treating this custom build type as equivalent to release for
            // dependency resolution purposes.
            matchingFallbacks += listOf("release")
            // Real release (above) minifies fine, but the fieldtest variant
            // specifically hit a genuine R8 class collision ("Type a.a is
            // defined multiple times" between firebase_messaging's and
            // flutter_local_notifications's obfuscated classes) that only
            // reproduces for this extra build type, not for release itself -
            // disabling minify/shrink here avoids that without touching
            // release's real behavior. Size is not a priority for this local
            // test-only build type.
            isMinifyEnabled = false
            isShrinkResources = false
        }
    }

    packaging {
        jniLibs {
            // agora_rtc_engine bundles its full native extension catalogue
            // unconditionally (~69MB across all ABIs) regardless of which
            // ones the app actually calls - minifyEnabled/shrinkResources
            // above never touch native .so files, so this is what was
            // actually blowing past the ~20MB budget noted above. None of
            // these are referenced anywhere in lib/ (no enableExtension/
            // virtual-background/beauty/spatial-audio/screen-capture/AV1/
            // face-detection calls) - core join/publish/subscribe audio+video
            // goes through libagora-rtc-sdk.so plus the base codec/wrapper
            // libs, which are left untouched. Verify lib/ still has no call
            // to one of these extension providers before adding a new
            // exclude here.
            excludes += setOf(
                "**/libagora_lip_sync_extension.so",
                "**/libagora_clear_vision_extension.so",
                "**/libagora_spatial_audio_extension.so",
                "**/libagora_ai_noise_suppression_extension.so",
                "**/libagora_face_capture_extension.so",
                "**/libagora_segmentation_extension.so",
                "**/libagora_ai_echo_cancellation_extension.so",
                "**/libagora_audio_beauty_extension.so",
                "**/libagora_content_inspect_extension.so",
                "**/libagora_video_quality_analyzer_extension.so",
                "**/libagora_video_av1_encoder_extension.so",
                "**/libagora_face_detection_extension.so",
                "**/libagora_screen_capture_extension.so",
            )
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}

dependencies {
    coreLibraryDesugaring("com.android.tools:desugar_jdk_libs:2.1.4")
}
