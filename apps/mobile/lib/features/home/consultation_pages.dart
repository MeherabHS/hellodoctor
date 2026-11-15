import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/doctor_widgets.dart';
import '../doctor/doctor_cubit.dart';
import '../doctor/doctor_models.dart';
import '../doctor/doctor_pages.dart';
import 'home_pages.dart';

/// Patient Details (panel 4) and Waiting Room (panel 5) of the Doctor
/// reference board. Presentation over existing repository calls; the join
/// window, call screen and chat are the same ones the app already had.

AppointmentDto? _find(DoctorState s, String id) {
  for (final a in s.queue.whereType<AppointmentDto>()) {
    if (a.id == id) return a;
  }
  return null;
}

void _go(BuildContext context, Widget page) => Navigator.push(
    context,
    MaterialPageRoute(
        builder: (_) => ColoredBox(color: DoctorTokens.screen, child: page)));

class _PatientHeader extends StatelessWidget {
  const _PatientHeader(this.name, {this.lines = const []});
  final String name;
  final List<String> lines;
  @override
  Widget build(BuildContext context) => Row(children: [
        InitialsAvatar(name, size: 68),
        const SizedBox(width: 14),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(name,
              style: const TextStyle(
                  fontSize: 19,
                  fontWeight: FontWeight.w800,
                  color: DoctorTokens.navy)),
          for (final line in lines)
            Padding(
                padding: const EdgeInsets.only(top: 3),
                child: Text(line,
                    style: const TextStyle(
                        fontSize: 14, color: DoctorTokens.fieldText)))
        ]))
      ]);
}

class _UnderlineTabs extends StatelessWidget {
  const _UnderlineTabs(this.labels, this.index, this.onChanged);
  final List<String> labels;
  final int index;
  final ValueChanged<int> onChanged;
  @override
  Widget build(BuildContext context) => Row(children: [
        for (var i = 0; i < labels.length; i++)
          Expanded(
              child: InkWell(
                  onTap: () => onChanged(i),
                  child: Container(
                      height: 44,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                          border: Border(
                              bottom: BorderSide(
                                  width: 2.5,
                                  color: i == index
                                      ? DoctorTokens.primary
                                      : Colors.transparent))),
                      child: Text(labels[i],
                          style: TextStyle(
                              fontSize: 14.5,
                              fontWeight: i == index
                                  ? FontWeight.w800
                                  : FontWeight.w500,
                              color: i == index
                                  ? DoctorTokens.navy
                                  : DoctorTokens.fieldText)))))
      ]);
}

class BoardInfoRow extends StatelessWidget {
  const BoardInfoRow(this.icon, this.label, this.value, {super.key});
  final IconData icon;
  final String label, value;
  @override
  Widget build(BuildContext context) => Container(
      margin: const EdgeInsets.only(bottom: 10),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
      decoration: BoxDecoration(
          color: const Color(0xFFF5F9FA),
          borderRadius: BorderRadius.circular(12)),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Icon(icon, size: 24, color: DoctorTokens.primary),
        const SizedBox(width: 12),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(label,
              style: const TextStyle(fontSize: 12, color: DoctorTokens.slate)),
          const SizedBox(height: 2),
          Text(value,
              style: const TextStyle(
                  fontSize: 14.5,
                  height: 1.35,
                  fontWeight: FontWeight.w700,
                  color: DoctorTokens.navy))
        ]))
      ]));
}

/// Wide green button with a leading icon (panels 4 and 5).
class BoardIconButton extends StatelessWidget {
  const BoardIconButton(
      {super.key, required this.icon, required this.label, this.onPressed});
  final IconData icon;
  final String label;
  final VoidCallback? onPressed;
  @override
  Widget build(BuildContext context) => SizedBox(
      width: double.infinity,
      height: 52,
      child: FilledButton.icon(
          onPressed: onPressed,
          icon: Icon(icon, size: 22),
          label: Text(label,
              textAlign: TextAlign.center,
              style:
                  const TextStyle(fontSize: 16, fontWeight: FontWeight.w800))));
}

// ----------------------------------------------------- Patient Details

class PatientDetailsPage extends StatefulWidget {
  const PatientDetailsPage({super.key, required this.appointmentId});
  final String appointmentId;
  @override
  State<PatientDetailsPage> createState() => _PatientDetailsState();
}

class _PatientDetailsState extends State<PatientDetailsPage> {
  int tab = 0;
  bool opening = false;
  Future<Json>? _summary;
  Future<List<Json>>? _history;

  DoctorRepositoryAccess get _repo =>
      DoctorRepositoryAccess(context.read<DoctorCubit>());

  @override
  void initState() {
    super.initState();
    _summary = _repo.summary(widget.appointmentId);
  }

  Future<void> _open(AppointmentDto a) async {
    if (!a.isChat) {
      _go(context, WaitingRoomPage(appointmentId: a.id));
      return;
    }
    if (opening) return;
    setState(() => opening = true);
    try {
      final chat = await _repo.conversation(a);
      if (mounted) _go(context, ChatThreadPage(chat: chat));
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => opening = false);
    }
  }

  @override
  Widget build(BuildContext c) {
    final a = _find(c.watch<DoctorCubit>().state, widget.appointmentId);
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            child: Column(children: [
          BoardTitleBar('Patient Details', onBack: () => Navigator.pop(c)),
          if (a == null)
            const Expanded(
                child: Center(
                    child: Text('This appointment is no longer listed.',
                        style: TextStyle(color: DoctorTokens.slate))))
          else ...[
            Padding(
                padding: const EdgeInsets.fromLTRB(18, 2, 18, 8),
                // The board also prints sex, age and a patient number. The
                // server does not give a doctor those, so they are absent.
                child: _PatientHeader(a.patientName ?? 'Patient',
                    lines: [if (a.number != null) 'Appointment ${a.number}'])),
            Padding(
                padding: const EdgeInsets.symmetric(horizontal: 18),
                child: _UnderlineTabs(
                    const ['Overview', 'History', 'Files'], tab, (i) {
                  setState(() {
                    tab = i;
                    if (i == 1) {
                      _history ??= _repo.history(widget.appointmentId);
                    }
                  });
                })),
            const Divider(height: 1, color: DoctorTokens.fieldBorder),
            Expanded(
                child: ListView(
                    padding: const EdgeInsets.fromLTRB(18, 14, 18, 12),
                    children: switch (tab) {
                      0 => _overview(a),
                      1 => [_historyTab()],
                      _ => [_filesTab(a)],
                    })),
            Padding(
                padding: const EdgeInsets.fromLTRB(18, 4, 18, 6),
                child: Column(children: [
                  BoardIconButton(
                      icon: a.isChat ? Icons.chat_outlined : Icons.videocam,
                      label: opening ? 'Opening…' : 'Open Consultation',
                      onPressed:
                          opening || !a.canStart ? null : () => _open(a)),
                  Wrap(alignment: WrapAlignment.center, spacing: 4, children: [
                    if (!a.isChat && a.conversationId != null)
                      TextButton(
                          onPressed: opening
                              ? null
                              : () async {
                                  setState(() => opening = true);
                                  try {
                                    final chat = await _repo.conversation(a);
                                    if (c.mounted) {
                                      _go(c, ChatThreadPage(chat: chat));
                                    }
                                  } catch (e) {
                                    if (c.mounted) showError(c, e);
                                  } finally {
                                    if (mounted) {
                                      setState(() => opening = false);
                                    }
                                  }
                                },
                          child: const Text('Messages')),
                    TextButton(
                        onPressed: () =>
                            _go(c, ScannerPage(initialAppointment: a)),
                        child: const Text('Prescription')),
                    TextButton(
                        onPressed: () =>
                            _go(c, ConsultationPage(appointment: a)),
                        child: const Text('Record outcome')),
                  ])
                ]))
          ]
        ])));
  }

  List<Widget> _overview(AppointmentDto a) => [
        BoardInfoRow(Icons.event_note_outlined, 'Consultation',
            a.isChat ? 'Chat Consultation' : 'Video Consultation'),
        BoardInfoRow(Icons.schedule, 'Date & Time',
            '${appointmentDateLabel(a.start)} • ${appointmentTimeRange(a)}'),
        BoardInfoRow(Icons.fact_check_outlined, 'Reason for Visit',
            a.complaint ?? 'Not provided'),
        FutureBuilder<Json>(
            future: _summary,
            builder: (_, snap) {
              final notes = snap.data?['clinical_notes'];
              // "Key Information" on the board is a structured clinical
              // summary the server does not hold. What exists is the
              // patient's own intake note, labelled as that.
              return notes == null || '$notes'.trim().isEmpty
                  ? const SizedBox.shrink()
                  : BoardInfoRow(Icons.assignment_outlined,
                      'Patient’s intake note', '$notes');
            }),
        if (!a.canStart)
          Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                  'Status: ${a.status.replaceAll('_', ' ').toLowerCase()}',
                  style: const TextStyle(color: DoctorTokens.slate))),
      ];

  Widget _historyTab() => FutureBuilder<List<Json>>(
      future: _history,
      builder: (c, snap) {
        if (snap.connectionState != ConnectionState.done) {
          return const Padding(
              padding: EdgeInsets.only(top: 40),
              child: Center(child: CircularProgressIndicator()));
        }
        if (snap.hasError) {
          return ErrorPanel('History is not available for this consultation.',
              onRetry: () => setState(
                  () => _history = _repo.history(widget.appointmentId)));
        }
        final episodes = snap.data!;
        if (episodes.isEmpty) {
          return const _Empty(
              'No earlier consultations you are authorised to see.');
        }
        return Column(children: [
          for (final e in episodes)
            BoardActionRow(
                icon: Icons.history,
                title: textOf(
                    e['date'] ??
                        e['scheduled_at'] ??
                        e['historical_consultation_id'],
                    'Episode'),
                subtitle: textOf(e['summary'] ?? e['presenting_problem'],
                    'Summary available'),
                onTap: () => openHistoryDetail(
                    c,
                    widget.appointmentId,
                    textOf(e['historical_consultation_id'] ??
                        e['historical_appointment_id'] ??
                        e['id'])))
        ]);
      });

  Widget _filesTab(AppointmentDto a) => FutureBuilder<Json>(
      future: _summary,
      builder: (_, snap) {
        if (snap.connectionState != ConnectionState.done) {
          return const Padding(
              padding: EdgeInsets.only(top: 40),
              child: Center(child: CircularProgressIndicator()));
        }
        if (snap.hasError) {
          return ErrorPanel('Files could not be loaded.',
              onRetry: () => setState(
                  () => _summary = _repo.summary(widget.appointmentId)));
        }
        final docs = findDocumentIds({
          'documents': snap.data!['documents'],
          'prescriptions': snap.data!['prescriptions']
        });
        if (docs.isEmpty) {
          return _Empty(filesAccessEnded(snap.data!)
              ? kFilesAccessEnded
              : 'No files for this consultation.');
        }
        return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          for (final d in docs)
            DocumentButton(
                appointmentId: a.id, documentId: d.id, mimeType: d.mimeType)
        ]);
      });
}

class _Empty extends StatelessWidget {
  const _Empty(this.text);
  final String text;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(top: 48),
      child: Center(
          child: Text(text,
              textAlign: TextAlign.center,
              style:
                  const TextStyle(fontSize: 15, color: DoctorTokens.slate))));
}

/// Thin named access to the repository calls these screens use, so the
/// screens read plainly.
class DoctorRepositoryAccess {
  const DoctorRepositoryAccess(this.cubit);
  final DoctorCubit cubit;
  Future<Json> summary(String id) => cubit.repo.summary(id);
  Future<List<Json>> history(String id) => cubit.repo.history(id);
  Future<ChatDto> conversation(AppointmentDto a) =>
      cubit.repo.conversationForAppointment(a);
}

// -------------------------------------------------------- Waiting Room

class WaitingRoomPage extends StatelessWidget {
  const WaitingRoomPage({super.key, required this.appointmentId});
  final String appointmentId;
  @override
  Widget build(BuildContext c) {
    final a = _find(c.watch<DoctorCubit>().state, appointmentId);
    // The server reports presence only for a session that has started.
    final present = a?.patientPresent == true;
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            child: Column(children: [
          BoardTitleBar('Waiting Room', onBack: () => Navigator.pop(c)),
          Expanded(
              child: a == null
                  ? const Center(
                      child: Text('This appointment is no longer listed.',
                          style: TextStyle(color: DoctorTokens.slate)))
                  : ListView(
                      padding: const EdgeInsets.fromLTRB(22, 10, 22, 12),
                      children: [
                          const _WaitingArt(),
                          const SizedBox(height: 18),
                          Text(present ? 'Patient is waiting' : 'Waiting Room',
                              textAlign: TextAlign.center,
                              style: const TextStyle(
                                  fontSize: 21,
                                  fontWeight: FontWeight.w800,
                                  color: DoctorTokens.navy)),
                          const SizedBox(height: 16),
                          Container(
                              padding: const EdgeInsets.all(12),
                              decoration: BoxDecoration(
                                  color: Colors.white,
                                  borderRadius: BorderRadius.circular(14),
                                  border: Border.all(
                                      color: DoctorTokens.fieldBorder)),
                              child: _PatientHeader(a.patientName ?? 'Patient',
                                  lines: [
                                    present
                                        ? 'In the consultation room'
                                        : appointmentTimeRange(a)
                                  ])),
                          const SizedBox(height: 16),
                          Container(
                              padding: const EdgeInsets.all(14),
                              decoration: BoxDecoration(
                                  color: const Color(0xFFE6F3E6),
                                  borderRadius: BorderRadius.circular(12)),
                              child: Row(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    const Icon(Icons.info_outline,
                                        size: 22, color: DoctorTokens.primary),
                                    const SizedBox(width: 10),
                                    Expanded(
                                        child: Text(
                                            a.joinBlocked
                                                ? (a.joinLockedLabel ??
                                                    'The consultation room is not open.')
                                                : present
                                                    ? 'The patient is in the waiting room. Start the consultation when you are ready.'
                                                    : 'The room is open. Start the consultation when you are ready; the patient can join at any time.',
                                            style: const TextStyle(
                                                fontSize: 14,
                                                height: 1.4,
                                                color: DoctorTokens.fieldText)))
                                  ])),
                        ])),
          if (a != null)
            Padding(
                padding: const EdgeInsets.fromLTRB(18, 4, 18, 12),
                child: BoardIconButton(
                    icon: Icons.videocam,
                    label: a.hasActiveConsultation
                        ? 'Rejoin Consultation'
                        : 'Start Consultation',
                    onPressed: a.joinBlocked || !a.canStart
                        ? null
                        : () => openConsultationCall(c, a.id)))
        ])));
  }
}

/// Stand-in for the board's waiting-room illustration, drawn natively until
/// a high-resolution image is supplied.
class _WaitingArt extends StatelessWidget {
  const _WaitingArt();
  @override
  Widget build(BuildContext context) => Center(
      child: Container(
          width: 210,
          height: 170,
          decoration: const BoxDecoration(
              color: Color(0xFFDFF1E6),
              borderRadius: BorderRadius.all(Radius.elliptical(105, 85))),
          alignment: Alignment.center,
          child: Container(
              width: 132,
              height: 96,
              decoration: BoxDecoration(
                  color: const Color(0xFF2F80ED),
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(color: const Color(0xFF1B4F9C), width: 5)),
              child: const Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(Icons.account_circle, size: 44, color: Colors.white),
                    Icon(Icons.videocam, size: 22, color: Colors.white),
                  ]))));
}
