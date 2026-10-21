plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
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
