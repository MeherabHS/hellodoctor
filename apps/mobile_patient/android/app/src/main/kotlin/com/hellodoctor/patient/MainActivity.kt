package com.hellodoctor.patient

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
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
    }
}
