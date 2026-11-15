import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/cache/patient_cache_store.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/connectivity_service.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../core/l10n/patient_labels.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';
import '../booking/paid_chat_booking_modal.dart';
import '../booking/patient_booking_page.dart';
import '../profile/family_profiles_cubit.dart';
import 'directory_cubit.dart';
import 'directory_models.dart';
import 'directory_repository.dart';
import 'doctor_profile_page.dart';
import 'consultation_actions.dart';

class PatientDirectoryPage extends StatefulWidget {
  const PatientDirectoryPage(
      {super.key,
      required this.api,
      required this.cache,
      required this.connectivity,
      required this.familyProfilesCubit,
      this.initialModality = 'all'});
  final String initialModality;
  final ApiClient api;
  final PatientCacheStore? cache;
  final ConnectivityService connectivity;
  final FamilyProfilesCubit familyProfilesCubit;
  @override
  State<PatientDirectoryPage> createState() => _PatientDirectoryPageState();
}

class _PatientDirectoryPageState extends State<PatientDirectoryPage> {
  late final DirectoryCubit _cubit;

  /// Server-controlled (`GET /config/booking-policy`). Defaults to `true`
  /// (today's paid behavior) until the policy is known; only an explicit
  /// `payment_required: false` unlocks unpriced-doctor booking.
  bool _paymentRequired = true;
  static const specialties = [
    'all',
    'Internal Medicine',
    'Cardiology',
    'Pediatrics',
    'Gynecology',
    'Dermatology'
  ];
  @override
  void initState() {
    super.initState();
    _cubit = DirectoryCubit(
        DirectoryRepository(widget.api,
            cache: widget.cache, connectivity: widget.connectivity),
        connectivity: widget.connectivity);
    unawaited(_loadPolicy());
    if (widget.initialModality == 'all') {
      _cubit.load();
    } else {
      _cubit.setModalityTab(widget.initialModality);
    }
  }

  @override
  void dispose() {
    _cubit.close();
    super.dispose();
  }

  Future<void> _loadPolicy() async {
    try {
      final policy =
          await BookingRepository(widget.api, connectivity: widget.connectivity)
              .bookingPolicy();
      if (mounted) setState(() => _paymentRequired = policy.paymentRequired);
    } catch (_) {
      // Keep the safe paid default; the booking page re-checks the policy.
    }
  }

  void profile(PublicDoctorDto d) =>
      Navigator.of(context).push(MaterialPageRoute(
          builder: (_) => DoctorProfilePage(
              doctorId: d.id,
              api: widget.api,
              cache: widget.cache,
              connectivity: widget.connectivity,
              familyProfilesCubit: widget.familyProfilesCubit)));
  void book(PublicDoctorDto d, String modality) {
    final repo =
        BookingRepository(widget.api, connectivity: widget.connectivity);
    if (modality == 'CHAT') {
      showPaidChatBookingModal(context,
          doctor: d,
          repo: repo,
          familyProfilesCubit: widget.familyProfilesCubit);
    } else {
      Navigator.of(context).push(MaterialPageRoute(
          builder: (_) => PatientBookingPage(
              doctor: d,
              modality: modality,
              repo: repo,
              familyProfilesCubit: widget.familyProfilesCubit)));
    }
  }

  @override
  Widget build(BuildContext context) {
    return BlocProvider.value(
      value: _cubit,
      child: Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
          child: BlocBuilder<DirectoryCubit, DirectoryState>(
            builder: (context, state) => Column(
              children: [
                Padding(
                  padding: const EdgeInsets.only(bottom: 14),
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        _DirectoryHeader(
                            title: AppLocalizations.of(context).discoveryTitle,
                            subtitle:
                                AppLocalizations.of(context).discoverySubtitle),
                        const SizedBox(height: 14),
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 20),
                          child: TextField(
                              onChanged: _cubit.setSearch,
                              decoration: InputDecoration(
                                  hintText: AppLocalizations.of(context)
                                      .discoverySearch,
                                  prefixIcon: const Icon(Icons.search_rounded),
                                  enabledBorder: OutlineInputBorder(
                                      borderRadius: BorderRadius.circular(18),
                                      borderSide: const BorderSide(
                                          color: PatientTokens.border)),
                                  border: OutlineInputBorder(
                                      borderRadius:
                                          BorderRadius.circular(18)))),
                        ),
                        const SizedBox(height: 12),
                        DoctorDirectoryFilters(
                          specialty: state.specialty == 'all'
                              ? AppLocalizations.of(context).filterSpecialty
                              : patientSpecialtyLabel(
                                  AppLocalizations.of(context),
                                  state.specialty),
                          availability: state.onDutyOnly
                              ? AppLocalizations.of(context).filterOnDuty
                              : AppLocalizations.of(context).filterAvailability,
                          visitType: state.modalityTab == 'all'
                              ? AppLocalizations.of(context).filterVisitType
                              : state.modalityTab == 'chat'
                                  ? AppLocalizations.of(context).consultChat
                                  : AppLocalizations.of(context).consultVideo,
                          onSpecialty: () => _showSpecialties(context, state),
                          onAvailability: () =>
                              _showAvailability(context, state),
                          onVisitType: () => _showVisitTypes(context, state),
                        ),
                      ]),
                ),
                Expanded(
                    child: RefreshIndicator(
                        onRefresh: _cubit.load, child: _list(context, state))),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _showSpecialties(
      BuildContext context, DirectoryState state) async {
    final selected = await showModalBottomSheet<String>(
        context: context,
        showDragHandle: true,
        builder: (_) => SafeArea(
            child: Column(
                mainAxisSize: MainAxisSize.min,
                children: specialties
                    .map((s) => ListTile(
                        title: Text(s == 'all'
                            ? 'All specialties'
                            : patientSpecialtyLabel(
                                AppLocalizations.of(context), s)),
                        trailing: state.specialty == s
                            ? const Icon(Icons.check,
                                color: PatientTokens.primary)
                            : null,
                        onTap: () => Navigator.pop(context, s)))
                    .toList())));
    if (selected != null) _cubit.setSpecialty(selected);
  }

  Future<void> _showAvailability(
      BuildContext context, DirectoryState state) async {
    final l = AppLocalizations.of(context);
    final selected = await showModalBottomSheet<bool>(
        context: context,
        showDragHandle: true,
        builder: (_) => SafeArea(
                child: Column(mainAxisSize: MainAxisSize.min, children: [
              ListTile(
                  title: Text(l.filterAllAvailability),
                  trailing: !state.onDutyOnly ? const Icon(Icons.check) : null,
                  onTap: () => Navigator.pop(context, false)),
              ListTile(
                  title: Text(l.filterOnDuty),
                  trailing: state.onDutyOnly ? const Icon(Icons.check) : null,
                  onTap: () => Navigator.pop(context, true)),
            ])));
    if (selected != null) _cubit.setOnDutyOnly(selected);
  }

  Future<void> _showVisitTypes(
      BuildContext context, DirectoryState state) async {
    final l = AppLocalizations.of(context);
    final selected = await showModalBottomSheet<String>(
        context: context,
        showDragHandle: true,
        builder: (_) => SafeArea(
                child: Column(mainAxisSize: MainAxisSize.min, children: [
              for (final option in [
                ('all', l.filterAllVisits),
                ('video', l.consultVideo),
                ('chat', l.consultChat)
              ])
                ListTile(
                    title: Text(option.$2),
                    trailing: state.modalityTab == option.$1
                        ? const Icon(Icons.check)
                        : null,
                    onTap: () => Navigator.pop(context, option.$1)),
            ])));
    if (selected != null) _cubit.setModalityTab(selected);
  }

  Widget _list(BuildContext context, DirectoryState state) {
    if (state.loading && state.doctors.isEmpty) {
      return ListView(padding: const EdgeInsets.all(20), children: const [
        SkeletonBlock(height: 180),
        SizedBox(height: 12),
        SkeletonBlock(height: 180)
      ]);
    }
    if (state.error != null && state.doctors.isEmpty) {
      return ErrorPanel(state.error!, onRetry: _cubit.load);
    }
    final visible = state.visible;
    if (visible.isEmpty) return _DirectoryEmpty(onReset: _cubit.resetFilters);
    return ListView(
        padding: const EdgeInsets.fromLTRB(20, 6, 20, 28),
        children: [
          Row(children: [
            Expanded(
                child: Text(
                    AppLocalizations.of(context)
                        .filterDoctorsCount(visible.length),
                    style: Theme.of(context).textTheme.titleMedium)),
            if (state.specialty != 'all' ||
                state.search.isNotEmpty ||
                state.modalityTab != 'all' ||
                state.onDutyOnly)
              TextButton(
                  onPressed: _cubit.resetFilters,
                  child:
                      Text(AppLocalizations.of(context).discoveryResetFilters))
          ]),
          const SizedBox(height: 12),
          ...visible.map((d) => Padding(
              padding: const EdgeInsets.only(bottom: 12),
              child: PatientDoctorCard(
                  doctor: d,
                  repo: _cubit.repo,
                  paymentRequired: _paymentRequired,
                  onProfile: () => profile(d),
                  onBook: (m) => book(d, m))))
        ]);
  }
}

/// Banner of the product owner's "Find a Doctor" design: sky wash, back
/// arrow and brand mark, the heading on the left and the doctor-and-clinic
/// artwork on the right. The heading is native text; the artwork holds none.
class _DirectoryHeader extends StatelessWidget {
  const _DirectoryHeader({required this.title, required this.subtitle});
  final String title, subtitle;
  @override
  Widget build(BuildContext context) => Container(
        decoration: const BoxDecoration(
            gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [Color(0xFFCFEAF8), Color(0xFFEAF7F4)])),
        child: Column(children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 6, 12, 0),
            child: Row(children: [
              IconButton(
                  onPressed: () => Navigator.maybePop(context),
                  tooltip: MaterialLocalizations.of(context).backButtonTooltip,
                  icon: const Icon(Icons.arrow_back_rounded,
                      color: PatientTokens.navy)),
              const Expanded(child: Center(child: AfyaBrandLockup())),
              const SizedBox(width: 48),
            ]),
          ),
          IntrinsicHeight(
            child: Row(crossAxisAlignment: CrossAxisAlignment.end, children: [
              Expanded(
                flex: 9,
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(20, 4, 4, 18),
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Text(title,
                            style: const TextStyle(
                                fontSize: 27,
                                height: 1.12,
                                fontWeight: FontWeight.w800,
                                letterSpacing: -.4,
                                color: PatientTokens.navy)),
                        const SizedBox(height: 6),
                        Text(subtitle,
                            style: const TextStyle(
                                fontSize: 14,
                                height: 1.35,
                                color: PatientTokens.navy)),
                      ]),
                ),
              ),
              Expanded(
                flex: 10,
                child: ClipRRect(
                  borderRadius:
                      const BorderRadius.only(topLeft: Radius.circular(18)),
                  child: Image.asset(
                      'assets/illustrations/doctor_search_header.webp',
                      height: 124,
                      fit: BoxFit.cover,
                      alignment: Alignment.bottomLeft,
                      excludeFromSemantics: true),
                ),
              ),
            ]),
          ),
        ]),
      );
}

/// Empty state of the "Find a Doctor" design: illustration, heading, one
/// sentence, a "Reset filters" action and the hills artwork at the bottom.
class _DirectoryEmpty extends StatelessWidget {
  const _DirectoryEmpty({required this.onReset});
  final VoidCallback onReset;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return LayoutBuilder(builder: (context, box) {
      return SingleChildScrollView(
        physics: const AlwaysScrollableScrollPhysics(),
        child: ConstrainedBox(
          constraints: BoxConstraints(minHeight: box.maxHeight),
          child: Column(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(28, 12, 28, 18),
                  child: Column(children: [
                    Image.asset('assets/illustrations/doctor_search_empty.webp',
                        height: 168,
                        fit: BoxFit.contain,
                        excludeFromSemantics: true),
                    const SizedBox(height: 16),
                    Text(l.discoveryEmptyTitle,
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                            fontSize: 22,
                            fontWeight: FontWeight.w800,
                            color: PatientTokens.navy)),
                    const SizedBox(height: 8),
                    Text(l.discoveryEmptyBody,
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                            fontSize: 15,
                            height: 1.45,
                            color: PatientTokens.textSub)),
                    const SizedBox(height: 18),
                    OutlinedButton.icon(
                        onPressed: onReset,
                        icon: const Icon(Icons.refresh_rounded, size: 22),
                        label: Text(l.discoveryResetFilters),
                        style: OutlinedButton.styleFrom(
                            foregroundColor: PatientTokens.primaryDark,
                            minimumSize: const Size(220, 50),
                            side: const BorderSide(
                                color: PatientTokens.primaryDark, width: 1.5),
                            shape: const StadiumBorder(),
                            textStyle: const TextStyle(
                                fontFamily: 'Plus Jakarta Sans',
                                fontSize: 16,
                                fontWeight: FontWeight.w700))),
                  ]),
                ),
                Image.asset('assets/illustrations/hills_footer.webp',
                    width: double.infinity,
                    fit: BoxFit.fitWidth,
                    excludeFromSemantics: true),
              ]),
        ),
      );
    });
  }
}
