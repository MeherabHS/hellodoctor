import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import '../../l10n/app_localizations.dart';

/// Battery-optimisation guidance (2026-11-18, product-owner decision).
///
/// Many Android phones sold in Kenya (Tecno, Infinix, itel, Xiaomi, Oppo,
/// Vivo...) stop background apps to save battery, which delays or drops FCM
/// pushes and reminders. Once per install, after sign-in, an Android user
/// whose phone restricts this app is shown how to allow background running
/// on their brand of phone, with a button to the app's settings page.
///
/// The one-tap system dialog (`ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`)
/// is deliberately not used: it needs the Play-restricted
/// `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` permission.
class BatteryOptimization {
  BatteryOptimization._();

  static const channel = MethodChannel('hellodoctor/battery');
  static const promptedKey = 'battery_optimization_prompt_shown_v1';

  /// Its own preferences file, so it never shares one with the session.
  static FlutterSecureStorage storage = const FlutterSecureStorage(
      aOptions: AndroidOptions(
          sharedPreferencesName: 'hellodoctor_battery_guidance'));

  /// The phone's current state, or null off Android or when unknown.
  static Future<BatteryStatus?> status() async {
    if (kIsWeb || defaultTargetPlatform != TargetPlatform.android) return null;
    try {
      final raw = await channel.invokeMapMethod<String, Object?>('status');
      if (raw == null) return null;
      return BatteryStatus(
        restricted: raw['ignoring'] != true,
        maker: phoneMakerOf('${raw['manufacturer'] ?? ''}'),
      );
    } catch (_) {
      return null;
    }
  }

  /// Opens this app's settings page (battery and auto-start live there).
  static Future<bool> openSettings() async {
    try {
      return await channel.invokeMethod<bool>('openSettings') ?? false;
    } catch (_) {
      return false;
    }
  }

  /// Shows the guidance once per install, and only when the phone is
  /// currently restricting the app. Never throws.
  static Future<void> maybePrompt(BuildContext context) async {
    try {
      if (await storage.read(key: promptedKey) != null) return;
      final current = await status();
      if (current == null || !current.restricted) return;
      if (!context.mounted) return;
      await storage.write(key: promptedKey, value: '1');
      if (!context.mounted) return;
      final open = await showDialog<bool>(
        context: context,
        builder: (_) => BatteryOptimizationDialog(maker: current.maker),
      );
      if (open == true) await openSettings();
    } catch (_) {
      // Guidance only: never block or break the screen that asked.
    }
  }
}

class BatteryStatus {
  const BatteryStatus({required this.restricted, required this.maker});

  /// True when Android is optimising (restricting) this app's battery use.
  final bool restricted;
  final PhoneMaker maker;
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

/// The steps for this brand of phone, in the user's language.
String batteryStepsFor(AppLocalizations? l10n, PhoneMaker maker) =>
    switch (maker) {
      PhoneMaker.samsung => l10n?.batteryStepsSamsung ??
          'Tap Open settings, then Battery, and choose Unrestricted. Also check that this app is not under Sleeping apps (Settings > Battery > Background usage limits).',
      PhoneMaker.transsion => l10n?.batteryStepsTranssion ??
          'Tap Open settings, then Battery, and allow background running (turn battery optimisation off). In Phone Master, allow this app to auto-start.',
      PhoneMaker.xiaomi => l10n?.batteryStepsXiaomi ??
          'Tap Open settings, then Battery saver, and choose No restrictions. Then turn on Autostart.',
      PhoneMaker.oppo => l10n?.batteryStepsOppo ??
          'Tap Open settings, then Battery, and turn on Allow background activity. Also allow Auto launch.',
      PhoneMaker.vivo => l10n?.batteryStepsVivo ??
          'Tap Open settings, then Battery, and allow high background power consumption. Also allow Auto-start.',
      PhoneMaker.huawei => l10n?.batteryStepsHuawei ??
          'Tap Open settings, then Battery, then App launch. Set this app to Manage manually and turn on all three switches.',
      PhoneMaker.other => l10n?.batteryStepsOther ??
          'Tap Open settings, then Battery, and choose Unrestricted (or Don\'t optimise).',
    };

class BatteryOptimizationDialog extends StatelessWidget {
  const BatteryOptimizationDialog({super.key, required this.maker});
  final PhoneMaker maker;

  @override
  Widget build(BuildContext context) {
    // Falls back to English where no localizations are installed.
    final l10n = Localizations.of<AppLocalizations>(context, AppLocalizations);
    return AlertDialog(
      key: const ValueKey('battery-optimization-dialog'),
      title: Text(l10n?.batteryPromptTitle ?? 'Get reminders on time'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(l10n?.batteryPromptBody ??
                'Your phone may stop this app in the background to save battery. That can delay or block consultation reminders and messages. To get them on time, allow the app to run in the background.'),
            const SizedBox(height: 12),
            Text(batteryStepsFor(l10n, maker),
                style: const TextStyle(fontWeight: FontWeight.w600)),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n?.batteryPromptLater ?? 'Not now'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(l10n?.batteryPromptOpenSettings ?? 'Open settings'),
        ),
      ],
    );
  }
}
