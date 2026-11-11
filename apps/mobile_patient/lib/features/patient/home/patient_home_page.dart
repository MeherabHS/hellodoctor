import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../../notifications/notification_center_page.dart';
import '../../notifications/notification_cubit.dart';
import '../booking/booking_repository.dart';
import '../booking/waiting_room_page.dart';
import '../directory/directory_repository.dart';
import '../directory/doctor_photo_avatar.dart';
import '../directory/patient_directory_page.dart';
import '../chat/patient_chat_list_page.dart';
import '../profile/add_dependent_sheet.dart';
import '../profile/family_profile_chip_row.dart';
import '../profile/family_profiles_cubit.dart';
import '../profile/profile_cubit.dart';
import '../qna/qna_page.dart';
import '../qna/qna_repository.dart';
import '../vault/health_vault_page.dart';
import '../vault/vault_repository.dart';
import 'emergency_contact_banner.dart';
import 'home_cubit.dart';
import 'home_models.dart';

/// `prototype/index.html` view-0 — "AESTHETIC HOME (4 core services)".
///
/// Directory, Health Vault (M7), and Q&A (M8, gap #6) are real now; Chat
/// remains a later-milestone gap — tapping it shows an honest "coming soon"
/// notice rather than navigating to nothing or a fake screen, per CLAUDE.md's
/// "No Phantom Completions" rule. The avatar tap is the one exception: the
/// prototype routes it to the Profile screen
/// (`onclick="switchScreen(7)"`), and that screen is now real (the Account
/// tab), so [onOpenAccountTab] switches to it directly instead of showing a
/// "coming soon" notice for a destination that already exists.
class PatientHomePage extends StatefulWidget {
  const PatientHomePage({super.key, this.onOpenAccountTab});
  final VoidCallback? onOpenAccountTab;

  @override
  State<PatientHomePage> createState() => _PatientHomePageState();
}

class _PatientHomePageState extends State<PatientHomePage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<HomeCubit>().load();
      context.read<ProfileCubit>().load();
      context.read<FamilyProfilesCubit>().load();
    });
  }

  void _comingSoon(String feature) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
          content:
              Text(AppLocalizations.of(context).comingSoonSnackbar(feature))),
    );
  }

  /// Opens the real specialist Directory (M4) — reads the already-constructed
  /// `ApiClient`/`PatientCacheStore`/`ConnectivityService` off `HomeCubit`'s
  /// own repository rather than threading three new constructor params
  /// through `PatientHomePage`, since `HomeRepository` already exposes them
  /// publicly and this page is a tab (not a pushed route), so `context.read`
  /// on cubits declared in `_SignedInRootState` works normally here.
  void _openChats() => openPatientChats(context);

  void _openDirectory() {
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

  /// Opens the Waiting Room (M5) for the Home card's single nearest upcoming
  /// appointment. Refreshes `HomeCubit` on return in case its status changed
  /// (e.g. completed) while the patient was there.
  Future<void> _openWaitingRoom(AppointmentSummaryDto appointment) async {
    final homeRepo = context.read<HomeCubit>().repo;
    await Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => WaitingRoomPage(
        appointment: appointment,
        repo: BookingRepository(homeRepo.api,
            connectivity: homeRepo.connectivity),
        familyProfilesCubit: context.read<FamilyProfilesCubit>(),
        myUserId: context.read<ProfileCubit>().state.profile?.userId,
      ),
    ));
    if (mounted) context.read<HomeCubit>().load();
  }

  /// Opens the real Health Vault (M7, gap #2).
  void _openHealthVault() {
    final homeRepo = context.read<HomeCubit>().repo;
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => HealthVaultPage(
        repo: VaultRepository(homeRepo.api,
            cache: homeRepo.cache, connectivity: homeRepo.connectivity),
        familyProfilesCubit: context.read<FamilyProfilesCubit>(),
      ),
    ));
  }

  /// Opens the real Free Health Q&A feed (M8, gap #6).
  void _openQna() {
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
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        // Keeps `HomeCubit`'s active profile in sync with the Family
        // Profiles switcher (§5b) — switching re-queries Upcoming
        // Consultation for the newly active profile, without the page
        // itself needing to know the switcher ever changed.
        child: BlocListener<FamilyProfilesCubit, FamilyProfilesState>(
          listenWhen: (previous, current) =>
              previous.activeProfileId != current.activeProfileId,
          listener: (context, state) =>
              context.read<HomeCubit>().setActiveProfile(state.activeProfileId),
          child: RefreshIndicator(
            onRefresh: () async {
              await Future.wait([
                context.read<HomeCubit>().load(),
                context.read<ProfileCubit>().load(),
                context.read<FamilyProfilesCubit>().load(),
              ]);
            },
            child: BlocBuilder<ProfileCubit, ProfileState>(
              builder: (context, profileState) {
                return BlocBuilder<HomeCubit, HomeState>(
                  builder: (context, homeState) {
                    return ListView(
                      padding: const EdgeInsets.only(bottom: 24),
                      children: [
                        _Greeting(
                          name: profileState.profile?.fullName,
                          onAvatarTap: widget.onOpenAccountTab ??
                              () => _comingSoon(l10n.accountLabel),
                        ),
                        const SizedBox(height: 14),
                        _CoreServicesGrid(
                            onOpenDirectory: _openDirectory,
                            onOpenHealthVault: _openHealthVault,
                            onOpenQna: _openQna,
                            onOpenChats: _openChats),
                        const EmergencyContactBanner(),
                        const _ProfileSwitcherBar(),
                        _UpcomingConsultationSection(
                          state: homeState,
                          onManage: homeState.nextUpcoming == null
                              ? () {}
                              : () => _openWaitingRoom(homeState.nextUpcoming!),
                        ),
                      ],
                    );
                  },
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}

/// Compact "Managing for: X" entry point to the Family Profiles switcher
/// (§5b) — the plan's "reachable from Home" requirement, placed directly
/// above Upcoming Consultation so it's obvious which profile that card is
/// scoped to. Hidden until the list has actually loaded, rather than
/// flashing an empty/self-only bar before the first response lands.
class _ProfileSwitcherBar extends StatelessWidget {
  const _ProfileSwitcherBar();

  @override
  Widget build(BuildContext context) {
    return BlocBuilder<FamilyProfilesCubit, FamilyProfilesState>(
      builder: (context, state) {
        if (state.profiles.isEmpty) return const SizedBox.shrink();
        final active = state.activeProfile;
        final l10n = AppLocalizations.of(context);
        final label = active == null
            ? l10n.homeManagingForYou
            : '${l10n.homeManagingForName(active.fullName)}'
                '${active.isSelf ? '' : ' (${active.relationship ?? l10n.homeDependentFallback})'}';
        return Padding(
          padding: const EdgeInsets.fromLTRB(18, 10, 18, 0),
          child: InkWell(
            onTap: () => _openSwitcher(context),
            borderRadius: BorderRadius.circular(PatientTokens.chipRadius),
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              decoration: BoxDecoration(
                color: PatientTokens.surface,
                borderRadius: BorderRadius.circular(PatientTokens.chipRadius),
                border: Border.all(color: PatientTokens.border),
              ),
              child: Row(children: [
                Icon(
                    active == null || active.isSelf
                        ? Icons.person
                        : Icons.group_outlined,
                    size: 16,
                    color: PatientTokens.primaryDark),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(label,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.w700,
                          color: PatientTokens.text)),
                ),
                const Icon(Icons.unfold_more,
                    size: 16, color: PatientTokens.textSub),
              ]),
            ),
          ),
        );
      },
    );
  }

  void _openSwitcher(BuildContext pageContext) {
    final cubit = pageContext.read<FamilyProfilesCubit>();
    showModalBottomSheet(
      context: pageContext,
      isScrollControlled: true,
      builder: (sheetContext) => BlocProvider.value(
        value: cubit,
        child: _ProfileSwitcherSheet(
          onAddDependent: () {
            Navigator.of(sheetContext).pop();
            showAddDependentSheet(pageContext);
          },
        ),
      ),
    );
  }
}

class _ProfileSwitcherSheet extends StatelessWidget {
  const _ProfileSwitcherSheet({required this.onAddDependent});
  final VoidCallback onAddDependent;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(0, 20, 0, 24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: Text(AppLocalizations.of(context).homeSwitchProfile,
                style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
          ),
          const SizedBox(height: 14),
          BlocBuilder<FamilyProfilesCubit, FamilyProfilesState>(
            builder: (context, state) => FamilyProfileChipRow(
              profiles: state.profiles,
              activeProfileId: state.activeProfileId,
              onSelect: (id) {
                context.read<FamilyProfilesCubit>().setActiveProfile(id);
                Navigator.of(context).pop();
              },
              onAddDependent: onAddDependent,
            ),
          ),
        ],
      ),
    );
  }
}

class _Greeting extends StatelessWidget {
  const _Greeting({required this.name, required this.onAvatarTap});
  final String? name;
  final VoidCallback onAvatarTap;

  void _openNotifications(BuildContext context) {
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) =>
          NotificationCenterPage(cubit: context.read<NotificationCubit>()),
    ));
  }

  @override
  Widget build(BuildContext context) {
    final displayName = (name == null || name!.isEmpty) ? '' : name!;
    final l10n = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(18, 14, 18, 0),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(l10n.homeWelcomeBack,
                    style: const TextStyle(
                        fontSize: 13, color: PatientTokens.textSub)),
                Text(
                  displayName.isEmpty
                      ? l10n.homeGreetingDefault
                      : l10n.homeGreetingNamed(displayName),
                  style: const TextStyle(
                      fontSize: 19,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.text),
                ),
              ],
            ),
          ),
          BlocBuilder<NotificationCubit, NotificationState>(
            builder: (context, state) => Padding(
              padding: const EdgeInsets.only(right: 8),
              child: NotificationBellAction(
                unreadCount: state.unreadCount,
                onTap: () => _openNotifications(context),
              ),
            ),
          ),
          InkWell(
            onTap: onAvatarTap,
            borderRadius: BorderRadius.circular(22),
            child: const CircleAvatar(
              radius: 20,
              backgroundColor: PatientTokens.primaryLight,
              child: Icon(Icons.person, color: PatientTokens.primaryDark),
            ),
          ),
        ],
      ),
    );
  }
}


class _CoreServiceItem {
  const _CoreServiceItem({
    required this.icon,
    required this.iconBg,
    required this.iconFg,
    required this.badge,
    required this.badgeBg,
    required this.badgeFg,
    required this.title,
    required this.description,
    required this.onTap,
  });
  final IconData icon;
  final Color iconBg, iconFg, badgeBg, badgeFg;
  final String badge, title, description;
  final VoidCallback onTap;
}

/// `prototype/index.html` view-0's `.core-services-2x2` grid
/// (`.grid-service-card`) — each card is an icon chip, a colored status
/// badge, a title and a description, not just an icon-and-label tile. Copy
/// is the prototype's exact text except the Chat card's fee badge: the
/// prototype hardcodes "৳ 300 Fee" (Bangladesh taka, the prototype's
/// original market), but no Kenya KES consultation fee is specified
/// anywhere in `docs/` — inventing a number here would violate CLAUDE.md's
/// "No Unilateral Feature Inventions" rule, so this uses the honest, number-
/// free "Paid" badge instead.
class _CoreServicesGrid extends StatelessWidget {
  const _CoreServicesGrid(
      {required this.onOpenDirectory,
      required this.onOpenHealthVault,
      required this.onOpenQna,
      required this.onOpenChats});
  final VoidCallback onOpenChats;
  final VoidCallback onOpenDirectory, onOpenHealthVault, onOpenQna;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final items = <_CoreServiceItem>[
      _CoreServiceItem(
        icon: Icons.calendar_month_outlined,
        iconBg: PatientTokens.primaryLight,
        iconFg: PatientTokens.primaryDark,
        badge: l10n.homeServiceSpecialistBadge,
        badgeBg: PatientTokens.primaryLight,
        badgeFg: PatientTokens.primaryDark,
        title: l10n.homeServiceBookSpecialist,
        description: l10n.homeServiceBookSpecialistDesc,
        onTap: onOpenDirectory,
      ),
      _CoreServiceItem(
        icon: Icons.chat_bubble_outline,
        iconBg: PatientTokens.warningLight,
        iconFg: PatientTokens.amberText,
        badge: l10n.homeServicePaidBadge,
        badgeBg: PatientTokens.warningLight,
        badgeFg: PatientTokens.amberText,
        title: l10n.chatWithDoctorLabel,
        description: l10n.homeServiceChatDesc,
        onTap: onOpenChats,
      ),
      _CoreServiceItem(
        icon: Icons.health_and_safety_outlined,
        iconBg: PatientTokens.surfaceAlt,
        iconFg: PatientTokens.primary,
        badge: l10n.homeServiceFreeBadge,
        badgeBg: PatientTokens.surfaceAlt,
        badgeFg: PatientTokens.primaryDark,
        title: l10n.homeServiceQnaTitle,
        description: l10n.homeServiceQnaDesc,
        onTap: onOpenQna,
      ),
      _CoreServiceItem(
        icon: Icons.folder_shared_outlined,
        iconBg: PatientTokens.roseLight,
        iconFg: PatientTokens.roseText,
        badge: l10n.homeServiceVaultBadge,
        badgeBg: PatientTokens.roseLight,
        badgeFg: PatientTokens.roseText,
        title: l10n.homeServiceVaultTitle,
        description: l10n.homeServiceVaultDesc,
        onTap: onOpenHealthVault,
      ),
    ];
    return Padding(
      padding: const EdgeInsets.fromLTRB(18, 14, 18, 0),
      child: LayoutBuilder(
          builder: (context, constraints) => Wrap(
                spacing: 10,
                runSpacing: 10,
                children: [
                  for (final item in items)
                    SizedBox(
                        width: (constraints.maxWidth - 10) / 2,
                        child: InkWell(
                          onTap: item.onTap,
                          borderRadius:
                              BorderRadius.circular(PatientTokens.cardRadius),
                          child: PatientCard(
                            child: Column(
                              mainAxisSize: MainAxisSize.min,
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Row(
                                  mainAxisAlignment:
                                      MainAxisAlignment.spaceBetween,
                                  children: [
                                    Container(
                                      width: 38,
                                      height: 38,
                                      decoration: BoxDecoration(
                                          color: item.iconBg,
                                          shape: BoxShape.circle),
                                      alignment: Alignment.center,
                                      child: Icon(item.icon,
                                          color: item.iconFg, size: 20),
                                    ),
                                    const SizedBox(width: 8),
                                    Flexible(
                                        child: StatusChip(item.badge,
                                            color: item.badgeBg,
                                            textColor: item.badgeFg)),
                                  ],
                                ),
                                const SizedBox(height: 10),
                                Text(item.title,
                                    style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                        color: PatientTokens.text,
                                        fontSize: 13)),
                                const SizedBox(height: 2),
                                Text(item.description,
                                    style: const TextStyle(
                                        fontSize: 11,
                                        color: PatientTokens.textSub)),
                              ],
                            ),
                          ),
                        )),
                ],
              )),
    );
  }
}

class _UpcomingConsultationSection extends StatelessWidget {
  const _UpcomingConsultationSection(
      {required this.state, required this.onManage});
  final HomeState state;
  final VoidCallback onManage;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(18, 10, 18, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Expanded(
                  child: Text(l10n.homeUpcomingConsultation,
                      style: const TextStyle(
                          fontSize: 15,
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text))),
              if (state.nextUpcoming != null)
                TextButton(
                    onPressed: onManage, child: Text(l10n.homeManageButton)),
            ],
          ),
          if (state.loading)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Column(children: [
                SkeletonBlock(height: 70),
              ]),
            )
          else if (state.error != null && state.nextUpcoming == null)
            InlineError(state.error!)
          else if (state.nextUpcoming == null)
            PatientCard(
              child: Text(l10n.homeNoUpcoming,
                  style: const TextStyle(color: PatientTokens.textSub)),
            )
          else
            _ConsultCard(
                appointment: state.nextUpcoming!,
                onOpen: onManage,
                repo: DirectoryRepository(context.read<HomeCubit>().repo.api)),
        ],
      ),
    );
  }
}

class _ConsultCard extends StatelessWidget {
  const _ConsultCard(
      {required this.appointment, required this.onOpen, required this.repo});
  final AppointmentSummaryDto appointment;
  final VoidCallback onOpen;
  final DirectoryRepository repo;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final start = appointment.startTime;
    final time = formatTime12h(start);
    return PatientCard(
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        if (appointment.hasActiveConsultation)
          Container(
              padding: const EdgeInsets.all(12),
              margin: const EdgeInsets.only(bottom: 10),
              decoration: BoxDecoration(
                  color: const Color(0xFFDCFCE7),
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(color: const Color(0xFF86EFAC))),
              child: Row(children: [
                const Icon(Icons.videocam, color: Color(0xFF166534)),
                const SizedBox(width: 8),
                const Expanded(
                    child: Text('Active Consultation',
                        style: TextStyle(
                            color: Color(0xFF166534),
                            fontWeight: FontWeight.w800))),
                FilledButton(
                    style: FilledButton.styleFrom(
                        backgroundColor: const Color(0xFF16A34A)),
                    onPressed: onOpen,
                    child: const Text('Rejoin'))
              ])),
        Row(children: [
          DoctorPhotoAvatar(
            doctorId: appointment.doctorId,
            initials: appointment.doctorInitials,
            hasPhoto: appointment.hasPhoto,
            photoUpdatedAt: appointment.photoUpdatedAt,
            repo: repo,
            radius: 22,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(appointment.doctorDisplayName,
                    style: const TextStyle(
                        fontWeight: FontWeight.w800,
                        color: PatientTokens.text)),
                Text(appointment.primarySpecialty,
                    style: const TextStyle(
                        fontSize: 12, color: PatientTokens.textSub)),
                Text(
                    l10n.homeConsultCardLine(
                        '${start.day}',
                        '${start.month}',
                        time,
                        appointment.modality == 'VIDEO'
                            ? l10n.modalityVideo
                            : l10n.modalityChat),
                    style: const TextStyle(
                        fontSize: 11, color: PatientTokens.textSub)),
              ],
            ),
          ),
          StatusChip(_statusLabel(l10n, appointment.status)),
        ]),
        if (appointment.isWaitingForReconnect)
          Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                  appointment.doctorPresent == false &&
                          appointment.patientPresent == true
                      ? 'Doctor is reconnecting. Your consultation remains active.'
                      : appointment.doctorPresent == true &&
                              appointment.patientPresent == false
                          ? 'You are disconnected. Rejoin to continue the consultation.'
                          : 'The room is empty. Either participant can rejoin now.',
                  style: const TextStyle(
                      color: Color(0xFF166534),
                      fontSize: 11,
                      fontWeight: FontWeight.w600))),
      ]),
    );
  }

  String _statusLabel(AppLocalizations l10n, String status) => switch (status) {
        'PENDING_PAYMENT' => l10n.apptStatusPendingPayment,
        'CONFIRMED' => l10n.apptStatusConfirmed,
        'WAITING' => l10n.apptStatusWaiting,
        'IN_CONSULTATION' => l10n.apptStatusInConsultation,
        _ => status,
      };
}
