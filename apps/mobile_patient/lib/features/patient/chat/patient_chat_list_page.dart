import '../../../core/network/screen_refresh.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../booking/booking_repository.dart';
import '../directory/patient_directory_page.dart';
import '../home/home_cubit.dart';
import '../home/home_models.dart';
import '../profile/family_profiles_cubit.dart';
import '../profile/profile_cubit.dart';
import 'chat_repository.dart';
import 'consultation_status_page.dart';

void openPatientChats(BuildContext context) {
  final home = context.read<HomeCubit>().repo;
  final family = context.read<FamilyProfilesCubit>();
  final userId = context.read<ProfileCubit>().state.profile?.userId;
  Navigator.push(
      context,
      MaterialPageRoute(
          builder: (_) => PatientChatListPage(
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
                          initialModality: 'chat'))))));
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

  @override
  Widget build(BuildContext context) {
    final visible = appointments
        .where((a) => a.modality == 'CHAT' && isPast(a) == past)
        .toList();
    return Scaffold(
        appBar: AppBar(title: const Text('Chat consultations')),
        body: RefreshIndicator(
            onRefresh: refresh,
            child: ListView(padding: const EdgeInsets.all(20), children: [
              const Text('Secure conversations with your care team.'),
              const SizedBox(height: 16),
              FilledButton.icon(
                  onPressed: () => widget.onBook(context),
                  icon: const Icon(Icons.add),
                  label: const Text('Book a chat consultation')),
              const SizedBox(height: 16),
              Wrap(spacing: 8, children: [
                ChoiceChip(
                    label: const Text('Active'),
                    selected: !past,
                    onSelected: (_) {
                      setState(() => past = false);
                      unawaited(_screenRefresh.run());
                    }),
                ChoiceChip(
                    label: const Text('Past'),
                    selected: past,
                    onSelected: (_) => setState(() => past = true))
              ]),
              const SizedBox(height: 16),
              if (loading) const Center(child: CircularProgressIndicator()),
              if (error != null) InlineError(error!, onRetry: refresh),
              if (!loading && error == null && visible.isEmpty && !hasMore)
                const Text(
                    'No consultations here yet. Your booked conversations will appear here.'),
              for (final a in visible)
                Padding(
                    padding: const EdgeInsets.only(bottom: 12),
                    child: PatientCard(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                          Text(a.doctorDisplayName,
                              style: Theme.of(context).textTheme.titleMedium),
                          Text(a.primarySpecialty),
                          const SizedBox(height: 8),
                          Text(
                              'Chat consultation · ${a.status.replaceAll('_', ' ').toLowerCase()}'),
                          if (a.fee != null) Text('KES ${a.fee}'),
                          const SizedBox(height: 8),
                          OutlinedButton(
                              onPressed: () async {
                                await Navigator.push(
                                    context,
                                    MaterialPageRoute(
                                        builder: (_) => ConsultationStatusPage(
                                            appointment: a,
                                            repo: BookingRepository(
                                                widget.repo.api,
                                                connectivity:
                                                    widget.repo.connectivity),
                                            familyProfilesCubit:
                                                widget.familyProfilesCubit,
                                            myUserId: widget.myUserId)));
                                if (mounted) refresh();
                              },
                              child: Text(isPast(a)
                                  ? 'View consultation'
                                  : 'Open consultation')),
                        ]))),
              if (hasMore)
                TextButton(
                    onPressed: loadingMore ? null : () => refresh(older: true),
                    child: Text(
                        loadingMore ? 'Loading?' : 'Load more consultations')),
            ])));
  }
}
