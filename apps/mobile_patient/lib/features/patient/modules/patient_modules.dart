import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../blogs/blogs_repository.dart';
import '../booking/booking_repository.dart';
import '../consultation/consultation_history_page.dart';
import '../directory/patient_directory_page.dart';
import '../facilities/facility_directory_page.dart';
import '../facilities/facility_repository.dart';
import '../home/brand_header.dart';
import '../home/home_cubit.dart';
import '../peer/peer_repository.dart';
import '../profile/family_profiles_cubit.dart';
import '../profile/profile_cubit.dart';
import '../qna/qna_repository.dart';
import '../reminders/reminders_page.dart';
import '../vault/vault_repository.dart';
import 'module_screens.dart';

/// Callbacks into `PatientShell` for destinations that are tabs rather than
/// pushed routes.
class PatientShellNav {
  const PatientShellNav(
      {required this.openServices,
      required this.openMessages,
      required this.openProfile,
      required this.select});
  final VoidCallback openServices, openMessages, openProfile;

  /// Selects a tab by index (0 Home, 1 Services, 2 Messages, 3 Profile).
  final ValueChanged<int> select;
}

/// One of the nine approved Patient modules, in the approved order
/// (v2.1 brief section 15.1).
class PatientModule {
  const PatientModule(
      {required this.icon,
      required this.color,
      required this.background,
      required this.label,
      required this.description,
      required this.open,
      this.availableOffline = false});
  final IconData icon;
  final Color color, background;
  final String label, description;
  final void Function(BuildContext context) open;

  /// Works without a connection: Reminders are stored on the device and
  /// Health Records has an offline cache.
  final bool availableOffline;
}

/// The single source of the nine modules and where each one leads. Home and
/// Services both render this list, so a module has exactly one entry point.
///
/// Modules whose feature has no backend (Peer Navigator, Reminders,
/// Facilities, Videos) open an explicit unavailable state: no sample data and
/// no control that pretends to work.
List<PatientModule> patientModules(BuildContext context, PatientShellNav nav) {
  final l = AppLocalizations.of(context);
  void push(BuildContext c, Widget page) =>
      Navigator.of(c).push(MaterialPageRoute(builder: (_) => page));
  return [
    PatientModule(
        icon: Icons.health_and_safety_outlined,
        color: PatientTokens.tileGreen,
        background: PatientTokens.tileGreenBg,
        label: l.moduleHivPrevention,
        description: l.moduleHivPreventionDesc,
        open: (c) => openEducation(c, nav)),
    PatientModule(
        icon: Icons.sms_rounded,
        color: PatientTokens.tileBlue,
        background: PatientTokens.tileBlueBg,
        label: l.moduleSupportChat,
        description: l.moduleSupportChatDesc,
        open: (c) => push(
            c,
            SupportChatPage(
                nav: nav,
                repo: QnaRepository(c.read<HomeCubit>().repo.api),
                familyProfilesCubit: c.read<FamilyProfilesCubit>()))),
    PatientModule(
        icon: Icons.event_note_rounded,
        color: PatientTokens.tileRed,
        background: PatientTokens.tileRedBg,
        label: l.moduleBookConsultation,
        description: l.moduleBookConsultationDesc,
        open: (c) {
          final homeRepo = c.read<HomeCubit>().repo;
          final family = c.read<FamilyProfilesCubit>();
          // The landing shows the patient's upcoming consultation, so it is
          // given the session-scoped cubits it reads (a pushed route sits
          // outside the tab's provider scope).
          push(
              c,
              MultiBlocProvider(
                  providers: [
                    BlocProvider.value(value: c.read<HomeCubit>()),
                    BlocProvider.value(value: family),
                    BlocProvider.value(value: c.read<ProfileCubit>()),
                  ],
                  child: VideoConsultationPage(
                      nav: nav,
                      onFindDoctor: (modality) => push(
                          c,
                          PatientDirectoryPage(
                              api: homeRepo.api,
                              cache: homeRepo.cache,
                              connectivity: homeRepo.connectivity!,
                              familyProfilesCubit: family,
                              initialModality: modality)),
                      onHistory: () => push(
                          c,
                          ConsultationHistoryPage(
                              repo: BookingRepository(homeRepo.api,
                                  connectivity: homeRepo.connectivity),
                              familyProfilesCubit: family,
                              myUserId: c
                                  .read<ProfileCubit>()
                                  .state
                                  .profile
                                  ?.userId)))));
        }),
    PatientModule(
        icon: Icons.people_alt_rounded,
        color: PatientTokens.tileGreen,
        background: PatientTokens.tileGreenBg,
        label: l.modulePeerNavigator,
        description: l.modulePeerNavigatorDesc,
        open: (c) {
          final homeRepo = c.read<HomeCubit>().repo;
          push(
              c,
              PeerNavigatorPage(
                  nav: nav,
                  repo: PatientPeerRepository(homeRepo.api,
                      connectivity: homeRepo.connectivity),
                  bookingRepo: BookingRepository(homeRepo.api,
                      connectivity: homeRepo.connectivity),
                  myUserId: c.read<ProfileCubit>().state.profile?.userId));
        }),
    PatientModule(
        icon: Icons.notifications_rounded,
        color: PatientTokens.tileAmber,
        background: PatientTokens.tileAmberBg,
        label: l.moduleReminders,
        description: l.moduleRemindersDesc,
        availableOffline: true,
        open: (c) {
          // Reminders are stored per account, so the account id is required.
          final userId = c.read<ProfileCubit>().state.profile?.userId;
          if (userId == null || userId.isEmpty) return;
          push(c, RemindersPage(nav: nav, userId: userId));
        }),
    PatientModule(
        icon: Icons.description_rounded,
        color: PatientTokens.tilePurple,
        background: PatientTokens.tilePurpleBg,
        label: l.moduleHealthRecords,
        description: l.moduleHealthRecordsDesc,
        availableOffline: true,
        open: (c) {
          final homeRepo = c.read<HomeCubit>().repo;
          push(
              c,
              HealthRecordsHubPage(
                  nav: nav,
                  repo: VaultRepository(homeRepo.api,
                      cache: homeRepo.cache,
                      connectivity: homeRepo.connectivity),
                  familyProfilesCubit: c.read<FamilyProfilesCubit>()));
        }),
    PatientModule(
        icon: Icons.play_circle_fill_rounded,
        color: PatientTokens.tilePurple,
        background: PatientTokens.tilePurpleBg,
        label: l.moduleVideosQuizzes,
        description: l.moduleVideosQuizzesDesc,
        open: (c) => openEducation(c, nav, initialTab: 1)),
    PatientModule(
        icon: Icons.location_on_rounded,
        color: PatientTokens.tileGreen,
        background: PatientTokens.tileGreenBg,
        label: l.moduleFacilities,
        description: l.moduleFacilitiesDesc,
        open: (c) => push(
            c,
            FacilityDirectoryPage(
                repo: FacilityRepository(c.read<HomeCubit>().repo.api),
                nav: nav,
                appHeader: AppHeaderDeps.of(c, nav)))),
    PatientModule(
        icon: Icons.more_horiz_rounded,
        color: PatientTokens.tileSlateBlue,
        background: PatientTokens.tileSlateBlueBg,
        label: l.moduleMore,
        description: l.moduleMoreDesc,
        open: (c) => push(c, MorePage(nav: nav))),
  ];
}

/// Opens the education hub (panel 2). `context` must be a tab context under
/// `_SignedInRoot`'s providers: the `ApiClient` is read off `HomeCubit`'s
/// repository, as the previous Home and Services pages did.
void openEducation(BuildContext context, PatientShellNav nav,
    {int initialTab = 0}) {
  final homeRepo = context.read<HomeCubit>().repo;
  Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => EducationHubPage(
          nav: nav,
          repo: BlogsRepository(homeRepo.api),
          initialTab: initialTab,
          appHeader: AppHeaderDeps.of(context, nav))));
}

/// Honest state for a module with no backend yet. It shows the module's own
/// icon and name and says plainly that nothing here is active.
class ModuleUnavailablePage extends StatelessWidget {
  const ModuleUnavailablePage(
      {super.key,
      required this.title,
      required this.icon,
      required this.color,
      required this.background});
  final String title;
  final IconData icon;
  final Color color, background;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final flat = title.replaceAll('\n', ' ');
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          AfyaBackHeader(flat),
          Expanded(
            child: Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(28),
                child: Column(mainAxisSize: MainAxisSize.min, children: [
                  AfyaIconTile(
                      icon: icon,
                      color: color,
                      background: background,
                      size: 84),
                  const SizedBox(height: 20),
                  Text(l.moduleUnavailableTitle,
                      textAlign: TextAlign.center,
                      style: const TextStyle(
                          fontSize: 19,
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text)),
                  const SizedBox(height: 8),
                  Text(l.moduleUnavailableBody(flat),
                      textAlign: TextAlign.center,
                      style: const TextStyle(
                          fontSize: 14,
                          height: 1.45,
                          color: PatientTokens.textSub)),
                  const SizedBox(height: 22),
                  OutlinedButton(
                      onPressed: () => Navigator.maybePop(context),
                      child: Text(l.moduleUnavailableBack)),
                ]),
              ),
            ),
          ),
        ]),
      ),
    );
  }
}

/// "More" (module 9): exactly three destinations per the v2.1 brief.
class MorePage extends StatelessWidget {
  const MorePage({super.key, required this.nav});
  final PatientShellNav nav;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          AfyaBackHeader(l.moduleMore),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(20, 8, 20, 24),
              children: [
                AfyaModuleRow(
                    icon: Icons.person_rounded,
                    color: PatientTokens.tileGreen,
                    background: PatientTokens.tileGreenBg,
                    title: l.moreProfileSettings,
                    onTap: () {
                      Navigator.of(context).pop();
                      nav.openProfile();
                    }),
                AfyaModuleRow(
                    icon: Icons.support_agent_rounded,
                    color: PatientTokens.tileBlue,
                    background: PatientTokens.tileBlueBg,
                    title: l.moreHelpSupport,
                    onTap: () => Navigator.of(context).push(MaterialPageRoute(
                        builder: (_) => ModuleUnavailablePage(
                            title: l.moreHelpSupport,
                            icon: Icons.support_agent_rounded,
                            color: PatientTokens.tileBlue,
                            background: PatientTokens.tileBlueBg)))),
                AfyaModuleRow(
                    icon: Icons.lock_rounded,
                    color: PatientTokens.tilePurple,
                    background: PatientTokens.tilePurpleBg,
                    title: l.morePrivacyPolicies,
                    onTap: () => Navigator.of(context).push(MaterialPageRoute(
                        builder: (_) => PrivacySecurityPage(nav: nav)))),
              ],
            ),
          ),
        ]),
      ),
    );
  }
}

/// Icon tile + title (+ subtitle) + chevron on a soft white card.
class AfyaModuleRow extends StatelessWidget {
  const AfyaModuleRow(
      {super.key,
      required this.icon,
      required this.color,
      required this.background,
      required this.title,
      required this.onTap,
      this.subtitle});
  final IconData icon;
  final Color color, background;
  final String title;
  final String? subtitle;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: Container(
          decoration: BoxDecoration(
              color: PatientTokens.surface,
              borderRadius: BorderRadius.circular(14),
              boxShadow: PatientTokens.softShadow),
          child: Material(
            color: Colors.transparent,
            borderRadius: BorderRadius.circular(14),
            child: InkWell(
              onTap: onTap,
              borderRadius: BorderRadius.circular(14),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 12, 8, 12),
                child: Row(children: [
                  AfyaIconTile(
                      icon: icon,
                      color: color,
                      background: background,
                      size: 44),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(title.replaceAll('\n', ' '),
                              style: const TextStyle(
                                  fontSize: 14,
                                  fontWeight: FontWeight.w700,
                                  color: PatientTokens.text)),
                          if (subtitle != null) ...[
                            const SizedBox(height: 2),
                            Text(subtitle!,
                                style: const TextStyle(
                                    fontSize: 12,
                                    height: 1.35,
                                    color: PatientTokens.textSub)),
                          ],
                        ]),
                  ),
                  const Icon(Icons.chevron_right_rounded,
                      color: PatientTokens.navInactive),
                ]),
              ),
            ),
          ),
        ),
      );
}
