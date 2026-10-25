package com.hellodoctor.patient

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
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
    }
}
