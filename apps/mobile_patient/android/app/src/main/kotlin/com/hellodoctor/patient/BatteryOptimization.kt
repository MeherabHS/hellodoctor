package com.hellodoctor.patient

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.util.Locale

/**
 * Battery-optimisation status and settings shortcut for the one-time
 * "get reminders on time" guidance (2026-11-18). Opens settings pages only;
 * it never uses the Play-restricted REQUEST_IGNORE_BATTERY_OPTIMIZATIONS.
 */
object BatteryOptimization {
    fun register(engine: FlutterEngine, activity: Activity) {
        MethodChannel(engine.dartExecutor.binaryMessenger, "hellodoctor/battery")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "status" -> {
                        val power = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
                        result.success(mapOf(
                            "ignoring" to power.isIgnoringBatteryOptimizations(activity.packageName),
                            "manufacturer" to (Build.MANUFACTURER ?: "").lowercase(Locale.ROOT),
                        ))
                    }
                    "openSettings" -> result.success(openSettings(activity))
                    else -> result.notImplemented()
                }
            }
    }

    /** The app's own settings page (Battery and auto-start live there), else the battery list. */
    private fun openSettings(activity: Activity): Boolean {
        val intents = listOf(
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", activity.packageName, null)),
            Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS),
        )
        for (intent in intents) {
            try {
                activity.startActivity(intent)
                return true
            } catch (_: Exception) {
            }
        }
        return false
    }
}
