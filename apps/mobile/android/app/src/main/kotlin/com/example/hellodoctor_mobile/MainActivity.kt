package com.example.hellodoctor_mobile

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import android.os.Build
import android.os.Bundle

class MainActivity : FlutterActivity() {
    // FLAG_SECURE (screenshot/screen-recording/recent-apps-snapshot block for
    // this clinical screen content) was deliberately removed here at the
    // product owner's explicit request, after being told this trades away a
    // real PHI-exposure protection (screenshots/recordings of consultation
    // video, prescriptions and chat become possible again, in every build
    // including release) — not an oversight or a debug-only carve-out.
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        DeviceProof.register(flutterEngine, this)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "hellodoctor/build_metadata")
            .setMethodCallHandler { call, result ->
                if (call.method != "get") { result.notImplemented() }
                else {
                    try {
                        @Suppress("DEPRECATION")
                        val info = packageManager.getPackageInfo(packageName, 0)
                        @Suppress("DEPRECATION")
                        val number = if (Build.VERSION.SDK_INT >= 28) info.longVersionCode else info.versionCode.toLong()
                        result.success(mapOf("app_version" to (info.versionName ?: ""), "build_number" to number.toString()))
                    } catch (_: Exception) { result.error("METADATA_UNAVAILABLE", "Build metadata unavailable", null) }
                }
            }
    }
}
