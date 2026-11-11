import java.util.Properties

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

// Internal-testing-only release signing, same "conditional on a real file
// being present" pattern as google-services.json above: this repository has
// no production/Play Store signing key at all today (release builds fall
// back to the debug keystore, see below), so this is a dedicated,
// never-committed keystore supplied only through CI secrets for installable
// test builds -- explicitly not a production signing key, and never
// generated or overwritten by this build script itself.
val internalTestKeystoreProperties = Properties()
val internalTestKeystorePropertiesFile = file("internal-test-keystore.properties")
if (internalTestKeystorePropertiesFile.exists()) {
    internalTestKeystorePropertiesFile.inputStream().use { stream -> internalTestKeystoreProperties.load(stream) }
}

android {
    namespace = "com.example.hellodoctor_mobile"
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
        applicationId = "com.example.hellodoctor_mobile"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        if (internalTestKeystorePropertiesFile.exists()) {
            create("internalTesting") {
                storeFile = file(internalTestKeystoreProperties.getProperty("storeFile"))
                storePassword = internalTestKeystoreProperties.getProperty("storePassword")
                keyAlias = internalTestKeystoreProperties.getProperty("keyAlias")
                keyPassword = internalTestKeystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // Falls back to the debug keystore exactly as before when no
            // internal-test keystore has been supplied (e.g. a local
            // `flutter run --release`), so this is a strictly additive,
            // zero-behavior-change-by-default signing path.
            signingConfig = if (internalTestKeystorePropertiesFile.exists())
                signingConfigs.getByName("internalTesting")
            else
                signingConfigs.getByName("debug")
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
        // Behaves exactly like `release` (same signing/proguard via
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
            // initWith() did not reliably carry over release's unset
            // (AGP-default-false) isMinifyEnabled/isShrinkResources in
            // practice - a real fieldtest build first silently turned R8 on
            // (failed compiling firebase_messaging's classes), then with R8
            // off still inherited shrinkResources=true, which AGP rejects
            // without minification on. Pin both explicitly to match
            // release's actual behavior instead of relying on inheritance.
            isMinifyEnabled = false
            isShrinkResources = false
        }
    }

    packaging {
        jniLibs {
            // agora_rtc_engine bundles its full native extension catalogue
            // unconditionally (~69MB across all ABIs), regardless of which
            // ones the app actually calls. None of these are referenced
            // anywhere in lib/ (no enableExtension/virtual-background/beauty/
            // spatial-audio/screen-capture/AV1/face-detection calls) - core
            // join/publish/subscribe audio+video goes through
            // libagora-rtc-sdk.so plus the base codec/wrapper libs, which are
            // left untouched. Verify lib/ still has no call to one of these
            // extension providers before adding a new exclude here.
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
