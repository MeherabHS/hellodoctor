import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Delivery readiness: notification permission and battery optimisation.
///
/// Many Android phones sold in Kenya (Tecno, Infinix, itel, Xiaomi, Oppo,
/// Vivo...) stop background apps to save battery, and Android 13+ shows no
/// notification at all until the user allows it.
///
/// 2026-11-20 (product owner): after sign-in (and after any first-login
/// onboarding) the app shows Android's own two dialogs back to back, with
/// nothing of its own: the notification permission, then the one-tap "Let
/// app always run in background?" exemption (`ACTION_REQUEST_IGNORE_BATTERY_
/// OPTIMIZATIONS`, a Play-restricted permission declared in Play Console;
/// see docs/PLAY_BATTERY_EXEMPTION_DECLARATION.md). Neither can be granted
/// without the user's tap. The user is never redirected to a Settings
/// page. A refusal is asked again on a later launch, at most once every
/// [askInterval], while Android still allows the dialog.
class BatteryOptimization {
  BatteryOptimization._();

  static const channel = MethodChannel('hellodoctor/battery');
  static const lastAskedKey = 'delivery_readiness_last_asked_v1';
  static const permissionAskedKey = 'notification_permission_asked_v1';

  /// The flow is not repeated sooner than this after the user dismisses it.
  static const askInterval = Duration(hours: 20);

  /// Its own preferences file, so it never shares one with the session.
  static FlutterSecureStorage storage = const FlutterSecureStorage(
      aOptions: AndroidOptions(
          sharedPreferencesName: 'hellodoctor_battery_guidance'));

  /// Test seam for the clock.
  static DateTime Function() now = DateTime.now;

  /// The phone's current state, or null off Android or when unknown.
  static Future<DeliveryStatus?> status() async {
    if (kIsWeb || defaultTargetPlatform != TargetPlatform.android) return null;
    try {
      final raw = await channel.invokeMapMethod<String, Object?>('status');
      if (raw == null) return null;
      final notificationsEnabled = raw['notificationsEnabled'] != false;
      return DeliveryStatus(
        batteryRestricted: raw['ignoring'] != true,
        maker: phoneMakerOf('${raw['manufacturer'] ?? ''}'),
        notificationsEnabled: notificationsEnabled,
        permissionGranted:
            raw['permissionGranted'] as bool? ?? notificationsEnabled,
        showRationale: raw['showRationale'] == true,
        disabledChannels: [
          for (final id in (raw['disabledChannels'] as List?) ?? const []) '$id'
        ],
        sdk: (raw['sdk'] as num?)?.toInt() ?? 0,
      );
    } catch (_) {
      return null;
    }
  }

  static Future<bool> _invoke(String method) async {
    try {
      return await channel.invokeMethod<bool>(method) ?? false;
    } catch (_) {
      return false;
    }
  }

  /// Shows Android's one-tap "always run in background" dialog.
  static Future<bool> requestIgnore() => _invoke('requestIgnore');

  /// Whether Android will still show its notification-permission dialog:
  /// never asked yet, or refused once (the rationale case). After a second
  /// refusal Android stops showing it, and the app does not redirect the
  /// user to Settings (product owner, 2026-11-20).
  static Future<bool> canAskPermission(DeliveryStatus s) async {
    if (s.permissionGranted || s.sdk < 33) return false;
    if (s.showRationale) return true;
    return await storage.read(key: permissionAskedKey) == null;
  }

  /// Android's notification-permission dialog, then straight after it
  /// Android's one-tap battery-exemption dialog: two system taps and
  /// nothing of the app's own in between (product owner, 2026-11-20). Only
  /// what is still off is asked; at most once per [askInterval].
  /// [requestPermission] shows the permission dialog (the app's
  /// notification service). Never throws.
  static Future<void> ensureReady(BuildContext context,
      {required Future<bool> Function() requestPermission}) async {
    try {
      final last =
          DateTime.tryParse(await storage.read(key: lastAskedKey) ?? '');
      if (last != null && now().difference(last) < askInterval) return;
      var current = await status();
      if (current == null || !context.mounted) return;
      var asked = false;
      if (!current.permissionGranted && await canAskPermission(current)) {
        asked = true;
        await storage.write(key: permissionAskedKey, value: '1');
        await requestPermission();
        current = await status() ?? current;
      }
      if (current.batteryRestricted) {
        asked = true;
        await requestIgnore();
      }
      if (asked) {
        await storage.write(
            key: lastAskedKey, value: now().toUtc().toIso8601String());
      }
    } catch (_) {
      // Never block or break the screen that asked.
    }
  }
}

class DeliveryStatus {
  const DeliveryStatus({
    required this.batteryRestricted,
    required this.maker,
    this.notificationsEnabled = true,
    this.permissionGranted = true,
    this.showRationale = false,
    this.disabledChannels = const [],
    this.sdk = 0,
  });

  /// True when Android is optimising (restricting) this app's battery use.
  final bool batteryRestricted;
  final PhoneMaker maker;
  final bool notificationsEnabled, permissionGranted, showRationale;

  /// Channels the user switched off in the phone's settings.
  final List<String> disabledChannels;
  final int sdk;

  bool get notificationsOff =>
      !notificationsEnabled || disabledChannels.isNotEmpty;
  bool get allGood => !notificationsOff && !batteryRestricted;
}

enum PhoneMaker { samsung, transsion, xiaomi, oppo, vivo, huawei, other }

PhoneMaker phoneMakerOf(String manufacturer) {
  final m = manufacturer.toLowerCase().trim();
  if (m.contains('samsung')) return PhoneMaker.samsung;
  if (m.contains('tecno') ||
      m.contains('infinix') ||
      m.contains('itel') ||
      m.contains('transsion')) {
    return PhoneMaker.transsion;
  }
  if (m.contains('xiaomi') || m.contains('redmi') || m.contains('poco')) {
    return PhoneMaker.xiaomi;
  }
  if (m.contains('oppo') || m.contains('realme') || m.contains('oneplus')) {
    return PhoneMaker.oppo;
  }
  if (m.contains('vivo')) return PhoneMaker.vivo;
  if (m.contains('huawei') || m.contains('honor')) return PhoneMaker.huawei;
  return PhoneMaker.other;
}
