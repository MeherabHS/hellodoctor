// Debug-only design preview for Health Records (the Health Vault) with
// documents saved on the phone for offline use.
//
//   flutter run -t lib/dev/vault_preview_main.dart
//
// Sample documents answered in-process; nothing reaches a server. Refuses to
// run in release. `--dart-define=PREVIEW_OFFLINE=true` shows the phone
// offline.

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../core/network/api_client.dart';
import '../core/theme/tokens.dart';
import '../features/patient/profile/family_profiles_cubit.dart';
import '../features/patient/profile/family_profiles_repository.dart';
import '../features/patient/profile/profile_models.dart';
import '../features/patient/vault/health_vault_page.dart';
import '../features/patient/vault/vault_models.dart';
import '../features/patient/vault/vault_repository.dart';
import '../l10n/app_localizations.dart';

const _offline = bool.fromEnvironment('PREVIEW_OFFLINE');

class _NoTokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

ApiClient _api() => ApiClient(
    dio: Dio(BaseOptions(baseUrl: 'http://preview.invalid/api/v1')),
    tokenStore: _NoTokens());

String _ago(Duration d) => DateTime.now().toUtc().subtract(d).toIso8601String();

final _documents = [
  for (final (kind, id, page, ago) in [
    ('PRESCRIPTION', 'rx-1', 1, const Duration(days: 1)),
    ('INTAKE', 'up-1', 1, const Duration(days: 1, hours: 1)),
    ('INTAKE', 'up-2', 2, const Duration(days: 1, hours: 1)),
    ('PRESCRIPTION', 'rx-2', 1, const Duration(days: 9)),
  ])
    PatientDocumentDto.fromJson({
      'kind': kind,
      'id': id,
      'appointment_id': ago.inDays > 2 ? 'apt-2' : 'apt-1',
      'rx_number': kind == 'PRESCRIPTION' ? 'RX-SAMPLE-$page' : null,
      'page_number': page,
      'mime_type': 'image/jpeg',
      'doctor_display_name': 'Dr. Jane Mwangi (sample)',
      'primary_specialty': 'General Practice',
      'appointment_number': ago.inDays > 2 ? 'APT-SAMPLE-2' : 'APT-SAMPLE-1',
      'modality': ago.inDays > 2 ? 'VIDEO' : 'CHAT',
      'consultation_start': _ago(ago),
      'created_at': _ago(ago),
    }),
];

class _PreviewVault extends VaultRepository {
  _PreviewVault() : super(_api());
  @override
  bool get isOffline => _offline;
  @override
  Future<List<PatientDocumentDto>> documents({String? profileId}) async {
    if (_offline) throw const ApiException(null, 'Offline (preview).');
    return _documents;
  }

  @override
  Future<List<PatientDocumentDto>> cachedDocuments() async => _documents;

  /// Three of the four are saved on the phone.
  @override
  Future<Set<String>> savedDocumentIds() async => {'rx-1', 'up-1', 'rx-2'};
  @override
  Future<Set<String>> saveForOffline(List<PatientDocumentDto> documents) =>
      savedDocumentIds();
}

class _NoProfiles extends FamilyProfilesRepository {
  _NoProfiles() : super(_api());
  @override
  Future<List<PatientProfileDto>> list() async => const [];
  @override
  Future<List<PatientProfileDto>> cachedList() async => const [];
}

void main() {
  if (kReleaseMode) {
    runApp(const MaterialApp(
        home: Scaffold(
            body: Center(
                child: Text('The preview is not available in release.')))));
    return;
  }
  runApp(MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: PatientTokens.theme(),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Banner(
          message: 'PREVIEW DATA',
          location: BannerLocation.bottomStart,
          child: HealthVaultPage(
              repo: _PreviewVault(),
              familyProfilesCubit: FamilyProfilesCubit(_NoProfiles())))));
}
