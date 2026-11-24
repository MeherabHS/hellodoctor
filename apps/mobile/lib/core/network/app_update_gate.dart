import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:url_launcher/url_launcher.dart';

import 'api_client.dart';

/// Asks the doctor to update when this build is older than the server's
/// minimum (`GET /config/app-version`, `DOCTOR_APP_MIN_BUILD`): raised after
/// a release whose rules an old app would get wrong. Best effort: offline, an
/// older backend or a debug build without a build number change nothing.
Future<void> checkDoctorAppUpdate(BuildContext context, ApiClient api) async {
  try {
    final response = await api.get('/config/app-version');
    final body = response.data;
    final minBuild = body is Map && body['data'] is Map
        ? int.tryParse('${body['data']['doctor_app_min_build']}') ?? 0
        : 0;
    if (minBuild <= 0) return;
    final info = await const MethodChannel('hellodoctor/build_metadata')
        .invokeMapMethod<String, String>('get')
        .timeout(const Duration(seconds: 2));
    final build = int.tryParse(info?['build_number'] ?? '') ?? 0;
    if (!appUpdateRequired(build, minBuild) || !context.mounted) return;
    await showDialog<void>(
        context: context,
        barrierDismissible: false,
        builder: (_) => const AppUpdateRequiredDialog());
  } catch (_) {
    // Never block the app because the check itself failed.
  }
}

/// A known build older than the minimum.
bool appUpdateRequired(int build, int minBuild) =>
    minBuild > 0 && build > 0 && build < minBuild;

class AppUpdateRequiredDialog extends StatelessWidget {
  const AppUpdateRequiredDialog({super.key});

  static final _store = Uri.parse('market://details?id=com.hellodoctor.doctor');
  static final _web = Uri.parse(
      'https://play.google.com/store/apps/details?id=com.hellodoctor.doctor');

  @override
  Widget build(BuildContext context) => PopScope(
      canPop: false,
      child: AlertDialog(
          title: const Text('Update required'),
          content: const Text(
              'This version of the app is no longer supported. Update it from Google Play to keep sending prescriptions and seeing patients.'),
          actions: [
            FilledButton(
                onPressed: () async {
                  if (!await launchUrl(_store,
                      mode: LaunchMode.externalApplication)) {
                    await launchUrl(_web, mode: LaunchMode.externalApplication);
                  }
                },
                child: const Text('Update')),
          ]));
}
