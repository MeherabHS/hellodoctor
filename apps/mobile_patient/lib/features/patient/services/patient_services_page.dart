import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../directory/patient_directory_page.dart';
import '../chat/patient_chat_list_page.dart';
import '../home/home_cubit.dart';
import '../profile/family_profiles_cubit.dart';
import '../qna/qna_page.dart';
import '../qna/qna_repository.dart';
import '../vault/health_vault_page.dart';
import '../vault/vault_repository.dart';

/// Core care services. Chat opens the profile-scoped consultation list and
/// paid booking flow through the same repositories used by discovery.
class PatientServicesPage extends StatelessWidget {
  const PatientServicesPage({super.key});

  /// Reads the already-constructed `ApiClient`/`PatientCacheStore`/
  /// `ConnectivityService` off `HomeCubit`'s repository — see
  /// `patient_home_page.dart`'s identical `_openDirectory` for why this
  /// works here (same tab route, not a pushed one) and why it avoids
  /// threading three more constructor params through this page.
  void _openDirectory(BuildContext context) {
    final homeRepo = context.read<HomeCubit>().repo;
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => PatientDirectoryPage(
        api: homeRepo.api,
        cache: homeRepo.cache,
        connectivity: homeRepo.connectivity!,
        familyProfilesCubit: context.read<FamilyProfilesCubit>(),
      ),
    ));
  }

  /// Opens the real Health Vault (M7, gap #2) — same rationale as
  /// [_openDirectory] for reading services off `HomeCubit`'s repository.
  void _openHealthVault(BuildContext context) {
    final homeRepo = context.read<HomeCubit>().repo;
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => HealthVaultPage(
        repo: VaultRepository(homeRepo.api,
            cache: homeRepo.cache, connectivity: homeRepo.connectivity),
        familyProfilesCubit: context.read<FamilyProfilesCubit>(),
      ),
    ));
  }

  /// Opens the real Free Health Q&A feed (M8, gap #6) — same rationale as
  /// [_openDirectory] for reading services off `HomeCubit`'s repository.
  void _openQna(BuildContext context) {
    final homeRepo = context.read<HomeCubit>().repo;
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => QnaPage(
        repo: QnaRepository(homeRepo.api),
        familyProfilesCubit: context.read<FamilyProfilesCubit>(),
      ),
    ));
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final items = <(IconData, String, String, String)>[
      (
        Icons.event_available,
        'book_specialist',
        l10n.servicesBookSpecialist,
        l10n.servicesBookSpecialistDesc,
      ),
      (
        Icons.chat_bubble_outline,
        'chat_with_doctor',
        l10n.servicesChatWithDoctor,
        l10n.servicesChatWithDoctorDesc,
      ),
      (
        Icons.help_outline,
        'ask_free_question',
        l10n.servicesAskFreeQuestion,
        l10n.servicesAskFreeQuestionDesc,
      ),
      (
        Icons.folder_shared_outlined,
        'health_records',
        l10n.servicesHealthRecords,
        l10n.servicesHealthRecordsDesc,
      ),
      (
        Icons.local_pharmacy_outlined,
        'medicine_delivery',
        l10n.servicesMedicineDelivery,
        l10n.servicesMedicineDeliveryDesc,
      ),
      (
        Icons.emergency_outlined,
        'emergency_dispatch',
        l10n.servicesEmergencyDispatch,
        l10n.servicesEmergencyDispatchDesc,
      ),
    ];
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(
          children: [
            PatientPageHeader(l10n.servicesTitle, l10n.servicesSubtitle),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.all(18),
                children: [
                  for (final item in items)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 10),
                      child: InkWell(
                        onTap: switch (item.$2) {
                          'book_specialist' => () => _openDirectory(context),
                          'chat_with_doctor' => () => openPatientChats(context),
                          'health_records' => () => _openHealthVault(context),
                          'ask_free_question' => () => _openQna(context),
                          _ => () => ScaffoldMessenger.of(context).showSnackBar(
                                SnackBar(
                                    content:
                                        Text(l10n.comingSoonSnackbar(item.$3))),
                              ),
                        },
                        borderRadius:
                            BorderRadius.circular(PatientTokens.cardRadius),
                        child: PatientCard(
                          child: Row(children: [
                            Icon(item.$1, color: PatientTokens.primary),
                            const SizedBox(width: 12),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(item.$3,
                                      style: const TextStyle(
                                          fontWeight: FontWeight.w700,
                                          color: PatientTokens.text)),
                                  Text(item.$4,
                                      style: const TextStyle(
                                          fontSize: 12,
                                          color: PatientTokens.textSub)),
                                ],
                              ),
                            ),
                            const Icon(Icons.chevron_right,
                                color: PatientTokens.textSub),
                          ]),
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
