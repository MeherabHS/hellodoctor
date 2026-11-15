import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:intl/intl.dart';

import '../../../core/l10n/kenya_time.dart';
import '../../../core/network/connectivity_service.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../../../core/network/screen_refresh.dart';
import '../directory/directory_repository.dart';
import '../directory/doctor_photo_avatar.dart';
import '../modules/afya_module_widgets.dart';
import '../modules/patient_modules.dart';
import '../profile/family_profiles_cubit.dart';
import '../profile/profile_cubit.dart';
import 'brand_header.dart';
import 'home_cubit.dart';
import 'home_models.dart';
import 'upcoming_consultation_section.dart';

/// Home dashboard, rebuilt to reference panel 1 of
/// `docs/design/afyamkononi-patient-ui-reference.jpeg`: brand header, greeting
/// card, search field, hero banner and the 3 x 3 grid of the nine modules,
/// all visible without scrolling on a 411 dp wide phone at text scale 1.0.
/// Narrower screens and larger text scroll.
///
/// Differences from the panel, each by product-owner decision:
/// - Card 3 reads "Book Consultation" (v2.1 brief, M03).
/// - The search field opens the Services directory. It does not search
///   doctors (v2.1 brief: no homepage doctor search).
///
/// The Emergency Contact banner and the family-profile switcher are not on
/// Home. The Upcoming Consultation card appears only inside the server's
/// consultation window, from 15 minutes before the start (CLAUDE.md rule
/// 2.4); it is re-evaluated every 30 seconds, and the appointment list is
/// reloaded every minute, on resume, on reconnect and when Home is shown
/// again (e.g. after booking).
class PatientHomePage extends StatefulWidget {
  const PatientHomePage({super.key, required this.nav});
  final PatientShellNav nav;

  @override
  State<PatientHomePage> createState() => _PatientHomePageState();
}

class _PatientHomePageState extends State<PatientHomePage> {
  StreamSubscription<NetworkStatus>? _network;
  ScreenRefresh? _appointmentsRefresh;
  Timer? _windowTick;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _appointmentsRefresh ??= ScreenRefresh(
        () => context.read<HomeCubit>().load(),
        interval: const Duration(minutes: 1),
        hasError: () => context.read<HomeCubit>().state.error != null);
    _appointmentsRefresh!.attach(context);
  }

  @override
  void dispose() {
    _network?.cancel();
    _appointmentsRefresh?.dispose();
    _windowTick?.cancel();
    super.dispose();
  }

  @override
  void initState() {
    super.initState();
    _network =
        context.read<HomeCubit>().repo.connectivity?.changes.listen((status) {
      if (!mounted) return;
      setState(() {});
      if (status == NetworkStatus.online) unawaited(_reload());
    });
    // Re-evaluates the card against the server window without a request, so
    // it appears at T-15 and its "Starts in" badge stays current.
    _windowTick = Timer.periodic(const Duration(seconds: 30), (_) {
      if (mounted) setState(() {});
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<HomeCubit>().load();
      context.read<ProfileCubit>().load();
      context.read<FamilyProfilesCubit>().load();
    });
  }

  Future<void> _reload() async {
    await Future.wait([
      context.read<HomeCubit>().load(),
      context.read<ProfileCubit>().load(),
      context.read<FamilyProfilesCubit>().load(),
    ]);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        // Keeps `HomeCubit`'s active profile in sync with the Family
        // Profiles switcher: switching re-queries Upcoming Consultation for
        // the newly active profile.
        child: BlocListener<FamilyProfilesCubit, FamilyProfilesState>(
          listenWhen: (previous, current) =>
              previous.activeProfileId != current.activeProfileId,
          listener: (context, state) =>
              context.read<HomeCubit>().setActiveProfile(state.activeProfileId),
          child: RefreshIndicator(
            onRefresh: _reload,
            child: BlocBuilder<ProfileCubit, ProfileState>(
              builder: (context, profileState) {
                return BlocBuilder<HomeCubit, HomeState>(
                  builder: (context, homeState) {
                    final name = profileState.profile?.fullName;
                    // Offline as soon as the device reports no connection,
                    // not only after a request has failed.
                    final offline = homeState.offline ||
                        (context
                                .read<HomeCubit>()
                                .repo
                                .connectivity
                                ?.isOffline ??
                            false);
                    return ListView(
                      padding: const EdgeInsets.only(bottom: 10),
                      children: [
                        AfyaBrandHeader(
                            deps: AppHeaderDeps.of(context, widget.nav)),
                        _GreetingCard(name: name),
                        _SearchField(onTap: widget.nav.openServices),
                        if (!offline && homeState.homeCardAppointment() != null)
                          _UpcomingCard(
                              appointment: homeState.homeCardAppointment()!,
                              onOpen: () => openWaitingRoomFor(
                                  context, homeState.homeCardAppointment()!)),
                        if (offline)
                          _OfflineBanner(onRetry: _reload)
                        else
                          _HeroBanner(
                              onExplore: () =>
                                  openEducation(context, widget.nav)),
                        _ServiceGrid(nav: widget.nav, offline: offline),
                        if (offline)
                          Padding(
                            padding: const EdgeInsets.fromLTRB(20, 12, 20, 0),
                            child: AfyaInfoCard(
                                icon: Icons.info_outline_rounded,
                                title: AppLocalizations.of(context)
                                    .offlineLimitedTitle,
                                body: AppLocalizations.of(context)
                                    .offlineLimitedBody,
                                background: PatientTokens.mintCard,
                                accent: PatientTokens.primaryDark),
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

class _GreetingCard extends StatelessWidget {
  const _GreetingCard({required this.name});
  final String? name;
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final first = (name ?? '').trim().split(RegExp(r'\s+')).first;
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 10, 20, 0),
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 13),
      decoration: BoxDecoration(
          color: PatientTokens.primaryLight,
          borderRadius: BorderRadius.circular(12)),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text(first.isEmpty ? l10n.homeHiDefault : l10n.homeHiNamed(first),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(
                fontSize: 19,
                fontWeight: FontWeight.w800,
                color: PatientTokens.text)),
        const SizedBox(height: 4),
        Text(l10n.homeGreetingSub,
            style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w500,
                color: PatientTokens.greetingSub)),
      ]),
    );
  }
}

/// The reference's search pill. It is an entry point to the Services
/// directory, not a text search: there is no search backend for services or
/// topics, and the v2.1 brief rules out a homepage doctor search.
class _SearchField extends StatelessWidget {
  const _SearchField({required this.onTap});
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.fromLTRB(20, 12, 20, 0),
        child: Semantics(
          button: true,
          child: Container(
            constraints: const BoxConstraints(minHeight: 44),
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
                  padding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
                  child: Row(children: [
                    const Icon(Icons.search_rounded,
                        size: 21, color: PatientTokens.navInactive),
                    const SizedBox(width: 8),
                    Expanded(
                        child: Text(AppLocalizations.of(context).homeSearchHint,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                                fontSize: 12.5, color: PatientTokens.hint))),
                  ]),
                ),
              ),
            ),
          ),
        ),
      );
}

class _HeroBanner extends StatelessWidget {
  const _HeroBanner({required this.onExplore});
  final VoidCallback onExplore;
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    const headline = TextStyle(
        fontSize: 17,
        height: 1.4,
        fontWeight: FontWeight.w800,
        color: PatientTokens.navy);
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      constraints: const BoxConstraints(minHeight: 150),
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(14),
          gradient: const LinearGradient(
              begin: Alignment.bottomLeft,
              end: Alignment.topRight,
              colors: [PatientTokens.heroSkyLight, PatientTokens.heroSky])),
      child: LayoutBuilder(builder: (context, constraints) {
        final artWidth = constraints.maxWidth * .40;
        return Stack(children: [
          Positioned(
              left: 0,
              bottom: 0,
              top: 0,
              width: artWidth,
              child: const _HeroArt()),
          Padding(
            padding: EdgeInsets.fromLTRB(artWidth + 8, 14, 14, 12),
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(l10n.homeHeroLine1, style: headline),
                  Text(l10n.homeHeroLine2, style: headline),
                  Text(l10n.homeHeroLine3, style: headline),
                  const SizedBox(height: 10),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton(
                      onPressed: onExplore,
                      style: FilledButton.styleFrom(
                          backgroundColor: PatientTokens.heroCta,
                          foregroundColor: Colors.white,
                          minimumSize: const Size(0, 36),
                          padding: const EdgeInsets.symmetric(
                              horizontal: 16, vertical: 8),
                          tapTargetSize: MaterialTapTargetSize.padded,
                          shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(10)),
                          textStyle: const TextStyle(
                              fontFamily: 'Plus Jakarta Sans',
                              fontSize: 12,
                              fontWeight: FontWeight.w700)),
                      child:
                          Text(l10n.homeHeroCta, textAlign: TextAlign.center),
                    ),
                  ),
                ]),
          ),
        ]);
      }),
    );
  }
}

/// The hero character: the product owner's high-resolution regeneration of
/// the reference's illustration, on a transparent background so it sits on
/// the banner gradient. Anchored bottom-left and never cropped at the head.
class _HeroArt extends StatelessWidget {
  const _HeroArt();
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(top: 6),
        child: Image.asset(
          'assets/illustrations/home_hero_girl.png',
          fit: BoxFit.cover,
          alignment: Alignment.topCenter,
          filterQuality: FilterQuality.medium,
          excludeFromSemantics: true,
        ),
      );
}

/// The nine modules as a 3 x 3 grid, each row sized to its tallest card.
class _ServiceGrid extends StatelessWidget {
  const _ServiceGrid({required this.nav, this.offline = false});
  final PatientShellNav nav;

  /// Offline design: modules that need the network are greyed out and
  /// explain themselves when tapped; the two that work offline are marked.
  final bool offline;
  @override
  Widget build(BuildContext context) {
    final modules = patientModules(context, nav);
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 12, 20, 4),
      // Measured here, outside the rows' IntrinsicHeight: each tile is a
      // third of the width less the two gaps, and its label has 4 px of
      // padding either side.
      child: LayoutBuilder(
          builder: (context, c) => Column(children: [
                for (var row = 0; row < 3; row++)
                  Padding(
                    padding: EdgeInsets.only(bottom: row == 2 ? 0 : 10),
                    child: IntrinsicHeight(
                      child: Row(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            for (var col = 0; col < 3; col++) ...[
                              if (col > 0) const SizedBox(width: 10),
                              Expanded(
                                  child: AfyaServiceTile(
                                      icon: modules[row * 3 + col].icon,
                                      color: modules[row * 3 + col].color,
                                      background:
                                          modules[row * 3 + col].background,
                                      label: modules[row * 3 + col].label,
                                      labelWidth: (c.maxWidth - 20) / 3 - 8,
                                      enabled: !offline ||
                                          modules[row * 3 + col]
                                              .availableOffline,
                                      badge: offline &&
                                              modules[row * 3 + col]
                                                  .availableOffline
                                          ? AppLocalizations.of(context)
                                              .offlineAvailable
                                          : null,
                                      onTap: () {
                                        final m = modules[row * 3 + col];
                                        if (offline && !m.availableOffline) {
                                          ScaffoldMessenger.of(context)
                                            ..hideCurrentSnackBar()
                                            ..showSnackBar(SnackBar(
                                                content: Text(AppLocalizations
                                                        .of(context)
                                                    .offlineNeedsInternet)));
                                        } else {
                                          m.open(context);
                                        }
                                      })),
                            ],
                          ]),
                    ),
                  ),
              ])),
    );
  }
}

/// Upcoming-consultation card of the product owner's Home design. It is
/// shown only while the patient has an upcoming or active consultation.
/// The button opens the waiting room, where joining stays gated by the
/// server's join window; it never starts a call directly.
class _UpcomingCard extends StatelessWidget {
  const _UpcomingCard({required this.appointment, required this.onOpen});
  final AppointmentSummaryDto appointment;
  final VoidCallback onOpen;

  String? _badge(AppLocalizations l) {
    if (appointment.hasActiveConsultation) return l.homeInProgress;
    // Server clock, not the device clock (clock skew must not move it).
    final now = appointment.join?.serverNow() ?? DateTime.now().toUtc();
    final minutes = appointment.startTime.toUtc().difference(now).inMinutes;
    if (minutes > 60) return null;
    return minutes <= 0 ? l.homeStartsNow : l.homeStartsInMin('$minutes');
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final a = appointment;
    final locale = Localizations.localeOf(context).toLanguageTag();
    final video = a.modality == 'VIDEO';
    final badge = _badge(l);
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
      decoration: BoxDecoration(
          color: const Color(0xFFE9F8F0),
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: const Color(0xFFCDEEDB))),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Row(children: [
          Icon(video ? Icons.videocam_rounded : Icons.chat_bubble_rounded,
              size: 24, color: PatientTokens.primaryDark),
          const SizedBox(width: 10),
          Expanded(
            child: Text(l.homeUpcomingConsultation,
                style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.navy)),
          ),
          if (badge != null)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
              decoration: BoxDecoration(
                  color: const Color(0xFFCDEEDB),
                  borderRadius: BorderRadius.circular(20),
                  border: Border.all(color: PatientTokens.primary)),
              child: Text(badge,
                  style: const TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w700,
                      color: PatientTokens.primaryDark)),
            ),
        ]),
        const SizedBox(height: 10),
        Row(children: [
          DoctorPhotoAvatar(
              doctorId: a.doctorId,
              initials: a.doctorInitials,
              hasPhoto: a.hasPhoto,
              photoUpdatedAt: a.photoUpdatedAt,
              repo: DirectoryRepository(context.read<HomeCubit>().repo.api),
              radius: 26),
          const SizedBox(width: 12),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(a.doctorDisplayName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 15.5,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.navy)),
              Text(a.primarySpecialty,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style:
                      const TextStyle(fontSize: 13, color: PatientTokens.ink)),
              const SizedBox(height: 4),
              Text(
                  '${DateFormat('EEE, d MMM y', locale).format(kenyaTime(a.startTime))}'
                  '  •  ${formatTimeEat(a.startTime)}',
                  style: const TextStyle(
                      fontSize: 12.5,
                      fontWeight: FontWeight.w600,
                      color: PatientTokens.ink)),
            ]),
          ),
        ]),
        const SizedBox(height: 12),
        FilledButton.icon(
            onPressed: onOpen,
            icon: Icon(video
                ? Icons.videocam_rounded
                : Icons.chat_bubble_outline_rounded),
            label:
                Text(video ? l.homeJoinConsultation : l.homeOpenConsultation),
            style: FilledButton.styleFrom(
                backgroundColor: PatientTokens.primaryDark,
                minimumSize: const Size.fromHeight(46),
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(14)))),
      ]),
    );
  }
}

/// Replaces the hero while the device has no connection (product owner's
/// offline design): what is wrong, what still works, and a retry.
class _OfflineBanner extends StatelessWidget {
  const _OfflineBanner({required this.onRetry});
  final Future<void> Function() onRetry;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
          color: const Color(0xFFE4F4FB),
          borderRadius: BorderRadius.circular(14)),
      child: IntrinsicHeight(
        child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Expanded(
            flex: 9,
            child: Image.asset('assets/illustrations/offline_hero.jpg',
                fit: BoxFit.cover,
                alignment: Alignment.bottomLeft,
                excludeFromSemantics: true),
          ),
          Expanded(
            flex: 11,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(10, 14, 14, 14),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Text(l.offlineTitle,
                        style: const TextStyle(
                            fontSize: 20,
                            fontWeight: FontWeight.w800,
                            color: PatientTokens.navy)),
                    const SizedBox(height: 4),
                    Text(l.offlineBody,
                        style: const TextStyle(
                            fontSize: 13,
                            height: 1.35,
                            color: PatientTokens.ink)),
                    const SizedBox(height: 10),
                    FilledButton.icon(
                        onPressed: onRetry,
                        icon: const Icon(Icons.refresh_rounded, size: 20),
                        label: Text(l.offlineTryAgain),
                        style: FilledButton.styleFrom(
                            backgroundColor: PatientTokens.primaryDark,
                            minimumSize: const Size(0, 42),
                            padding: const EdgeInsets.symmetric(horizontal: 16),
                            shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(10)))),
                  ]),
            ),
          ),
        ]),
      ),
    );
  }
}
