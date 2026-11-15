import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/time/kenya_time.dart';
import '../../core/widgets/doctor_widgets.dart';
import '../doctor/doctor_cubit.dart';
import '../doctor/doctor_models.dart';
import '../doctor/doctor_pages.dart';
import 'home_pages.dart' show personInitials;

/// Consultation History tab (bottom nav "History"). It replaces the board's
/// panel 8 "Patients" list by product-owner instruction (2026-11-13), drawn
/// from two reference images:
///   docs/design/doctor-consultation-history-list-reference.png
///   docs/design/doctor-consultation-history-empty-reference.png
/// Values are measured from those 941 px wide images at 2.284 px per dp.
///
/// Data is the real `view=history` list, narrowed to completed
/// consultations ("Review completed patient consultations"). Patients have
/// no photographs in this system, so the portraits in the reference are
/// shown as initials, as the reference itself does for "JO".

const historyGreen = Color(0xFF297E58); // "All" pill fill, sampled
const _completedBg = Color(0xFFE6F7E5); // "Completed" pill, sampled
const _initialsBg = Color(0xFFDFF2EA); // initials avatar, sampled
const _cardBorder = Color(0xFFD5DADF); // card outline, sampled
const _outline = Color(0xFF7E8A85); // button outline, sampled
const _pillOutline = Color(0xFF89A498); // Video / Chat pill outline
const _subtitle = Color(0xFF5F6175);
const _meta = Color(0xFF3B3E4C);

const _months = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', //
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'
];

/// "Nov 13 · 8:33 AM" in Kenya time.
String historyWhen(DateTime? utc) {
  if (utc == null) return 'Date unavailable';
  final t = kenyaTime(utc);
  final h = t.hour % 12 == 0 ? 12 : t.hour % 12;
  return '${_months[t.month - 1]} ${t.day} · $h:${t.minute.toString().padLeft(2, '0')} ${t.hour >= 12 ? 'PM' : 'AM'}';
}

class ConsultationHistoryTab extends StatefulWidget {
  const ConsultationHistoryTab(
      {super.key, required this.onBack, required this.onOpenAppointments});
  final VoidCallback onBack;
  final VoidCallback onOpenAppointments;
  @override
  State<ConsultationHistoryTab> createState() => _ConsultationHistoryTabState();
}

class _ConsultationHistoryTabState extends State<ConsultationHistoryTab> {
  final items = <AppointmentDto>[];
  bool loading = false, hasMore = true, loaded = false;
  String? error;
  int filter = 0; // 0 all, 1 video, 2 chat

  @override
  void initState() {
    super.initState();
    _load(reset: true);
  }

  Future<void> _load({bool reset = false}) async {
    if (loading) return;
    setState(() => loading = true);
    try {
      final rows = await context
          .read<DoctorCubit>()
          .repo
          .consultationHistory(offset: reset ? 0 : items.length);
      if (!mounted) return;
      setState(() {
        if (reset) items.clear();
        final known = items.map((a) => a.id).toSet();
        items.addAll(rows.where((a) => known.add(a.id)));
        items.sort((a, b) =>
            (b.start ?? DateTime(0)).compareTo(a.start ?? DateTime(0)));
        hasMore = rows.length == 100;
        error = null;
        loaded = true;
      });
    } catch (_) {
      if (mounted) {
        setState(() => error =
            'Unable to load consultation history. Check your connection and try again.');
      }
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  List<AppointmentDto> get _completed =>
      items.where((a) => a.status == 'COMPLETED').toList();

  /// Completed consultations with the same patient before [a].
  List<AppointmentDto> _earlier(AppointmentDto a) => a.patientId == null
      ? const []
      : _completed
          .where((x) =>
              x.id != a.id &&
              x.patientId == a.patientId &&
              (a.start == null ||
                  x.start == null ||
                  x.start!.isBefore(a.start!)))
          .toList();

  void _open(AppointmentDto a) => Navigator.push(
      context,
      MaterialPageRoute(
          builder: (_) => ColoredBox(
              color: DoctorTokens.screen,
              child: EncounterHistoryPage(
                  appointment: a,
                  previous: _completed
                      .where((x) =>
                          x.id != a.id &&
                          a.patientId != null &&
                          x.patientId == a.patientId)
                      .toList()))));

  @override
  Widget build(BuildContext context) {
    final completed = _completed;
    final shown = completed
        .where((a) => filter == 0 || (filter == 2) == a.isChat)
        .toList();
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            bottom: false,
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              _TitleBar(onBack: widget.onBack),
              const Padding(
                  padding: EdgeInsets.fromLTRB(17.5, 2, 17.5, 0),
                  child: Text('Review completed patient consultations',
                      style: TextStyle(fontSize: 14.5, color: _subtitle))),
              Padding(
                  padding: const EdgeInsets.fromLTRB(15, 16, 15, 0),
                  child: Row(children: [
                    _FilterPill(
                        label: 'All',
                        width: 64,
                        active: filter == 0,
                        onTap: () => setState(() => filter = 0)),
                    const SizedBox(width: 9),
                    _FilterPill(
                        label: 'Video',
                        icon: Icons.videocam_outlined,
                        width: 98,
                        active: filter == 1,
                        onTap: () => setState(() => filter = 1)),
                    const SizedBox(width: 9),
                    _FilterPill(
                        label: 'Chat',
                        icon: Icons.forum_outlined,
                        width: 96,
                        active: filter == 2,
                        onTap: () => setState(() => filter = 2)),
                  ])),
              const SizedBox(height: 23),
              Expanded(
                  child: RefreshIndicator(
                      onRefresh: () => _load(reset: true),
                      child: ListView(
                          padding: const EdgeInsets.fromLTRB(10, 0, 11, 24),
                          children: [
                            if (error != null && items.isEmpty)
                              ErrorPanel(error!,
                                  onRetry: () => _load(reset: true))
                            else if (!loaded)
                              const Padding(
                                  padding: EdgeInsets.only(top: 80),
                                  child: Center(
                                      child: CircularProgressIndicator()))
                            else if (shown.isEmpty)
                              _EmptyHistory(
                                  title: switch (filter) {
                                    1 when completed.isNotEmpty =>
                                      'No video consultations yet',
                                    2 when completed.isNotEmpty =>
                                      'No chat consultations yet',
                                    _ => 'No consultation history yet',
                                  },
                                  onOpenAppointments: widget.onOpenAppointments)
                            else ...[
                              if (error != null)
                                InlineError(error!, onRetry: () => _load()),
                              for (final a in shown)
                                HistoryCard(
                                    appointment: a,
                                    previousCount: _earlier(a).length,
                                    onOpen: () => _open(a)),
                            ],
                            if (loaded && loading)
                              const Padding(
                                  padding: EdgeInsets.all(18),
                                  child: Center(
                                      child: CircularProgressIndicator())),
                            if (loaded &&
                                hasMore &&
                                !loading &&
                                items.isNotEmpty)
                              TextButton(
                                  onPressed: () => _load(),
                                  child: const Text('Load more history',
                                      style: TextStyle(color: historyGreen))),
                          ])))
            ])));
  }
}

/// Back arrow at the left, title centred on the screen.
class _TitleBar extends StatelessWidget {
  const _TitleBar({required this.onBack});
  final VoidCallback onBack;
  @override
  Widget build(BuildContext context) => SizedBox(
      width: double.infinity,
      height: 48,
      child: Stack(alignment: Alignment.center, children: [
        const Padding(
            padding: EdgeInsets.symmetric(horizontal: 56),
            child: FittedBox(
                fit: BoxFit.scaleDown,
                child: Text('Consultation History',
                    maxLines: 1,
                    style: TextStyle(
                        fontSize: 22,
                        fontWeight: FontWeight.w800,
                        color: DoctorTokens.navy)))),
        Positioned(
            left: 6,
            child: IconButton(
                onPressed: onBack,
                tooltip: 'Back',
                icon: const Icon(Icons.arrow_back,
                    size: 26, color: DoctorTokens.navy))),
      ]));
}

class _FilterPill extends StatelessWidget {
  const _FilterPill(
      {required this.label,
      required this.width,
      required this.active,
      required this.onTap,
      this.icon});
  final String label;
  final IconData? icon;
  final double width;
  final bool active;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) {
    final fg = active ? Colors.white : historyGreen;
    return Semantics(
        button: true,
        selected: active,
        child: Material(
            color: active ? historyGreen : Colors.white,
            shape: StadiumBorder(
                side: BorderSide(
                    color: active ? historyGreen : _pillOutline, width: 1)),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
                onTap: onTap,
                child: SizedBox(
                    width: width,
                    height: 33,
                    child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          if (icon != null) ...[
                            Icon(icon, size: 17, color: fg),
                            const SizedBox(width: 8),
                          ],
                          Flexible(
                              child: Text(label,
                                  maxLines: 1,
                                  overflow: TextOverflow.fade,
                                  softWrap: false,
                                  style: TextStyle(
                                      fontSize: 13.5,
                                      fontWeight: FontWeight.w600,
                                      color: fg))),
                        ])))));
  }
}

BoxDecoration get _cardDecoration => BoxDecoration(
    color: Colors.white,
    borderRadius: BorderRadius.circular(12),
    border: Border.all(color: _cardBorder));

/// Full-width outlined button used in the cards (34 dp) and the empty
/// state (43 dp).
class _OutlineAction extends StatelessWidget {
  const _OutlineAction(
      {required this.icon,
      required this.label,
      required this.onTap,
      this.height = 34,
      this.fontSize = 14});
  final IconData icon;
  final String label;
  final VoidCallback onTap;
  final double height, fontSize;
  @override
  Widget build(BuildContext context) => Material(
      color: Colors.white,
      shape: const StadiumBorder(side: BorderSide(color: _outline)),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
          onTap: onTap,
          child: SizedBox(
              height: height,
              child:
                  Row(mainAxisAlignment: MainAxisAlignment.center, children: [
                Icon(icon, size: fontSize + 4, color: historyGreen),
                const SizedBox(width: 10),
                Flexible(
                    child: Text(label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                            fontSize: fontSize,
                            fontWeight: FontWeight.w600,
                            color: historyGreen))),
              ]))));
}

/// One completed consultation (list reference, cards 1–3).
class HistoryCard extends StatelessWidget {
  const HistoryCard(
      {super.key,
      required this.appointment,
      required this.previousCount,
      required this.onOpen});
  final AppointmentDto appointment;
  final int previousCount;
  final VoidCallback onOpen;

  /// A fixed-height line of the card, as measured on the reference.
  static Widget _line(double height, Widget child) => SizedBox(
      height: height,
      child: Align(alignment: Alignment.centerLeft, child: child));

  Widget _detail(IconData icon, String text) => _line(
      20,
      Row(children: [
        icon == Icons.attach_file
            // The reference's paperclip leans diagonally.
            ? Transform.rotate(
                angle: 0.785, child: Icon(icon, size: 15, color: _meta))
            : Icon(icon, size: 15, color: _meta),
        const SizedBox(width: 6),
        Expanded(
            child: Text(text,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                    fontSize: 12.5,
                    fontWeight: FontWeight.w500,
                    color: _meta))),
      ]));

  // Positions inside the card, in dp from the reference (card 1):
  // avatar 14,17 (54); text column x 82; name centre 27.5; meta 51; date 71;
  // detail icons x 77.5, rows 95 and 117; button 34 high, 15 above the
  // card's bottom edge.
  @override
  Widget build(BuildContext context) {
    final a = appointment;
    final name = a.patientName ?? 'Patient';
    final kind = a.isChat ? 'Chat consultation' : 'Video consultation';
    const meta =
        TextStyle(fontSize: 12.5, fontWeight: FontWeight.w500, color: _meta);
    return Container(
        margin: const EdgeInsets.only(bottom: 13.5),
        decoration: _cardDecoration,
        child: Material(
            color: Colors.transparent,
            child: InkWell(
                borderRadius: BorderRadius.circular(12),
                onTap: onOpen,
                child: Padding(
                    padding: const EdgeInsets.fromLTRB(12, 16, 12, 15),
                    child: Column(children: [
                      Stack(children: [
                        Positioned(
                            left: 2,
                            top: 1,
                            child: Container(
                                width: 54,
                                height: 54,
                                alignment: Alignment.center,
                                decoration: const BoxDecoration(
                                    color: _initialsBg, shape: BoxShape.circle),
                                child: Text(personInitials(name),
                                    style: const TextStyle(
                                        fontSize: 23,
                                        fontWeight: FontWeight.w800,
                                        color: Color(0xFF1C6A45))))),
                        Column(children: [
                          Padding(
                              padding: const EdgeInsets.only(left: 70),
                              child: Column(children: [
                                SizedBox(
                                    height: 23,
                                    child: Row(children: [
                                      Expanded(
                                          child: Text(name,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: const TextStyle(
                                                  fontSize: 16.5,
                                                  fontWeight: FontWeight.w800,
                                                  color: DoctorTokens.navy))),
                                      const SizedBox(width: 6),
                                      const _CompletedPill(),
                                    ])),
                                const SizedBox(height: 2),
                                _line(
                                    20,
                                    Row(children: [
                                      Icon(
                                          a.isChat
                                              ? Icons.forum_outlined
                                              : Icons.videocam_outlined,
                                          size: 16,
                                          color: historyGreen),
                                      const SizedBox(width: 6),
                                      Expanded(
                                          child: Text('$kind · Completed',
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: meta)),
                                    ])),
                                _line(20,
                                    Text(historyWhen(a.start), style: meta)),
                              ])),
                          const SizedBox(height: 4),
                          Padding(
                              padding: const EdgeInsets.only(left: 65.5),
                              child: Column(children: [
                                _detail(Icons.attach_file,
                                    'Clinical attachments and prescriptions'),
                                if (previousCount > 0) ...[
                                  const SizedBox(height: 2),
                                  _detail(Icons.history,
                                      'Previous consultations: $previousCount'),
                                ],
                              ])),
                        ]),
                      ]),
                      const SizedBox(height: 11),
                      _OutlineAction(
                          icon: Icons.description_outlined,
                          label: 'View details',
                          onTap: onOpen),
                    ])))));
  }
}

class _CompletedPill extends StatelessWidget {
  const _CompletedPill();
  @override
  Widget build(BuildContext context) => Container(
      height: 23,
      padding: const EdgeInsets.symmetric(horizontal: 8),
      decoration: BoxDecoration(
          color: _completedBg, borderRadius: BorderRadius.circular(7)),
      child: const Row(mainAxisSize: MainAxisSize.min, children: [
        Icon(Icons.check_circle_outline, size: 14, color: Color(0xFF1C6A45)),
        SizedBox(width: 5),
        Text('Completed',
            style: TextStyle(
                fontSize: 11.5,
                fontWeight: FontWeight.w600,
                color: Color(0xFF1C6A45))),
      ]));
}

/// Empty-state card (empty reference). The clipboard illustration is
/// extracted from the reference image (product-owner authorized imagery).
class _EmptyHistory extends StatelessWidget {
  const _EmptyHistory({required this.title, required this.onOpenAppointments});
  final String title;
  final VoidCallback onOpenAppointments;
  @override
  Widget build(BuildContext context) => Container(
      decoration: _cardDecoration,
      padding: const EdgeInsets.fromLTRB(20, 49, 20, 42),
      child: Column(children: [
        Image.asset('assets/history/history_empty_clipboard.png',
            width: 265, height: 214, fit: BoxFit.contain),
        const SizedBox(height: 22),
        Text(title,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 23,
                fontWeight: FontWeight.w800,
                color: DoctorTokens.navy)),
        const SizedBox(height: 12),
        const Text(
            'Completed consultations will appear here once you finish a session.',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 16.5, height: 1.4, color: _subtitle)),
        const SizedBox(height: 30),
        Padding(
            padding: const EdgeInsets.symmetric(horizontal: 14),
            child: _OutlineAction(
                icon: Icons.calendar_month_outlined,
                label: 'Go to appointments',
                height: 43,
                fontSize: 17,
                onTap: onOpenAppointments)),
      ]));
}
