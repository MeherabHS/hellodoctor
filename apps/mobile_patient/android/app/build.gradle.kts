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
            // zero-behavior-change-by-default signing path. Replace with a
            // real Play Store signing config before any real distribution.
            signingConfig = if (internalTestKeystorePropertiesFile.exists())
                signingConfigs.getByName("internalTesting")
            else
                signingConfigs.getByName("debug")
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
