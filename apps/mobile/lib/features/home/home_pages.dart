import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../app.dart';
import '../../core/theme/tokens.dart';
import '../../core/time/kenya_time.dart';
import '../../core/widgets/afya_auth_widgets.dart';
import '../../core/widgets/doctor_widgets.dart';
import '../auth/auth_cubit.dart';
import '../doctor/articles_page.dart';
import '../doctor/doctor_cubit.dart';
import '../doctor/doctor_models.dart';
import '../doctor/doctor_pages.dart';
import '../doctor/profile_photo_avatar.dart';
import '../notifications/notification_center_page.dart';
import '../notifications/notification_cubit.dart';
import '../settings/notification_settings_page.dart';
import '../settings/settings_pages.dart';
import 'consultation_pages.dart';

/// Screens for the four-tab shell of the Doctor reference board: Home
/// (panel 2), Appointments (panel 3), Patients (panel 8) and More. They are
/// presentation over the existing [DoctorCubit] state; no request is added.

const _wordmark = Color(0xFF0E4A4A);
const _greetingBg = Color(0xFFDAEDE9); // board, panel 2 greeting card
const _pillBg = Color(0xFFE2F3E8);

String personInitials(String name) => name
    .trim()
    .split(RegExp(r'\s+'))
    .where((p) => p.isNotEmpty && p.toLowerCase() != 'dr.' && p != 'Dr')
    .take(2)
    .map((p) => p[0])
    .join()
    .toUpperCase();

String _clock(DateTime utc) {
  final t = kenyaTime(utc);
  final hour = t.hour % 12 == 0 ? 12 : t.hour % 12;
  return '$hour:${t.minute.toString().padLeft(2, '0')} ${t.hour >= 12 ? 'PM' : 'AM'}';
}

String appointmentTimeRange(AppointmentDto a) {
  if (a.start == null) return 'Time unavailable';
  return a.end == null
      ? _clock(a.start!)
      : '${_clock(a.start!)} – ${_clock(a.end!)}';
}

bool _isToday(DateTime? utc) {
  if (utc == null) return false;
  final a = kenyaTime(utc), b = kenyaTime(DateTime.now());
  return a.year == b.year && a.month == b.month && a.day == b.day;
}

const _open = ['CONFIRMED', 'WAITING'];

/// Wording of the status pill (panel 3). Anything the board does not name
/// is shown as the server's own status, never as "Upcoming".
(String, Color, Color) appointmentPill(AppointmentDto a) => switch (a.status) {
      'CONFIRMED' || 'WAITING' => (
          'Upcoming',
          _pillBg,
          DoctorTokens.primaryDark
        ),
      'IN_CONSULTATION' => (
          'In Progress',
          DoctorTokens.warningLight,
          const Color(0xFFB45309)
        ),
      'COMPLETED' => (
          'Completed',
          DoctorTokens.surfaceSubtle,
          DoctorTokens.slate
        ),
      _ => (
          a.status.replaceAll('_', ' ').toLowerCase(),
          DoctorTokens.surfaceSubtle,
          DoctorTokens.slate
        ),
    };

List<AppointmentDto> _appointments(DoctorState s) => s.queue
    .whereType<AppointmentDto>()
    .toList()
  ..sort((a, b) => (a.start ?? DateTime(0)).compareTo(b.start ?? DateTime(0)));

void _push(BuildContext context, Widget page) => Navigator.push(
    context,
    MaterialPageRoute(
        // Some legacy screens draw inside their own SafeArea with nothing
        // behind the status bar; give every pushed page a background.
        builder: (_) => ColoredBox(color: DoctorTokens.screen, child: page)));

/// Initials avatar. Patients have no photographs in this system, so the
/// portraits drawn on the board are not reproduced for real people.
class InitialsAvatar extends StatelessWidget {
  const InitialsAvatar(this.name, {super.key, this.size = 46});
  final String name;
  final double size;
  @override
  Widget build(BuildContext context) => Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration:
          const BoxDecoration(color: _greetingBg, shape: BoxShape.circle),
      child: Text(personInitials(name),
          style: TextStyle(
              fontSize: size * .36,
              fontWeight: FontWeight.w800,
              color: DoctorTokens.primaryDark)));
}

/// Back-arrow title bar used by panels 3–11.
class BoardTitleBar extends StatelessWidget {
  const BoardTitleBar(this.title,
      {super.key, this.onBack, this.actions = const []});
  final String title;
  final VoidCallback? onBack;
  final List<Widget> actions;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.fromLTRB(6, 6, 10, 6),
      child: Row(children: [
        if (onBack != null)
          IconButton(
              onPressed: onBack,
              tooltip: 'Back',
              icon: const Icon(Icons.arrow_back, color: DoctorTokens.navy))
        else
          const SizedBox(width: 14, height: 48),
        const SizedBox(width: 6),
        Expanded(
            child: Text(title,
                style: const TextStyle(
                    fontSize: 20,
                    fontWeight: FontWeight.w800,
                    color: DoctorTokens.navy))),
        ...actions
      ]));
}

// ---------------------------------------------------------------- Home

class HomePage extends StatelessWidget {
  const HomePage({super.key, required this.onOpenTab});

  /// Switches the shell to another bottom tab (1 Appointments, 2 Patients,
  /// 3 More).
  final ValueChanged<int> onOpenTab;

  static String _greetingName(String fullName) {
    final parts = fullName
        .trim()
        .split(RegExp(r'\s+'))
        .where((p) => p.isNotEmpty && p.toLowerCase() != 'dr.' && p != 'Dr')
        .toList();
    return parts.isEmpty ? 'Doctor' : 'Dr. ${parts.first}';
  }

  @override
  Widget build(BuildContext c) {
    final cubit = c.read<DoctorCubit>();
    final s = c.watch<DoctorCubit>().state;
    final all = _appointments(s);
    final now = DateTime.now();
    final today = all.where((a) => _isToday(a.start)).toList();
    final upcoming = all
        .where((a) =>
            _open.contains(a.status) && (a.end == null || a.end!.isAfter(now)))
        .length;
    final inProgress = all.where((a) => a.status == 'IN_CONSULTATION').length;
    final completed = today.where((a) => a.status == 'COMPLETED').length;
    final profile = s.profile;
    final unread = c.watch<NotificationCubit>().state.unreadCount;
    final openQuestions =
        s.questions.whereType<QnaDto>().where((q) => q.status == 'OPEN').length;

    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            bottom: false,
            child: RefreshIndicator(
                onRefresh: cubit.loadAll,
                child: ListView(padding: EdgeInsets.zero, children: [
                  Padding(
                      padding: const EdgeInsets.fromLTRB(6, 4, 12, 4),
                      child: Row(children: [
                        IconButton(
                            onPressed: () => onOpenTab(3),
                            tooltip: 'More',
                            icon: const Icon(Icons.menu,
                                color: DoctorTokens.navy)),
                        // Scales down rather than overflowing on narrow
                        // phones or at large text sizes.
                        const Expanded(
                            child: Center(
                                child: FittedBox(
                                    fit: BoxFit.scaleDown,
                                    child: Row(
                                        mainAxisSize: MainAxisSize.min,
                                        children: [
                                          AfyaLogoMark(size: 34),
                                          SizedBox(width: 6),
                                          Text('AfyaMkononi',
                                              style: TextStyle(
                                                  fontSize: 19,
                                                  fontWeight: FontWeight.w800,
                                                  letterSpacing: -0.3,
                                                  color: _wordmark)),
                                        ])))),
                        NotificationBellAction(
                            unreadCount: unread,
                            onTap: () =>
                                _push(c, const NotificationCenterPage())),
                        InkWell(
                            customBorder: const CircleBorder(),
                            onTap: () => _push(c, const ProfilePage()),
                            child: Container(
                                width: 34,
                                height: 34,
                                alignment: Alignment.center,
                                decoration: const BoxDecoration(
                                    color: DoctorTokens.primary,
                                    shape: BoxShape.circle),
                                child: Text(
                                    profile == null
                                        ? ''
                                        : personInitials(profile.name),
                                    style: const TextStyle(
                                        fontSize: 12,
                                        fontWeight: FontWeight.w800,
                                        color: Colors.white)))),
                      ])),
                  CacheStatusBar(
                      offline: s.offline,
                      refreshing: s.refreshing,
                      lastUpdated: null),
                  if (s.error != null)
                    InlineError(s.error!, onRetry: cubit.loadAll),
                  Container(
                      margin: const EdgeInsets.fromLTRB(18, 10, 18, 14),
                      padding: const EdgeInsets.fromLTRB(18, 16, 18, 16),
                      decoration: BoxDecoration(
                          color: _greetingBg,
                          borderRadius: BorderRadius.circular(14)),
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                                'Hello ${_greetingName(profile?.name ?? '')} 👋',
                                style: const TextStyle(
                                    fontSize: 21,
                                    fontWeight: FontWeight.w800,
                                    color: DoctorTokens.navy)),
                            const SizedBox(height: 6),
                            Text(
                                s.loading && all.isEmpty
                                    ? 'Loading your consultations…'
                                    : today.length == 1
                                        ? 'You have 1 consultation today.'
                                        : 'You have ${today.length} consultations today.',
                                style: const TextStyle(
                                    fontSize: 15, color: DoctorTokens.slate)),
                          ])),
                  Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 18),
                      child: Column(children: [
                        Row(children: [
                          Expanded(
                              child: _CounterTile(Icons.calendar_today_outlined,
                                  today.length, 'Today')),
                          const SizedBox(width: 12),
                          Expanded(
                              child: _CounterTile(
                                  Icons.schedule, upcoming, 'Upcoming')),
                        ]),
                        const SizedBox(height: 12),
                        Row(children: [
                          Expanded(
                              child: _CounterTile(Icons.videocam_outlined,
                                  inProgress, 'In Progress')),
                          const SizedBox(width: 12),
                          Expanded(
                              child: _CounterTile(Icons.check_circle_outline,
                                  completed, 'Completed')),
                        ]),
                      ])),
                  // Not on the board: the duty switch the app already has.
                  // Kept on Home by product-owner decision (2026-11-13).
                  Padding(
                      padding: const EdgeInsets.fromLTRB(18, 14, 18, 0),
                      child: _BoardCard(
                          child: Row(children: [
                        Icon(Icons.circle,
                            size: 10,
                            color: s.duty
                                ? DoctorTokens.primary
                                : DoctorTokens.warning),
                        const SizedBox(width: 10),
                        Expanded(
                            child: Text(
                                s.duty
                                    ? 'Online — available for consultations'
                                    : 'On break — not taking consultations',
                                style: const TextStyle(
                                    fontSize: 14,
                                    fontWeight: FontWeight.w700,
                                    color: DoctorTokens.navy))),
                        Switch(
                            value: s.duty,
                            activeTrackColor: DoctorTokens.primary,
                            onChanged: s.dutyUpdating
                                ? null
                                : (_) => cubit.toggleDuty())
                      ]))),
                  const Padding(
                      padding: EdgeInsets.fromLTRB(18, 18, 18, 8),
                      child: Text('Quick Actions',
                          style: TextStyle(
                              fontSize: 17,
                              fontWeight: FontWeight.w800,
                              color: DoctorTokens.navy))),
                  Padding(
                      padding: const EdgeInsets.fromLTRB(18, 0, 18, 24),
                      child: Column(children: [
                        BoardActionRow(
                            icon: Icons.event_available_outlined,
                            title: 'My Appointments',
                            subtitle: 'View and manage consultations',
                            onTap: () => onOpenTab(1)),
                        // Added by product-owner instruction (2026-11-13);
                        // not on the reference board. Kept second so it is
                        // visible without scrolling.
                        BoardActionRow(
                            icon: Icons.question_answer_outlined,
                            title:
                                'Answer HIV-related open questions from patients',
                            subtitle: openQuestions > 0
                                ? '$openQuestions unanswered'
                                : 'Anonymous questions awaiting a doctor’s answer',
                            onTap: () => showDoctorQna(c)),
                        // Chat consultations and consultation history:
                        // product-owner instruction (2026-11-13); history
                        // replaces "Patient Records".
                        BoardActionRow(
                            icon: Icons.chat_bubble_outline,
                            title: 'Chat Consultations',
                            subtitle: 'Open patient chats and reply',
                            onTap: () => _push(c, const ChatPage())),
                        BoardActionRow(
                            icon: Icons.calendar_month_outlined,
                            title: 'Availability',
                            subtitle: 'Set your consultation schedule',
                            onTap: () => DoctorShell.openSchedule(c)),
                        BoardActionRow(
                            icon: Icons.history,
                            title: 'Consultation History',
                            subtitle: 'Past consultations, notes and documents',
                            onTap: () => onOpenTab(2)),
                      ])),
                ]))));
  }
}

class _BoardCard extends StatelessWidget {
  const _BoardCard({required this.child, this.padding});
  final Widget child;
  final EdgeInsets? padding;
  @override
  Widget build(BuildContext context) => Container(
      padding:
          padding ?? const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
      decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(14),
          border: Border.all(color: DoctorTokens.fieldBorder)),
      child: child);
}

class _CounterTile extends StatelessWidget {
  const _CounterTile(this.icon, this.value, this.label);
  final IconData icon;
  final int value;
  final String label;
  @override
  Widget build(BuildContext context) => _BoardCard(
      padding: const EdgeInsets.symmetric(vertical: 14),
      child: Column(children: [
        Icon(icon, size: 26, color: DoctorTokens.primary),
        const SizedBox(height: 6),
        Text('$value',
            style: const TextStyle(
                fontSize: 23,
                height: 1.1,
                fontWeight: FontWeight.w800,
                color: DoctorTokens.navy)),
        const SizedBox(height: 2),
        Text(label,
            style: const TextStyle(fontSize: 13.5, color: DoctorTokens.slate)),
      ]));
}

/// Icon, title, subtitle and chevron (panel 2 quick actions, panel 10 rows).
class BoardActionRow extends StatelessWidget {
  const BoardActionRow(
      {super.key,
      required this.icon,
      required this.title,
      required this.subtitle,
      required this.onTap,
      this.color = DoctorTokens.primary});
  final IconData icon;
  final String title, subtitle;
  final VoidCallback onTap;
  final Color color;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Material(
          color: Colors.white,
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
              side: const BorderSide(color: DoctorTokens.fieldBorder)),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
              onTap: onTap,
              child: Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 14, vertical: 13),
                  child: Row(children: [
                    Icon(icon, size: 28, color: color),
                    const SizedBox(width: 14),
                    Expanded(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          Text(title,
                              style: const TextStyle(
                                  fontSize: 15,
                                  fontWeight: FontWeight.w800,
                                  color: DoctorTokens.navy)),
                          const SizedBox(height: 2),
                          Text(subtitle,
                              style: const TextStyle(
                                  fontSize: 12.5, color: DoctorTokens.slate)),
                        ])),
                    const Icon(Icons.chevron_right,
                        color: DoctorTokens.navy, size: 22)
                  ])))));
}

// -------------------------------------------------------- Appointments

class AppointmentsPage extends StatefulWidget {
  const AppointmentsPage(
      {super.key, required this.onBack, required this.onOpenSchedule});
  final VoidCallback onBack;

  /// Switches this tab to the consultation schedule.
  final VoidCallback onOpenSchedule;
  @override
  State<AppointmentsPage> createState() => _AppointmentsState();
}

class _AppointmentsState extends State<AppointmentsPage> {
  int tab = 0;

  @override
  Widget build(BuildContext c) {
    final cubit = c.read<DoctorCubit>();
    final s = c.watch<DoctorCubit>().state;
    final all = _appointments(s);
    final now = DateTime.now();
    final today = all.where((a) => _isToday(a.start)).toList();
    final upcoming = all
        .where((a) =>
            !_isToday(a.start) &&
            a.start != null &&
            a.start!.isAfter(now) &&
            (_open.contains(a.status) || a.status == 'IN_CONSULTATION'))
        .toList();
    final completed =
        all.where((a) => a.status == 'COMPLETED').toList().reversed.toList();
    final shown = [today, upcoming, completed][tab];
    const empty = [
      'No consultations today',
      'No upcoming consultations',
      'No completed consultations yet'
    ];
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            bottom: false,
            child: Column(children: [
              BoardTitleBar('Appointments', onBack: widget.onBack, actions: [
                IconButton(
                    tooltip: 'Schedule',
                    onPressed: widget.onOpenSchedule,
                    icon: const Icon(Icons.edit_calendar_outlined,
                        color: DoctorTokens.navy)),
                IconButton(
                    tooltip: 'Consultation chats',
                    onPressed: () => _push(c, const ChatPage()),
                    icon: const Icon(Icons.chat_bubble_outline,
                        color: DoctorTokens.navy))
              ]),
              Padding(
                  padding: const EdgeInsets.fromLTRB(18, 4, 18, 10),
                  child: Row(children: [
                    _Segment('Today (${today.length})', tab == 0,
                        () => setState(() => tab = 0)),
                    const SizedBox(width: 8),
                    _Segment(
                        'Upcoming', tab == 1, () => setState(() => tab = 1)),
                    const SizedBox(width: 8),
                    _Segment(
                        'Completed', tab == 2, () => setState(() => tab = 2)),
                  ])),
              CacheStatusBar(
                  offline: s.offline,
                  refreshing: s.refreshing,
                  lastUpdated: null),
              if (s.error != null && all.isNotEmpty)
                InlineError(s.error!, onRetry: cubit.loadAll),
              Expanded(
                  child: s.loading && all.isEmpty
                      ? const Center(child: CircularProgressIndicator())
                      : RefreshIndicator(
                          onRefresh: cubit.loadAll,
                          child: ListView(
                              padding: const EdgeInsets.fromLTRB(18, 2, 18, 24),
                              children: [
                                if (s.error != null && all.isEmpty)
                                  ErrorPanel(s.error!, onRetry: cubit.loadAll)
                                else if (shown.isEmpty)
                                  Padding(
                                      padding: const EdgeInsets.only(top: 64),
                                      child: Center(
                                          child: Text(empty[tab],
                                              style: const TextStyle(
                                                  fontSize: 15,
                                                  color: DoctorTokens.slate))))
                                else
                                  for (final a in shown)
                                    AppointmentRow(a,
                                        onTap: () => _push(
                                            c,
                                            a.status == 'COMPLETED'
                                                ? EncounterHistoryPage(
                                                    appointment: a)
                                                : PatientDetailsPage(
                                                    appointmentId: a.id))),
                                if (tab == 2)
                                  TextButton(
                                      onPressed: () => _push(
                                          c, const ConsultationHistoryPage()),
                                      child: const Text(
                                          'Open full consultation history'))
                              ])))
            ])));
  }
}

class _Segment extends StatelessWidget {
  const _Segment(this.label, this.active, this.onTap);
  final String label;
  final bool active;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Expanded(
      child: Material(
          color: active ? DoctorTokens.primary : Colors.white,
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
              side: BorderSide(
                  color: active
                      ? DoctorTokens.primary
                      : DoctorTokens.fieldBorder)),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
              onTap: onTap,
              child: SizedBox(
                  height: 48,
                  child: Center(
                      child: Text(label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                              fontSize: 13.5,
                              fontWeight: FontWeight.w700,
                              color: active
                                  ? Colors.white
                                  : DoctorTokens.fieldText)))))));
}

/// One appointment (panel 3).
class AppointmentRow extends StatelessWidget {
  const AppointmentRow(this.a, {super.key, required this.onTap});
  final AppointmentDto a;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) {
    final (label, bg, fg) = appointmentPill(a);
    final name = a.patientName ?? 'Patient';
    return Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Material(
            color: Colors.white,
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(14),
                side: const BorderSide(color: DoctorTokens.fieldBorder)),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
                onTap: onTap,
                child: Padding(
                    padding: const EdgeInsets.fromLTRB(12, 12, 12, 12),
                    child: Row(children: [
                      InitialsAvatar(name, size: 50),
                      const SizedBox(width: 12),
                      Expanded(
                          child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                            Text(name,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 16,
                                    fontWeight: FontWeight.w800,
                                    color: DoctorTokens.navy)),
                            const SizedBox(height: 2),
                            Text(
                                _isToday(a.start) || a.start == null
                                    ? appointmentTimeRange(a)
                                    : '${appointmentDateLabel(a.start)} • ${appointmentTimeRange(a)}',
                                style: const TextStyle(
                                    fontSize: 13.5,
                                    color: DoctorTokens.fieldText)),
                            const SizedBox(height: 3),
                            Row(children: [
                              Icon(
                                  a.isChat
                                      ? Icons.chat_bubble_outline
                                      : Icons.videocam_outlined,
                                  size: 16,
                                  color: DoctorTokens.primary),
                              const SizedBox(width: 5),
                              Flexible(
                                  child: Text(
                                      a.isChat
                                          ? 'Chat Consultation'
                                          : 'Video Consultation',
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: const TextStyle(
                                          fontSize: 12.5,
                                          color: DoctorTokens.slate)))
                            ])
                          ])),
                      const SizedBox(width: 8),
                      Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 10, vertical: 5),
                          decoration: BoxDecoration(
                              color: bg,
                              borderRadius: BorderRadius.circular(8)),
                          child: Text(label,
                              style: TextStyle(
                                  fontSize: 12,
                                  fontWeight: FontWeight.w700,
                                  color: fg)))
                    ])))));
  }
}

const _months = [
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
const _weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

/// "Mon, 16 Nov 2026" in Kenya time.
String appointmentDateLabel(DateTime? utc) {
  if (utc == null) return 'Date unavailable';
  final t = kenyaTime(utc);
  return '${_weekdays[t.weekday - 1]}, ${t.day} ${_months[t.month - 1]} ${t.year}';
}

// ---------------------------------------------------------------- More

class MorePage extends StatelessWidget {
  const MorePage({super.key, required this.onBack});
  final VoidCallback onBack;
  @override
  Widget build(BuildContext c) {
    final s = c.watch<DoctorCubit>().state;
    final open =
        s.questions.whereType<QnaDto>().where((q) => q.status == 'OPEN').length;
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            bottom: false,
            child: Column(children: [
              BoardTitleBar('More', onBack: onBack),
              Expanded(
                  child: ListView(
                      padding: const EdgeInsets.fromLTRB(18, 4, 18, 24),
                      children: [
                    BoardActionRow(
                        icon: Icons.badge_outlined,
                        title: 'My Profile',
                        subtitle: 'Qualifications, license, experience',
                        onTap: () => _push(c, const MyProfilePage())),
                    BoardActionRow(
                        icon: Icons.calendar_month_outlined,
                        title: 'Availability & Schedule',
                        subtitle: 'Set your consultation slots',
                        onTap: () => DoctorShell.openSchedule(c)),
                    BoardActionRow(
                        icon: Icons.chat_bubble_outline,
                        title: 'Consultation Chats',
                        subtitle: 'Patient messages and follow-ups',
                        onTap: () => _push(c, const ChatPage())),
                    BoardActionRow(
                        icon: Icons.receipt_long_outlined,
                        title: 'Prescriptions',
                        subtitle: 'Capture and send a prescription',
                        onTap: () => _push(c, const ScannerPage())),
                    BoardActionRow(
                        icon: Icons.account_balance_wallet_outlined,
                        title: 'Earnings & Wallet',
                        subtitle: 'Earnings and payout destination',
                        onTap: () => _push(c, const WalletPage())),
                    BoardActionRow(
                        icon: Icons.help_outline,
                        title:
                            'Answer HIV-related open questions from patients',
                        subtitle: open > 0
                            ? '$open unanswered'
                            : 'Anonymous questions awaiting a doctor’s answer',
                        onTap: () => showDoctorQna(c)),
                    BoardActionRow(
                        icon: Icons.article_outlined,
                        title: 'My Articles',
                        subtitle: 'Write and publish educational articles',
                        onTap: () => _push(
                            c,
                            DoctorArticlesPage(
                                api: c.read<DoctorCubit>().repo.api))),
                    BoardActionRow(
                        icon: Icons.notifications_outlined,
                        title: 'Notification Settings',
                        subtitle: 'Language, categories and quiet hours',
                        onTap: () =>
                            _push(c, const NotificationSettingsPage())),
                    BoardActionRow(
                        icon: Icons.lock_outline,
                        title: 'Security',
                        subtitle: 'Session and sign-out',
                        onTap: () => _push(c, const SecuritySettingsPage())),
                    const SizedBox(height: 4),
                    TextButton.icon(
                        onPressed: () => c.read<AuthCubit>().logout(),
                        icon: const Icon(Icons.logout),
                        label: const Text('Log out')),
                  ]))
            ])));
  }
}

// ---------------------------------------------------------- My Profile

/// Panel 10. The header and rows are real; "Consultation Settings" on the
/// board has nothing behind it, so that row opens the notification settings
/// the app does have and is named for what it opens.
class MyProfilePage extends StatelessWidget {
  const MyProfilePage({super.key});
  @override
  Widget build(BuildContext c) {
    final p = c.watch<DoctorCubit>().state.profile;
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            child: Column(children: [
          BoardTitleBar('My Profile', onBack: () => Navigator.pop(c)),
          Expanded(
              child: ListView(
                  padding: const EdgeInsets.fromLTRB(18, 4, 18, 24),
                  children: [
                Container(
                    padding: const EdgeInsets.all(14),
                    margin: const EdgeInsets.only(bottom: 14),
                    decoration: BoxDecoration(
                        color: const Color(0xFFF5F9FA),
                        borderRadius: BorderRadius.circular(14)),
                    child: Row(children: [
                      const ProfilePhotoAvatar(radius: 36),
                      const SizedBox(width: 14),
                      Expanded(
                          child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                            Text(p?.name ?? 'Loading profile…',
                                style: const TextStyle(
                                    fontSize: 18,
                                    fontWeight: FontWeight.w800,
                                    color: DoctorTokens.navy)),
                            if (p != null && p.specialty.isNotEmpty)
                              Padding(
                                  padding: const EdgeInsets.only(top: 2),
                                  child: Text(p.specialty,
                                      style: const TextStyle(
                                          fontSize: 14,
                                          color: DoctorTokens.fieldText))),
                            if (p != null)
                              Container(
                                  margin: const EdgeInsets.only(top: 8),
                                  padding: const EdgeInsets.symmetric(
                                      horizontal: 10, vertical: 5),
                                  decoration: BoxDecoration(
                                      color: p.verified
                                          ? _pillBg
                                          : DoctorTokens.warningLight,
                                      borderRadius: BorderRadius.circular(8)),
                                  child: Row(
                                      mainAxisSize: MainAxisSize.min,
                                      children: [
                                        Icon(
                                            p.verified
                                                ? Icons.verified
                                                : Icons.hourglass_top,
                                            size: 16,
                                            color: p.verified
                                                ? DoctorTokens.primary
                                                : DoctorTokens.warning),
                                        const SizedBox(width: 6),
                                        Text(
                                            p.verified
                                                ? 'Verified Doctor'
                                                : p.verificationLabel,
                                            style: const TextStyle(
                                                fontSize: 13,
                                                fontWeight: FontWeight.w700,
                                                color: DoctorTokens.navy))
                                      ]))
                          ]))
                    ])),
                BoardActionRow(
                    icon: Icons.badge_outlined,
                    title: 'Professional Profile',
                    subtitle: 'Qualifications, license, experience',
                    onTap: () => _push(c, const ProfilePage())),
                BoardActionRow(
                    icon: Icons.calendar_month_outlined,
                    title: 'Availability & Schedule',
                    subtitle: 'Set your consultation slots',
                    onTap: () => DoctorShell.openSchedule(c)),
                BoardActionRow(
                    icon: Icons.settings_outlined,
                    title: 'Notification Settings',
                    subtitle: 'Language, categories and quiet hours',
                    onTap: () => _push(c, const NotificationSettingsPage())),
                BoardActionRow(
                    icon: Icons.lock_outline,
                    title: 'Security',
                    subtitle: 'Session and sign-out',
                    onTap: () => _push(c, const SecuritySettingsPage())),
              ]))
        ])));
  }
}
