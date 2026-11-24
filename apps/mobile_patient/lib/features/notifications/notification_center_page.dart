import '../../core/notifications/notification_taps.dart';
import 'notification_navigation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/l10n/kenya_time.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import '../../l10n/app_localizations.dart';
import 'notification_cubit.dart';
import 'notification_models.dart';

const _monthNames = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', //
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'
];

/// Shown in Kenya time (EAT) like every other appointment time in the app,
/// whatever the device's own time zone is.
String _mediumDateTime(BuildContext context, DateTime utc) {
  final t = kenyaTime(utc);
  if (Localizations.localeOf(context).languageCode == 'sw') {
    final material = MaterialLocalizations.of(context);
    return '${material.formatMediumDate(t)} \u00b7 ${material.formatTimeOfDay(TimeOfDay.fromDateTime(t))}';
  }
  final hour = t.hour % 12 == 0 ? 12 : t.hour % 12;
  final minute = t.minute.toString().padLeft(2, '0');
  final suffix = t.hour >= 12 ? 'PM' : 'AM';
  return '${_monthNames[t.month - 1]} ${t.day} · $hour:$minute $suffix';
}

/// Owner-scoped inbox. Explicit taps use the same authorization checks as OS
/// notification taps; receipts alone never navigate or mark notices read.
class NotificationCenterPage extends StatefulWidget {
  const NotificationCenterPage({super.key, required this.cubit, this.deps});
  final NotificationCubit cubit;

  /// Session dependencies for appointment destinations (waiting room,
  /// outcome). Without them an appointment opens the plain status page.
  final AppointmentRouteDeps? deps;

  @override
  State<NotificationCenterPage> createState() => _NotificationCenterPageState();
}

class _NotificationCenterPageState extends State<NotificationCenterPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => widget.cubit.load());
  }

  final _tapGate = NotificationTapGate();
  Future<void> _handleTap(NotificationDto n) async {
    try {
      await openNotificationDestination(context, widget.cubit, n.id, _tapGate,
          fromInbox: true, deps: widget.deps);
    } catch (_) {}
  }

  @override
  void dispose() {
    _tapGate.dispose();
    super.dispose();
  }

  /// "Just now", "5 min ago", "3 h ago", else the EAT date and time.
  String _ago(BuildContext context, DateTime utc) {
    final l10n = AppLocalizations.of(context);
    final minutes = DateTime.now().toUtc().difference(utc).inMinutes;
    if (minutes < 1) return l10n.notificationsJustNow;
    if (minutes < 60) return l10n.notificationsMinutesAgo(minutes);
    if (minutes < 12 * 60) return l10n.notificationsHoursAgo(minutes ~/ 60);
    return _mediumDateTime(context, utc);
  }

  /// Today / Yesterday / Earlier, in Kenya time.
  String _group(BuildContext context, DateTime utc) {
    final l10n = AppLocalizations.of(context);
    final day = kenyaTime(utc);
    final today = kenyaTime(DateTime.now().toUtc());
    final diff = DateTime(today.year, today.month, today.day)
        .difference(DateTime(day.year, day.month, day.day))
        .inDays;
    return diff <= 0
        ? l10n.peerToday
        : diff == 1
            ? l10n.peerYesterday
            : l10n.notificationsEarlier;
  }

  @override
  Widget build(BuildContext context) {
    return BlocProvider.value(
      value: widget.cubit,
      child: BlocBuilder<NotificationCubit, NotificationState>(
        builder: (context, s) {
          final l10n = AppLocalizations.of(context);
          final items = [...s.items]
            ..sort((a, b) => b.createdAt.compareTo(a.createdAt));
          final groups = <String, List<NotificationDto>>{};
          for (final n in items) {
            groups.putIfAbsent(_group(context, n.createdAt), () => []).add(n);
          }
          // The Peer Navigator notifications design (product owner,
          // 2026-11-20).
          return Scaffold(
            backgroundColor: Colors.white,
            body: SafeArea(
              bottom: false,
              child: Column(children: [
                _NotificationsHeader(
                  title: l10n.notificationsTitle,
                  subtitle: l10n.notificationsSubtitle,
                  onRefresh: widget.cubit.refresh,
                ),
                if (s.unreadCount > 0)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(18, 0, 18, 4),
                    child: Align(
                      alignment: Alignment.centerRight,
                      child: TextButton(
                        key: const ValueKey('notifications-mark-all-read'),
                        style: TextButton.styleFrom(
                            foregroundColor: PatientTokens.primaryDark,
                            backgroundColor: PatientTokens.mintCard,
                            shape: const StadiumBorder()),
                        onPressed: s.busy ? null : widget.cubit.markAllRead,
                        child: s.busy
                            ? const SizedBox(
                                width: 16,
                                height: 16,
                                child:
                                    CircularProgressIndicator(strokeWidth: 2))
                            : Text(l10n.notificationsMarkAllRead,
                                style: const TextStyle(
                                    fontSize: 12.5,
                                    fontWeight: FontWeight.w800)),
                      ),
                    ),
                  ),
                CacheStatusBar(
                    offline: false,
                    refreshing: s.refreshing,
                    lastUpdated: s.lastUpdatedAt),
                Expanded(
                  child: s.loading && items.isEmpty
                      ? const Center(child: CircularProgressIndicator())
                      : s.error != null && items.isEmpty
                          ? ErrorPanel(l10n.notificationsLoadError,
                              onRetry: widget.cubit.load)
                          : RefreshIndicator(
                              onRefresh: widget.cubit.refresh,
                              child: items.isEmpty && s.error == null
                                  ? _EmptyNotifications(
                                      title: l10n.notificationsEmpty,
                                      body: l10n.notificationsEmptyBody)
                                  : ListView(
                                      padding: const EdgeInsets.fromLTRB(
                                          0, 4, 0, 24),
                                      children: [
                                        if (s.error != null)
                                          InlineError(
                                              l10n.notificationsRefreshError,
                                              onRetry: widget.cubit.refresh),
                                        for (final g in groups.entries) ...[
                                          _GroupHeader(
                                              label: g.key,
                                              count: g.value.length),
                                          for (final n in g.value)
                                            Padding(
                                              padding:
                                                  const EdgeInsets.fromLTRB(
                                                      14, 0, 14, 12),
                                              child: _NotificationTile(
                                                  key: ValueKey(n.id),
                                                  notification: n,
                                                  time: _ago(
                                                      context, n.createdAt),
                                                  onTap: () => _handleTap(n)),
                                            ),
                                        ],
                                        if (s.hasOlder)
                                          TextButton(
                                              onPressed: s.loadingOlder
                                                  ? null
                                                  : widget.cubit.loadOlder,
                                              child: Text(s.loadingOlder
                                                  ? l10n
                                                      .notificationsLoadingOlder
                                                  : l10n
                                                      .notificationsLoadOlder)),
                                      ],
                                    ),
                            ),
                ),
              ]),
            ),
          );
        },
      ),
    );
  }
}

/// "Today  3" — the group label with its count in a green badge.
class _GroupHeader extends StatelessWidget {
  const _GroupHeader({required this.label, required this.count});
  final String label;
  final int count;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.fromLTRB(18, 14, 18, 10),
        child: Row(children: [
          Text(label,
              style: const TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.w800,
                  color: PatientTokens.textSub)),
          const SizedBox(width: 10),
          Container(
            constraints: const BoxConstraints(minWidth: 26),
            height: 26,
            alignment: Alignment.center,
            padding: const EdgeInsets.symmetric(horizontal: 8),
            decoration: BoxDecoration(
                color: PatientTokens.primaryDark,
                borderRadius: BorderRadius.circular(13)),
            child: Text('$count',
                style: const TextStyle(
                    color: Colors.white, fontWeight: FontWeight.w800)),
          ),
        ]),
      );
}

/// One notification in the Peer Navigator design: coloured icon tile, title,
/// body, when, unread dot and chevron.
class _NotificationTile extends StatelessWidget {
  const _NotificationTile(
      {super.key,
      required this.notification,
      required this.time,
      required this.onTap});
  final NotificationDto notification;
  final String time;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final n = notification;
    final unread = !n.isRead;
    final t = n.type;
    final (IconData icon, Color bg, Color fg) = t == 'NEW_CHAT_MESSAGE'
        ? (
            Icons.chat_bubble_outline_rounded,
            const Color(0xFFF0E7FD),
            const Color(0xFF7B3FD1)
          )
        : t.contains('REMINDER')
            ? (
                Icons.alarm_rounded,
                const Color(0xFFFFF5E5),
                const Color(0xFFD98A00)
              )
            : t.contains('CANCEL') || t.contains('EXPIRED')
                ? (
                    Icons.event_busy_outlined,
                    const Color(0xFFFFF5E5),
                    const Color(0xFFD98A00)
                  )
                : t == 'PRESCRIPTION_AVAILABLE' || t == 'PRESCRIPTION_CORRECTED'
                    ? (
                        Icons.medication_outlined,
                        PatientTokens.mintCard,
                        PatientTokens.primaryDark
                      )
                    : t.startsWith('BOOKING_') ||
                            t.startsWith('CONSULTATION_') ||
                            t.startsWith('PEER_')
                        ? (
                            Icons.event_available_outlined,
                            const Color(0xFFE6F1FD),
                            const Color(0xFF1D5FB8)
                          )
                        : (
                            Icons.notifications_none_rounded,
                            PatientTokens.mintCard,
                            PatientTokens.primaryDark
                          );
    return Material(
      color: unread ? const Color(0xFFF1FAF5) : Colors.white,
      shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(20),
          side: const BorderSide(color: Color(0xFFE3E7EC))),
      child: InkWell(
        onTap: onTap,
        customBorder:
            RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 10, 16),
          child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Container(
                width: 50,
                height: 50,
                decoration: BoxDecoration(
                    color: bg, borderRadius: BorderRadius.circular(16)),
                child: Icon(icon, color: fg, size: 28)),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(n.title,
                        style: TextStyle(
                            fontSize: 15,
                            fontWeight:
                                unread ? FontWeight.w800 : FontWeight.w700,
                            color: PatientTokens.navy)),
                    const SizedBox(height: 3),
                    Text(n.body,
                        style: const TextStyle(
                            fontSize: 13.5,
                            height: 1.35,
                            color: PatientTokens.ink)),
                    const SizedBox(height: 4),
                    Text(time,
                        style: const TextStyle(
                            fontSize: 12.5, color: PatientTokens.textSub)),
                  ]),
            ),
            Column(children: [
              SizedBox(
                  height: 14,
                  child: unread
                      ? Container(
                          key: ValueKey('notification-unread-${n.id}'),
                          width: 10,
                          height: 10,
                          decoration: const BoxDecoration(
                              color: PatientTokens.primaryDark,
                              shape: BoxShape.circle))
                      : null),
              const SizedBox(height: 8),
              const Icon(Icons.chevron_right_rounded,
                  color: PatientTokens.navy),
            ]),
          ]),
        ),
      ),
    );
  }
}

/// The Peer Navigator header: circle back button, title block, then the
/// mark-all-read pill and a circle refresh button.
class _NotificationsHeader extends StatelessWidget {
  const _NotificationsHeader(
      {required this.title, required this.subtitle, required this.onRefresh});
  final String title, subtitle;
  final VoidCallback onRefresh;

  Widget _circle(BuildContext context,
          {required IconData icon,
          required String tooltip,
          required VoidCallback onTap}) =>
      Tooltip(
        message: tooltip,
        child: Material(
          color: Colors.white,
          shape: const CircleBorder(
              side: BorderSide(color: Color(0xFFDCE3E8), width: 1.2)),
          child: InkWell(
            customBorder: const CircleBorder(),
            onTap: onTap,
            child: SizedBox(
                width: 46,
                height: 46,
                child: Icon(icon, color: PatientTokens.navy, size: 23)),
          ),
        ),
      );

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 10),
        child: Row(children: [
          if (Navigator.of(context).canPop()) ...[
            _circle(context,
                icon: Icons.arrow_back_rounded,
                tooltip: MaterialLocalizations.of(context).backButtonTooltip,
                onTap: () => Navigator.maybePop(context)),
            const SizedBox(width: 12),
          ],
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(title,
                  style: const TextStyle(
                      fontSize: 24,
                      height: 1.15,
                      fontWeight: FontWeight.w800,
                      letterSpacing: -.3,
                      color: PatientTokens.navy)),
              const SizedBox(height: 2),
              Text(subtitle,
                  style: const TextStyle(
                      fontSize: 13.5, color: PatientTokens.textSub)),
            ]),
          ),
          _circle(context,
              icon: Icons.refresh_rounded,
              tooltip: MaterialLocalizations.of(context)
                  .refreshIndicatorSemanticLabel,
              onTap: onRefresh),
        ]),
      );
}

/// Empty state of the notifications design: illustration, heading, one
/// sentence, and the wave artwork resting on the bottom edge. It stays
/// scrollable so pull-to-refresh works, and it scrolls on short screens.
class _EmptyNotifications extends StatelessWidget {
  const _EmptyNotifications({required this.title, required this.body});
  final String title, body;
  @override
  Widget build(BuildContext context) => LayoutBuilder(builder: (context, box) {
        return SingleChildScrollView(
          physics: const AlwaysScrollableScrollPhysics(),
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: box.maxHeight),
            child: Column(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(28, 28, 28, 20),
                    child: Column(children: [
                      Image.asset('assets/illustrations/woman_phone_bell.webp',
                          height: 210,
                          fit: BoxFit.contain,
                          excludeFromSemantics: true),
                      const SizedBox(height: 22),
                      Text(title,
                          textAlign: TextAlign.center,
                          style: const TextStyle(
                              fontSize: 25,
                              fontWeight: FontWeight.w800,
                              letterSpacing: -.3,
                              color: PatientTokens.text)),
                      const SizedBox(height: 12),
                      Text(body,
                          textAlign: TextAlign.center,
                          style: const TextStyle(
                              fontSize: 15.5,
                              height: 1.5,
                              color: PatientTokens.textSub)),
                    ]),
                  ),
                  Image.asset('assets/illustrations/wave_footer.webp',
                      width: double.infinity,
                      fit: BoxFit.fitWidth,
                      excludeFromSemantics: true),
                ]),
          ),
        );
      });
}
