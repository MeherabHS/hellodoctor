import '../../../core/network/screen_refresh.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';
import '../directory/patient_directory_page.dart';
import '../home/home_cubit.dart';
import '../home/home_models.dart';
import '../modules/afya_module_widgets.dart';
import '../profile/family_profiles_cubit.dart';
import '../profile/profile_cubit.dart';
import 'chat_repository.dart';
import 'consultation_status_page.dart';

/// The Messages tab body. `context` must sit under `_SignedInRoot`'s
/// providers (it is called from `PatientShell.build`).
Widget buildPatientMessagesTab(BuildContext context) {
  final home = context.read<HomeCubit>().repo;
  final family = context.read<FamilyProfilesCubit>();
  final userId = context.read<ProfileCubit>().state.profile?.userId;
  return PatientChatListPage(
      repo: ChatRepository(home.api, connectivity: home.connectivity),
      familyProfilesCubit: family,
      myUserId: userId,
      onBook: (context) => Navigator.push(
          context,
          MaterialPageRoute(
              builder: (_) => PatientDirectoryPage(
                  api: home.api,
                  cache: home.cache,
                  connectivity: home.connectivity!,
                  familyProfilesCubit: family,
                  initialModality: 'chat'))));
}

class PatientChatListPage extends StatefulWidget {
  const PatientChatListPage(
      {super.key,
      required this.repo,
      required this.familyProfilesCubit,
      required this.onBook,
      this.myUserId});
  final ChatRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;
  final void Function(BuildContext) onBook;
  final String? myUserId;
  @override
  State<PatientChatListPage> createState() => _PatientChatListPageState();
}

class _PatientChatListPageState extends State<PatientChatListPage> {
  List<AppointmentSummaryDto> appointments = [];
  bool loading = true, past = false, hasMore = false, loadingMore = false;
  int pageDepth = 1;
  String? error;
  late final ScreenRefresh _screenRefresh;
  int generation = 0;
  StreamSubscription<FamilyProfilesState>? profileSubscription;
  bool isPast(AppointmentSummaryDto a) =>
      const [
        'COMPLETED',
        'CANCELLED',
        'REFUNDED',
        'EXPIRED',
        'PREMATURE_TERMINATION',
        'DISPUTED',
      ].contains(a.status) ||
      a.chatLocked == true ||
      (a.chatExpiresAt?.isBefore(DateTime.now()) ?? false);
  @override
  void initState() {
    super.initState();
    refresh();
    _screenRefresh = ScreenRefresh(refresh,
        interval: const Duration(seconds: 30),
        networkChanges: widget.repo.connectivity?.changes,
        hasError: () => error != null,
        enabled: () => !past);
    String? profileId = widget.familyProfilesCubit.state.activeProfileId;
    profileSubscription = widget.familyProfilesCubit.stream.listen((s) {
      if (s.activeProfileId != profileId) {
        profileId = s.activeProfileId;
        setState(() {
          generation++;
          pageDepth = 1;
          hasMore = false;
          loadingMore = false;
          appointments = [];
          loading = true;
        });
        refresh();
      }
    });
  }

  @override
  void dispose() {
    generation++;
    _screenRefresh.dispose();
    profileSubscription?.cancel();
    super.dispose();
  }

  Future<void> refresh({bool older = false}) async {
    final version = ++generation;
    final profileId = widget.familyProfilesCubit.state.activeProfileId;
    final depth = older ? pageDepth + 1 : pageDepth;
    if (older) setState(() => loadingMore = true);
    try {
      final next = older
          ? List<AppointmentSummaryDto>.from(appointments)
          : <AppointmentSummaryDto>[];
      var more = false;
      for (var page = older ? pageDepth : 0; page < depth; page++) {
        final batch = await widget.repo
            .consultationPage(profileId: profileId, offset: page * 100);
        if (!mounted || version != generation) return;
        next.addAll(batch);
        more = batch.length == 100;
        if (!more) break;
      }
      if (mounted && version == generation) {
        setState(() {
          appointments = next;
          pageDepth = (next.length / 100).ceil().clamp(1, depth);
          hasMore = more;
          loadingMore = false;
          error = null;
          loading = false;
        });
      }
    } catch (_) {
      if (mounted && version == generation) {
        setState(() {
          loadingMore = false;
          error =
              'Unable to load consultations. Check your connection and retry.';
          loading = false;
        });
      }
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _screenRefresh.attach(context);
  }

  String _statusLabel(AppLocalizations l, String status) => switch (status) {
        'PENDING_PAYMENT' => l.appointmentStatusPendingPayment,
        'CONFIRMED' => l.appointmentStatusConfirmed,
        'WAITING' => l.appointmentStatusWaiting,
        'IN_CONSULTATION' => l.appointmentStatusInConsultation,
        'COMPLETED' => l.appointmentStatusCompleted,
        'CANCELLED' => l.appointmentStatusCancelled,
        'EXPIRED' => l.appointmentStatusExpired,
        'REFUNDED' => l.appointmentStatusRefunded,
        'DISPUTED' => l.appointmentStatusDisputed,
        'PREMATURE_TERMINATION' => l.appointmentStatusPrematureTermination,
        _ => status.replaceAll('_', ' ').toLowerCase(),
      };

  Future<void> _open(AppointmentSummaryDto a) async {
    await Navigator.push(
        context,
        MaterialPageRoute(
            builder: (_) => ConsultationStatusPage(
                appointment: a,
                repo: BookingRepository(widget.repo.api,
                    connectivity: widget.repo.connectivity),
                familyProfilesCubit: widget.familyProfilesCubit,
                myUserId: widget.myUserId)));
    if (mounted) refresh();
  }

  /// Messages inbox. There is no reference panel for this screen; the layout
  /// is derived from the reference's design language (title block, pill
  /// tabs, soft cards, pill call-to-action) and needs visual review.
  ///
  /// A card shows what the consultations endpoint returns: doctor, specialty
  /// and status. It has no message preview, time or unread badge because the
  /// API does not provide them.
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final visible = appointments
        .where((a) => a.modality == 'CHAT' && isPast(a) == past)
        .toList();
    return Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
            child: RefreshIndicator(
                onRefresh: refresh,
                child: ListView(
                    padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
                    children: [
                      Text(l.navMessages,
                          style: const TextStyle(
                              fontSize: 22,
                              fontWeight: FontWeight.w800,
                              color: PatientTokens.text)),
                      const SizedBox(height: 2),
                      Text(l.messagesSubtitle,
                          style: const TextStyle(
                              fontSize: 13, color: PatientTokens.greetingSub)),
                      const SizedBox(height: 16),
                      AfyaSegmentedTabs(
                          expand: true,
                          labels: [l.messagesTabActive, l.messagesTabPast],
                          selected: past ? 1 : 0,
                          onSelected: (i) {
                            setState(() => past = i == 1);
                            if (i == 0) unawaited(_screenRefresh.run());
                          }),
                      const SizedBox(height: 16),
                      if (loading)
                        const Padding(
                            padding: EdgeInsets.symmetric(vertical: 8),
                            child: Column(children: [
                              SkeletonBlock(height: 76),
                              SizedBox(height: 12),
                              SkeletonBlock(height: 76),
                            ])),
                      if (error != null)
                        InlineError(l.messagesLoadError, onRetry: refresh),
                      if (!loading &&
                          error == null &&
                          visible.isEmpty &&
                          !hasMore)
                        AfyaUnavailableNote(
                            icon: Icons.chat_bubble_outline_rounded,
                            title: l.messagesEmptyTitle,
                            body: l.messagesEmptyBody),
                      for (final a in visible)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 12),
                          child: AfyaCard(
                            onTap: () => _open(a),
                            padding: const EdgeInsets.fromLTRB(12, 12, 6, 12),
                            child: Row(children: [
                              AfyaInitialsAvatar(
                                  name: a.doctorDisplayName
                                      .replaceFirst(RegExp(r'^Dr\.?\s*'), ''),
                                  size: 46),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: [
                                      Text(a.doctorDisplayName,
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: const TextStyle(
                                              fontSize: 14.5,
                                              fontWeight: FontWeight.w700,
                                              color: PatientTokens.navy)),
                                      const SizedBox(height: 2),
                                      Text(a.primarySpecialty,
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: const TextStyle(
                                              fontSize: 12.5,
                                              color: PatientTokens.ink)),
                                      const SizedBox(height: 6),
                                      Wrap(
                                          spacing: 8,
                                          runSpacing: 4,
                                          crossAxisAlignment:
                                              WrapCrossAlignment.center,
                                          children: [
                                            StatusChip(
                                                _statusLabel(l, a.status)),
                                            Text(l.messagesChatConsultation,
                                                style: const TextStyle(
                                                    fontSize: 11.5,
                                                    color:
                                                        PatientTokens.textSub)),
                                            if (a.paymentRequired &&
                                                a.fee != null)
                                              Text('KES ${a.fee}',
                                                  style: const TextStyle(
                                                      fontSize: 11.5,
                                                      fontWeight:
                                                          FontWeight.w700,
                                                      color:
                                                          PatientTokens.ink)),
                                          ]),
                                    ]),
                              ),
                              const Icon(Icons.chevron_right_rounded,
                                  color: PatientTokens.navy),
                            ]),
                          ),
                        ),
                      if (hasMore)
                        TextButton(
                            onPressed:
                                loadingMore ? null : () => refresh(older: true),
                            child: Text(loadingMore
                                ? l.messagesLoading
                                : l.messagesLoadMore)),
                      const SizedBox(height: 8),
                      AfyaPillButton(
                          label: l.messagesBookChat,
                          icon: Icons.add_comment_rounded,
                          onPressed: () => widget.onBook(context)),
                    ]))));
  }
}
