import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'dart:math';
import 'dart:typed_data';
import 'clinical_image_viewer.dart';
import 'clinical_pdf_viewer.dart';
import 'clinical_message_bubble.dart';
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:image_picker/image_picker.dart';
import 'package:file_picker/file_picker.dart';
import '../../core/network/api_client.dart';
import '../../core/network/chat_live.dart';
import '../../core/network/screen_refresh.dart';
import '../../core/theme/tokens.dart';
import '../../core/time/kenya_time.dart';
import '../../core/validation/kenya_phone.dart';
import '../../core/widgets/doctor_widgets.dart';
import '../../core/widgets/person_photo.dart';
import '../auth/auth_cubit.dart';
import 'profile_photo_avatar.dart';
import '../notifications/notification_center_page.dart';
import '../notifications/notification_cubit.dart';
import 'doctor_cubit.dart';
import 'doctor_models.dart';
import 'doctor_repository.dart';
import 'agora_page.dart';
import 'articles_page.dart';
import '../../core/video/call_screen_guard.dart';
import '../peer/peer_ui.dart'
    show
        PeerAvatar,
        PeerCard,
        PeerCircleButton,
        PeerHeader,
        PeerPillTabs,
        PeerTokens,
        peerListTime;

/// Shared across every "join/rejoin a live video consultation" entry point
/// in this file — see [CallScreenGuard]'s own doc comment for why one
/// instance, not one per call site.
final _callScreenGuard = CallScreenGuard();

/// Opens the video call for [appointmentId] through the same single guard as
/// every other entry point in this file, for screens that live elsewhere.
void openConsultationCall(BuildContext context, String appointmentId) =>
    _callScreenGuard.push(
        context, (_) => AgoraPage(appointmentId: appointmentId));

/// Shared bell builder so every tab constructs the same `onTap` navigation
/// and reads the same live `NotificationCubit` unread count, without
/// duplicating that wiring five times.
List<Widget> _notificationBellActions(BuildContext context) => [
      NotificationBellAction(
          unreadCount: context.watch<NotificationCubit>().state.unreadCount,
          onTap: () => Navigator.push(
              context,
              MaterialPageRoute(
                  builder: (_) => const NotificationCenterPage())))
    ];

const bool _devPayoutBinding =
    bool.fromEnvironment('DEV_PAYOUT_BINDING', defaultValue: false);

void showError(BuildContext c, Object e, {String? conflict}) {
  final text = e is ApiException && e.status == 409 && conflict != null
      ? conflict
      : e is ApiException
          ? e.userMessage
          : 'The request could not be completed.';
  ScaffoldMessenger.of(c).showSnackBar(SnackBar(content: Text(text)));
}

void showMessage(BuildContext c, String text) =>
    ScaffoldMessenger.of(c).showSnackBar(SnackBar(content: Text(text)));

class QueuePage extends StatefulWidget {
  const QueuePage({super.key});
  @override
  State<QueuePage> createState() => _QueuePageState();
}

class _QueuePageState extends State<QueuePage> {
  String query = '';

  List<AppointmentDto> visible(List<dynamic> source) {
    final items = source.whereType<AppointmentDto>().where((a) {
      final haystack =
          '${a.patientName ?? ''} ${a.complaint ?? ''}'.toLowerCase();
      final matchesQuery =
          query.trim().isEmpty || haystack.contains(query.toLowerCase());
      final matchesFilter = a.isQueueActionable;
      return matchesQuery && matchesFilter;
    }).toList();
    return items;
  }

  @override
  Widget build(BuildContext c) {
    final cubit = c.read<DoctorCubit>();
    final s = c.watch<DoctorCubit>().state;
    final appointments = visible(s.queue);
    final body = <Widget>[
      CacheStatusBar(
          offline: s.offline,
          refreshing: s.refreshing,
          lastUpdated: s.lastUpdatedAt),
      if (s.error != null && s.queue.isNotEmpty)
        InlineError(s.error!, onRetry: cubit.loadAll),
      _DoctorProfileBar(
          profile: s.profile,
          duty: s.duty,
          updating: s.dutyUpdating,
          onDutyChanged: (_) => cubit.toggleDuty(),
          actions: [
            IconButton(
                tooltip: 'Write an article',
                icon: const Icon(Icons.edit_note_outlined),
                onPressed: () => openDoctorArticleEditor(c, cubit.repo.api)),
            ..._notificationBellActions(c)
          ]),
      _QnaBanner(
          unansweredCount: s.questions
              .whereType<QnaDto>()
              .where((q) => q.status == 'OPEN')
              .length,
          onTap: () => showDoctorQna(c)),
      Padding(
          padding: const EdgeInsets.fromLTRB(18, 8, 18, 8),
          child: Row(children: [
            const Expanded(
                child: Text("Current and remaining today",
                    style:
                        TextStyle(fontSize: 15, fontWeight: FontWeight.w800))),
            InkWell(
                onTap: cubit.loadAll,
                child: const Text('↻ Refresh',
                    style: TextStyle(
                        color: DoctorTokens.primaryDark,
                        fontSize: 11,
                        fontWeight: FontWeight.w700)))
          ])),
      Padding(
          padding: const EdgeInsets.fromLTRB(18, 0, 18, 8),
          child: TextField(
              onChanged: (v) => setState(() => query = v),
              style: const TextStyle(fontSize: 13),
              decoration: const InputDecoration(
                  hintText: 'Search waiting patients, symptoms, vitals…',
                  hintStyle: TextStyle(fontSize: 13),
                  prefixIcon: Icon(Icons.search, size: 18),
                  isDense: true,
                  contentPadding: EdgeInsets.symmetric(vertical: 10)))),
      Padding(
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 8),
          child: Row(children: [
            Expanded(
                child: OutlinedButton.icon(
                    icon: const Icon(Icons.history),
                    label: const Text('Consultation History'),
                    onPressed: () => Navigator.push(
                        c,
                        MaterialPageRoute(
                            builder: (_) => const ConsultationHistoryPage())))),
          ]))
    ];
    if (s.error != null && s.queue.isEmpty) {
      body.add(ErrorPanel(s.error!, onRetry: cubit.loadAll));
    } else if (s.queue.isEmpty) {
      body.add(const Padding(
          padding: EdgeInsets.all(32),
          child: Center(child: Text('No scheduled consultations'))));
    } else {
      body.addAll(appointments.map((a) => AppointmentCard(a)));
    }
    return Scaffold(
        appBar: AppBar(toolbarHeight: 0, elevation: 0),
        body: s.loading && s.queue.isEmpty
            ? const Center(child: CircularProgressIndicator())
            : RefreshIndicator(
                onRefresh: cubit.loadAll, child: ListView(children: body)));
  }
}

class ConsultationHistoryPage extends StatefulWidget {
  const ConsultationHistoryPage({super.key});
  @override
  State<ConsultationHistoryPage> createState() => _ConsultationHistoryState();
}

class _ConsultationHistoryState extends State<ConsultationHistoryPage> {
  final items = <AppointmentDto>[];
  bool loading = false, hasMore = true;
  String? error;
  @override
  void initState() {
    super.initState();
    load();
  }

  Future<void> load({bool reset = false}) async {
    if (loading) return;
    setState(() => loading = true);
    try {
      final rows = await context
          .read<DoctorCubit>()
          .repo
          .consultationHistory(offset: reset ? 0 : items.length);
      if (mounted) {
        setState(() {
          if (reset) items.clear();
          items.addAll(rows);
          hasMore = rows.length == 100;
          error = null;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() => error =
            e is ApiException ? e.userMessage : 'Unable to load history.');
      }
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Consultation History')),
      body: SafeArea(
          child: RefreshIndicator(
              onRefresh: () => load(reset: true),
              child: ListView(children: [
                const Padding(
                    padding: EdgeInsets.all(18),
                    child: Text(
                        'Your past encounters. Closed chat consultations remain available as read-only history.')),
                if (error != null) ErrorPanel(error!, onRetry: () => load()),
                ...items.map((a) => ListTile(
                    leading: const Icon(Icons.assignment_outlined),
                    title: Text(a.patientName ?? 'Patient'),
                    subtitle: Text(
                        '${a.isChat ? 'Chat' : 'Video'} consultation · ${_mediumDateTime(a.start?.toIso8601String())} · ${a.status.replaceAll('_', ' ').toLowerCase()}'),
                    onTap: () => Navigator.push(
                        context,
                        MaterialPageRoute(
                            builder: (_) =>
                                EncounterHistoryPage(appointment: a))))),
                if (loading) const Center(child: CircularProgressIndicator()),
                if (!loading && items.isEmpty && error == null)
                  const Padding(
                      padding: EdgeInsets.all(24),
                      child: Text('No consultation history yet.')),
                if (hasMore && !loading && items.isNotEmpty)
                  TextButton(
                      onPressed: () => load(),
                      child: const Text('Load more history'))
              ]))));
}

/// The server accepts a doctor's prescription pages until 24 hours after a
/// consultation ends (`doctor_history_context_allowed`). The button is
/// offered only inside that window; the server still decides.
bool rxWindowOpen(AppointmentDto a, [DateTime? now]) {
  if (a.status != 'COMPLETED' || a.rxDecided) return false;
  final ended = DateTime.tryParse(a.completedAt ?? '') ?? a.end;
  if (ended == null) return false;
  return (now ?? DateTime.now())
      .toUtc()
      .isBefore(ended.toUtc().add(const Duration(hours: 24)));
}

/// "Gap Audit Tester · Video · Nov 13 · 11:27 AM": tells apart several
/// consultations with the same patient.
String rxConsultationLabel(AppointmentDto a) =>
    '${a.patientName ?? 'Patient'} · ${a.isChat ? 'Chat' : 'Video'} · ${_mediumDateTime(a.start?.toIso8601String())}';

class EncounterHistoryPage extends StatefulWidget {
  const EncounterHistoryPage(
      {super.key, required this.appointment, this.previous});
  final AppointmentDto appointment;

  /// The doctor's other consultations with this patient, when the caller
  /// already has them; otherwise they are loaded from the history list.
  final List<AppointmentDto>? previous;
  @override
  State<EncounterHistoryPage> createState() => _EncounterHistoryState();
}

class _EncounterHistoryState extends State<EncounterHistoryPage> {
  late final Future<Json> details;
  late final Future<List<AppointmentDto>> others;
  @override
  void initState() {
    super.initState();
    final repo = context.read<DoctorCubit>().repo;
    details = repo.summary(widget.appointment.id);
    final a = widget.appointment;
    others = widget.previous != null
        ? Future.value(widget.previous)
        : a.patientId == null
            ? Future.value(const <AppointmentDto>[])
            : repo.consultationHistory().then((rows) => rows
                .where((x) => x.patientId == a.patientId && x.id != a.id)
                .toList());
  }

  /// Consultations with this patient that started before this one,
  /// newest first.
  List<AppointmentDto> _earlier(List<AppointmentDto> all) {
    final start = widget.appointment.start;
    final list = all
        .where((x) =>
            x.id != widget.appointment.id &&
            (start == null || x.start == null || x.start!.isBefore(start)))
        .toList()
      ..sort(
          (a, b) => (b.start ?? DateTime(0)).compareTo(a.start ?? DateTime(0)));
    return list;
  }

  Widget _previousSection() => FutureBuilder<List<AppointmentDto>>(
      future: others,
      builder: (context, snap) {
        final children = <Widget>[
          const Padding(
              padding: EdgeInsets.only(top: 20, bottom: 8),
              child: Text('Previous consultations with this patient',
                  style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w800,
                      color: DoctorTokens.navy))),
        ];
        if (snap.connectionState != ConnectionState.done) {
          children.add(const LinearProgressIndicator());
        } else if (snap.hasError) {
          children.add(const Text('Unable to load previous consultations.'));
        } else {
          final earlier = _earlier(snap.data ?? const []);
          if (earlier.isEmpty) {
            children.add(const Text('No previous consultations.',
                style: TextStyle(color: DoctorTokens.slate)));
          }
          for (final p in earlier) {
            children.add(Card(
                margin: const EdgeInsets.only(bottom: 8),
                child: ListTile(
                    leading: Icon(p.isChat ? Icons.chat_bubble_outline : Icons.videocam_outlined,
                        color: DoctorTokens.primary),
                    title: Text(
                        '${p.isChat ? 'Chat' : 'Video'} consultation · ${_mediumDateTime(p.start?.toIso8601String())}'),
                    subtitle: Text([
                      p.status.replaceAll('_', ' ').toLowerCase(),
                      if (p.number != null) p.number!
                    ].join(' · ')),
                    trailing: const Icon(Icons.chevron_right),
                    onTap: () => Navigator.push(
                        context,
                        MaterialPageRoute(
                            builder: (_) => EncounterHistoryPage(appointment: p, previous: [
                                  ...snap.data!.where((x) => x.id != p.id),
                                  widget.appointment
                                ]))))));
          }
        }
        return Column(
            crossAxisAlignment: CrossAxisAlignment.start, children: children);
      });

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Consultation history')),
      body: SafeArea(
          child: FutureBuilder<Json>(
              future: details,
              builder: (context, snapshot) {
                if (snapshot.connectionState != ConnectionState.done) {
                  return const Center(child: CircularProgressIndicator());
                }
                if (snapshot.hasError) {
                  return const Center(
                      child: Text(
                          'Unable to retrieve this encounter. Reopen to retry.'));
                }
                final data = snapshot.data!;
                final documents = findDocumentIds({
                  'documents': data['documents'],
                  'prescriptions': data['prescriptions']
                });
                return ListView(padding: const EdgeInsets.all(18), children: [
                  Text(widget.appointment.patientName ?? 'Patient',
                      style: Theme.of(context).textTheme.titleLarge),
                  Text(
                      '${widget.appointment.isChat ? 'Chat' : 'Video'} consultation · ${widget.appointment.status.replaceAll('_', ' ').toLowerCase()}'),
                  Text(_mediumDateTime(
                      widget.appointment.start?.toIso8601String())),
                  if (data['chief_complaint'] != null)
                    ListTile(
                        title: const Text('Presenting concern'),
                        subtitle: Text('${data['chief_complaint']}')),
                  if (data['clinical_notes'] != null)
                    ListTile(
                        title: const Text('Clinical notes'),
                        subtitle: Text('${data['clinical_notes']}')),
                  const SizedBox(height: 16),
                  OutlinedButton.icon(
                      icon: const Icon(Icons.forum_outlined),
                      label: const Text('View chat transcript'),
                      onPressed: () async {
                        try {
                          final chat = await context
                              .read<DoctorCubit>()
                              .repo
                              .conversationForAppointment(widget.appointment);
                          if (context.mounted) {
                            Navigator.push(
                                context,
                                MaterialPageRoute(
                                    builder: (_) =>
                                        ChatThreadPage(chat: chat)));
                          }
                        } catch (e) {
                          if (context.mounted) showError(context, e);
                        }
                      }),
                  if (rxWindowOpen(widget.appointment)) ...[
                    const SizedBox(height: 10),
                    FilledButton.icon(
                        icon: const Icon(Icons.receipt_long),
                        label: const Text('Send prescription'),
                        onPressed: () => Navigator.push(
                            context,
                            MaterialPageRoute(
                                builder: (_) => ScannerPage(
                                    initialAppointment: widget.appointment)))),
                    const Padding(
                        padding: EdgeInsets.only(top: 4),
                        child: Text(
                            'You can send a prescription up to 24 hours after the consultation ends.',
                            style: TextStyle(
                                fontSize: 12, color: DoctorTokens.textSub))),
                  ],
                  const Padding(
                      padding: EdgeInsets.symmetric(vertical: 12),
                      child: Text('Clinical attachments and prescriptions')),
                  if (documents.isEmpty)
                    Text(filesAccessEnded(data)
                        ? kFilesAccessEnded
                        : 'No clinical documents recorded.'),
                  ...documents.map((doc) => DocumentButton(
                      appointmentId: widget.appointment.id,
                      documentId: doc.id,
                      mimeType: doc.mimeType)),
                  _previousSection(),
                ]);
              })));
}

class _DoctorProfileBar extends StatelessWidget {
  const _DoctorProfileBar(
      {required this.profile,
      required this.duty,
      required this.updating,
      required this.onDutyChanged,
      this.actions = const []});
  final ProfileDto? profile;
  final bool duty, updating;
  final ValueChanged<bool> onDutyChanged;
  // QueuePage has no DoctorPageHeader (it uses this profile bar as its top
  // bar instead), so the shared notification bell is plugged in here rather
  // than duplicating DoctorPageHeader's layout for a single screen.
  final List<Widget> actions;

  static String _initials(String name) {
    final parts = name.trim().split(RegExp(r'\s+'));
    return parts
        .where((p) => p.isNotEmpty)
        .take(2)
        .map((p) => p[0])
        .join()
        .toUpperCase();
  }

  @override
  Widget build(BuildContext context) {
    final p = profile;
    return Container(
        padding: const EdgeInsets.fromLTRB(18, 10, 18, 12),
        decoration: const BoxDecoration(
            color: Colors.white,
            border: Border(bottom: BorderSide(color: DoctorTokens.border))),
        child: Row(children: [
          DoctorOwnPhoto(
              size: 42,
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: DoctorTokens.primary),
              fallback: Container(
                  width: 42,
                  height: 42,
                  decoration: BoxDecoration(
                      color: DoctorTokens.primaryLight,
                      borderRadius: BorderRadius.circular(14),
                      border: Border.all(color: DoctorTokens.primary)),
                  alignment: Alignment.center,
                  child: Text(p == null ? '' : _initials(p.name),
                      style: const TextStyle(
                          fontWeight: FontWeight.w800,
                          color: DoctorTokens.primaryDark)))),
          const SizedBox(width: 10),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(p?.name ?? 'Loading profile…',
                    style: const TextStyle(
                        fontSize: 14, fontWeight: FontWeight.w800)),
                const SizedBox(height: 2),
                Text(
                    p == null
                        ? ' '
                        : '${p.verified ? '✓  ' : ''}KMPDC Registration: ${p.license}',
                    style: const TextStyle(
                        fontSize: 10,
                        fontWeight: FontWeight.w700,
                        color: DoctorTokens.primaryDark))
              ])),
          Container(
              padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 6),
              decoration: BoxDecoration(
                  color: duty
                      ? const Color(0xFFECFDF5)
                      : DoctorTokens.warningLight,
                  border: Border.all(
                      color: duty
                          ? const Color(0xFF10B981)
                          : DoctorTokens.warning),
                  borderRadius: BorderRadius.circular(20)),
              child: InkWell(
                  onTap: updating ? null : () => onDutyChanged(!duty),
                  child: Row(children: [
                    Container(
                        width: 8,
                        height: 8,
                        decoration: BoxDecoration(
                            color: duty
                                ? const Color(0xFF10B981)
                                : DoctorTokens.warning,
                            shape: BoxShape.circle)),
                    const SizedBox(width: 6),
                    Text(duty ? 'Online' : 'Break',
                        style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w800,
                            color: duty
                                ? DoctorTokens.primaryDark
                                : const Color(0xFFB45309)))
                  ]))),
          ...actions,
        ]));
  }
}

class _QnaBanner extends StatelessWidget {
  const _QnaBanner({required this.onTap, required this.unansweredCount});
  final VoidCallback onTap;
  final int unansweredCount;
  @override
  Widget build(BuildContext context) => InkWell(
      onTap: onTap,
      child: Container(
          margin: const EdgeInsets.fromLTRB(18, 10, 18, 4),
          padding: const EdgeInsets.symmetric(horizontal: 15, vertical: 13),
          decoration: BoxDecoration(
              gradient: const LinearGradient(
                  colors: [Color(0xFF064E3B), Color(0xFF047857)]),
              borderRadius: BorderRadius.circular(16),
              boxShadow: const [
                BoxShadow(
                    color: Color(0x22059669),
                    blurRadius: 12,
                    offset: Offset(0, 3))
              ]),
          child: Row(children: [
            Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                    color: Colors.white.withValues(alpha: .18),
                    borderRadius: BorderRadius.circular(12)),
                alignment: Alignment.center,
                child: const Text('❓', style: TextStyle(fontSize: 18))),
            const SizedBox(width: 12),
            Expanded(
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                  Row(children: [
                    const Flexible(
                        child: Text('Open Patient Q&A',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                                color: Colors.white,
                                fontSize: 13,
                                fontWeight: FontWeight.w800))),
                    if (unansweredCount > 0) ...[
                      const SizedBox(width: 6),
                      Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 6, vertical: 1),
                          decoration: BoxDecoration(
                              color: const Color(0xFFFEF08A),
                              borderRadius: BorderRadius.circular(4)),
                          child: Text('$unansweredCount Unanswered',
                              style: const TextStyle(
                                  color: Color(0xFF854D0E),
                                  fontSize: 9.5,
                                  fontWeight: FontWeight.w800)))
                    ]
                  ]),
                  const SizedBox(height: 2),
                  const Text(
                      'Review anonymous questions & publish verified advice',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(color: Colors.white, fontSize: 10.5))
                ])),
            const SizedBox(width: 6),
            const Text('Answer →',
                style: TextStyle(
                    color: Colors.white,
                    fontSize: 10.5,
                    fontWeight: FontWeight.w700))
          ])));
}

class _QueueFilter extends StatelessWidget {
  const _QueueFilter(this.label, this.value, this.current, this.onTap);
  final String label, value, current;
  final ValueChanged<String> onTap;
  @override
  Widget build(BuildContext context) {
    final active = current == value;
    return Padding(
        padding: const EdgeInsets.only(right: 6),
        child: InkWell(
            onTap: () => onTap(value),
            child: Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 13, vertical: 6),
                decoration: BoxDecoration(
                    color: active ? DoctorTokens.primary : Colors.white,
                    border: Border.all(
                        color: active
                            ? DoctorTokens.primary
                            : DoctorTokens.border),
                    borderRadius: BorderRadius.circular(20)),
                child: Text(label,
                    style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w600,
                        color:
                            active ? Colors.white : DoctorTokens.textSub)))));
  }
}

class AppointmentCard extends StatelessWidget {
  const AppointmentCard(this.a, {super.key});
  final AppointmentDto a;
  @override
  Widget build(BuildContext c) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 6),
      child: DoctorCard(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        if (a.hasActiveConsultation)
          Container(
              width: double.infinity,
              margin: const EdgeInsets.only(bottom: 12),
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                  color: const Color(0xFFDCFCE7),
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(color: const Color(0xFF86EFAC))),
              child: Row(children: [
                const Icon(Icons.videocam, color: Color(0xFF166534)),
                const SizedBox(width: 8),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      const Text('Active Consultation',
                          style: TextStyle(
                              color: Color(0xFF166534),
                              fontWeight: FontWeight.w800)),
                      Text(
                          a.isWaitingForReconnect
                              ? (a.patientPresent == false &&
                                      a.doctorPresent == true
                                  ? 'The patient is reconnecting. You can rejoin now.'
                                  : a.patientPresent == true &&
                                          a.doctorPresent == false
                                      ? 'You are disconnected. Rejoin to continue the consultation.'
                                      : 'The room is empty. Either participant can rejoin now.')
                              : 'The consultation is still active.',
                          style: const TextStyle(
                              color: Color(0xFF166534), fontSize: 11)),
                    ])),
                FilledButton(
                    style: FilledButton.styleFrom(
                        backgroundColor: const Color(0xFF16A34A)),
                    onPressed: () => _callScreenGuard.push(
                        c, (_) => AgoraPage(appointmentId: a.id)),
                    child: const Text('Rejoin'))
              ])),
        Row(children: [
          CircleAvatar(
              radius: 20,
              backgroundColor: _avatarColors(a.patientId ?? a.id).$1,
              child: Text(_initials(a.patientName ?? 'Patient'),
                  style: TextStyle(
                      fontWeight: FontWeight.w800,
                      color: _avatarColors(a.patientId ?? a.id).$2))),
          const SizedBox(width: 10),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(a.patientName ?? 'Patient',
                    style: const TextStyle(
                        fontSize: 13.5, fontWeight: FontWeight.w700)),
                Text(a.isChat ? 'Chat consultation' : 'Video consultation',
                    style: const TextStyle(
                        fontSize: 11, color: DoctorTokens.textSub))
              ])),
          StatusChip(_timeLabel(a.start),
              color: const Color(0xFFECFDF5),
              textColor: DoctorTokens.primaryDark)
        ]),
        const SizedBox(height: 9),
        Container(
            width: double.infinity,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            decoration: BoxDecoration(
                color: DoctorTokens.surfaceSubtle,
                borderRadius: BorderRadius.circular(10)),
            child: Text(
                'Chief Complaint: ${a.complaint ?? 'Consultation details are available from the backend.'}',
                style: const TextStyle(
                    fontSize: 11.5, color: DoctorTokens.muted, height: 1.35))),
        const SizedBox(height: 10),
        ConsultationActions(appointment: a)
      ])));

  static String _initials(String name) => name
      .trim()
      .split(RegExp(r'\s+'))
      .take(2)
      .map((part) => part.isEmpty ? '' : part[0])
      .join()
      .toUpperCase();
  static String _timeLabel(DateTime? time) {
    if (time == null) return 'Scheduled';
    final local = kenyaTime(time);
    final hour = local.hour % 12 == 0 ? 12 : local.hour % 12;
    return '$hour:${local.minute.toString().padLeft(2, '0')} ${local.hour >= 12 ? 'PM' : 'AM'}';
  }
}

class ConsultationActions extends StatefulWidget {
  const ConsultationActions({super.key, required this.appointment});
  final AppointmentDto appointment;
  @override
  State<ConsultationActions> createState() => _ConsultationActionsState();
}

class _ConsultationActionsState extends State<ConsultationActions> {
  bool busy = false;
  Future<void> openChat() async {
    if (busy) return;
    setState(() => busy = true);
    try {
      final chat = await context
          .read<DoctorCubit>()
          .repo
          .conversationForAppointment(widget.appointment);
      if (mounted) {
        await Navigator.push(context,
            MaterialPageRoute(builder: (_) => ChatThreadPage(chat: chat)));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final a = widget.appointment;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      if (a.canStart)
        FilledButton.icon(
            onPressed: busy || !a.isQueueActionable || a.joinBlocked
                ? null
                : a.isChat
                    ? openChat
                    : () => _callScreenGuard.push(
                        context, (_) => AgoraPage(appointmentId: a.id)),
            icon:
                Icon(a.isChat ? Icons.chat_outlined : Icons.videocam_outlined),
            label: Text(
                busy
                    ? 'Opening consultation...'
                    : (a.joinLockedLabel ?? a.startLabel),
                textAlign: TextAlign.center)),
      if (!a.canStart) Text(a.status.replaceAll('_', ' ')),
      Wrap(spacing: 8, children: [
        if (a.isChat && !a.canStart && a.conversationId != null)
          TextButton(
              onPressed: busy ? null : openChat,
              child: const Text('View consultation history')),
        if (a.modality == 'VIDEO' && a.conversationId != null)
          TextButton(
              onPressed: busy ? null : openChat,
              child: const Text('Follow-up messages')),
        if (a.canStart)
          TextButton(
              onPressed: () => Navigator.push(
                  context,
                  MaterialPageRoute(
                      builder: (_) => ScannerPage(initialAppointment: a))),
              child: const Text('Write prescription')),
      ]),
    ]);
  }
}

DateTime _calendarDate(DateTime date) =>
    DateTime.utc(date.year, date.month, date.day);

bool _sameDate(DateTime a, DateTime b) =>
    a.year == b.year && a.month == b.month && a.day == b.day;

String _timeRange(DateTime? start, DateTime? end) {
  if (start == null || end == null) return 'Time unavailable';
  final local = kenyaTime(start);
  final localEnd = kenyaTime(end);
  final duration = end.difference(start).inMinutes;
  return '${_shortTime(local.toIso8601String())} – '
      '${_shortTime(localEnd.toIso8601String())} (${duration}m)';
}

class SchedulePage extends StatefulWidget {
  const SchedulePage({super.key, this.header});

  /// The Home logo bar (DoctorTopBar), supplied by the shell.
  final Widget? header;
  @override
  State<SchedulePage> createState() => _ScheduleState();
}

class _ScheduleState extends State<SchedulePage> {
  late final ScreenRefresh _screenRefresh;
  @override
  void initState() {
    super.initState();
    unawaited(context.read<DoctorCubit>().refreshSlots(day: selectedDate));
    _screenRefresh = ScreenRefresh(
        () => context.read<DoctorCubit>().refreshSlots(day: selectedDate),
        interval: const Duration(seconds: 30),
        networkChanges: context.read<DoctorCubit>().connectivity?.changes);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _screenRefresh.attach(context);
  }

  @override
  void dispose() {
    _screenRefresh.dispose();
    super.dispose();
  }

  bool busy = false;
  String filter = 'all';
  late DateTime selectedDate = _calendarDate(kenyaTime(DateTime.now()));

  @override
  Widget build(BuildContext c) {
    final s = c.watch<DoctorCubit>().state;
    final allSlots = s.slots.cast<SlotDto>().toList()
      ..sort(
          (a, b) => (a.start ?? DateTime(0)).compareTo(b.start ?? DateTime(0)));
    final today = kenyaTime(DateTime.now());
    final todaySlots = allSlots
        .where((x) => x.start != null && _sameDate(kenyaTime(x.start!), today));
    final leftToday = todaySlots
        .where((x) =>
            x.end != null &&
            x.end!.isAfter(today) &&
            x.status != 'CANCELLED' &&
            x.status != 'BLOCKED')
        .length;
    final completedToday = todaySlots
        .where((x) =>
            x.end != null &&
            !x.end!.isAfter(today) &&
            x.status != 'CANCELLED' &&
            x.status != 'BLOCKED')
        .length;

    final visible = allSlots
        .where((x) => x.status != 'CANCELLED')
        .where((x) =>
            x.start != null && _sameDate(kenyaTime(x.start!), selectedDate))
        .where((x) =>
            filter == 'all' ||
            (filter == 'booked' &&
                (x.status == 'BOOKED' || x.status == 'LOCKED_IN_PAYMENT')) ||
            (filter == 'blocked' && x.status == 'BLOCKED') ||
            (filter == 'available' && x.status == 'AVAILABLE' && x.bookable))
        .toList();
    final dayBooked = allSlots
        .where((x) =>
            x.start != null &&
            _sameDate(kenyaTime(x.start!), selectedDate) &&
            (x.status == 'BOOKED' || x.status == 'LOCKED_IN_PAYMENT'))
        .length;
    final dayAvailable = allSlots
        .where((x) =>
            x.start != null &&
            _sameDate(kenyaTime(x.start!), selectedDate) &&
            x.status == 'AVAILABLE' &&
            x.bookable)
        .length;

    final dayCount = allSlots
        .where((x) =>
            x.start != null &&
            _sameDate(kenyaTime(x.start!), selectedDate) &&
            x.status != 'CANCELLED')
        .length;
    final bookedToday = todaySlots
        .where((x) => x.status == 'BOOKED' || x.status == 'LOCKED_IN_PAYMENT')
        .length;
    void setFilter(String v) => setState(() => filter = v);

    // No AppBar here, so without this the header below renders straight
    // under the status bar (confirmed on-device).
    return SafeArea(
        bottom: false,
        child: Scaffold(
            backgroundColor: _schBg,
            body: s.loading && s.slots.isEmpty
                ? const Center(child: CircularProgressIndicator())
                : RefreshIndicator(
                    onRefresh: () =>
                        c.read<DoctorCubit>().refreshSlots(day: selectedDate),
                    child: ListView(padding: EdgeInsets.zero, children: [
                      if (widget.header != null) widget.header!,
                      const _ScheduleTitle(),
                      // The references have no "last updated" line; the
                      // time is still shown when offline, as on Home.
                      CacheStatusBar(
                          offline: s.offline,
                          refreshing: s.refreshing,
                          lastUpdated: s.offline ? s.lastUpdatedAt : null),
                      if (s.error != null)
                        InlineError(s.error!,
                            onRetry: () => c
                                .read<DoctorCubit>()
                                .refreshSlots(day: selectedDate)),
                      _ScheduleSummaryBanner(
                          leftToday: leftToday,
                          completedToday: completedToday,
                          bookedToday: bookedToday,
                          busy: busy,
                          onCreateSlots: createBulkSlots),
                      Padding(
                          padding: const EdgeInsets.fromLTRB(16, 0, 8, 8),
                          child: Row(children: [
                            const Expanded(
                                child: Text('Select Date',
                                    style: TextStyle(
                                        fontSize: 17,
                                        fontWeight: FontWeight.w800,
                                        color: _schInk))),
                            TextButton.icon(
                                onPressed: busy ? null : createSlot,
                                style: TextButton.styleFrom(
                                    foregroundColor: _schGreen),
                                icon: const Icon(Icons.add, size: 22),
                                label: const Text('Add Slot',
                                    style: TextStyle(
                                        fontSize: 15,
                                        fontWeight: FontWeight.w700)))
                          ])),
                      _DateSlider(
                          selected: selectedDate,
                          onSelected: (d) {
                            setState(() => selectedDate = d);
                            unawaited(
                                c.read<DoctorCubit>().refreshSlots(day: d));
                          }),
                      Padding(
                          padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
                          child: Row(children: [
                            Expanded(
                                flex: 12,
                                child: _SchedulePill('All Slots ($dayCount)',
                                    filter == 'all', () => setFilter('all'))),
                            const SizedBox(width: 8),
                            Expanded(
                                flex: 9,
                                child: _SchedulePill(
                                    'Blocked',
                                    filter == 'blocked',
                                    () => setFilter('blocked'))),
                            const SizedBox(width: 8),
                            Expanded(
                                flex: 10,
                                child: _SchedulePill(
                                    dayBooked > 0
                                        ? 'Booked ($dayBooked)'
                                        : 'Booked',
                                    filter == 'booked',
                                    () => setFilter('booked'))),
                            const SizedBox(width: 8),
                            Expanded(
                                flex: 11,
                                child: _SchedulePill(
                                    dayAvailable > 0
                                        ? 'Available ($dayAvailable)'
                                        : 'Available',
                                    filter == 'available',
                                    () => setFilter('available'))),
                          ])),
                      const Padding(
                          padding: EdgeInsets.fromLTRB(18, 0, 18, 16),
                          child: Wrap(spacing: 24, runSpacing: 6, children: [
                            _SlotLegend('Available', _schAvailable),
                            _SlotLegend('Booked', _schBooked),
                            _SlotLegend('Blocked', _schBlocked),
                          ])),
                      if (visible.isEmpty)
                        _ScheduleEmptyCard(busy: busy, onAddSlot: createSlot)
                      else
                        ...visible.map((slot) => _ScheduleSlotCard(
                            slot: slot,
                            busy: busy,
                            onCancel: () => cancelSlot(slot.id),
                            onBlock: () => updateSlotStatus(slot.id, 'BLOCKED'),
                            onUnblock: () =>
                                updateSlotStatus(slot.id, 'AVAILABLE'),
                            appointment: _matchingAppointment(s, slot),
                            chat: _matchingChat(s, slot)))
                    ]))));
  }

  AppointmentDto? _matchingAppointment(DoctorState s, SlotDto slot) {
    if (slot.appointmentId == null) return null;
    for (final a in s.queue.cast<AppointmentDto>()) {
      if (a.id == slot.appointmentId) return a;
    }
    return null;
  }

  ChatDto? _matchingChat(DoctorState s, SlotDto slot) {
    if (slot.appointmentId == null) return null;
    for (final x in s.chats.cast<ChatDto>()) {
      if (x.appointmentId == slot.appointmentId) return x;
    }
    return null;
  }

  Future<void> createSlot() async {
    final cubit = context.read<DoctorCubit>();
    final now = kenyaTime(DateTime.now());
    final date = await showDatePicker(
        context: context,
        firstDate: DateTime(now.year, now.month, now.day),
        lastDate: now.add(const Duration(days: 90)),
        initialDate: _calendarDate(selectedDate).isBefore(_calendarDate(now))
            ? now
            : selectedDate);
    if (!mounted || date == null) return;
    final startTime = await showTimePicker(
        context: context,
        initialTime: TimeOfDay.fromDateTime(now.add(const Duration(hours: 2))));
    if (!mounted || startTime == null) return;
    final start = kenyaWallTime(
        date.year, date.month, date.day, startTime.hour, startTime.minute);
    final endTime = await showTimePicker(
        context: context,
        initialTime:
            TimeOfDay.fromDateTime(start.add(const Duration(minutes: 10))));
    if (!mounted || endTime == null) return;
    final end = kenyaWallTime(
        date.year, date.month, date.day, endTime.hour, endTime.minute);
    if (!start.isAfter(now) || !end.isAfter(start)) {
      showMessage(
          context, 'Choose a future date with an end time after start.');
      return;
    }
    setState(() => busy = true);
    try {
      await cubit.repo.createSlot(start, end);
      await cubit.refreshSlots(day: selectedDate);
      if (mounted) {
        setState(() => selectedDate = _calendarDate(date));
      }
    } catch (e) {
      if (mounted) {
        showError(context, e, conflict: 'That time overlaps an existing slot.');
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  /// "⚡ Create 10-Min Slots" in the prototype has no dedicated bulk-create
  /// endpoint behind it — there is only POST /doctor/slots for one slot at a
  /// time. This reproduces the same real, sequential create for 6
  /// consecutive 10-minute slots from a chosen start, reporting exactly how
  /// many succeeded (e.g. an overlap conflict can fail one in the middle
  /// without silently claiming the whole batch worked).
  Future<void> createBulkSlots() async {
    final cubit = context.read<DoctorCubit>();
    final now = kenyaTime(DateTime.now());
    final startTime = await showTimePicker(
        context: context,
        helpText: 'START OF FIRST 10-MIN SLOT',
        initialTime: TimeOfDay.fromDateTime(now.add(const Duration(hours: 1))));
    if (!mounted || startTime == null) return;
    final date = _calendarDate(selectedDate).isBefore(_calendarDate(now))
        ? DateTime(now.year, now.month, now.day)
        : selectedDate;
    var cursor = kenyaWallTime(
        date.year, date.month, date.day, startTime.hour, startTime.minute);
    setState(() => busy = true);
    var created = 0;
    String? lastError;
    try {
      for (var i = 0; i < 6; i++) {
        final end = cursor.add(const Duration(minutes: 10));
        try {
          await cubit.repo.createSlot(cursor, end);
          created++;
        } on ApiException catch (e) {
          lastError = e.userMessage;
        }
        cursor = end;
      }
      await cubit.refreshSlots(day: selectedDate);
    } finally {
      if (mounted) setState(() => busy = false);
    }
    if (!mounted) return;
    showMessage(
        context,
        created == 6
            ? 'Created 6 ten-minute slots.'
            : 'Created $created of 6 slots.'
                '${lastError != null ? ' ($lastError)' : ''}');
  }

  Future<void> cancelSlot(String id) async {
    final cubit = context.read<DoctorCubit>();
    setState(() => busy = true);
    try {
      await cubit.repo.cancelSlot(id);
      await cubit.refreshSlots(day: selectedDate);
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> updateSlotStatus(String id, String status) async {
    final cubit = context.read<DoctorCubit>();
    setState(() => busy = true);
    try {
      await cubit.repo.updateSlotStatus(id, status);
      await cubit.refreshSlots(day: selectedDate);
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }
}

// ---------------------------------------------------------------------------
// Consultation Schedule, built to the product-owner references (2026-11-13):
// docs/design/afyamkononi-doctor-schedule-empty-reference.png and
// docs/design/afyamkononi-doctor-schedule-populated-reference.png.
// Colours are sampled from those images.
// ---------------------------------------------------------------------------

const _schBg = Color(0xFFF4F9F7);
const _schSummaryBg = Color(0xFFDFF2ED);
const _schGreen = Color(0xFF027E52);
const _schGreenDark = Color(0xFF006441);
const _schInk = Color(0xFF0D1B26);
const _schMuted = Color(0xFF4A5565);
const _schLine = Color(0xFFE1EAE6);
const _schLink = Color(0xFF0A7040);
const _schAvailable = Color(0xFFFDEBB1);
const _schBooked = Color(0xFFCBE1FB);
const _schBlocked = Color(0xFFE7EEEE);

/// Title block: "Consultation Schedule", its subtitle and the stethoscope
/// artwork extracted from the reference.
class _ScheduleTitle extends StatelessWidget {
  const _ScheduleTitle();
  @override
  Widget build(BuildContext context) => SizedBox(
      height: 84,
      child: Stack(children: [
        Positioned(
            right: 0,
            top: 0,
            bottom: 0,
            child: Image.asset('assets/doctor/schedule_header_stethoscope.webp',
                fit: BoxFit.fitHeight, excludeFromSemantics: true)),
        const Padding(
            padding: EdgeInsets.fromLTRB(18, 6, 92, 0),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text('Consultation Schedule',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                      fontSize: 25,
                      height: 1.15,
                      fontWeight: FontWeight.w800,
                      letterSpacing: -0.4,
                      color: _schInk)),
              SizedBox(height: 6),
              Text(
                  'Manage your live availability and calendar slots for patient consultations.',
                  maxLines: 2,
                  style:
                      TextStyle(fontSize: 13.5, height: 1.3, color: _schMuted)),
            ])),
      ]));
}

/// "TODAY'S SCHEDULE" card. With nothing on today's calendar it reads
/// "0 Booked / Consultations" (empty reference); otherwise
/// "N Left To See (M Completed)" (populated reference).
class _ScheduleSummaryBanner extends StatelessWidget {
  const _ScheduleSummaryBanner(
      {required this.leftToday,
      required this.completedToday,
      required this.bookedToday,
      required this.busy,
      required this.onCreateSlots});
  final int leftToday, completedToday, bookedToday;
  final bool busy;
  final VoidCallback onCreateSlots;
  @override
  Widget build(BuildContext c) {
    final empty = leftToday == 0 && completedToday == 0;
    return Container(
        margin: const EdgeInsets.fromLTRB(14, 10, 14, 14),
        padding: const EdgeInsets.fromLTRB(16, 13, 12, 13),
        decoration: BoxDecoration(
            color: _schSummaryBg, borderRadius: BorderRadius.circular(18)),
        child: Row(children: [
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                const Text("TODAY'S SCHEDULE",
                    style: TextStyle(
                        fontSize: 11,
                        letterSpacing: .6,
                        fontWeight: FontWeight.w800,
                        color: Color(0xFF3E4252))),
                const SizedBox(height: 4),
                if (empty) ...[
                  Text('$bookedToday Booked',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          fontSize: 22,
                          height: 1.1,
                          fontWeight: FontWeight.w800,
                          color: _schInk)),
                  const SizedBox(height: 2),
                  const Text('Consultations',
                      style: TextStyle(fontSize: 14.5, color: _schMuted)),
                ] else
                  // "(N Completed)" drops under the count when the
                  // phone is too narrow for both on one line.
                  Wrap(
                      crossAxisAlignment: WrapCrossAlignment.end,
                      spacing: 8,
                      children: [
                        Text('$leftToday Left To See',
                            style: const TextStyle(
                                fontSize: 20,
                                height: 1.15,
                                fontWeight: FontWeight.w800,
                                color: _schInk)),
                        Padding(
                            padding: const EdgeInsets.only(bottom: 2),
                            child: Text('($completedToday Completed)',
                                style: const TextStyle(
                                    fontSize: 12.5, color: _schMuted))),
                      ]),
              ])),
          const SizedBox(width: 8),
          FilledButton.icon(
              onPressed: busy ? null : onCreateSlots,
              style: FilledButton.styleFrom(
                  backgroundColor: _schGreen,
                  foregroundColor: Colors.white,
                  shape: const StadiumBorder(),
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  minimumSize: const Size(0, 38),
                  textStyle: const TextStyle(
                      fontSize: 12.5, fontWeight: FontWeight.w700)),
              icon: const Icon(Icons.bolt, size: 17, color: Color(0xFFF0D330)),
              label: const Text('Create 10-Min Slots')),
        ]));
  }
}

/// Today and the next five days, filling the width (both references).
class _DateSlider extends StatelessWidget {
  const _DateSlider({required this.selected, required this.onSelected});
  final DateTime selected;
  final ValueChanged<DateTime> onSelected;
  static const _days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  @override
  Widget build(BuildContext c) {
    final base = _calendarDate(kenyaTime(DateTime.now()));
    final dates = List.generate(6, (i) => base.add(Duration(days: i)));
    return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14),
        child: Row(children: [
          for (var i = 0; i < dates.length; i++) ...[
            if (i > 0) const SizedBox(width: 8),
            Expanded(child: _chip(dates[i], i)),
          ]
        ]));
  }

  Widget _chip(DateTime d, int i) {
    final on = _sameDate(d, selected);
    return InkWell(
        onTap: () => onSelected(d),
        borderRadius: BorderRadius.circular(16),
        child: Container(
            padding: const EdgeInsets.symmetric(vertical: 9),
            decoration: BoxDecoration(
                color: on ? _schGreen : Colors.white,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: on ? _schGreen : _schLine),
                boxShadow: on
                    ? null
                    : const [
                        BoxShadow(
                            color: Color(0x0A0F3D2E),
                            blurRadius: 6,
                            offset: Offset(0, 2))
                      ]),
            child: Column(mainAxisSize: MainAxisSize.min, children: [
              Text(i == 0 ? 'Today' : _days[d.weekday % 7],
                  style: TextStyle(
                      fontSize: 12,
                      color: on ? Colors.white : const Color(0xFF415263))),
              const SizedBox(height: 2),
              Text('${d.day}',
                  style: TextStyle(
                      fontSize: 20,
                      height: 1.1,
                      fontWeight: FontWeight.w800,
                      color: on ? Colors.white : _schInk)),
              const SizedBox(height: 3),
              // Marks today, as in both references.
              Container(
                  width: 5,
                  height: 5,
                  decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: i != 0
                          ? Colors.transparent
                          : on
                              ? Colors.white
                              : _schGreen)),
            ])));
  }
}

class _SchedulePill extends StatelessWidget {
  const _SchedulePill(this.label, this.selected, this.onTap);
  final String label;
  final bool selected;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(20),
      child: Container(
          height: 34,
          alignment: Alignment.center,
          padding: const EdgeInsets.symmetric(horizontal: 6),
          decoration: BoxDecoration(
              color: selected ? _schGreen : Colors.white,
              borderRadius: BorderRadius.circular(20),
              border: Border.all(color: selected ? _schGreen : _schLine)),
          child: FittedBox(
              fit: BoxFit.scaleDown,
              child: Text(label,
                  maxLines: 1,
                  style: TextStyle(
                      fontSize: 13.5,
                      fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
                      color: selected
                          ? Colors.white
                          : const Color(0xFF29494D))))));
}

class _SlotLegend extends StatelessWidget {
  const _SlotLegend(this.label, this.color);
  final String label;
  final Color color;
  @override
  Widget build(BuildContext context) =>
      Row(mainAxisSize: MainAxisSize.min, children: [
        Container(
            width: 12,
            height: 12,
            decoration: BoxDecoration(
                color: color,
                borderRadius: BorderRadius.circular(3),
                border:
                    Border.all(color: Colors.black.withValues(alpha: .04)))),
        const SizedBox(width: 8),
        Text(label,
            style: const TextStyle(fontSize: 13, color: Color(0xFF495565)))
      ]);
}

/// Empty state from the empty reference: illustration, message, Add Slot.
class _ScheduleEmptyCard extends StatelessWidget {
  const _ScheduleEmptyCard({required this.busy, required this.onAddSlot});
  final bool busy;
  final VoidCallback onAddSlot;
  @override
  Widget build(BuildContext context) => Container(
      margin: const EdgeInsets.fromLTRB(14, 4, 14, 20),
      padding: const EdgeInsets.fromLTRB(18, 18, 18, 18),
      decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(18),
          border: Border.all(color: _schLine)),
      child: Column(children: [
        Image.asset('assets/doctor/schedule_empty_illustration.webp',
            height: 150, fit: BoxFit.contain, excludeFromSemantics: true),
        const SizedBox(height: 14),
        const Text('No consultations scheduled yet',
            textAlign: TextAlign.center,
            style: TextStyle(
                fontSize: 19,
                fontWeight: FontWeight.w800,
                letterSpacing: -0.2,
                color: _schInk)),
        const SizedBox(height: 6),
        const Text(
            'Create new time slots or wait for patient bookings to appear here.',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 14, height: 1.35, color: _schMuted)),
        const SizedBox(height: 16),
        FilledButton.icon(
            onPressed: busy ? null : onAddSlot,
            style: FilledButton.styleFrom(
                backgroundColor: _schGreen,
                shape: const StadiumBorder(),
                minimumSize: const Size(176, 44),
                textStyle:
                    const TextStyle(fontSize: 15, fontWeight: FontWeight.w700)),
            icon: const Icon(Icons.add, size: 20),
            label: const Text('Add Slot')),
      ]));
}

class _ScheduleSlotCard extends StatelessWidget {
  const _ScheduleSlotCard(
      {required this.slot,
      required this.busy,
      required this.onCancel,
      required this.onBlock,
      required this.onUnblock,
      this.appointment,
      this.chat});
  final SlotDto slot;
  final bool busy;
  final VoidCallback onCancel;
  final VoidCallback onBlock;
  final VoidCallback onUnblock;
  final AppointmentDto? appointment;
  final ChatDto? chat;

  String get _range {
    final s = slot.start, e = slot.end;
    if (s == null || e == null) return 'Time unavailable';
    return '${_shortTime(kenyaTime(s).toIso8601String())} – '
        '${_shortTime(kenyaTime(e).toIso8601String())}';
  }

  @override
  Widget build(BuildContext c) {
    final now = kenyaTime(DateTime.now());
    final blocked = slot.status == 'BLOCKED';
    final booked =
        slot.status == 'BOOKED' || slot.status == 'LOCKED_IN_PAYMENT';
    final live = booked &&
        slot.start != null &&
        slot.end != null &&
        now.isAfter(slot.start!) &&
        now.isBefore(slot.end!);
    final past = slot.end != null && !slot.end!.isAfter(now);
    final done =
        booked && (appointment?.status == 'COMPLETED' || (past && !live));
    // (label, icon, accent, row tint, pill bg, pill text)
    final (label, icon, accent, tint, pillBg, pillText) = blocked
        ? (
            'Blocked',
            Icons.block,
            const Color(0xFF9CA3AF),
            const Color(0xFFF5F7F7),
            _schBlocked,
            const Color(0xFF5B656B)
          )
        : !booked
            ? (
                // Named as in the legend; the body line says it is open
                // on the patient booking calendar.
                slot.bookable ? 'Available' : 'Past opening',
                Icons.event_available_outlined,
                const Color(0xFFF5B83D),
                const Color(0xFFFFFBEF),
                const Color(0xFFFEF3C7),
                const Color(0xFFB45309)
              )
            : done
                ? (
                    'Completed',
                    Icons.check_circle,
                    const Color(0xFF22C052),
                    const Color(0xFFF0FBF7),
                    const Color(0xFFD0FCD6),
                    const Color(0xFF0B7A35)
                  )
                : live
                    ? (
                        'In Progress',
                        Icons.videocam,
                        const Color(0xFF22C052),
                        const Color(0xFFF0FBF7),
                        const Color(0xFFD0FCD6),
                        const Color(0xFF0B7A35)
                      )
                    : (
                        'Upcoming',
                        Icons.calendar_month_outlined,
                        const Color(0xFF5CA7FD),
                        const Color(0xFFF4F9FE),
                        const Color(0xFFD9EBFE),
                        const Color(0xFF1D4ED8)
                      );
    final minutes = slot.start != null && slot.end != null
        ? slot.end!.difference(slot.start!).inMinutes
        : null;
    return Container(
        margin: const EdgeInsets.fromLTRB(14, 0, 14, 14),
        padding: const EdgeInsets.fromLTRB(4, 4, 4, 12),
        decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(18),
            boxShadow: const [
              BoxShadow(
                  color: Color(0x140F3D2E),
                  blurRadius: 14,
                  offset: Offset(0, 4))
            ]),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          // Time row with the coloured accent bar.
          Container(
              height: 40,
              decoration: BoxDecoration(
                  color: tint, borderRadius: BorderRadius.circular(14)),
              child: Row(children: [
                Container(
                    width: 3.5,
                    height: 28,
                    decoration: BoxDecoration(
                        color: accent, borderRadius: BorderRadius.circular(4))),
                const SizedBox(width: 11),
                // Time and length take the space left of the status pill,
                // which stays pinned right as in the reference.
                Expanded(
                    child: Row(children: [
                  const Icon(Icons.schedule,
                      size: 18, color: Color(0xFF0B6D3E)),
                  const SizedBox(width: 8),
                  // Scales down rather than cutting the time off.
                  Flexible(
                      child: FittedBox(
                          fit: BoxFit.scaleDown,
                          alignment: Alignment.centerLeft,
                          child: Text(_range,
                              maxLines: 1,
                              style: const TextStyle(
                                  fontSize: 14.5,
                                  fontWeight: FontWeight.w800,
                                  color: _schInk)))),
                  if (minutes != null) ...[
                    Container(
                        width: 1,
                        height: 18,
                        margin: const EdgeInsets.symmetric(horizontal: 7),
                        color: const Color(0xFFCBD5D1)),
                    Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 8, vertical: 2),
                        decoration: BoxDecoration(
                            color: const Color(0xFFDDF0E2),
                            borderRadius: BorderRadius.circular(10)),
                        child: Text('${minutes}m',
                            style: const TextStyle(
                                fontSize: 12.5,
                                fontWeight: FontWeight.w600,
                                color: _schInk))),
                  ],
                ])),
                const SizedBox(width: 6),
                Container(
                    margin: const EdgeInsets.only(right: 6),
                    padding:
                        const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
                    decoration: BoxDecoration(
                        color: pillBg, borderRadius: BorderRadius.circular(14)),
                    child: Row(mainAxisSize: MainAxisSize.min, children: [
                      Icon(icon,
                          size: 15,
                          color: label == 'Completed'
                              ? const Color(0xFF0BB63C)
                              : pillText),
                      const SizedBox(width: 5),
                      Text(label,
                          style: TextStyle(
                              fontSize: 12.5,
                              fontWeight: FontWeight.w600,
                              color: pillText)),
                    ])),
              ])),
          Padding(
              padding: const EdgeInsets.fromLTRB(12, 10, 12, 0),
              child: booked
                  ? _bookedBody(c)
                  : blocked
                      ? _blockedBody()
                      : _openBody(c)),
        ]));
  }

  Widget _bookedBody(BuildContext c) {
    final a = appointment;
    final name = slot.patientName ?? 'Patient';
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      InkWell(
          onTap: a == null
              ? null
              : () => Navigator.push(
                  c,
                  MaterialPageRoute(
                      builder: (_) => ConsultationPage(appointment: a))),
          borderRadius: BorderRadius.circular(12),
          child: Row(children: [
            Container(
                width: 40,
                height: 40,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                    gradient: const LinearGradient(
                        begin: Alignment.topLeft,
                        end: Alignment.bottomRight,
                        colors: [Color(0xFFE6FDEE), Color(0xFFCBF5DD)]),
                    borderRadius: BorderRadius.circular(11)),
                child: Text(_personInitials(name),
                    style: const TextStyle(
                        fontSize: 15,
                        fontWeight: FontWeight.w800,
                        color: Color(0xFF0A5B35)))),
            Container(
                width: 1,
                height: 26,
                margin: const EdgeInsets.symmetric(horizontal: 12),
                color: _schLine),
            Expanded(
                child: Text(name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 17,
                        fontWeight: FontWeight.w800,
                        color: _schInk))),
            if (a != null)
              const Icon(Icons.chevron_right,
                  size: 24, color: Color(0xFF60676D)),
          ])),
      const SizedBox(height: 10),
      const Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Padding(
            padding: EdgeInsets.only(top: 1),
            child:
                Icon(Icons.lock_outline, size: 16, color: Color(0xFF5B656B))),
        SizedBox(width: 8),
        Expanded(
            child: Text(
                'Booked — cannot be self-cancelled. Contact patient support to reschedule.',
                style: TextStyle(
                    fontSize: 12.5, height: 1.35, color: Color(0xFF5B656B)))),
      ]),
      const SizedBox(height: 10),
      const Divider(height: 1, color: Color(0xFFE3E7E8)),
      const SizedBox(height: 10),
      if (a != null)
        _SlotConsultationActions(appointment: a)
      else
        const Text('Refresh appointments to load consultation details.',
            style: TextStyle(fontSize: 12.5, color: _schMuted)),
    ]);
  }

  Widget _blockedBody() =>
      Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const Text('Blocked — unavailable for patient booking',
            style: TextStyle(fontSize: 13, color: Color(0xFF5B656B))),
        const SizedBox(height: 10),
        OutlinedButton(
            onPressed: busy ? null : onUnblock,
            style: OutlinedButton.styleFrom(
                foregroundColor: _schGreen,
                side: const BorderSide(color: _schGreen),
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(14)),
                minimumSize: const Size(0, 42)),
            child: const Text('Unblock Slot')),
      ]);

  Widget _openBody(BuildContext c) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        const Text('10-minute slot open on the patient booking calendar',
            style: TextStyle(fontSize: 13, color: Color(0xFF5B656B))),
        const SizedBox(height: 10),
        Row(children: [
          Expanded(
              child: OutlinedButton(
                  onPressed: busy
                      ? null
                      : () async {
                          final confirmed = await showDialog<bool>(
                              context: c,
                              builder: (dialogContext) => AlertDialog(
                                      title: const Text('Cancel this slot?'),
                                      content: Text(
                                          'This removes the open ${_timeRange(slot.start, slot.end)} slot from the patient booking calendar. This cannot be undone.'),
                                      actions: [
                                        TextButton(
                                            onPressed: () => Navigator.pop(
                                                dialogContext, false),
                                            child: const Text('Keep slot')),
                                        FilledButton(
                                            onPressed: () => Navigator.pop(
                                                dialogContext, true),
                                            style: FilledButton.styleFrom(
                                                backgroundColor:
                                                    DoctorTokens.danger),
                                            child: const Text('Cancel slot')),
                                      ]));
                          if (confirmed == true) onCancel();
                        },
                  style: OutlinedButton.styleFrom(
                      foregroundColor: DoctorTokens.danger,
                      side: const BorderSide(color: Color(0xFFF3C7C7)),
                      shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(14)),
                      minimumSize: const Size(0, 42)),
                  child: const Text('Cancel Slot ✕'))),
          const SizedBox(width: 10),
          Expanded(
              child: OutlinedButton(
                  onPressed: busy ? null : onBlock,
                  style: OutlinedButton.styleFrom(
                      foregroundColor: const Color(0xFF5B656B),
                      side: const BorderSide(color: _schLine),
                      shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(14)),
                      minimumSize: const Size(0, 42)),
                  child: const Text('Block'))),
        ]),
      ]);
}

/// The booked-slot actions of the populated reference. Same behaviour as
/// [ConsultationActions]: start (video or chat) and Write prescription while
/// the consultation can start; afterwards Follow-up messages and the
/// consultation history.
class _SlotConsultationActions extends StatefulWidget {
  const _SlotConsultationActions({required this.appointment});
  final AppointmentDto appointment;
  @override
  State<_SlotConsultationActions> createState() =>
      _SlotConsultationActionsState();
}

class _SlotConsultationActionsState extends State<_SlotConsultationActions> {
  bool busy = false;

  Future<void> openChat() async {
    if (busy) return;
    setState(() => busy = true);
    try {
      final chat = await context
          .read<DoctorCubit>()
          .repo
          .conversationForAppointment(widget.appointment);
      if (mounted) {
        await Navigator.push(context,
            MaterialPageRoute(builder: (_) => ChatThreadPage(chat: chat)));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Widget _link(IconData icon, String label, VoidCallback? onTap) => InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(10),
      child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 4),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            Icon(icon, size: 19, color: _schLink),
            const SizedBox(width: 7),
            Flexible(
                child: FittedBox(
                    fit: BoxFit.scaleDown,
                    child: Text(label,
                        maxLines: 1,
                        style: const TextStyle(
                            fontSize: 13.5,
                            fontWeight: FontWeight.w700,
                            color: _schLink)))),
          ])));

  @override
  Widget build(BuildContext context) {
    final a = widget.appointment;
    if (a.canStart) {
      return Row(children: [
        Expanded(
            flex: 11,
            child: FilledButton.icon(
                onPressed: busy || !a.isQueueActionable || a.joinBlocked
                    ? null
                    : a.isChat
                        ? openChat
                        : () => _callScreenGuard.push(
                            context, (_) => AgoraPage(appointmentId: a.id)),
                style: FilledButton.styleFrom(
                    backgroundColor: _schGreenDark,
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(14)),
                    minimumSize: const Size(0, 46),
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    textStyle: const TextStyle(
                        fontSize: 14, fontWeight: FontWeight.w700)),
                icon: Icon(
                    a.isChat ? Icons.chat_outlined : Icons.videocam_outlined,
                    size: 20),
                label: FittedBox(
                    fit: BoxFit.scaleDown,
                    child: Text(busy
                        ? 'Opening consultation...'
                        : (a.joinLockedLabel ?? a.startLabel))))),
        const SizedBox(width: 10),
        Expanded(
            flex: 8,
            child: OutlinedButton.icon(
                onPressed: () => Navigator.push(
                    context,
                    MaterialPageRoute(
                        builder: (_) => ScannerPage(initialAppointment: a))),
                style: OutlinedButton.styleFrom(
                    foregroundColor: const Color(0xFF0A5B35),
                    backgroundColor: const Color(0xFFF3FBF7),
                    side: const BorderSide(color: Color(0xFFCFE3DA)),
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(14)),
                    minimumSize: const Size(0, 46),
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                    textStyle: const TextStyle(
                        fontSize: 14, fontWeight: FontWeight.w700)),
                icon: const Icon(Icons.description_outlined, size: 19),
                label: const FittedBox(
                    fit: BoxFit.scaleDown, child: Text('Write prescription')))),
      ]);
    }
    final followUp = a.modality == 'VIDEO' && a.conversationId != null;
    final chatHistory = a.isChat && a.conversationId != null;
    final links = <Widget>[
      if (followUp)
        _link(Icons.chat_bubble_outline, 'Follow-up messages',
            busy ? null : openChat),
      _link(
          Icons.history,
          'View consultation history',
          busy
              ? null
              : chatHistory
                  ? openChat
                  : () => Navigator.push(
                      context,
                      MaterialPageRoute(
                          builder: (_) => HistoryPage(appointmentId: a.id)))),
    ];
    return Row(children: [
      for (var i = 0; i < links.length; i++) ...[
        if (i > 0)
          Container(
              width: 1,
              height: 26,
              margin: const EdgeInsets.symmetric(horizontal: 8),
              color: const Color(0xFFE3E7E8)),
        Expanded(
            flex: links.length == 2 && i == 1 ? 6 : 5,
            child: Center(child: links[i])),
      ]
    ]);
  }
}

const _monthNames = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec'
];

String _mediumDateTime(String? iso) {
  final parsed = DateTime.tryParse(iso ?? '');
  final t = parsed == null ? null : kenyaTime(parsed);
  if (t == null) return 'Date unavailable';
  return '${_monthNames[t.month - 1]} ${t.day} · ${_shortTime(t.toIso8601String())}';
}

String _money(String currency, String? value) {
  final n = double.tryParse(value ?? '');
  if (n == null) return '$currency —';
  final fixed = n.toStringAsFixed(2);
  final parts = fixed.split('.');
  final whole =
      parts[0].replaceAllMapped(RegExp(r'\B(?=(\d{3})+(?!\d))'), (m) => ',');
  return '$currency $whole.${parts[1]}';
}

(String, Color, Color) settlementBadge(String status) => switch (status) {
      'SETTLED_TO_DOCTOR' => (
          'Settled ✓',
          const Color(0xFFECFDF5),
          const Color(0xFF047857)
        ),
      'DISBURSED' => (
          'Disbursed ✓',
          const Color(0xFFECFDF5),
          const Color(0xFF047857)
        ),
      // PAYMENT_HELD is a real, non-final state — it must never be presented
      // as settled/escrow per the approved business wording.
      'PAYMENT_HELD' => (
          'Held',
          const Color(0xFFFEF3C7),
          const Color(0xFFB45309)
        ),
      'INITIATED' => (
          'Processing',
          DoctorTokens.surfaceSubtle,
          DoctorTokens.textSub
        ),
      'FAILED' => ('Failed', const Color(0xFFFEE2E2), DoctorTokens.danger),
      'REFUNDED_TO_PATIENT' => (
          'Refunded',
          DoctorTokens.surfaceSubtle,
          DoctorTokens.textSub
        ),
      _ => (status, DoctorTokens.surfaceSubtle, DoctorTokens.textSub),
    };

/// Replaces the real Earnings & Wallet UI whenever the server-authoritative
/// `GET /config/booking-policy` reports the financial module as deferred
/// (`FINANCIAL_MODULE_MODE=deferred`) — see `DoctorState.paymentRequired`.
/// A standalone widget, rather than inline in [_WalletState.build], so it
/// can be pumped and tested directly without pulling in `WalletPage`'s own
/// real `initState` network calls (`repo.ledger()`, etc.).
class DeferredWalletNotice extends StatelessWidget {
  const DeferredWalletNotice({super.key, this.actions = const []});
  final List<Widget> actions;
  @override
  Widget build(BuildContext context) => SafeArea(
      bottom: false,
      child: Scaffold(
          backgroundColor: DoctorTokens.bg,
          body: Column(children: [
            DoctorPageHeader(
                'Doctor Earnings & Wallet', 'Financial module not yet active',
                actions: actions),
            const Expanded(
                child: Center(
                    child: Padding(
                        padding: EdgeInsets.all(24),
                        child:
                            Column(mainAxisSize: MainAxisSize.min, children: [
                          Icon(Icons.info_outline,
                              color: DoctorTokens.textSub, size: 34),
                          SizedBox(height: 10),
                          Text(
                              'Payment collection is not yet active for this launch.',
                              textAlign: TextAlign.center,
                              style: TextStyle(fontWeight: FontWeight.w600)),
                          SizedBox(height: 6),
                          Text(
                              'Consultations are proceeding without charge. Earnings and payouts will appear here once the financial module is enabled.',
                              textAlign: TextAlign.center,
                              style: TextStyle(
                                  fontSize: 13, color: DoctorTokens.textSub)),
                        ])))),
          ])));
}

class WalletPage extends StatefulWidget {
  const WalletPage({super.key});
  @override
  State<WalletPage> createState() => _WalletState();
}

class _WalletState extends State<WalletPage> {
  List<Json> ledger = [];
  bool loading = false;
  String? ledgerError;
  Json? payoutMethod;
  bool payoutLoading = false;
  late String selectedMonth;
  Json? monthlyStatement;
  bool statementLoading = false;
  String? statementError;
  DateTime? statementUpdatedAt;
  bool statementOffline = false;
  @override
  void initState() {
    super.initState();
    final now = DateTime.now();
    selectedMonth = '${now.year}-${now.month.toString().padLeft(2, '0')}';
    loadLedger();
    loadPayoutMethod();
    loadMonthlyStatement();
  }

  Future<void> loadMonthlyStatement() async {
    final repo = context.read<DoctorCubit>().repo;
    final requestedMonth = selectedMonth;
    final cached = await repo.cachedMonthlyStatement(requestedMonth);
    final metadata = await repo.cached('statement_$requestedMonth');
    if (!mounted || selectedMonth != requestedMonth) return;
    if (cached != null) {
      setState(() {
        monthlyStatement = cached;
        statementUpdatedAt = metadata?.cachedAt;
        statementOffline = repo.isOffline;
      });
    }
    setState(() => statementLoading = true);
    try {
      final fresh = await repo.monthlyStatement(requestedMonth);
      if (!mounted || selectedMonth != requestedMonth) return;
      monthlyStatement = fresh;
      statementUpdatedAt = DateTime.now();
      statementOffline = false;
      statementError = null;
    } on ApiException catch (e) {
      if (!mounted || selectedMonth != requestedMonth) return;
      statementError = e.userMessage;
      statementOffline = repo.isOffline;
    } catch (_) {
      if (!mounted || selectedMonth != requestedMonth) return;
      statementError = 'Monthly statement is unavailable.';
    } finally {
      if (mounted && selectedMonth == requestedMonth) {
        setState(() => statementLoading = false);
      }
    }
  }

  void shiftStatementMonth(int delta) {
    final parts = selectedMonth.split('-').map(int.parse).toList();
    final next = DateTime(parts[0], parts[1] + delta, 1);
    final now = DateTime.now();
    final current = DateTime(now.year, now.month, 1);
    if (next.isAfter(current)) return;
    setState(() {
      selectedMonth = '${next.year}-${next.month.toString().padLeft(2, '0')}';
      monthlyStatement = null;
      statementError = null;
      statementLoading = true;
      statementUpdatedAt = null;
    });
    loadMonthlyStatement();
  }

  Future<void> loadLedger() async {
    final repo = context.read<DoctorCubit>().repo;
    final cached = await repo.cachedLedger();
    if (cached != null && mounted) {
      setState(() => ledger = cached);
    }
    if (!mounted) return;
    setState(() => loading = true);
    try {
      ledger = await repo.ledger();
      ledgerError = null;
    } on ApiException catch (e) {
      ledgerError = e.userMessage;
    } catch (_) {
      ledgerError = 'Ledger is unavailable.';
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  Future<void> loadPayoutMethod() async {
    final repo = context.read<DoctorCubit>().repo;
    final cached = await repo.cachedPayoutMethod();
    if (cached != null && mounted) setState(() => payoutMethod = cached);
    if (!mounted) return;
    setState(() => payoutLoading = true);
    try {
      payoutMethod = await repo.payoutMethod();
    } catch (_) {
      // Non-fatal: the breakdown/ledger are the primary content of this
      // screen. A missing payout method just means the card below prompts
      // the doctor to set one up.
    } finally {
      if (mounted) setState(() => payoutLoading = false);
    }
  }

  Future<void> editPayoutMethod() async {
    final repo = context.read<DoctorCubit>().repo;
    final bound = await showModalBottomSheet<Json>(
        context: context,
        isScrollControlled: true,
        builder: (_) => _PayoutBindingSheet(
              onDevConnected: () async {
                if (!mounted) return;
                try {
                  final connected = await repo.devBindPayoutMethod();
                  if (!mounted) return;
                  setState(() => payoutMethod = {
                        ...connected,
                        'configured': true,
                        'display_label': connected['masked_destination'] ??
                            'M-Pesa •••• 3812',
                      });
                } catch (_) {
                  if (mounted) {
                    ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
                        content: Text(
                            'Could not connect the development payout destination.')));
                  }
                  return;
                }
                ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
                    content: Text(
                        'DEV MODE: M-Pesa destination connected (simulated).'),
                    behavior: SnackBarBehavior.floating));
              },
              onBind: (phoneNumber) =>
                  repo.bindPayoutDestination(phoneNumber, makePrimary: true),
            ));
    if (bound == null || !mounted) return;
    setState(() => payoutMethod = {
          ...bound,
          'configured': true,
          'display_label':
              '${bound['method_type'] ?? 'MPESA'} ${bound['masked_destination'] ?? ''}',
        });
    ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
        content: Text('Payout destination connected.'),
        behavior: SnackBarBehavior.floating));
  }

  @override
  Widget build(BuildContext c) {
    final state = c.watch<DoctorCubit>().state;
    if (!state.paymentRequired) {
      return DeferredWalletNotice(actions: _notificationBellActions(c));
    }
    final w = state.wallet as WalletDto?;
    final currency = w?.currency ?? 'KES';
    // No AppBar here, so without this the custom DoctorPageHeader below
    // renders straight under the status bar (confirmed on-device).
    return SafeArea(
        bottom: false,
        child: Scaffold(
            backgroundColor: DoctorTokens.bg,
            body: w == null && (state.error != null || ledgerError != null)
                ? ErrorPanel(state.error ?? ledgerError!, onRetry: () {
                    c.read<DoctorCubit>().loadAll();
                    loadLedger();
                  })
                : w == null
                    ? const Center(child: CircularProgressIndicator())
                    : RefreshIndicator(
                        onRefresh: () async {
                          await c.read<DoctorCubit>().loadAll();
                          await loadLedger();
                        },
                        child: ListView(padding: EdgeInsets.zero, children: [
                          DoctorPageHeader('Doctor Earnings & Wallet',
                              'Transparent 20% platform charge reconciliation & settlements',
                              actions: _notificationBellActions(c),
                              onRefresh: () {
                            c.read<DoctorCubit>().loadAll();
                            loadLedger();
                          }),
                          Padding(
                              padding: const EdgeInsets.all(18),
                              child: Column(children: [
                                CacheStatusBar(
                                    offline: state.offline,
                                    refreshing: state.refreshing,
                                    lastUpdated: state.lastUpdatedAt),
                                if (state.error != null)
                                  InlineError(state.error!, onRetry: () {
                                    c.read<DoctorCubit>().loadAll();
                                    loadLedger();
                                  }),
                                EarningsBreakdownCard(
                                    wallet: w, consultCount: ledger.length),
                                const SizedBox(height: 12),
                                MonthlyStatementCard(
                                    month: selectedMonth,
                                    statement: monthlyStatement,
                                    loading: statementLoading,
                                    error: statementError,
                                    offline: statementOffline,
                                    lastUpdated: statementUpdatedAt,
                                    onPrevious: () => shiftStatementMonth(-1),
                                    onNext: () => shiftStatementMonth(1)),
                                const SizedBox(height: 12),
                                PayoutMethodCard(
                                    payoutMethod: payoutMethod,
                                    loading: payoutLoading,
                                    onTap: editPayoutMethod),
                              ])),
                          const Padding(
                              padding: EdgeInsets.fromLTRB(18, 0, 18, 8),
                              child: Row(children: [
                                Expanded(
                                    child: Text(
                                        'Consultation Ledger & 20% Debarment',
                                        style: TextStyle(
                                            fontSize: 13,
                                            fontWeight: FontWeight.w800))),
                                Text('Total − 20% = Final',
                                    style: TextStyle(
                                        fontSize: 10.5,
                                        color: DoctorTokens.textSub))
                              ])),
                          if (ledgerError != null)
                            Padding(
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 18),
                                child: InlineError(ledgerError!,
                                    onRetry: loadLedger)),
                          if (loading && ledger.isEmpty)
                            const Padding(
                                padding: EdgeInsets.all(24),
                                child:
                                    Center(child: CircularProgressIndicator()))
                          else if (ledger.isEmpty)
                            const Padding(
                                padding: EdgeInsets.all(32),
                                child: Center(
                                    child: Text(
                                        'No ledger entries returned by the server.')))
                          else
                            Padding(
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 18),
                                child: Column(
                                    children: ledger
                                        .map((x) => _LedgerCard(
                                            entry: x, currency: currency))
                                        .toList()))
                        ]))));
  }
}

class MonthlyStatementCard extends StatelessWidget {
  const MonthlyStatementCard(
      {super.key,
      required this.month,
      required this.statement,
      required this.loading,
      required this.error,
      required this.onPrevious,
      required this.onNext,
      this.offline = false,
      this.lastUpdated});
  final String month;
  final Json? statement;
  final bool loading;
  final String? error;
  final bool offline;
  final DateTime? lastUpdated;
  final VoidCallback onPrevious, onNext;

  String _label(String value) {
    final p = value.split('-');
    if (p.length != 2) return value;
    final date = DateTime(int.tryParse(p[0]) ?? 2000, int.tryParse(p[1]) ?? 1);
    return '${_monthNames[date.month - 1]} ${date.year}';
  }

  @override
  Widget build(BuildContext context) {
    final s = statement;
    final status = s?['status']?.toString() ?? 'IN_PROGRESS';
    final estimated = status != 'FINALIZED';
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
          color: DoctorTokens.surface,
          border: Border.all(color: DoctorTokens.border),
          borderRadius: BorderRadius.circular(18)),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Row(children: [
          const Expanded(
              child: Text('Monthly Payout Statement',
                  style:
                      TextStyle(fontSize: 13.5, fontWeight: FontWeight.w800))),
          IconButton(
              onPressed: onPrevious,
              icon: const Icon(Icons.chevron_left, size: 20),
              visualDensity: VisualDensity.compact),
          Text(_label(month),
              style:
                  const TextStyle(fontSize: 11, fontWeight: FontWeight.w800)),
          IconButton(
              onPressed: onNext,
              icon: const Icon(Icons.chevron_right, size: 20),
              visualDensity: VisualDensity.compact),
        ]),
        if (loading && s == null) const LinearProgressIndicator(minHeight: 2),
        if (error != null && !offline)
          Text(error!,
              style: const TextStyle(color: DoctorTokens.danger, fontSize: 11)),
        if (s != null) ...[
          if (offline)
            const Text('Offline — showing saved statement',
                style: TextStyle(fontSize: 11, color: DoctorTokens.textSub)),
          CacheStatusBar(
              offline: false, refreshing: loading, lastUpdated: lastUpdated),
          Text(
              estimated
                  ? 'ESTIMATED PAYOUT · IN PROGRESS'
                  : 'FINAL PAYABLE · FINALIZED',
              style: TextStyle(
                  fontSize: 10,
                  fontWeight: FontWeight.w800,
                  color: estimated
                      ? const Color(0xFFB45309)
                      : const Color(0xFF047857))),
          const SizedBox(height: 12),
          _statementRow(
              'Gross eligible earnings',
              _money(s['currency']?.toString() ?? '',
                  s['gross_earnings']?.toString())),
          _statementRow('Platform charge',
              '- ${_money(s['currency']?.toString() ?? '', s['platform_charge']?.toString())}'),
          _statementRow(
              'Adjustments',
              _money(s['currency']?.toString() ?? '',
                  s['adjustments']?.toString())),
          const Divider(height: 16),
          _statementRow(estimated ? 'Estimated payout' : 'Final payable',
              _money(s['currency']?.toString() ?? '', s['payable']?.toString()),
              strong: true),
          if (s['payout_status'] != null)
            _statementRow('Payout status',
                s['payout_status'].toString().replaceAll('_', ' ')),
          if (s['payout_destination'] is Map) ...[
            const SizedBox(height: 8),
            Text(
                'Statement destination · ${(s['payout_destination'] as Map)['masked_destination'] ?? 'Not assigned'}',
                style:
                    const TextStyle(fontSize: 11, color: DoctorTokens.textSub)),
          ],
        ],
      ]),
    );
  }

  Widget _statementRow(String label, String value, {bool strong = false}) =>
      Padding(
          padding: const EdgeInsets.symmetric(vertical: 3),
          child: Row(children: [
            Expanded(
                child: Text(label,
                    style: TextStyle(
                        fontSize: 11,
                        fontWeight:
                            strong ? FontWeight.w800 : FontWeight.w500))),
            Text(value,
                style: TextStyle(
                    fontSize: 12,
                    fontWeight: strong ? FontWeight.w900 : FontWeight.w700,
                    color:
                        strong ? const Color(0xFF047857) : DoctorTokens.text))
          ]));
}

class EarningsBreakdownCard extends StatelessWidget {
  const EarningsBreakdownCard(
      {super.key, required this.wallet, required this.consultCount});
  final WalletDto wallet;
  final int consultCount;
  @override
  Widget build(BuildContext c) {
    final currency = wallet.currency ?? 'KES';
    return Container(
        margin: const EdgeInsets.only(top: 10),
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
            color: DoctorTokens.surface,
            border: Border.all(color: DoctorTokens.border),
            borderRadius: BorderRadius.circular(18),
            boxShadow: const [
              BoxShadow(
                  color: Color(0x0A000000),
                  blurRadius: 16,
                  offset: Offset(0, 4))
            ]),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Expanded(
                child: Wrap(
                    crossAxisAlignment: WrapCrossAlignment.center,
                    spacing: 6,
                    children: [
                  const Text('Settlement Breakdown',
                      style: TextStyle(
                          fontSize: 13.5, fontWeight: FontWeight.w800)),
                  Container(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 6, vertical: 1),
                      decoration: BoxDecoration(
                          color: const Color(0xFFFEE2E2),
                          borderRadius: BorderRadius.circular(6)),
                      child: const Text('Lifetime · monthly statement below',
                          style: TextStyle(
                              fontSize: 10,
                              fontWeight: FontWeight.w800,
                              color: Color(0xFFB91C1C))))
                ])),
            Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                decoration: BoxDecoration(
                    color: const Color(0xFFECFDF5),
                    border: Border.all(color: const Color(0xFFA7F3D0)),
                    borderRadius: BorderRadius.circular(20)),
                child: const Text('Processed by Finance',
                    style: TextStyle(
                        fontSize: 10.5,
                        fontWeight: FontWeight.w700,
                        color: Color(0xFF059669))))
          ]),
          const Padding(
              padding: EdgeInsets.only(top: 10, bottom: 10),
              child: Divider(height: 1, color: DoctorTokens.border)),
          _EarningsStep(
              icon: '💰',
              label: '1. Total Earnings (Gross)',
              sub:
                  'Patient fees before platform commission ($consultCount consults)',
              value: _money(currency, wallet.gross),
              valueColor: const Color(0xFF1D4ED8),
              background: const Color(0xFFEFF6FF)),
          const SizedBox(height: 6),
          _EarningsStep(
              icon: '📉',
              label: '2. Platform Charge (20% Withheld)',
              sub: 'Debarred platform commission & operational fee',
              value: '- ${_money(currency, wallet.withheld)}',
              valueColor: DoctorTokens.danger,
              background: const Color(0xFFFEF2F2),
              tag: '-${wallet.platformChargePercent ?? '20'}%'),
          const SizedBox(height: 8),
          _EarningsStep(
              icon: '💵',
              label: '3. Final Earning (Net Take-Home)',
              sub:
                  'Lifetime earnings before monthly adjustments; not proof of payout',
              value: _money(currency, wallet.net),
              valueColor: const Color(0xFF065F46),
              background: const Color(0xFFD1FAE5),
              emphasize: true),
          Container(
              margin: const EdgeInsets.only(top: 10),
              padding: const EdgeInsets.all(11),
              decoration: BoxDecoration(
                  color: const Color(0xFFF8FAFC),
                  border: Border.all(color: const Color(0xFFE2E8F0)),
                  borderRadius: BorderRadius.circular(10)),
              child: const Row(children: [
                Icon(Icons.info_outline, size: 15, color: Color(0xFF2563EB)),
                SizedBox(width: 8),
                Expanded(
                    child: Text.rich(TextSpan(children: [
                  TextSpan(
                      text: 'Fee Calculation Basis: ',
                      style: TextStyle(
                          fontWeight: FontWeight.w800,
                          color: Color(0xFF334155),
                          fontSize: 11)),
                  TextSpan(
                      text:
                          'Total calculation is based including the platform charge 20%. The 20% platform charge covers payment gateway interchange, automated dispute handling, teleconsultation infrastructure, and 24/7 technical support.',
                      style: TextStyle(color: Color(0xFF334155), fontSize: 11))
                ])))
              ])),
          const SizedBox(height: 12),
          for (final entry in [
            ('Finalized unpaid', wallet.finalizedUnpaid),
            ('Assigned for disbursement — unpaid', wallet.scheduledUnpaid),
            ('Provider-confirmed paid', wallet.totalPaid),
          ])
            if (entry.$2 != null)
              Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: Row(children: [
                    Expanded(
                        child: Text(entry.$1,
                            style: const TextStyle(
                                fontSize: 11, color: DoctorTokens.textSub))),
                    Text(_money(currency, entry.$2),
                        style: const TextStyle(
                            fontSize: 12, fontWeight: FontWeight.w800)),
                  ])),
          Row(children: [
            Expanded(
                child: Container(
                    padding: const EdgeInsets.symmetric(vertical: 9),
                    decoration: BoxDecoration(
                        color: DoctorTokens.surfaceSubtle,
                        borderRadius: BorderRadius.circular(10)),
                    alignment: Alignment.center,
                    child: const Text('Scheduled by Finance',
                        style: TextStyle(
                            fontSize: 11.5,
                            fontWeight: FontWeight.w700,
                            color: DoctorTokens.primaryDark)))),
            const SizedBox(width: 8),
            Expanded(
                child: Container(
                    padding: const EdgeInsets.symmetric(vertical: 9),
                    decoration: BoxDecoration(
                        border: Border.all(color: DoctorTokens.border),
                        borderRadius: BorderRadius.circular(10)),
                    alignment: Alignment.center,
                    child: const Text('Statement ↓ below',
                        style: TextStyle(
                            fontSize: 11.5,
                            fontWeight: FontWeight.w700,
                            color: DoctorTokens.textSub))))
          ])
        ]));
  }
}

class _EarningsStep extends StatelessWidget {
  const _EarningsStep(
      {required this.icon,
      required this.label,
      required this.sub,
      required this.value,
      required this.valueColor,
      required this.background,
      this.tag,
      this.emphasize = false});
  final String icon, label, sub, value;
  final Color valueColor, background;
  final String? tag;
  final bool emphasize;
  @override
  Widget build(BuildContext c) => Container(
      padding:
          EdgeInsets.symmetric(horizontal: 12, vertical: emphasize ? 11 : 9),
      decoration: BoxDecoration(
          color: background,
          borderRadius: BorderRadius.circular(10),
          border: emphasize
              ? Border.all(color: const Color(0xFF10B981), width: 1.5)
              : null),
      child: Row(children: [
        Text(icon, style: const TextStyle(fontSize: 17)),
        const SizedBox(width: 10),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(label,
              maxLines: 2,
              style: TextStyle(
                  fontSize: emphasize ? 13 : 12,
                  fontWeight: FontWeight.w700,
                  color:
                      emphasize ? const Color(0xFF064E3B) : DoctorTokens.text)),
          Text(sub,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                  fontSize: 10,
                  color: emphasize
                      ? const Color(0xFF047857)
                      : DoctorTokens.textSub))
        ])),
        const SizedBox(width: 6),
        Flexible(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
          FittedBox(
              fit: BoxFit.scaleDown,
              child: Text(value,
                  maxLines: 1,
                  softWrap: false,
                  style: TextStyle(
                      fontSize: emphasize ? 18 : 13.5,
                      fontWeight: FontWeight.w800,
                      color: valueColor))),
          if (tag != null)
            Container(
                margin: const EdgeInsets.only(top: 2),
                padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                decoration: BoxDecoration(
                    color: const Color(0xFFFEE2E2),
                    borderRadius: BorderRadius.circular(5)),
                child: Text(tag!,
                    style: const TextStyle(
                        fontSize: 9,
                        fontWeight: FontWeight.w800,
                        color: Color(0xFFB91C1C))))
        ]))
      ]));
}

class PayoutMethodCard extends StatelessWidget {
  const PayoutMethodCard(
      {super.key,
      required this.payoutMethod,
      required this.loading,
      required this.onTap});
  final Json? payoutMethod;
  final bool loading;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext c) {
    final configured = payoutMethod?['configured'] == true;
    return InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(14),
        child: Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
                color: DoctorTokens.surface,
                border: Border.all(color: DoctorTokens.border),
                borderRadius: BorderRadius.circular(14)),
            child: Row(children: [
              Container(
                  width: 38,
                  height: 38,
                  decoration: BoxDecoration(
                      color: configured
                          ? DoctorTokens.primaryLight
                          : DoctorTokens.warningLight,
                      borderRadius: BorderRadius.circular(10)),
                  alignment: Alignment.center,
                  child: Icon(
                      configured
                          ? Icons.account_balance_wallet
                          : Icons.warning_amber_rounded,
                      size: 18,
                      color: configured
                          ? DoctorTokens.primaryDark
                          : const Color(0xFFB45309))),
              const SizedBox(width: 12),
              Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                    const Text('Payout Method',
                        style: TextStyle(
                            fontSize: 12.5, fontWeight: FontWeight.w800)),
                    const SizedBox(height: 2),
                    Text(
                        loading && payoutMethod == null
                            ? 'Loading…'
                            : configured
                                ? '${payoutMethod!['display_label']} · ${payoutMethod!['verification_status'] == 'DEV_SIMULATED' || payoutMethod!['binding_status'] == 'DEV_SIMULATED_CONNECTED' ? 'DEV SIMULATED · ' : ''}scheduled disbursements destination'
                                : 'Payout Setup Required — connect a provider account',
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            fontSize: 11, color: DoctorTokens.textSub))
                  ])),
              const Icon(Icons.chevron_right,
                  size: 18, color: DoctorTokens.textSub)
            ])));
  }
}

class _PayoutBindingSheet extends StatefulWidget {
  const _PayoutBindingSheet(
      {required this.onDevConnected, required this.onBind});
  final Future<void> Function() onDevConnected;

  /// Real bind call (`DoctorRepository.bindPayoutDestination`), given an
  /// already-normalized phone number — returns the bound destination's JSON
  /// on success, throws (typically `ApiException`) on failure, including the
  /// honest 409 the backend returns when the platform has no M-Pesa provider
  /// configured yet.
  final Future<Json> Function(String phoneNumber) onBind;

  @override
  State<_PayoutBindingSheet> createState() => _PayoutBindingSheetState();
}

class _PayoutBindingSheetState extends State<_PayoutBindingSheet> {
  final _phoneController = TextEditingController();
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _phoneController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    setState(() => _error = null);
    final normalized = normalizeKenyaPhone(_phoneController.text);
    if (normalized == null) {
      setState(() => _error = 'Enter a valid Kenyan phone number.');
      return;
    }
    setState(() => _submitting = true);
    try {
      final destination = await widget.onBind(normalized);
      if (!mounted) return;
      Navigator.pop(context, destination);
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = e is ApiException
            ? e.userMessage
            : 'Could not connect this payout destination.';
      });
    }
  }

  @override
  Widget build(BuildContext c) => Padding(
      padding: EdgeInsets.only(
          left: 20,
          right: 20,
          top: 18,
          bottom: MediaQuery.of(c).viewInsets.bottom + 28),
      child: SingleChildScrollView(
          child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
            const Text('Payout Setup',
                style: TextStyle(fontSize: 17, fontWeight: FontWeight.w800)),
            const SizedBox(height: 6),
            const Text(
                'Connect your M-Pesa number so Finance can send approved earnings to your verified destination.',
                style: TextStyle(fontSize: 11.5, color: DoctorTokens.textSub)),
            const SizedBox(height: 14),
            Container(
                width: double.infinity,
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                    color: DoctorTokens.warningLight,
                    borderRadius: BorderRadius.circular(12)),
                child: const Text(
                    'Provider authorization is required. AfyaMkononi never verifies a manually typed wallet or card number and never stores provider PINs or OTPs.',
                    style: TextStyle(
                        fontSize: 11, color: Color(0xFF92400E), height: 1.35))),
            const SizedBox(height: 14),
            TextField(
              controller: _phoneController,
              keyboardType: TextInputType.phone,
              decoration: const InputDecoration(
                hintText: 'e.g. 0712 345 678',
              ),
            ),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(_error!,
                  style: const TextStyle(fontSize: 11.5, color: Colors.red)),
            ],
            const SizedBox(height: 14),
            SizedBox(
                width: double.infinity,
                child: FilledButton.icon(
                    onPressed: _submitting ? null : _submit,
                    icon: _submitting
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(
                                strokeWidth: 2, color: Colors.white))
                        : const Icon(Icons.account_balance_wallet_outlined),
                    label: const Text('Connect M-Pesa'))),
            if (_devPayoutBinding) ...[
              const SizedBox(height: 10),
              SizedBox(
                  width: double.infinity,
                  child: OutlinedButton(
                      onPressed: () {
                        Navigator.pop(c);
                        widget.onDevConnected();
                      },
                      child: const Text('Connect M-Pesa (Dev Simulation)'))),
              const SizedBox(height: 6),
              const Center(
                  child: Text(
                      'Development simulation only. No provider request or payout state is changed.',
                      textAlign: TextAlign.center,
                      style: TextStyle(
                          fontSize: 10, color: DoctorTokens.textSub))),
            ],
          ])));
}

class _LedgerCard extends StatelessWidget {
  const _LedgerCard({required this.entry, required this.currency});
  final Json entry;
  final String currency;
  @override
  Widget build(BuildContext c) {
    final (badgeLabel, badgeBg, badgeFg) =
        settlementBadge(textOf(entry['payment_status']));
    final name = optionalText(entry['patient_display_name']) ?? 'Patient';
    return Container(
        margin: const EdgeInsets.only(bottom: 10),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        decoration: BoxDecoration(
            color: DoctorTokens.surface,
            border: Border.all(color: DoctorTokens.border),
            borderRadius: BorderRadius.circular(14)),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Row(children: [
                  Flexible(
                      child: Text(name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 13, fontWeight: FontWeight.w800))),
                  const SizedBox(width: 6),
                  StatusChip(badgeLabel, color: badgeBg, textColor: badgeFg)
                ]),
                const SizedBox(height: 2),
                Text(
                    '${textOf(entry['gateway'])} · ${_mediumDateTime(entry['created_at']?.toString())}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 11, color: DoctorTokens.textSub)),
                const SizedBox(height: 4),
                Wrap(
                    spacing: 6,
                    runSpacing: 2,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      Text.rich(TextSpan(children: [
                        const TextSpan(
                            text: 'Total: ',
                            style: TextStyle(
                                fontSize: 10.5, color: DoctorTokens.textSub)),
                        TextSpan(
                            text: _money(
                                currency, entry['gross_amount']?.toString()),
                            style: const TextStyle(
                                fontSize: 10.5,
                                fontWeight: FontWeight.w800,
                                color: DoctorTokens.text))
                      ])),
                      Text.rich(TextSpan(children: [
                        const TextSpan(
                            text: '20% Fee: ',
                            style: TextStyle(
                                fontSize: 10.5, color: DoctorTokens.danger)),
                        TextSpan(
                            text:
                                '- ${_money(currency, entry['platform_fee_amount']?.toString())}',
                            style: const TextStyle(
                                fontSize: 10.5,
                                fontWeight: FontWeight.w800,
                                color: DoctorTokens.danger))
                      ])),
                      Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 5, vertical: 1),
                          decoration: BoxDecoration(
                              color: const Color(0xFFF1F5F9),
                              border:
                                  Border.all(color: const Color(0xFFCBD5E1)),
                              borderRadius: BorderRadius.circular(5)),
                          child: const Text('Debarred 20%',
                              style: TextStyle(
                                  fontSize: 9,
                                  fontFamily: 'monospace',
                                  color: Color(0xFF475569))))
                    ])
              ])),
          const SizedBox(width: 8),
          Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
            Text('+ ${_money(currency, entry['net_amount']?.toString())}',
                style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w800,
                    color: Color(0xFF059669))),
            const Text('Final Earning',
                style: TextStyle(fontSize: 9.5, color: DoctorTokens.textSub))
          ])
        ]));
  }
}

/// Deterministic, non-fabricated avatar colors: a fixed demo palette cycled
/// by a hash of a stable id (patientId, falling back to the chat/appointment
/// id), not tied to any specific patient. Mirrors the prototype's per-card
/// avatar coloring without inventing per-name meaning.
const _avatarPalette = <(Color, Color)>[
  (Color(0xFFFEE2E2), Color(0xFFDC2626)),
  (Color(0xFFF3E8FF), Color(0xFF7E22CE)),
  (Color(0xFFFEF3C7), Color(0xFFB45309)),
  (Color(0xFFECFDF5), Color(0xFF059669)),
  (Color(0xFFEFF6FF), Color(0xFF2563EB)),
];
(Color, Color) _avatarColors(String seed) {
  var hash = 0;
  for (final unit in seed.codeUnits) {
    hash = (hash * 31 + unit) & 0x7fffffff;
  }
  return _avatarPalette[hash % _avatarPalette.length];
}

String _personInitials(String name) {
  final parts = name.trim().split(RegExp(r'\s+'));
  final initials = parts
      .where((p) => p.isNotEmpty)
      .take(2)
      .map((p) => p[0])
      .join()
      .toUpperCase();
  return initials.isEmpty ? '?' : initials;
}

String _shortTime(String? iso) {
  final parsed = DateTime.tryParse(iso ?? '');
  final time = parsed == null ? null : kenyaTime(parsed);
  if (time == null) return '—';
  final hour = time.hour % 12 == 0 ? 12 : time.hour % 12;
  return '$hour:${time.minute.toString().padLeft(2, '0')} ${time.hour >= 12 ? 'PM' : 'AM'}';
}

class ChatPage extends StatelessWidget {
  const ChatPage({super.key});
  @override
  Widget build(BuildContext c) => const ChatListPage(focus: null);
}

class ChatListPage extends StatefulWidget {
  const ChatListPage({super.key, this.focus});
  final AppointmentDto? focus;
  @override
  State<ChatListPage> createState() => _ChatListState();
}

class _ChatListState extends State<ChatListPage> {
  late final ScreenRefresh _refresh;
  late final LiveChatHook _live;
  @override
  void initState() {
    super.initState();
    _refresh = ScreenRefresh(() => context.read<DoctorCubit>().refreshChats(),
        interval: const Duration(seconds: 30),
        enabled: () => filter != 'closed',
        networkChanges: context.read<DoctorCubit>().connectivity?.changes);
    // New messages update the list at once.
    _live = LiveChatHook(
        wants: (nudge) => nudge.isResync || nudge.kind == 'CHAT',
        onNudge: () => unawaited(_refresh.run().catchError((Object _) {})));
    unawaited(context.read<DoctorCubit>().refreshChats());
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _refresh.attach(context);
  }

  @override
  void dispose() {
    _live.dispose();
    _refresh.dispose();
    super.dispose();
  }

  String query = '';
  String filter = 'all';

  List<ChatDto> _visible(List<dynamic> source) {
    final base = widget.focus == null
        ? source.cast<ChatDto>()
        : source
            .cast<ChatDto>()
            .where((chat) => chat.appointmentId == widget.focus!.id);
    return base.where((chat) {
      final haystack = (chat.patientName ?? '').toLowerCase();
      final matchesQuery =
          query.trim().isEmpty || haystack.contains(query.toLowerCase());
      final active = !chat.isClosed;
      final matchesFilter = filter == 'all' ||
          (filter == 'active' && active) ||
          (filter == 'closed' && !active);
      return matchesQuery && matchesFilter;
    }).toList();
  }

  bool searching = false;

  @override
  Widget build(BuildContext c) {
    final s = c.watch<DoctorCubit>().state;
    final all = s.chats.cast<ChatDto>().toList();
    final chats = _visible(s.chats);
    final activeCount = all.where((x) => !x.isClosed).length;
    final closedCount = all.length - activeCount;
    final cubit = c.read<DoctorCubit>();
    const filters = ['all', 'active', 'closed'];
    // The Peer Navigator chat list's look (product owner, 2026-11-20).
    // No AppBar here, so without SafeArea the header renders straight under
    // the status bar (confirmed on-device).
    return SafeArea(
        bottom: false,
        child: Scaffold(
            backgroundColor: DoctorTokens.bg,
            body: s.loading && s.chats.isEmpty
                ? const Center(child: CircularProgressIndicator())
                : s.error != null && s.chats.isEmpty
                    ? ErrorPanel(s.error!, onRetry: cubit.refreshChats)
                    : RefreshIndicator(
                        onRefresh: cubit.refreshChats,
                        child: ListView(
                            padding: const EdgeInsets.only(bottom: 24),
                            children: [
                              PeerHeader(
                                  title: 'Consultation Chats',
                                  subtitle: 'Your patient conversations',
                                  actions: [
                                    PeerCircleButton(
                                        key: const ValueKey(
                                            'doctor-chats-search'),
                                        icon: Icons.search_rounded,
                                        tooltip: 'Search',
                                        active: searching,
                                        onTap: () => setState(() {
                                              searching = !searching;
                                              if (!searching) query = '';
                                            })),
                                    ..._notificationBellActions(c),
                                  ]),
                              CacheStatusBar(
                                  offline: s.offline,
                                  refreshing: s.refreshing,
                                  lastUpdated: s.lastUpdatedAt),
                              if (s.error != null)
                                InlineError(s.error!,
                                    onRetry: cubit.refreshChats),
                              if (searching)
                                Padding(
                                    padding: const EdgeInsets.fromLTRB(
                                        18, 0, 18, 12),
                                    child: TextField(
                                        key: const ValueKey(
                                            'doctor-chats-query'),
                                        autofocus: true,
                                        onChanged: (v) =>
                                            setState(() => query = v),
                                        decoration: InputDecoration(
                                            hintText:
                                                'Search patient conversations…',
                                            prefixIcon: const Icon(
                                                Icons.search_rounded),
                                            filled: true,
                                            fillColor: Colors.white,
                                            border: OutlineInputBorder(
                                                borderRadius:
                                                    BorderRadius.circular(24),
                                                borderSide: const BorderSide(
                                                    color: PeerTokens.line))))),
                              PeerPillTabs(
                                  labels: const [
                                    'All Chats',
                                    'Active',
                                    'Expired'
                                  ],
                                  selected: filters.indexOf(filter),
                                  counts: {
                                    0: all.length,
                                    1: activeCount,
                                    2: closedCount
                                  },
                                  onSelected: (i) =>
                                      setState(() => filter = filters[i])),
                              const SizedBox(height: 14),
                              if (chats.isEmpty)
                                Padding(
                                    padding: const EdgeInsets.all(36),
                                    child: Text(
                                        query.trim().isNotEmpty
                                            ? 'No conversations match your search.'
                                            : filter == 'closed'
                                                ? 'Conversations move here when their consultation time is over. The history stays readable.'
                                                : 'No conversations found',
                                        textAlign: TextAlign.center,
                                        style: const TextStyle(
                                            color: PeerTokens.grey)))
                              else
                                for (final chat in chats)
                                  Padding(
                                      padding: const EdgeInsets.fromLTRB(
                                          14, 0, 14, 10),
                                      child: _ConvCard(
                                          chat: chat,
                                          onTap: () => Navigator.push(
                                              c,
                                              MaterialPageRoute(
                                                  builder: (_) =>
                                                      ChatThreadPage(
                                                          chat: chat))))),
                              if (s.hasMoreChats)
                                TextButton(
                                    onPressed: s.loadingMoreChats
                                        ? null
                                        : () => cubit.refreshChats(older: true),
                                    child: Text(s.loadingMoreChats
                                        ? 'Loading…'
                                        : 'Load more conversations'))
                            ]))));
  }
}

/// One conversation in the Peer Navigator chat list's look: photo, name,
/// what to do next, last activity and an Active / Expired pill.
class _ConvCard extends StatelessWidget {
  const _ConvCard({required this.chat, required this.onTap});
  final ChatDto chat;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext c) {
    final active = !chat.isClosed;
    final name = chat.patientName ?? 'Patient';
    final last = DateTime.tryParse(chat.lastMessageAt ?? '')
        ?.toUtc()
        .add(const Duration(hours: 3));
    final now = DateTime.now().toUtc().add(const Duration(hours: 3));
    return PeerCard(
        key: ValueKey('doctor-chat-${chat.id}'),
        radius: 22,
        onTap: onTap,
        padding: const EdgeInsets.fromLTRB(18, 16, 16, 16),
        child: Row(children: [
          PeerAvatar(name,
              size: 56,
              api: c.read<DoctorCubit>().repo.api,
              photoPath: chat.appointmentId == null
                  ? null
                  : appointmentCounterpartPhotoPath(chat.appointmentId!)),
          const SizedBox(width: 16),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 17,
                        fontWeight: FontWeight.w800,
                        color: PeerTokens.navy)),
                const SizedBox(height: 4),
                Text(active ? chat.actionLabel : 'Consultation time is over',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 14.5, color: PeerTokens.grey)),
              ])),
          const SizedBox(width: 8),
          Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
            Text(last == null ? '' : peerListTime(last, now: now),
                style: const TextStyle(fontSize: 13, color: PeerTokens.grey)),
            const SizedBox(height: 8),
            Container(
                padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
                decoration: BoxDecoration(
                    color: active ? PeerTokens.mint : PeerTokens.field,
                    borderRadius: BorderRadius.circular(14)),
                child: Text(active ? 'Active' : 'Expired',
                    style: TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w700,
                        color: active ? PeerTokens.green : PeerTokens.grey))),
          ]),
        ]));
  }
}

class ChatThreadPage extends StatefulWidget {
  const ChatThreadPage({super.key, required this.chat, this.inCall = false});
  final ChatDto chat;

  /// Shown over a live call (in-call chat): no "Start Video Call" button.
  final bool inCall;
  @override
  State<ChatThreadPage> createState() => _ChatThreadState();
}

class _ChatThreadState extends State<ChatThreadPage> {
  final input = TextEditingController();
  late final ScreenRefresh _screenRefresh;
  late final LiveChatHook _live;
  bool _polling = false;
  DateTime? _refreshFailure;
  bool hasOlder = true, loadingOlder = false;
  final _acknowledged = <String>{};
  final _displayed = <String>{};
  bool _ackBusy = false;
  List<MessageDto> messages = [];
  bool loading = true, sending = false, uploadingAttachment = false;
  Uint8List? pendingAttachment;
  String? pendingMime, pendingUploadKey, attachmentError;
  bool notSent = false;
  bool serverClosed = false;
  String? error;

  /// Read receipts and adaptive polling (2026-11-17): how far the patient has
  /// read, the text being sent (shown as "Sending…"), the last activity and
  /// the newest incoming message already marked read.
  DateTime? _peerReadUpTo;
  String? _pendingText;
  DateTime? _lastActivity;
  String? _lastMarkedRead;

  /// 3 s while active (a message in the last 2 minutes), 10 s for the next
  /// 10 minutes, then 30 s — instead of a fixed 10 s for every open chat.
  Duration _pollInterval(DateTime now) {
    final last = _lastActivity;
    if (last == null) return const Duration(seconds: 10);
    final quiet = now.difference(last);
    if (quiet < const Duration(minutes: 2)) return const Duration(seconds: 3);
    if (quiet < const Duration(minutes: 12)) {
      return const Duration(seconds: 10);
    }
    return const Duration(seconds: 30);
  }

  /// After every successful fetch: picks up the patient's read receipt, notes
  /// activity, and marks the newest patient message read (once each).
  void _afterRefresh(List<MessageDto> fetched) {
    if (!mounted) return;
    final cubit = context.read<DoctorCubit>();
    final receipt = cubit.repo.peerReadUpTo(widget.chat.id);
    if (receipt != null && receipt != _peerReadUpTo) {
      setState(() => _peerReadUpTo = receipt);
    }
    if (fetched.isNotEmpty) _lastActivity = DateTime.now();
    final me = cubit.state.profile?.userId;
    if (me == null) return;
    for (final m in messages.reversed) {
      if (m.senderId == me) continue;
      if (m.id != _lastMarkedRead) {
        _lastMarkedRead = m.id;
        unawaited(cubit.repo
            .markChatRead(widget.chat.id, m.id)
            .catchError((Object _) => _lastMarkedRead = null));
      }
      break;
    }
  }

  /// Rebuilds the moment the chat window closes, so a screen left open turns
  /// read-only and shows "Expired" without being reopened.
  Timer? _expiryTimer;

  @override
  void dispose() {
    _live.dispose();
    _screenRefresh.dispose();
    _expiryTimer?.cancel();
    pendingAttachment = null;
    input.dispose();
    super.dispose();
  }

  @override
  void initState() {
    super.initState();
    input.addListener(() {
      if (mounted) setState(() {});
    });
    load();
    unawaited(ack('CHAT_OPENED'));
    unawaited(ack('CONSULTATION_OPENED'));
    // A live nudge for this conversation fetches it at once.
    _live = LiveChatHook(
        wants: (nudge) => nudge.concerns(widget.chat.id),
        onNudge: () =>
            unawaited(_screenRefresh.run().catchError((Object _) {})),
        onConnectionChange: () => _screenRefresh.reschedule());
    _screenRefresh = ScreenRefresh(() async {
      if (_polling || !mounted) return;
      _polling = true;
      try {
        final next = await refreshMessages();
        final failedAt = _refreshFailure;
        if (failedAt != null && mounted) {
          try {
            await context.read<DoctorCubit>().repo.acknowledgeChat(
                widget.chat, 'CHAT_REFRESH_FAILED',
                clientOccurredAt: failedAt);
            await ack('CHAT_REFRESH_SUCCEEDED');
            _refreshFailure = null;
          } catch (_) {
            /* Retry observational acknowledgement after recovery. */
          }
        }
        if (mounted) {
          setState(() {
            messages = merge(messages, next);
            error = null;
          });
          _afterRefresh(next);
        }
      } catch (_) {
        _refreshFailure ??= DateTime.now().toUtc();
        /* Keep visible history; manual refresh reports errors. */
      } finally {
        _polling = false;
      }
    },
        interval: const Duration(seconds: 10),
        // 30 s throughout while live nudges arrive.
        intervalFor: () => _live.pollEvery(_pollInterval(DateTime.now())),
        enabled: () => !widget.chat.isClosed && !serverClosed,
        hasError: () => _refreshFailure != null,
        networkChanges: context.read<DoctorCubit>().connectivity?.changes);
    final expiresAt = DateTime.tryParse(widget.chat.expiresAt ?? '');
    final untilExpiry = expiresAt?.difference(DateTime.now().toUtc());
    if (untilExpiry != null && !untilExpiry.isNegative) {
      _expiryTimer = Timer(untilExpiry + const Duration(seconds: 1), () {
        if (mounted) setState(() {});
      });
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _screenRefresh.attach(context);
  }

  Future<List<MessageDto>> refreshMessages() async {
    final source = context.read<DoctorCubit>().repo;
    if (messages.isEmpty) return source.messages(widget.chat.id);
    final result = <MessageDto>[];
    var after = messages.last;
    // Catch up incrementally; older pages stay in memory and are never redownloaded by a tick.
    for (var pageNumber = 0; pageNumber < 4; pageNumber++) {
      final page = await source.newerMessages(widget.chat.id, after);
      result.addAll(page);
      if (page.length < 50) break;
      after = page.last;
    }
    return result;
  }

  List<MessageDto> merge(List<MessageDto> old, List<MessageDto> next) {
    final all = {for (final m in old) m.id: m, for (final m in next) m.id: m}
        .values
        .toList();
    all.sort((a, b) {
      final time = compareMessageInstants(a.sentAt, b.sentAt);
      return time == 0 ? a.id.compareTo(b.id) : time;
    });
    return all;
  }

  Future<void> ack(String event, {List<String> ids = const []}) async {
    try {
      await context
          .read<DoctorCubit>()
          .repo
          .acknowledgeChat(widget.chat, event, messages: ids);
    } catch (_) {
      /* Observational acknowledgement; sending remains server-authorized. */
    }
  }

  Future<void> acknowledgeDisplayed() async {
    if (_ackBusy || !mounted) return;
    final ids = _displayed.difference(_acknowledged).take(50).toList();
    if (ids.isEmpty) return;
    _ackBusy = true;
    try {
      await context.read<DoctorCubit>().repo.acknowledgeChat(
          widget.chat, 'CHAT_READ_ACKNOWLEDGED',
          messages: ids);
      _acknowledged.addAll(ids);
    } catch (_) {/* Retry after another displayed refresh. */} finally {
      _ackBusy = false;
    }
  }

  Future<void> loadOlder() async {
    if (loadingOlder || !hasOlder || messages.isEmpty) return;
    setState(() => loadingOlder = true);
    try {
      final old = await context
          .read<DoctorCubit>()
          .repo
          .olderMessages(widget.chat.id, messages.first);
      if (mounted) {
        setState(() {
          messages = merge(messages, old);
          hasOlder = old.length == 50;
        });
      }
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => loadingOlder = false);
    }
  }

  Future<void> load() async {
    if (mounted) setState(() => loading = true);
    try {
      final next = await refreshMessages();
      if (messages.isEmpty) hasOlder = next.length == 50;
      messages = merge(messages, next);
      error = null;
      _afterRefresh(next);
    } catch (_) {
      error = 'Unable to load this conversation.';
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  Future<void> send() async {
    final text = input.text.trim();
    if (text.isEmpty || sending || widget.chat.isClosed || serverClosed) return;
    await _sendText(text);
    if (!notSent) input.clear();
  }

  /// Shared by the text-field send button and the canned-reply pills below —
  /// mirrors the prototype's `sendDoctorCanned(text)`, which dispatches the
  /// preset clinical text immediately rather than merely prefilling the box
  /// (`prototype/index.html` ~14763).
  Future<void> _sendText(String text) async {
    if (text.isEmpty || sending || widget.chat.isClosed || serverClosed) return;
    setState(() {
      sending = true;
      _pendingText = text;
    });
    _lastActivity = DateTime.now();
    try {
      final sent = await context
          .read<DoctorCubit>()
          .repo
          .sendMessage(widget.chat.id, text);
      if (mounted) {
        // The stored message replaces the "Sending…" bubble in one frame.
        setState(() {
          notSent = false;
          _pendingText = null;
          messages = merge(messages, [MessageDto.fromJson(sent)]);
        });
      }
      _screenRefresh.reschedule();
      await load();
      // Refresh the bounded conversation-list cache after a confirmed send.
      if (mounted) await context.read<DoctorCubit>().loadAll();
    } catch (e) {
      if (mounted) {
        setState(() {
          notSent = true;
          if (e is ApiException && (e.status == 403 || e.status == 409)) {
            serverClosed = true;
          }
        });
      }
      if (mounted) showError(context, e);
    } finally {
      if (mounted) {
        setState(() {
          sending = false;
          _pendingText = null;
        });
      }
    }
  }

  Future<void> pickAttachment() async {
    if (uploadingAttachment || widget.chat.isClosed || serverClosed) return;
    final select = await showModalBottomSheet<String>(
        context: context,
        builder: (context) => SafeArea(
            child: Padding(
                padding: const EdgeInsets.all(20),
                child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text('Attach to consultation',
                          style: TextStyle(
                              fontSize: 20, fontWeight: FontWeight.w600)),
                      const SizedBox(height: 4),
                      const Text('A JPEG or PNG clinical image',
                          style: TextStyle(color: DoctorTokens.textSub)),
                      const SizedBox(height: 8),
                      ListTile(
                          leading: const Icon(Icons.photo_camera_outlined),
                          title: const Text('Take a photo'),
                          onTap: () => Navigator.pop(context, 'camera')),
                      ListTile(
                          leading: const Icon(Icons.photo_library_outlined),
                          title: const Text('Choose from gallery'),
                          onTap: () => Navigator.pop(context, 'gallery')),
                      ListTile(
                          leading: const Icon(Icons.description_outlined),
                          title: const Text('Choose a file'),
                          onTap: () => Navigator.pop(context, 'file')),
                    ]))));
    if (select == null || !mounted) return;
    if (select != 'file') {
      await _attachPhoto(
          select == 'camera' ? ImageSource.camera : ImageSource.gallery);
      return;
    }
    try {
      final result = await FilePicker.platform.pickFiles(
          type: FileType.custom,
          allowedExtensions: const ['jpg', 'jpeg', 'png'],
          withData: true);
      if (result == null) return;
      final file = result.files.single;
      Uint8List bytes;
      try {
        if (!mounted) return;
        if (file.size > 10 * 1024 * 1024) {
          throw const ApiException(413, 'Select an image smaller than 10 MB.');
        }
        bytes = file.bytes ?? await File(file.path!).readAsBytes();
      } finally {
        await FilePicker.platform.clearTemporaryFiles();
      }
      if (!mounted) return;
      pendingAttachment = bytes;
      pendingMime =
          file.extension?.toLowerCase() == 'png' ? 'image/png' : 'image/jpeg';
      pendingUploadKey = List.generate(
          16,
          (_) => Random.secure()
              .nextInt(256)
              .toRadixString(16)
              .padLeft(2, '0')).join();
      await uploadAttachment();
    } catch (e) {
      if (mounted) {
        setState(() => attachmentError = e is ApiException
            ? e.userMessage
            : 'Unable to select this image. Please retry.');
      }
    }
  }

  /// Camera or gallery. The picker's own copy is deleted once read, so the
  /// clinical photo is not left in a generic disk cache.
  Future<void> _attachPhoto(ImageSource source) async {
    try {
      final file = await ImagePicker().pickImage(
          source: source, imageQuality: 85, maxWidth: 2000, maxHeight: 2000);
      if (file == null || !mounted) return;
      final name = file.name.toLowerCase();
      final png = name.endsWith('.png');
      final jpeg = name.endsWith('.jpg') || name.endsWith('.jpeg');
      Uint8List? bytes;
      try {
        if (!png && !jpeg) {
          throw const ApiException(422, 'Select a JPEG or PNG image.');
        }
        if (await file.length() > 10 * 1024 * 1024) {
          throw const ApiException(413, 'Select an image smaller than 10 MB.');
        }
        bytes = await file.readAsBytes();
      } finally {
        try {
          final copy = File(file.path);
          if (await copy.exists()) await copy.delete();
        } catch (_) {/* best-effort cleanup */}
      }
      if (!mounted) return;
      pendingAttachment = bytes;
      pendingMime = png ? 'image/png' : 'image/jpeg';
      pendingUploadKey = List.generate(
          16,
          (_) => Random.secure()
              .nextInt(256)
              .toRadixString(16)
              .padLeft(2, '0')).join();
      await uploadAttachment();
    } catch (e) {
      if (mounted) {
        setState(() => attachmentError = e is ApiException
            ? e.userMessage
            : 'Unable to use this image. Please retry.');
      }
    }
  }

  Future<void> uploadAttachment() async {
    if (uploadingAttachment || pendingAttachment == null) return;
    setState(() {
      uploadingAttachment = true;
      attachmentError = null;
    });
    try {
      final message = await context
          .read<DoctorCubit>()
          .repo
          .uploadClinicalImage(widget.chat.id, pendingAttachment!, pendingMime!,
              pendingUploadKey!);
      if (!mounted) return;
      setState(() {
        messages = [...messages.where((m) => m.id != message.id), message];
        pendingAttachment = null;
        pendingMime = null;
        pendingUploadKey = null;
      });
    } catch (e) {
      if (mounted) {
        setState(() => attachmentError = e is ApiException && e.status == 409
            ? 'Document upload is currently unavailable. Your image was not sent.'
            : e is ApiException
                ? e.userMessage
                : 'Upload was not confirmed. Please retry.');
      }
    } finally {
      if (mounted) setState(() => uploadingAttachment = false);
    }
  }

  @override
  Widget build(BuildContext c) {
    final chat = widget.chat;
    final name = chat.patientName ?? 'Patient';
    final myUserId = c.watch<DoctorCubit>().state.profile?.userId;
    return Scaffold(
        backgroundColor: DoctorTokens.bg,
        body: SafeArea(
            child: Column(children: [
          _ChatTopBar(
              name: name,
              chat: chat,
              closed: chat.isClosed || serverClosed,
              inCall: widget.inCall),
          if (chat.expiresAt != null || chat.isClosed)
            _ChatExpiryBar(chat: chat),
          Expanded(
              child: loading
                  ? const Center(child: CircularProgressIndicator())
                  : error != null
                      ? ErrorPanel(error!, onRetry: load)
                      : messages.isEmpty
                          ? const Center(
                              child: Text('No messages yet',
                                  style:
                                      TextStyle(color: DoctorTokens.textSub)))
                          : ListView.builder(
                              reverse: true,
                              scrollCacheExtent:
                                  const ScrollCacheExtent.pixels(0),
                              padding: const EdgeInsets.all(16),
                              itemCount: messages.length +
                                  1 +
                                  (_pendingText != null ? 1 : 0),
                              itemBuilder: (_, index) {
                                final pending = _pendingText;
                                if (pending != null && index == 0) {
                                  return _PendingDoctorBubble(text: pending);
                                }
                                final i = pending != null ? index - 1 : index;
                                if (i == messages.length) {
                                  return hasOlder
                                      ? TextButton(
                                          onPressed:
                                              loadingOlder ? null : loadOlder,
                                          child: Text(loadingOlder
                                              ? 'Loading history...'
                                              : 'Load earlier messages'))
                                      : const SizedBox.shrink();
                                }
                                final m = messages[messages.length - 1 - i];
                                if (m.senderId != myUserId) {
                                  _displayed.add(m.id);
                                  WidgetsBinding.instance.addPostFrameCallback(
                                      (_) => acknowledgeDisplayed());
                                }
                                return ClinicalMessageBubble(
                                    message: m,
                                    myUserId: myUserId,
                                    receipt:
                                        receiptFor(m.sentAt, _peerReadUpTo),
                                    onViewAttachment: () => Navigator.push(
                                        c,
                                        MaterialPageRoute(
                                            builder: (_) => ClinicalImageViewer(
                                                load: () => c
                                                    .read<DoctorCubit>()
                                                    .repo
                                                    .clinicalImage(
                                                        chat.id, m.id)))));
                              })),
          if (chat.isClosed || serverClosed)
            const Padding(
                padding: EdgeInsets.all(14),
                child: Text('Consultation ended. This history is read-only.',
                    textAlign: TextAlign.center,
                    style: TextStyle(color: DoctorTokens.danger, fontSize: 12)))
          else ...[
            Container(
                padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
                decoration: const BoxDecoration(
                    color: DoctorTokens.surface,
                    border:
                        Border(top: BorderSide(color: DoctorTokens.border))),
                child: Column(children: [
                  if (notSent)
                    const Padding(
                        padding: EdgeInsets.only(bottom: 6),
                        child: Align(
                            alignment: Alignment.centerLeft,
                            child: Text(
                                'NOT SENT — reconnect and tap send again.',
                                style: TextStyle(
                                    color: DoctorTokens.danger,
                                    fontSize: 11)))),
                  if (attachmentError != null)
                    Text(attachmentError!,
                        style: const TextStyle(color: DoctorTokens.danger)),
                  if (pendingAttachment != null && !uploadingAttachment)
                    TextButton(
                        onPressed: uploadAttachment,
                        child: const Text('Retry image upload')),
                  Row(children: [
                    IconButton(
                        tooltip: 'Attach clinical image',
                        onPressed: uploadingAttachment ? null : pickAttachment,
                        icon: uploadingAttachment
                            ? const SizedBox(
                                width: 20,
                                height: 20,
                                child:
                                    CircularProgressIndicator(strokeWidth: 2))
                            : const Icon(Icons.attach_file,
                                color: DoctorTokens.primary),
                        constraints:
                            const BoxConstraints(minWidth: 48, minHeight: 48)),
                    Expanded(
                        child: Container(
                            decoration: BoxDecoration(
                                color: DoctorTokens.surfaceSubtle,
                                borderRadius: BorderRadius.circular(20),
                                border: Border.all(color: DoctorTokens.border)),
                            padding: const EdgeInsets.symmetric(horizontal: 14),
                            child: TextField(
                                controller: input,
                                textInputAction: TextInputAction.send,
                                onSubmitted: (_) => send(),
                                minLines: 1,
                                maxLines: 4,
                                maxLength: 2000,
                                style: const TextStyle(fontSize: 12.5),
                                decoration: const InputDecoration(
                                    filled: false,
                                    border: InputBorder.none,
                                    enabledBorder: InputBorder.none,
                                    focusedBorder: InputBorder.none,
                                    disabledBorder: InputBorder.none,
                                    hintText: 'Type your message',
                                    isDense: true,
                                    counterText: '',
                                    contentPadding:
                                        EdgeInsets.symmetric(vertical: 10))))),
                    const SizedBox(width: 8),
                    IconButton.filled(
                        tooltip: 'Send message',
                        onPressed:
                            sending || input.text.trim().isEmpty ? null : send,
                        style: IconButton.styleFrom(
                            minimumSize: const Size(48, 48),
                            backgroundColor: DoctorTokens.primary,
                            foregroundColor: Colors.white,
                            disabledBackgroundColor:
                                DoctorTokens.surfaceSubtle),
                        icon: sending
                            ? const SizedBox(
                                width: 20,
                                height: 20,
                                child: CircularProgressIndicator(
                                    strokeWidth: 2, color: Colors.white))
                            : const Icon(Icons.send, size: 20)),
                  ])
                ]))
          ]
        ])));
  }
}

class _ChatTopBar extends StatelessWidget {
  const _ChatTopBar(
      {required this.name,
      required this.chat,
      required this.closed,
      this.inCall = false});

  /// The chat can no longer be messaged (its time is over or it was closed).
  final bool closed;
  final bool inCall;
  final String name;
  final ChatDto chat;
  @override
  Widget build(BuildContext c) {
    final colors = _avatarColors(chat.patientId ?? chat.id);
    return Container(
        padding: const EdgeInsets.fromLTRB(14, 8, 14, 10),
        decoration: const BoxDecoration(
            color: DoctorTokens.surface,
            border: Border(bottom: BorderSide(color: DoctorTokens.border))),
        child: Row(children: [
          InkWell(
              onTap: () => Navigator.pop(c),
              borderRadius: BorderRadius.circular(11),
              child: Container(
                  width: 34,
                  height: 34,
                  decoration: BoxDecoration(
                      color: DoctorTokens.surfaceSubtle,
                      borderRadius: BorderRadius.circular(11),
                      border: Border.all(color: DoctorTokens.border)),
                  alignment: Alignment.center,
                  child: const Icon(Icons.arrow_back, size: 18))),
          const SizedBox(width: 8),
          PersonPhoto(
              api: c.read<DoctorCubit>().repo.api,
              path: chat.appointmentId == null
                  ? null
                  : appointmentCounterpartPhotoPath(chat.appointmentId!),
              size: 36,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: DoctorTokens.primary, width: 1.5),
              fallback: Container(
                  width: 36,
                  height: 36,
                  decoration: BoxDecoration(
                      color: colors.$1,
                      borderRadius: BorderRadius.circular(12),
                      border:
                          Border.all(color: DoctorTokens.primary, width: 1.5)),
                  alignment: Alignment.center,
                  child: Text(_personInitials(name),
                      style: TextStyle(
                          fontWeight: FontWeight.w800,
                          fontSize: 13,
                          color: colors.$2)))),
          const SizedBox(width: 10),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(name,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 13, fontWeight: FontWeight.w800)),
                const SizedBox(height: 3),
                StatusChip(closed ? 'Expired' : 'Active',
                    key: ValueKey(
                        closed ? 'chat-status-expired' : 'chat-status-active'),
                    color: closed
                        ? DoctorTokens.surfaceSubtle
                        : DoctorTokens.primaryLight,
                    textColor: closed
                        ? DoctorTokens.textSub
                        : DoctorTokens.primaryDark)
              ])),
          InkWell(
              onTap: chat.appointmentId == null
                  ? null
                  : () => Navigator.push(
                      c,
                      MaterialPageRoute(
                          builder: (_) =>
                              HistoryPage(appointmentId: chat.appointmentId!))),
              borderRadius: BorderRadius.circular(10),
              child: Container(
                  width: 34,
                  height: 34,
                  decoration: BoxDecoration(
                      color: DoctorTokens.surfaceSubtle,
                      borderRadius: BorderRadius.circular(10),
                      border: Border.all(color: DoctorTokens.border)),
                  alignment: Alignment.center,
                  child: const Icon(Icons.history, size: 17))),
          const SizedBox(width: 6),
          if (chat.modality == 'VIDEO' && !closed && !inCall)
            _ChatIconBtn(
                icon: Icons.videocam_outlined,
                tooltip: 'Start Video Call',
                onTap: chat.modality != 'VIDEO' ||
                        chat.appointmentId == null ||
                        chat.isClosed
                    ? null
                    : () => _callScreenGuard.push(c,
                        (_) => AgoraPage(appointmentId: chat.appointmentId!))),
          const SizedBox(width: 6),
          if (!chat.isClosed)
            _ChatIconBtn(
                icon: Icons.medical_services_outlined,
                tooltip: 'Snap & Send Rx',
                // Opens with this chat's own consultation selected.
                onTap: () => Navigator.push(
                    c,
                    MaterialPageRoute(
                        builder: (_) => ScannerPage(
                            initialAppointment: c
                                .read<DoctorCubit>()
                                .state
                                .queue
                                .whereType<AppointmentDto>()
                                .where((a) => a.id == chat.appointmentId)
                                .firstOrNull))))
        ]));
  }
}

/// Small square icon-button used in the chat thread's top bar for quick
/// actions (prototype `.chat-icon-btn`, `prototype/index.html` ~7470-7486).
class _ChatIconBtn extends StatelessWidget {
  const _ChatIconBtn(
      {required this.icon, required this.tooltip, required this.onTap});
  final IconData icon;
  final String tooltip;
  final VoidCallback? onTap;
  @override
  Widget build(BuildContext context) {
    return Tooltip(
        message: tooltip,
        child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(10),
            child: Container(
                width: 34,
                height: 34,
                decoration: BoxDecoration(
                    color: DoctorTokens.surfaceSubtle,
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: DoctorTokens.border)),
                alignment: Alignment.center,
                child: Icon(icon,
                    size: 17,
                    color: onTap == null
                        ? DoctorTokens.textSub.withValues(alpha: 0.5)
                        : null))));
  }
}

class _ChatExpiryBar extends StatelessWidget {
  const _ChatExpiryBar({required this.chat});
  final ChatDto chat;
  @override
  Widget build(BuildContext c) {
    final locked = chat.isClosed;
    final expiresAt = DateTime.tryParse(chat.expiresAt ?? '');
    final remaining = expiresAt?.difference(DateTime.now().toUtc());
    final label = locked
        ? 'Consultation time is over.'
        : (remaining == null || remaining.isNegative)
            ? 'Consultation chat active'
            : remaining.inHours > 0
                ? 'Chat open (${remaining.inHours}h ${remaining.inMinutes % 60}m left)'
                : 'Chat open (${remaining.inMinutes + 1} min left)';
    return Container(
        margin: const EdgeInsets.fromLTRB(14, 8, 14, 0),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration: BoxDecoration(
            color: locked ? const Color(0xFFFEF2F2) : const Color(0xFFECFDF5),
            border: Border.all(
                color:
                    locked ? const Color(0xFFFECACA) : const Color(0xFFA7F3D0)),
            borderRadius: BorderRadius.circular(10)),
        child: Row(children: [
          Icon(locked ? Icons.lock_outline : Icons.hourglass_bottom,
              size: 14,
              color: locked ? DoctorTokens.danger : const Color(0xFF065F46)),
          const SizedBox(width: 6),
          Expanded(
              child: Text(label,
                  style: TextStyle(
                      fontSize: 11,
                      color: locked
                          ? DoctorTokens.danger
                          : const Color(0xFF065F46))))
        ]));
  }
}

enum _PageStatus { pending, uploading, done, failed }

class _RxPage {
  _RxPage(this.file, {this.fromFilePicker = false});
  final XFile file;

  /// True when this page's on-disk file was created by file_picker
  /// (pickFile()) rather than image_picker (pick()) — cleanup for the two
  /// plugins uses different, plugin-safe mechanisms (see _cleanupPage).
  final bool fromFilePicker;
  _PageStatus status = _PageStatus.pending;
  String? error;

  /// Fraction sent so far (0.0-1.0) while `status == uploading`, from the
  /// real `onSendProgress` byte counter — null only before the first
  /// progress event arrives (or if the total size is unknown), in which case
  /// the UI falls back to an indeterminate spinner rather than a fake value.
  double? progress;
  bool get isPdf => file.path.toLowerCase().endsWith('.pdf');
}

/// Best-effort cleanup of one picked Rx page's on-disk file. Extracted as a
/// standalone, plugin-call-injectable function (rather than inlined in
/// `_ScannerState`) so its "never raw-delete a file_picker-sourced path"
/// safety property is directly unit-testable without needing to drive
/// image_picker/file_picker's platform channels.
///
/// Only ever deletes a file HelloDoctor itself is responsible for cleaning
/// up:
/// - image_picker-sourced pages (`fromFilePicker: false`): the plugin's own
///   app-cache-dir copy, via a direct delete of the exact path the plugin
///   handed back.
/// - file_picker-sourced pages (`fromFilePicker: true`): NEVER deleted by
///   path directly — file_picker can in principle surface a path outside its
///   managed cache depending on platform/options. Instead this calls the
///   plugin's own `clearTemporaryFiles()` API, which only ever clears
///   file_picker's own managed temp files and never a user's external
///   original. That call is skipped while `otherFilePickerPagesPending` is
///   true, so a batch of several file_picker-sourced pages doesn't pull a
///   file out from under a sibling still awaiting upload.
///
/// Failures are swallowed — cleanup must never surface to the UI or affect
/// upload/removal outcome.
@visibleForTesting
Future<void> cleanupPickedRxFile({
  required String path,
  required bool fromFilePicker,
  required bool otherFilePickerPagesPending,
  Future<void> Function() clearFilePickerTemp = _clearFilePickerTempFiles,
}) async {
  try {
    if (fromFilePicker) {
      if (!otherFilePickerPagesPending) await clearFilePickerTemp();
    } else {
      final f = File(path);
      if (await f.exists()) await f.delete();
    }
  } catch (_) {
    // best-effort only
  }
}

Future<void> _clearFilePickerTempFiles() async {
  await FilePicker.platform.clearTemporaryFiles();
}

const _maxRxPages = 5;

/// Whether a prescription can still be dispatched for [a]: a live
/// consultation, or a completed one inside the 24-hour late-Rx window whose
/// Rx decision is not yet recorded. Once dispatched (or "no Rx needed"),
/// never again.
bool rxDispatchAllowed(AppointmentDto a, [DateTime? now]) =>
    const ['CONFIRMED', 'WAITING', 'IN_CONSULTATION'].contains(a.status) ||
    rxWindowOpen(a, now);

class ScannerPage extends StatefulWidget {
  const ScannerPage(
      {super.key, this.initialAppointment, this.closeOnDispatch = false});
  final AppointmentDto? initialAppointment;

  /// Pops with `true` after a successful dispatch (used by the post-call
  /// close-out, which then closes too).
  final bool closeOnDispatch;
  @override
  State<ScannerPage> createState() => _ScannerState();
}

class _ScannerState extends State<ScannerPage> {
  final List<_RxPage> pages = [];
  final notes = TextEditingController();
  AppointmentDto? appointment;
  bool dispatching = false;

  /// Opened for a consultation whose Rx decision is already final.
  bool decidedNotice = false;

  /// Consultations dispatched from this screen instance.
  final Set<String> _dispatched = {};
  @override
  void initState() {
    super.initState();
    final initial = widget.initialAppointment;
    if (initial != null && !rxDispatchAllowed(initial)) {
      decidedNotice = true;
    } else {
      appointment = initial;
    }
  }

  @override
  void dispose() {
    // This screen's `pages` list is purely ephemeral in-memory state (never
    // persisted), so once this widget is torn down — whether by navigating
    // away or by the session ending at logout — any not-yet-uploaded page's
    // on-disk file becomes permanently unreachable PHI residue unless swept
    // here. Already-`done` pages were already cleaned up at upload-success
    // time in _uploadPage. The "pending siblings" snapshot is computed once,
    // up front, so multiple still-pending file_picker pages don't each defer
    // to one another (each seeing the others as "still pending") and cause
    // clearTemporaryFiles() to never fire.
    final remaining = pages.where((p) => p.status != _PageStatus.done).toList();
    final lastFilePickerIndex =
        remaining.lastIndexWhere((p) => p.fromFilePicker);
    for (var i = 0; i < remaining.length; i++) {
      final page = remaining[i];
      unawaited(cleanupPickedRxFile(
          path: page.file.path,
          fromFilePicker: page.fromFilePicker,
          // Only the LAST file_picker-sourced page in this batch triggers
          // the (process-wide) clearTemporaryFiles() call, so it fires
          // exactly once per dispose() even with several pending pages.
          otherFilePickerPagesPending: i != lastFilePickerIndex));
    }
    notes.dispose();
    super.dispose();
  }

  Future<void> pick(ImageSource source) async {
    final picker = ImagePicker();
    final List<XFile> selected;
    if (source == ImageSource.gallery) {
      selected = await picker.pickMultiImage(
          imageQuality: 85, maxWidth: 2000, maxHeight: 2000);
    } else {
      final one = await picker.pickImage(
          source: source, imageQuality: 85, maxWidth: 2000, maxHeight: 2000);
      selected = one == null ? <XFile>[] : [one];
    }
    if (selected.isEmpty || !mounted) return;
    setState(() {
      for (final item in selected) {
        if (pages.length < _maxRxPages) pages.add(_RxPage(item));
      }
    });
  }

  Future<void> pickFile() async {
    final selected = await FilePicker.platform.pickFiles(
        type: FileType.custom,
        allowedExtensions: const ['jpg', 'jpeg', 'png', 'pdf'],
        withData: false);
    final path = selected?.files.single.path;
    if (path == null || !mounted) return;
    setState(() {
      if (pages.length < _maxRxPages) {
        pages.add(_RxPage(XFile(path), fromFilePicker: true));
      }
    });
  }

  Future<void> _cleanupPage(_RxPage page) => cleanupPickedRxFile(
      path: page.file.path,
      fromFilePicker: page.fromFilePicker,
      otherFilePickerPagesPending:
          pages.any((p) => p != page && p.fromFilePicker));

  Future<void> _uploadPage(_RxPage page) async {
    if (appointment == null ||
        page.status == _PageStatus.uploading ||
        page.status == _PageStatus.done) {
      return;
    }
    final appointmentId = appointment!.id;
    final doctorNotes = notes.text.trim();
    setState(() {
      page.status = _PageStatus.uploading;
      page.error = null;
      page.progress = null;
    });
    final repo = context.read<DoctorCubit>().repo;
    try {
      final bytes = await File(page.file.path).readAsBytes();
      if (bytes.length > 10 * 1024 * 1024) {
        throw const ApiException(413, 'This page exceeds the 10 MB limit.');
      }
      await repo.uploadPrescription(appointmentId, base64Encode(bytes),
          doctorNotes.isEmpty ? null : doctorNotes,
          mimeType: page.isPdf
              ? 'application/pdf'
              : bytes.length >= 4 && bytes[0] == 0x89 && bytes[1] == 0x50
                  ? 'image/png'
                  : 'image/jpeg', onSendProgress: (sent, total) {
        if (!mounted || total <= 0) return;
        setState(() => page.progress = sent / total);
      });
      if (mounted) setState(() => page.status = _PageStatus.done);
      unawaited(_cleanupPage(page));
    } catch (e) {
      if (mounted) {
        setState(() {
          page.status = _PageStatus.failed;
          page.error =
              e is ApiException ? e.userMessage : 'Upload failed. Retry.';
        });
      }
    }
  }

  Future<void> dispatch() async {
    if (dispatching) return;
    if (pages.isEmpty) {
      showMessage(context, 'Capture or attach a prescription page first.');
      return;
    }
    if (appointment == null) {
      showMessage(
          context, 'Select the consultation this prescription belongs to.');
      return;
    }
    setState(() => dispatching = true);
    final doctorCubit = context.read<DoctorCubit>();
    var sealed = false;
    try {
      // Every page is attempted even if an earlier one fails — a doctor
      // must be able to see exactly which pages went through and retry
      // only the ones that didn't, not resend pages that already succeeded.
      for (final page in pages) {
        if (page.status != _PageStatus.done) await _uploadPage(page);
      }
      if (!mounted || pages.any((p) => p.status != _PageStatus.done)) return;
      // Every page is on the server: dispatching completes a live
      // consultation as COMPLETED_WITH_RX, after which the server refuses
      // further pages. (A late Rx for an already-completed consultation
      // keeps that consultation's recorded outcome.) Completion is
      // idempotent, so "Finish dispatch" can safely retry it.
      final a = appointment!;
      if (a.status != 'COMPLETED') {
        await doctorCubit.repo.complete(a.id, 'COMPLETED_WITH_RX');
      }
      sealed = true;
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => dispatching = false);
    }
    if (sealed && mounted) await _finishDispatch(doctorCubit);
  }

  /// Resets the screen after a successful dispatch: the consultation leaves
  /// the picker (its Rx decision is final) and the form starts empty.
  Future<void> _finishDispatch(DoctorCubit doctorCubit) async {
    final patient = appointment?.patientName ?? 'the patient';
    setState(() {
      if (appointment != null) _dispatched.add(appointment!.id);
      pages.clear();
      notes.clear();
      appointment = null;
      decidedNotice = false;
    });
    unawaited(doctorCubit.loadAll());
    await showDialog<void>(
        context: context,
        builder: (dialogContext) => AlertDialog(
                icon: const Icon(Icons.check_circle,
                    size: 56, color: DoctorTokens.primary),
                title: const Text('Prescription Submitted!',
                    textAlign: TextAlign.center),
                content: Text(
                    'The prescription has been added to $patient’s health record.',
                    textAlign: TextAlign.center),
                actions: [
                  FilledButton(
                      onPressed: () => Navigator.pop(dialogContext),
                      child: const Text('Done'))
                ]));
    if (mounted && widget.closeOnDispatch) Navigator.pop(context, true);
  }

  void remove(_RxPage page) {
    if (dispatching || page.status == _PageStatus.uploading) return;
    if (page.status != _PageStatus.done) unawaited(_cleanupPage(page));
    setState(() => pages.remove(page));
  }

  @override
  Widget build(BuildContext c) {
    final state = c.watch<DoctorCubit>().state;
    // Only consultations whose Rx can still be dispatched; one dispatched
    // in this screen leaves at once, before the queue reload lands.
    final appointments = state.queue
        .cast<AppointmentDto>()
        .where((a) => rxDispatchAllowed(a) && !_dispatched.contains(a.id))
        .toList();
    if (appointment != null &&
        !appointments.any((a) => a.id == appointment!.id)) {
      appointments.add(appointment!);
    }
    final uploading =
        dispatching || pages.any((p) => p.status == _PageStatus.uploading);
    final profile = state.profile;
    final doneCount = pages.where((p) => p.status == _PageStatus.done).length;
    final failedCount =
        pages.where((p) => p.status == _PageStatus.failed).length;
    final allDone = pages.isNotEmpty && doneCount == pages.length;
    // No AppBar here, so without this the custom DoctorPageHeader below
    // renders straight under the status bar (confirmed on-device).
    return SafeArea(
        bottom: false,
        child: Scaffold(
            backgroundColor: DoctorTokens.bg,
            body: ListView(padding: EdgeInsets.zero, children: [
              DoctorPageHeader('Smart Rx Scanner & Dispatch',
                  'Photo Capture Pad · Auto-Syncs Patient & Date',
                  actions: _notificationBellActions(c)),
              Padding(
                  padding: const EdgeInsets.all(18),
                  child: Column(children: [
                    CacheStatusBar(
                        offline: c.read<DoctorCubit>().repo.isOffline,
                        refreshing: false),
                    if (decidedNotice)
                      Container(
                          width: double.infinity,
                          margin: const EdgeInsets.only(bottom: 12),
                          padding: const EdgeInsets.all(14),
                          decoration: BoxDecoration(
                              color: DoctorTokens.noticeBg,
                              borderRadius: BorderRadius.circular(14)),
                          child: Row(children: [
                            const Icon(Icons.lock_outline,
                                color: DoctorTokens.primaryDark),
                            const SizedBox(width: 10),
                            Expanded(
                                child: Text(
                                    'The prescription decision for ${widget.initialAppointment?.patientName ?? 'this consultation'} is already recorded and cannot be changed.',
                                    style: const TextStyle(
                                        fontSize: 13,
                                        height: 1.4,
                                        color: DoctorTokens.fieldText)))
                          ])),
                    DoctorCard(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          const Text('Select consultation',
                              style: TextStyle(fontWeight: FontWeight.w800)),
                          const SizedBox(height: 6),
                          DropdownButton<String>(
                              isExpanded: true,
                              value: appointment?.id,
                              hint: const Text('Select current consultation'),
                              items: appointments
                                  .map((a) => DropdownMenuItem(
                                      value: a.id,
                                      child: Text(rxConsultationLabel(a),
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis)))
                                  .toList(),
                              onChanged: uploading
                                  ? null
                                  : (id) => setState(() {
                                        if (id == appointment?.id) return;
                                        appointment = appointments
                                            .firstWhere((a) => a.id == id);
                                        pages.clear();
                                        notes.clear();
                                      })),
                          if (appointment != null) ...[
                            const SizedBox(height: 10),
                            _RxMetaBar(appointment: appointment!),
                            const SizedBox(height: 10),
                            InkWell(
                                onTap: () => Navigator.push(
                                    c,
                                    MaterialPageRoute(
                                        builder: (_) => HistoryPage(
                                            appointmentId: appointment!.id))),
                                borderRadius: BorderRadius.circular(10),
                                child: Container(
                                    width: double.infinity,
                                    padding: const EdgeInsets.symmetric(
                                        horizontal: 12, vertical: 9),
                                    decoration: BoxDecoration(
                                        color: DoctorTokens.surfaceSubtle,
                                        borderRadius: BorderRadius.circular(10),
                                        border: Border.all(
                                            color: DoctorTokens.border)),
                                    child: const Row(children: [
                                      Icon(Icons.history,
                                          size: 16,
                                          color: DoctorTokens.primaryDark),
                                      SizedBox(width: 8),
                                      Text('Patient Case History',
                                          style: TextStyle(
                                              fontSize: 12,
                                              fontWeight: FontWeight.w700,
                                              color: DoctorTokens.primaryDark))
                                    ])))
                          ]
                        ])),
                    const SizedBox(height: 12),
                    DoctorCard(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          const Text('Capture Handwritten Prescription',
                              style: TextStyle(fontWeight: FontWeight.w800)),
                          const SizedBox(height: 10),
                          if (pages.isNotEmpty) ...[
                            _RxPagesStrip(
                                pages: pages,
                                onRemove: remove,
                                onRetry: _uploadPage),
                            const SizedBox(height: 8),
                            Text(
                                '${pages.length}/$_maxRxPages pages'
                                '${failedCount > 0 ? ' · $failedCount failed' : doneCount > 0 ? ' · $doneCount dispatched' : ''}',
                                style: const TextStyle(
                                    color: DoctorTokens.textSub, fontSize: 11)),
                            const SizedBox(height: 10)
                          ],
                          Row(children: [
                            Expanded(
                                child: FilledButton.icon(
                                    onPressed: dispatching ||
                                            pages.length >= _maxRxPages
                                        ? null
                                        : () => pick(ImageSource.camera),
                                    icon: const Icon(Icons.camera_alt),
                                    label: const Text('Camera'))),
                            const SizedBox(width: 8),
                            Expanded(
                                child: OutlinedButton.icon(
                                    onPressed: dispatching ||
                                            pages.length >= _maxRxPages
                                        ? null
                                        : () => pick(ImageSource.gallery),
                                    icon: const Icon(Icons.photo_library),
                                    label: const Text('Gallery')))
                          ]),
                          const SizedBox(height: 8),
                          SizedBox(
                              width: double.infinity,
                              child: OutlinedButton.icon(
                                  onPressed:
                                      dispatching || pages.length >= _maxRxPages
                                          ? null
                                          : pickFile,
                                  icon: const Icon(Icons.attach_file),
                                  label:
                                      const Text('Choose image or PDF file'))),
                          const SizedBox(height: 14),
                          const Text('Advice for the patient (optional)',
                              style: TextStyle(
                                  fontWeight: FontWeight.w700, fontSize: 12)),
                          const SizedBox(height: 6),
                          TextField(
                              controller: notes,
                              maxLines: 3,
                              maxLength: 1000,
                              style: const TextStyle(fontSize: 12.5),
                              decoration: const InputDecoration(
                                  hintText:
                                      'e.g. Take after meals with warm water…',
                                  isDense: true,
                                  counterText: '')),
                          if (profile != null) ...[
                            const SizedBox(height: 14),
                            _RxPadFooter(profile: profile)
                          ],
                          const SizedBox(height: 14),
                          SizedBox(
                              width: double.infinity,
                              child: FilledButton(
                                  onPressed: dispatching ? null : dispatch,
                                  child: dispatching
                                      ? const SizedBox(
                                          width: 18,
                                          height: 18,
                                          child: CircularProgressIndicator(
                                              strokeWidth: 2,
                                              color: Colors.white))
                                      // allDone here means every page is
                                      // stored but completion did not go
                                      // through yet (the screen resets on
                                      // success).
                                      : Text(allDone
                                          ? 'Finish dispatch'
                                          : failedCount > 0
                                              ? 'Retry failed pages'
                                              : 'Dispatch Prescription to Patient'))),
                          if (pages.isNotEmpty &&
                              c.read<DoctorCubit>().repo.isOffline)
                            const Padding(
                                padding: EdgeInsets.only(top: 8),
                                child: Text(
                                    'NOT SENT — internet connection required.',
                                    style:
                                        TextStyle(color: DoctorTokens.danger)))
                        ]))
                  ]))
            ])));
  }
}

class _RxMetaBar extends StatelessWidget {
  const _RxMetaBar({required this.appointment});
  final AppointmentDto appointment;
  @override
  Widget build(BuildContext c) {
    final today = DateTime.now();
    final date = '${today.year}-${today.month.toString().padLeft(2, '0')}-'
        '${today.day.toString().padLeft(2, '0')}';
    Widget cell(String label, String value) => Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(label,
              style:
                  const TextStyle(fontSize: 9.5, color: DoctorTokens.textSub)),
          const SizedBox(height: 2),
          Text(value,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style:
                  const TextStyle(fontSize: 11.5, fontWeight: FontWeight.w700))
        ]));
    return Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
        decoration: BoxDecoration(
            color: DoctorTokens.surfaceSubtle,
            borderRadius: BorderRadius.circular(10)),
        child: Row(children: [
          cell('PATIENT', appointment.patientName ?? 'Patient'),
          cell('DATE', date),
          cell('TRACKING', appointment.number ?? appointment.id)
        ]));
  }
}

class _RxPagesStrip extends StatelessWidget {
  const _RxPagesStrip(
      {required this.pages, required this.onRemove, required this.onRetry});
  final List<_RxPage> pages;
  final ValueChanged<_RxPage> onRemove;
  final ValueChanged<_RxPage> onRetry;
  @override
  Widget build(BuildContext c) => SizedBox(
      height: 92,
      child: ListView.separated(
          scrollDirection: Axis.horizontal,
          itemCount: pages.length,
          separatorBuilder: (_, __) => const SizedBox(width: 8),
          itemBuilder: (_, i) => _RxPageThumb(
              page: pages[i],
              index: i,
              onRemove: () => onRemove(pages[i]),
              onRetry: () => onRetry(pages[i]))));
}

class _RxPageThumb extends StatelessWidget {
  const _RxPageThumb(
      {required this.page,
      required this.index,
      required this.onRemove,
      required this.onRetry});
  final _RxPage page;
  final int index;
  final VoidCallback onRemove, onRetry;
  @override
  Widget build(BuildContext c) => SizedBox(
      width: 78,
      child: Stack(clipBehavior: Clip.none, children: [
        Container(
            width: 78,
            height: 78,
            decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(10),
                border: Border.all(
                    color: page.status == _PageStatus.failed
                        ? DoctorTokens.danger
                        : DoctorTokens.border,
                    width: page.status == _PageStatus.failed ? 1.5 : 1)),
            clipBehavior: Clip.antiAlias,
            child: page.isPdf
                ? const ColoredBox(
                    color: DoctorTokens.surfaceSubtle,
                    child: Center(
                        child: Icon(Icons.picture_as_pdf,
                            color: DoctorTokens.muted)))
                : Image.file(File(page.file.path),
                    fit: BoxFit.cover,
                    // A `done` page's on-disk file is deleted immediately
                    // after a successful upload (PHI lifecycle cleanup); the
                    // thumbnail falls back to a status icon rather than
                    // surfacing a raw file-not-found error.
                    errorBuilder: (_, __, ___) => ColoredBox(
                        color: DoctorTokens.surfaceSubtle,
                        child: Center(
                            child: Icon(
                                page.status == _PageStatus.done
                                    ? Icons.check_circle
                                    : Icons.image_not_supported,
                                color: DoctorTokens.muted))))),
        Positioned(
            left: 4,
            bottom: 4,
            child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                decoration: BoxDecoration(
                    color: Colors.black.withValues(alpha: .55),
                    borderRadius: BorderRadius.circular(4)),
                child: Text('${index + 1}',
                    style: const TextStyle(
                        color: Colors.white,
                        fontSize: 9,
                        fontWeight: FontWeight.w700)))),
        if (page.status == _PageStatus.uploading)
          Positioned.fill(
              child: ColoredBox(
                  color: const Color(0x66000000),
                  child: Center(
                      child: Column(mainAxisSize: MainAxisSize.min, children: [
                    SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: Colors.white,
                            // Real upload progress from `onSendProgress`
                            // (bytes sent / total) once known; `null`
                            // renders Flutter's own indeterminate
                            // animation for the brief window before the
                            // first progress event arrives.
                            value: page.progress)),
                    if (page.progress != null) ...[
                      const SizedBox(height: 4),
                      Text('${(page.progress! * 100).round()}%',
                          style: const TextStyle(
                              color: Colors.white,
                              fontSize: 9,
                              fontWeight: FontWeight.w700))
                    ]
                  ])))),
        if (page.status == _PageStatus.done)
          const Positioned(
              right: 2,
              top: 2,
              child:
                  Icon(Icons.check_circle, color: Color(0xFF10B981), size: 18)),
        if (page.status == _PageStatus.failed)
          Positioned(
              right: 2,
              top: 2,
              child: InkWell(
                  onTap: onRetry,
                  child: const Icon(Icons.error, color: Colors.red, size: 18))),
        if (page.status != _PageStatus.uploading)
          Positioned(
              left: -6,
              top: -6,
              child: InkWell(
                  onTap: onRemove,
                  borderRadius: BorderRadius.circular(10),
                  child: Container(
                      width: 20,
                      height: 20,
                      decoration: const BoxDecoration(
                          color: DoctorTokens.text, shape: BoxShape.circle),
                      alignment: Alignment.center,
                      child: const Icon(Icons.close,
                          size: 13, color: Colors.white))))
      ]));
}

class _RxPadFooter extends StatelessWidget {
  const _RxPadFooter({required this.profile});
  final ProfileDto profile;
  @override
  Widget build(BuildContext c) => Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
          color: DoctorTokens.surfaceSubtle,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: DoctorTokens.border)),
      child: Row(children: [
        const Text('℞',
            style: TextStyle(
                fontSize: 22,
                fontWeight: FontWeight.w800,
                color: DoctorTokens.primaryDark)),
        const SizedBox(width: 10),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(profile.name,
              style:
                  const TextStyle(fontSize: 12.5, fontWeight: FontWeight.w800)),
          Text('KMPDC Reg #${profile.license}',
              style:
                  const TextStyle(fontSize: 10.5, color: DoctorTokens.textSub))
        ]))
      ]));
}

String _relativeAge(String? iso) {
  final t = DateTime.tryParse(iso ?? '');
  if (t == null) return 'Recently';
  final d = DateTime.now().toUtc().difference(t.toUtc());
  if (d.inMinutes < 1) return 'Just now';
  if (d.inHours < 1) return '${d.inMinutes}m ago';
  if (d.inDays < 1) return '${d.inHours}h ago';
  return '${d.inDays}d ago';
}

/// Q&A deliberately does not reuse `.dir-header`/`DoctorPageHeader` — the
/// prototype gives it its own `.flow-top`/`.flow-back-btn` structure
/// (prototype/index.html line ~7533), which this ports directly rather than
/// forcing component reuse.
class _QnaFlowTop extends StatelessWidget {
  const _QnaFlowTop({this.license});
  final String? license;
  @override
  Widget build(BuildContext c) => Container(
      padding: const EdgeInsets.fromLTRB(14, 10, 14, 10),
      decoration: const BoxDecoration(
          color: DoctorTokens.surface,
          border: Border(bottom: BorderSide(color: DoctorTokens.border))),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        IconButton(
            tooltip: 'Back to Queue',
            onPressed: () => Navigator.maybePop(c),
            icon: const Icon(Icons.arrow_back, size: 18)),
        const SizedBox(width: 6),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          const Text('Open Health Q&A Triage',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.w800)),
          const Text('Answer anonymous community questions',
              style: TextStyle(fontSize: 10.5, color: DoctorTokens.textSub)),
          if (license != null)
            Padding(
                padding: const EdgeInsets.only(top: 6),
                child: StatusChip('KMPDC #$license',
                    color: DoctorTokens.primaryLight,
                    textColor: DoctorTokens.primaryDark))
        ]))
      ]));
}

/// Both Queue entry points use a bounded, keyboard-aware full-height route.
Future<void> showDoctorQna(BuildContext context) {
  unawaited(context.read<DoctorCubit>().refreshQuestions());
  return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => const QnaPage());
}

class _QnaHeroBanner extends StatelessWidget {
  const _QnaHeroBanner();
  @override
  Widget build(BuildContext c) => Container(
      margin: const EdgeInsets.fromLTRB(18, 12, 18, 4),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
          gradient: const LinearGradient(
              colors: [Color(0xFF064E3B), Color(0xFF047857)]),
          borderRadius: BorderRadius.circular(18),
          boxShadow: const [
            BoxShadow(
                color: Color(0x33047857), blurRadius: 14, offset: Offset(0, 4))
          ]),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Container(
              padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
              decoration: BoxDecoration(
                  color: const Color(0xFFA7F3D0),
                  borderRadius: BorderRadius.circular(6)),
              child: const Text('Physician Community Care',
                  style: TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w800,
                      color: Color(0xFF064E3B)))),
          const SizedBox(height: 7),
          const Text('Resolve Open Patient Inquiries',
              style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w800,
                  color: Colors.white,
                  height: 1.3)),
          const SizedBox(height: 2),
          const Text('Answers are shared with the community',
              style: TextStyle(fontSize: 11, color: Colors.white70))
        ])),
        Container(
            width: 38,
            height: 38,
            decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: .18),
                borderRadius: BorderRadius.circular(12)),
            alignment: Alignment.center,
            child: const Icon(Icons.question_answer_outlined,
                color: Colors.white, size: 22))
      ]));
}

class QnaPage extends StatefulWidget {
  const QnaPage({super.key});
  @override
  State<QnaPage> createState() => _QnaPageState();
}

class _QnaPageState extends State<QnaPage> {
  String filter = 'OPEN';
  @override
  Widget build(BuildContext c) {
    final s = c.watch<DoctorCubit>().state;
    final allQuestions = s.questions.cast<QnaDto>();
    final questions = allQuestions
        .where((q) => filter == 'ALL' || q.status == filter)
        .toList();
    final unanswered = allQuestions.where((q) => q.status == 'OPEN').length;
    return Scaffold(
        backgroundColor: DoctorTokens.bg,
        // QnaPage is presented as a full-bleed modal sheet (showModalBottomSheet),
        // not a normal pushed route, so nothing else gives its custom
        // flow-header a top inset — without this it renders straight under
        // the status bar.
        body: SafeArea(
            child: Column(children: [
          _QnaFlowTop(license: s.profile?.license),
          Expanded(
              child: s.loading && questions.isEmpty
                  ? const Center(child: CircularProgressIndicator())
                  : s.error != null && questions.isEmpty
                      ? ErrorPanel(
                          'Questions could not be loaded. Please retry.',
                          onRetry: c.read<DoctorCubit>().refreshQuestions)
                      : RefreshIndicator(
                          onRefresh: c.read<DoctorCubit>().refreshQuestions,
                          child: ListView(
                              keyboardDismissBehavior:
                                  ScrollViewKeyboardDismissBehavior.onDrag,
                              padding: const EdgeInsets.only(bottom: 32),
                              children: [
                                CacheStatusBar(
                                    offline: s.offline,
                                    refreshing: s.refreshing,
                                    lastUpdated: s.lastUpdatedAt),
                                if (s.error != null)
                                  InlineError(
                                      'Questions could not be refreshed. Please retry.',
                                      onRetry: c
                                          .read<DoctorCubit>()
                                          .refreshQuestions),
                                const _QnaHeroBanner(),
                                SingleChildScrollView(
                                    scrollDirection: Axis.horizontal,
                                    padding: const EdgeInsets.fromLTRB(
                                        18, 12, 18, 0),
                                    child: Row(children: [
                                      for (final choice in const [
                                        ('Unanswered', 'OPEN'),
                                        ('Answered', 'ANSWERED'),
                                        ('All', 'ALL')
                                      ])
                                        _QueueFilter(
                                            choice.$1,
                                            choice.$2,
                                            filter,
                                            (value) =>
                                                setState(() => filter = value)),
                                    ])),
                                Padding(
                                    padding: const EdgeInsets.fromLTRB(
                                        18, 10, 18, 4),
                                    child: Text(
                                        questions.isEmpty
                                            ? (filter == 'ANSWERED'
                                                ? 'No answered questions on this page'
                                                : 'No open questions')
                                            : '$unanswered unanswered · ${questions.length} shown',
                                        style: const TextStyle(
                                            fontSize: 12,
                                            fontWeight: FontWeight.w700,
                                            color: DoctorTokens.textSub))),
                                if (questions.isNotEmpty)
                                  Padding(
                                      key: const ValueKey(
                                          'qna-question-container'),
                                      padding: const EdgeInsets.symmetric(
                                          horizontal: 18),
                                      child: Column(
                                          children: questions
                                              .map((q) => q.status == 'OPEN'
                                                  ? _QnaOpenCard(
                                                      key: ValueKey(q.id),
                                                      question: q)
                                                  : _QnaAnsweredCard(
                                                      question: q))
                                              .toList()))
                              ])))
        ])));
  }
}

class _QnaOpenCard extends StatefulWidget {
  const _QnaOpenCard({super.key, required this.question});
  final QnaDto question;
  @override
  State<_QnaOpenCard> createState() => _QnaOpenCardState();
}

class _QnaAnsweredCard extends StatelessWidget {
  const _QnaAnsweredCard({required this.question});
  final QnaDto question;
  @override
  Widget build(BuildContext context) => Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
          color: DoctorTokens.surface,
          border: Border.all(color: DoctorTokens.border),
          borderRadius: BorderRadius.circular(16)),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        const StatusChip('Answered',
            color: DoctorTokens.primaryLight,
            textColor: DoctorTokens.primaryDark),
        const SizedBox(height: 8),
        Text(question.question,
            style: const TextStyle(
                fontSize: 13, fontWeight: FontWeight.w700, height: 1.35)),
        const SizedBox(height: 10),
        Text(question.answer ?? 'Answer unavailable.',
            style: const TextStyle(fontSize: 12, height: 1.45)),
        const SizedBox(height: 8),
        Text(
            '${question.answeringDoctor ?? 'Doctor'} · ${_mediumDateTime(question.answeredAt)}',
            style: const TextStyle(fontSize: 10, color: DoctorTokens.textSub)),
      ]));
}

class _QnaOpenCardState extends State<_QnaOpenCard> {
  final controller = TextEditingController();
  bool publishing = false, answerPublished = false;

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  Future<void> publish() async {
    final text = controller.text.trim();
    if (text.isEmpty) {
      showMessage(context, 'Write an answer before publishing.');
      return;
    }
    final cubit = context.read<DoctorCubit>();
    final messenger = ScaffoldMessenger.of(context);
    setState(() => publishing = true);
    try {
      await cubit.repo.answer(widget.question.id, text);
      if (mounted) setState(() => answerPublished = true);
      await cubit.refreshQuestions();
      if (messenger.mounted) {
        messenger
            .showSnackBar(const SnackBar(content: Text('Answer published.')));
      }
    } catch (e) {
      if (e is ApiException && e.status == 409) {
        if (mounted) setState(() => answerPublished = true);
        await cubit.refreshQuestions();
      }
      if (messenger.mounted) {
        messenger.showSnackBar(SnackBar(
            content: Text(e is ApiException && e.status == 409
                ? 'This question has already been answered.'
                : 'Your answer could not be published. Please retry.')));
      }
    } finally {
      if (mounted) setState(() => publishing = false);
    }
  }

  Widget _preset(String label, String text) => Padding(
      padding: const EdgeInsets.only(right: 6),
      child: InkWell(
          onTap: () => setState(() => controller.text = text),
          borderRadius: BorderRadius.circular(14),
          child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              decoration: BoxDecoration(
                  color: DoctorTokens.surfaceSubtle,
                  border: Border.all(color: DoctorTokens.border),
                  borderRadius: BorderRadius.circular(14)),
              child: Text(label,
                  style: const TextStyle(
                      fontSize: 10, fontWeight: FontWeight.w700)))));

  @override
  Widget build(BuildContext c) {
    final q = widget.question;
    if (answerPublished) {
      return DoctorCard(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text(q.question),
        const SizedBox(height: 8),
        const Text('This question has already been answered.'),
        TextButton(
            onPressed: c.read<DoctorCubit>().refreshQuestions,
            child: const Text('Refresh answer')),
      ]));
    }
    return Container(
        margin: const EdgeInsets.only(bottom: 12),
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
            color: DoctorTokens.surface,
            border: Border.all(color: const Color(0xFFF59E0B), width: 1.5),
            borderRadius: BorderRadius.circular(16)),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Flexible(
                child: StatusChip(q.category ?? 'General',
                    color: DoctorTokens.infoLight,
                    textColor: DoctorTokens.info)),
            const SizedBox(width: 6),
            Flexible(
                child: Text(
                    'Needs Doctor Advice • ${_relativeAge(q.createdAt)}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    textAlign: TextAlign.end,
                    style: const TextStyle(
                        fontSize: 10,
                        fontWeight: FontWeight.w800,
                        color: Color(0xFFD97706))))
          ]),
          const SizedBox(height: 6),
          Text(q.question,
              style: const TextStyle(
                  fontSize: 13, fontWeight: FontWeight.w700, height: 1.35)),
          const SizedBox(height: 10),
          const Text('QUICK PRESETS',
              style: TextStyle(
                  fontSize: 9.5,
                  fontWeight: FontWeight.w700,
                  color: DoctorTokens.muted)),
          const SizedBox(height: 4),
          SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(children: [
                _preset('Hydration & ORS',
                    'Oral hydration with ORS + Paracetamol 500mg (max 4/day). Avoid antibiotics without blood test.'),
                _preset('Fasting Lab',
                    'Fasting blood glucose & HbA1c test recommended at a verified lab. Cut refined sugars.'),
                _preset('Pediatric Care',
                    'Keep room humidified and provide warm fluids. If symptoms worsen, please visit hospital.')
              ])),
          const SizedBox(height: 10),
          TextField(
              controller: controller,
              maxLines: 3,
              maxLength: 2000,
              style: const TextStyle(fontSize: 11.5),
              decoration: const InputDecoration(
                  hintText: 'Write general health information…',
                  isDense: true,
                  counterText: '')),
          const SizedBox(height: 10),
          SizedBox(
              width: double.infinity,
              child: FilledButton(
                  key: ValueKey('qna-publish-${q.id}'),
                  onPressed: publishing ? null : publish,
                  child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 10),
                      child: Row(children: [
                        publishing
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                    strokeWidth: 2, color: Colors.white))
                            : const Icon(Icons.send, size: 15),
                        const SizedBox(width: 8),
                        const Expanded(
                            child: Text('Publish Verified Doctor Answer',
                                textAlign: TextAlign.center))
                      ]))))
        ]));
  }
}

/// The no-prescription reason choices (product-owner rule, 2026-11-13).
const noRxFollowUpReason = 'Follow-up — no prescription needed';
const _noRxOtherReason = 'Other reason';

/// Asks why no prescription is needed: a dropdown of "Follow-up" or "Other
/// reason"; "Other reason" must be explained in the text box. Returns the
/// reason to record, or null when cancelled.
Future<String?> askNoRxReason(BuildContext context) => showDialog<String>(
    context: context, builder: (_) => const _NoRxReasonDialog());

class _NoRxReasonDialog extends StatefulWidget {
  const _NoRxReasonDialog();
  @override
  State<_NoRxReasonDialog> createState() => _NoRxReasonDialogState();
}

class _NoRxReasonDialogState extends State<_NoRxReasonDialog> {
  String? choice;
  final other = TextEditingController();
  bool showError = false;

  @override
  void dispose() {
    other.dispose();
    super.dispose();
  }

  void submit() {
    if (choice == null) {
      setState(() => showError = true);
      return;
    }
    if (choice == noRxFollowUpReason) {
      Navigator.pop(context, noRxFollowUpReason);
      return;
    }
    final text = other.text.trim();
    if (text.isEmpty) {
      setState(() => showError = true);
      return;
    }
    Navigator.pop(context, text);
  }

  @override
  Widget build(BuildContext context) {
    final isOther = choice == _noRxOtherReason;
    return AlertDialog(
        title: const Text('Reason for no prescription'),
        content: SingleChildScrollView(
            child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
              DropdownButtonFormField<String>(
                  initialValue: choice,
                  isExpanded: true,
                  hint: const Text('Select a reason'),
                  decoration: InputDecoration(
                      isDense: true,
                      errorText: showError && choice == null
                          ? 'Select a reason.'
                          : null),
                  items: const [
                    DropdownMenuItem(
                        value: noRxFollowUpReason,
                        child: Text(noRxFollowUpReason)),
                    DropdownMenuItem(
                        value: _noRxOtherReason, child: Text(_noRxOtherReason)),
                  ],
                  onChanged: (v) => setState(() {
                        choice = v;
                        showError = false;
                      })),
              if (isOther) ...[
                const SizedBox(height: 12),
                TextField(
                    controller: other,
                    autofocus: true,
                    maxLines: 3,
                    maxLength: 1000,
                    onChanged: (_) {
                      if (showError) setState(() => showError = false);
                    },
                    decoration: InputDecoration(
                        hintText: 'Explain why no prescription is needed',
                        errorText: showError && other.text.trim().isEmpty
                            ? 'Please explain the reason.'
                            : null)),
              ]
            ])),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel')),
          FilledButton(onPressed: submit, child: const Text('Continue')),
        ]);
  }
}

class ConsultationPage extends StatefulWidget {
  const ConsultationPage({super.key, required this.appointment});
  final AppointmentDto appointment;
  @override
  State<ConsultationPage> createState() => _ConsultationState();
}

class _ConsultationState extends State<ConsultationPage> {
  Json? summary;
  bool loading = true, completing = false, refreshing = false, offline = false;
  DateTime? lastUpdated;
  String? error;
  @override
  void initState() {
    super.initState();
    load();
    unawaited(_ackOpen());
  }

  Future<void> _ackOpen() async {
    try {
      await context
          .read<DoctorCubit>()
          .repo
          .acknowledgeConsultation(widget.appointment.id);
    } catch (_) {
      // Best-effort display acknowledgement; backend access remains authoritative.
    }
  }

  Future<void> load() async {
    final repo = context.read<DoctorCubit>().repo;
    final cached = await repo.cachedSummary(widget.appointment.id);
    if (cached != null) {
      summary = cached;
      if (mounted) {
        setState(() {
          loading = false;
          refreshing = true;
          offline = false;
          error = null;
        });
      }
    } else if (mounted) {
      setState(() {
        loading = true;
        refreshing = false;
        error = null;
      });
    }
    try {
      summary = await repo.summary(widget.appointment.id);
      if (mounted) {
        setState(() {
          error = null;
          refreshing = false;
          offline = false;
          lastUpdated = DateTime.now().toUtc();
        });
      }
    } on ApiException catch (e) {
      if (mounted) {
        setState(() {
          error = e.userMessage;
          refreshing = false;
          offline = summary != null && (repo.isOffline || e.status == null);
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          error = 'Consultation details are unavailable.';
          refreshing = false;
          offline = summary != null && repo.isOffline;
        });
      }
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  Future<void> complete(String outcome, {String? noRxReason}) async {
    if (completing) return;
    setState(() => completing = true);
    final doctorCubit = context.read<DoctorCubit>();
    final repo = doctorCubit.repo;
    try {
      if (outcome == 'COMPLETED_NO_RX') {
        await repo.completeNoRx(widget.appointment.id, noRxReason!);
      } else {
        await repo.complete(widget.appointment.id, outcome);
      }
      await doctorCubit.loadAll();
      if (mounted) {
        showMessage(context, 'Consultation completed by the server.');
      }
    } catch (e) {
      if (mounted) {
        showError(context, e,
            conflict: 'This consultation has already been completed.');
      }
    } finally {
      if (mounted) setState(() => completing = false);
    }
  }

  Future<void> chooseCompletion() async {
    final choice = await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
                title: const Text('Complete consultation'),
                content:
                    const Text('Choose the outcome recorded by the server.'),
                actions: [
                  TextButton(
                      onPressed: () =>
                          Navigator.pop(dialogContext, 'ESCALATED'),
                      child: const Text('Escalate')),
                  TextButton(
                      onPressed: () => Navigator.pop(dialogContext, 'REFERRED'),
                      child: const Text('Refer patient')),
                  TextButton(
                      onPressed: () =>
                          Navigator.pop(dialogContext, 'COMPLETED_NO_RX'),
                      child: const Text('Complete without Rx')),
                  FilledButton(
                      onPressed: () =>
                          Navigator.pop(dialogContext, 'COMPLETED_WITH_RX'),
                      child: const Text('Complete with Rx')),
                ]));
    if (!mounted || choice == null) return;
    if (choice == 'COMPLETED_NO_RX') {
      final value = await askNoRxReason(context);
      if (value == null) return;
      await complete(choice, noRxReason: value);
    } else if (choice == 'COMPLETED_WITH_RX') {
      // "With Rx" means the Rx is dispatched: the scanner sends the pages
      // and completes the consultation itself, so it is never completed
      // WITH_RX without a prescription on the server.
      final sent = await Navigator.push<bool>(
          context,
          MaterialPageRoute(
              builder: (_) => ScannerPage(
                  initialAppointment: widget.appointment,
                  closeOnDispatch: true)));
      if (sent == true && mounted) Navigator.pop(context);
    } else {
      await complete(choice);
    }
  }

  @override
  Widget build(BuildContext c) {
    final a = widget.appointment;
    final doctorOffline = c.watch<DoctorCubit>().state.offline;
    if (loading) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    if (error != null) {
      return Scaffold(
          appBar: AppBar(title: const Text('Consultation Notes')),
          body: ErrorPanel(error!, onRetry: load));
    }
    return Scaffold(
        appBar: AppBar(title: const Text('Consultation Notes')),
        body: ListView(padding: const EdgeInsets.all(18), children: [
          CacheStatusBar(
              offline: offline,
              refreshing: refreshing,
              lastUpdated: lastUpdated),
          if (error != null) InlineError(error!, onRetry: load),
          DoctorCard(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(a.patientName ?? 'Patient',
                    style: const TextStyle(
                        fontSize: 18, fontWeight: FontWeight.w800)),
                Text('Status: ${a.status}'),
                if (summary?['chief_complaint'] != null)
                  Text('Chief complaint: ${summary!['chief_complaint']}'),
                if (summary?['clinical_notes'] != null)
                  Text('Patient intake note: ${summary!['clinical_notes']}'),
                const SizedBox(height: 12),
                // Panel 7 also draws clinical notes, diagnosis, plan and
                // follow-up fields. Nothing on the server stores them, so
                // they are not offered: typing them here would lose them.
                Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                        color: DoctorTokens.noticeBg,
                        borderRadius: BorderRadius.circular(12)),
                    child: const Text(
                        'Record the consultation outcome below. Written clinical notes, diagnosis, plan and follow-up cannot be saved in the app yet.',
                        style: TextStyle(
                            fontSize: 13.5,
                            height: 1.4,
                            color: DoctorTokens.fieldText))),
                const SizedBox(height: 14),
                Wrap(spacing: 8, runSpacing: 8, children: [
                  OutlinedButton.icon(
                      onPressed: () => showModalBottomSheet(
                          context: c,
                          isScrollControlled: true,
                          builder: (_) => HistoryPage(appointmentId: a.id)),
                      icon: const Icon(Icons.history),
                      label: const Text('Patient history')),
                  FilledButton.icon(
                      onPressed: a.joinBlocked
                          ? null
                          : () => _callScreenGuard.push(
                              c, (_) => AgoraPage(appointmentId: a.id)),
                      icon: const Icon(Icons.video_call),
                      label: Text(a.joinLockedLabel ?? 'Join video call')),
                  OutlinedButton.icon(
                      onPressed: () => Navigator.push(
                          c,
                          MaterialPageRoute(
                              builder: (_) =>
                                  ScannerPage(initialAppointment: a))),
                      icon: const Icon(Icons.receipt_long),
                      label: const Text('Prescription'))
                ]),
                const SizedBox(height: 12),
                if (offline || doctorOffline)
                  const Padding(
                      padding: EdgeInsets.only(bottom: 8),
                      child: Text(
                          'Internet connection is required to complete this consultation.',
                          style: TextStyle(color: DoctorTokens.danger))),
                SizedBox(
                    width: double.infinity,
                    child: FilledButton(
                        onPressed: completing || offline || doctorOffline
                            ? null
                            : chooseCompletion,
                        child: completing
                            ? const CircularProgressIndicator()
                            : const Text('Complete consultation')))
              ]))
        ]));
  }
}

class HistoryPage extends StatefulWidget {
  const HistoryPage({super.key, required this.appointmentId});
  final String appointmentId;
  @override
  State<HistoryPage> createState() => _HistoryState();
}

class _HistoryState extends State<HistoryPage> {
  bool loading = true;
  String? error;
  List<Json> episodes = [];
  @override
  void initState() {
    super.initState();
    load();
  }

  Future<void> load() async {
    if (mounted) setState(() => loading = true);
    try {
      episodes =
          await context.read<DoctorCubit>().repo.history(widget.appointmentId);
      error = null;
    } on ApiException catch (e) {
      error = e.userMessage;
    } catch (_) {
      error = 'History is not available for this consultation.';
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  @override
  Widget build(BuildContext c) {
    Widget body;
    if (loading) {
      body = const Center(child: CircularProgressIndicator());
    } else if (error != null) {
      body = ErrorPanel(error!, onRetry: load);
    } else if (episodes.isEmpty) {
      body = const Center(child: Text('No authorized historical episodes.'));
    } else {
      body = ListView(
          padding: const EdgeInsets.all(18),
          children: episodes
              .map((e) => DoctorCard(
                  child: ListTile(
                      title: Text(textOf(
                          e['date'] ??
                              e['scheduled_at'] ??
                              e['historical_consultation_id'],
                          'Episode')),
                      subtitle: Text(textOf(
                          e['summary'] ?? e['presenting_problem'],
                          'Summary available')),
                      onTap: () => openHistoryDetail(
                          c,
                          widget.appointmentId,
                          textOf(e['historical_consultation_id'] ??
                              e['historical_appointment_id'] ??
                              e['id'])))))
              .toList());
    }
    return SizedBox(
        height: MediaQuery.sizeOf(c).height * .85,
        child: Scaffold(
            appBar: AppBar(title: const Text('Patient History')), body: body));
  }
}

Future<void> openHistoryDetail(
    BuildContext c, String appointment, String episode) async {
  try {
    final detail =
        await c.read<DoctorCubit>().repo.historyDetail(appointment, episode);
    if (c.mounted) {
      final ended = filesAccessEnded(detail);
      final documents = ended ? const <DocumentRef>[] : findDocumentIds(detail);
      final historicalAppointment = textOf(detail['historical_appointment_id']);
      showDialog(
          context: c,
          builder: (_) => AlertDialog(
              title: const Text('Historical episode'),
              content: SingleChildScrollView(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                    Text(const JsonEncoder.withIndent('  ').convert(detail)),
                    if (ended) const Text(kFilesAccessEnded),
                    for (final doc in documents)
                      DocumentButton(
                          appointmentId: historicalAppointment,
                          currentAppointmentId: appointment,
                          documentId: doc.id,
                          mimeType: doc.mimeType)
                  ]))));
    }
  } catch (e) {
    if (c.mounted) showError(c, e);
  }
}

String? findDocumentId(dynamic value) {
  if (value is Map) {
    for (final key in const ['document_id', 'prescription_document_id']) {
      if (value[key] != null) return value[key].toString();
    }
    if (value['id'] != null &&
        (value['mime_type'] != null || value['security_status'] != null)) {
      return value['id'].toString();
    }
    if (value['id'] != null && value['rx_number'] != null) {
      return value['id'].toString();
    }
    for (final child in value.values) {
      final found = findDocumentId(child);
      if (found != null) return found;
    }
  } else if (value is List) {
    for (final child in value) {
      final found = findDocumentId(child);
      if (found != null) return found;
    }
  }
  return null;
}

/// A clinical document's id paired with its backend-declared mime type
/// (Remediation E2) — carried through so [DocumentButton] can route to a
/// safe in-app viewer instead of an external application, the same way the
/// Patient app's Health Vault already routes on its own declared `mimeType`.
/// A doctor can open a consultation's files for 20 days after it ends; after
/// that the server withholds them and reports `files_access_ended`.
const kFilesAccessEnded =
    'Access to this consultation’s files ended 20 days after it.';

bool filesAccessEnded(Map<String, dynamic> data) =>
    data['files_access_ended'] == true;

class DocumentRef {
  const DocumentRef(this.id, this.mimeType);
  final String id;
  final String? mimeType;
}

List<DocumentRef> findDocumentIds(dynamic value) {
  final seen = <String>{};
  final refs = <DocumentRef>[];
  void visit(dynamic node) {
    if (node is Map) {
      if (node['id'] != null &&
          node['is_no_rx_required'] != true &&
          (node['mime_type'] != null || node['security_status'] != null)) {
        final id = node['id'].toString();
        if (seen.add(id)) {
          refs.add(DocumentRef(id, node['mime_type']?.toString()));
        }
      }
      for (final child in node.values) {
        visit(child);
      }
    } else if (node is List) {
      for (final child in node) {
        visit(child);
      }
    }
  }

  visit(value);
  return refs;
}

class DocumentButton extends StatefulWidget {
  const DocumentButton(
      {super.key,
      required this.appointmentId,
      required this.documentId,
      this.currentAppointmentId,
      this.mimeType});
  final String appointmentId, documentId;
  final String? currentAppointmentId;

  /// The backend-declared mime type for this document (Remediation E2).
  /// `null` means the backend did not report one — the safe default is to
  /// refuse to preview rather than guess or fall back to an external
  /// application.
  final String? mimeType;
  @override
  State<DocumentButton> createState() => _DocumentButtonState();
}

class _DocumentButtonState extends State<DocumentButton> {
  bool busy = false;

  /// Fetches the document's bytes and cross-checks the server's own
  /// authoritative `Content-Type` response header against the mime type
  /// declared in the document listing. A mismatch means the listing and the
  /// actual stored object disagree about what this file is — refuse to
  /// render either way rather than trusting either source alone
  /// (Remediation E2's "MIME-type validation before choosing a renderer").
  Future<Uint8List> _loadVerifiedBytes(DoctorRepository repo) async {
    final result = await repo.fetchDocumentBytes(
        widget.appointmentId, widget.documentId,
        currentAppointmentId: widget.currentAppointmentId);
    final declared = widget.mimeType;
    if (declared != null &&
        result.contentType != null &&
        declared.toLowerCase() != result.contentType) {
      throw const ApiException(
          409, 'This document could not be verified and was not opened.');
    }
    return result.bytes;
  }

  @override
  Widget build(BuildContext c) => TextButton.icon(
      onPressed: busy
          ? null
          : () async {
              setState(() => busy = true);
              try {
                final repo = c.read<DoctorCubit>().repo;
                final mime = widget.mimeType?.toLowerCase();
                if (mime == 'image/jpeg' || mime == 'image/png') {
                  if (c.mounted) {
                    await Navigator.push(
                        c,
                        MaterialPageRoute(
                            builder: (_) => ClinicalImageViewer(
                                load: () => _loadVerifiedBytes(repo))));
                  }
                } else if (mime == 'application/pdf') {
                  if (c.mounted) {
                    await Navigator.push(
                        c,
                        MaterialPageRoute(
                            builder: (_) => ClinicalPdfViewer(
                                load: () => _loadVerifiedBytes(repo))));
                  }
                } else if (mounted) {
                  showMessage(
                      c, 'This document type cannot be previewed in the app.');
                }
              } catch (e) {
                if (mounted) showError(context, e);
              } finally {
                if (mounted) setState(() => busy = false);
              }
            },
      icon: const Icon(Icons.open_in_new),
      label: Text(busy ? 'Opening…' : 'Open authorized document'));
}

class ProfilePage extends StatefulWidget {
  const ProfilePage({super.key});
  @override
  State<ProfilePage> createState() => _ProfileState();
}

class _ProfileState extends State<ProfilePage> {
  ProfileDto? profile;
  final name = TextEditingController();
  bool loading = true, saving = false, refreshing = false, offline = false;
  String? error;
  DateTime? lastUpdated;
  @override
  void dispose() {
    name.dispose();
    super.dispose();
  }

  @override
  void initState() {
    super.initState();
    load();
  }

  Future<void> load() async {
    final repo = context.read<DoctorCubit>().repo;
    final cached = await repo.cachedProfile();
    if (cached != null) {
      profile = cached;
      name.text = profile!.name;
      if (mounted) {
        setState(() {
          loading = false;
          refreshing = true;
          offline = false;
          lastUpdated = DateTime.now().toUtc();
          error = null;
        });
      }
    } else if (mounted) {
      setState(() {
        loading = true;
        refreshing = false;
        error = null;
      });
    }
    try {
      profile = await repo.profile();
      name.text = profile!.name;
      if (mounted) {
        setState(() {
          error = null;
          offline = false;
          refreshing = false;
          lastUpdated = DateTime.now().toUtc();
        });
      }
    } on ApiException catch (e) {
      if (mounted) {
        setState(() {
          error = e.userMessage;
          offline = profile != null && (repo.isOffline || e.status == null);
          refreshing = false;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          error = 'Profile is unavailable.';
          offline = profile != null && repo.isOffline;
          refreshing = false;
        });
      }
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  Future<void> save() async {
    if (name.text.trim().isEmpty || saving) return;
    setState(() => saving = true);
    try {
      await context.read<DoctorCubit>().repo.updateProfile(name.text.trim());
      await load();
      if (mounted) showMessage(context, 'Profile updated.');
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => saving = false);
    }
  }

  @override
  Widget build(BuildContext c) {
    if (loading) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    if (profile == null) {
      return Scaffold(
          appBar: AppBar(title: const Text('Profile & Settings')),
          body: ErrorPanel(error ?? 'Profile is unavailable.', onRetry: load));
    }
    return Scaffold(
        appBar: AppBar(title: const Text('Profile & Settings'), actions: [
          IconButton(
              onPressed: () => c.read<AuthCubit>().logout(),
              icon: const Icon(Icons.logout))
        ]),
        body: ListView(padding: const EdgeInsets.all(18), children: [
          CacheStatusBar(
              offline: offline,
              refreshing: refreshing,
              lastUpdated: lastUpdated),
          if (error != null) InlineError(error!, onRetry: load),
          DoctorCard(
              child: ListTile(
                  leading: const Icon(Icons.article_outlined),
                  title: const Text('My Articles'),
                  subtitle:
                      const Text('Write and publish educational articles'),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => Navigator.of(c).push(MaterialPageRoute(
                      builder: (_) => DoctorArticlesPage(
                          api: c.read<DoctorCubit>().repo.api))))),
          DoctorCard(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                const Center(child: ProfilePhotoAvatar(radius: 36)),
                const SizedBox(height: 14),
                TextField(
                    controller: name,
                    maxLength: 150,
                    decoration: const InputDecoration(labelText: 'Full name')),
                const SizedBox(height: 12),
                readOnly('Regulatory authority',
                    'Kenya Medical Practitioners and Dentists Council (${profile!.licenseAuthority})'),
                readOnly('Country', 'Kenya (${profile!.licenseCountry})'),
                readOnly('KMPDC Registration Number', profile!.license),
                readOnly('Specialty', profile!.specialty),
                readOnly('Current hospital (verified)', profile!.hospital),
                readOnly('Qualifications (verified)',
                    profile!.qualifications.join(', ')),
                readOnly('Verification', profile!.verificationLabel),
                const SizedBox(height: 14),
                SizedBox(
                    width: double.infinity,
                    child: FilledButton(
                        onPressed: saving ? null : save,
                        child: saving
                            ? const CircularProgressIndicator()
                            : const Text('Save profile')))
              ]))
        ]));
  }
}

Widget readOnly(String label, String value) => Padding(
    padding: const EdgeInsets.only(bottom: 10),
    child: InputDecorator(
        decoration: InputDecoration(labelText: label),
        child: Text(value.isEmpty ? '—' : value)));

/// The doctor's message being sent, until the server stores it.
class _PendingDoctorBubble extends StatelessWidget {
  const _PendingDoctorBubble({required this.text});
  final String text;

  @override
  Widget build(BuildContext context) => Align(
        alignment: Alignment.centerRight,
        child: Container(
          key: const ValueKey('message-pending'),
          constraints:
              BoxConstraints(maxWidth: MediaQuery.sizeOf(context).width * .82),
          margin: const EdgeInsets.only(bottom: 12),
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
              color: DoctorTokens.primary.withValues(alpha: .75),
              borderRadius: BorderRadius.circular(12)),
          child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(text,
                    style: const TextStyle(color: Colors.white, height: 1.45)),
                const SizedBox(height: 4),
                const ReceiptTick(receipt: MessageReceipt.sending),
              ]),
        ),
      );
}
