package com.example.hellodoctor_mobile

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import android.os.Build
import android.os.Bundle
import android.view.WindowManager

class MainActivity : FlutterActivity() {
    // Screen content here is clinical (consultation video, prescriptions,
    // Health Vault documents) — FLAG_SECURE stops both a screenshot/screen-
    // recording of this content by another app and the OS's own recent-apps
    // task-switcher snapshot from ever capturing it, the same PHI-exposure
    // side door CLAUDE.md's existing "no generic disk caching of medical
    // images" and "never log PHI" rules already guard against by other means.
    override fun onCreate(savedInstanceState: Bundle?) {
        window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
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
