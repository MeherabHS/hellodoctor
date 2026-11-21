package com.hellodoctor.doctor

import android.Manifest
import android.app.Activity
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.util.Locale

/**
 * Delivery readiness for reminders and messages: battery optimisation and
 * notification permission.
 *
 * 2026-11-20 (product owner): the one-tap system dialog
 * ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS is used (it needs the
 * Play-restricted REQUEST_IGNORE_BATTERY_OPTIMIZATIONS permission, declared
 * in Play Console; see docs/PLAY_BATTERY_EXEMPTION_DECLARATION.md). Neither
 * exemption nor permission can be granted without the user's tap.
 */
object BatteryOptimization {
    fun register(engine: FlutterEngine, activity: Activity) {
        MethodChannel(engine.dartExecutor.binaryMessenger, "hellodoctor/battery")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "status" -> result.success(status(activity))
                    "requestIgnore" -> result.success(requestIgnore(activity))
                    else -> result.notImplemented()
                }
            }
    }

    private fun status(activity: Activity): Map<String, Any> {
        val power = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        val notifications =
            activity.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val disabled = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            notifications.notificationChannels
                .filter { it.importance == NotificationManager.IMPORTANCE_NONE }
                .map { it.id }
        } else emptyList()
        val permission = Build.VERSION.SDK_INT < 33 ||
            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
        return mapOf(
            "ignoring" to power.isIgnoringBatteryOptimizations(activity.packageName),
            "manufacturer" to (Build.MANUFACTURER ?: "").lowercase(Locale.ROOT),
            "notificationsEnabled" to notifications.areNotificationsEnabled(),
            "permissionGranted" to permission,
            // After one denial Android shows a rationale; after the second it
            // stops showing the dialog at all (only settings can help then).
            "showRationale" to (Build.VERSION.SDK_INT >= 33 &&
                activity.shouldShowRequestPermissionRationale(
                    Manifest.permission.POST_NOTIFICATIONS)),
            "disabledChannels" to disabled,
            "sdk" to Build.VERSION.SDK_INT,
        )
    }

    /** The system "Let app always run in background?" Allow / Deny dialog. */
    private fun requestIgnore(activity: Activity): Boolean {
        val power = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        if (power.isIgnoringBatteryOptimizations(activity.packageName)) return true
        return try {
            activity.startActivity(
                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                    .setData(Uri.parse("package:${activity.packageName}"))
            )
            true
        } catch (_: Exception) {
            // No redirect to Settings (product owner, 2026-11-20).
            false
        }
    }

}
