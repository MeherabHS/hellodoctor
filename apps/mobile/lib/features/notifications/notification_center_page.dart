import '../../core/l10n/doctor_locale.dart';
import '../../core/notifications/notification_taps.dart';
import 'notification_navigation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/time/kenya_time.dart';
import '../../core/widgets/doctor_widgets.dart';
import 'notification_cubit.dart';
import 'notification_models.dart';
import '../peer/peer_ui.dart'
    show PeerCard, PeerEmptyState, PeerHeader, PeerPillTabs, PeerTokens;

const _monthNames = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', //
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'
];

/// Kenya time (EAT), as everywhere else in the Doctor app; never the
/// device's own timezone.
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

/// Full notification-inbox screen. Reuses `DoctorPageHeader` (no bespoke
/// header widget) and matches `QnaPage`'s existing loading/empty/error/
/// populated state-handling shape.
/// Groups a server notification type under the board's filter chips.
String notificationCategory(String type) {
  if (type == 'NEW_CHAT_MESSAGE') return 'messages';
  if (type.startsWith('BOOKING_') ||
      type.startsWith('CONSULTATION_') ||
      type == 'PRESCRIPTION_AVAILABLE' ||
      type == 'RX_WINDOW_CLOSING') {
    return 'appointments';
  }
  return 'system';
}

class NotificationCenterPage extends StatefulWidget {
  const NotificationCenterPage({super.key});
  @override
  State<NotificationCenterPage> createState() => _NotificationCenterPageState();
}

class _NotificationCenterPageState extends State<NotificationCenterPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance
        .addPostFrameCallback((_) => context.read<NotificationCubit>().load());
  }

  /// all | appointments | messages | system — a view filter only; nothing is
  /// requested differently.
  String _filter = 'all';

  final _tapGate = NotificationTapGate();
  Future<void> _handleTap(NotificationDto n) async {
    try {
      await openNotificationDestination(
          context, context.read<NotificationCubit>(), n.id, _tapGate,
          fromInbox: true);
    } catch (_) {}
  }

  @override
  void dispose() {
    _tapGate.dispose();
    super.dispose();
  }

  /// "Just now", "5 min ago", "3 h ago", else the EAT date and time.
  String _ago(BuildContext context, DateTime utc) {
    final l10n = doctorL10n(context);
    final minutes = DateTime.now().toUtc().difference(utc).inMinutes;
    if (minutes < 1) return l10n.notificationsJustNow;
    if (minutes < 60) return l10n.notificationsMinutesAgo(minutes);
    if (minutes < 12 * 60) return l10n.notificationsHoursAgo(minutes ~/ 60);
    return _mediumDateTime(context, utc);
  }

  /// Today / Yesterday / Earlier, in Kenya time.
  String _group(BuildContext context, DateTime utc) {
    final l10n = doctorL10n(context);
    final day = kenyaTime(utc);
    final today = kenyaTime(DateTime.now().toUtc());
    final diff = DateTime(today.year, today.month, today.day)
        .difference(DateTime(day.year, day.month, day.day))
        .inDays;
    return diff <= 0
        ? l10n.notificationsToday
        : diff == 1
            ? l10n.notificationsYesterday
            : l10n.notificationsEarlier;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = doctorL10n(context);
    final cubit = context.read<NotificationCubit>();
    final s = context.watch<NotificationCubit>().state;
    final all = [...s.items]
      ..sort((a, b) => b.createdAt.compareTo(a.createdAt));
    const filters = ['all', 'appointments', 'messages', 'system'];
    final items = _filter == 'all'
        ? all
        : all.where((n) => notificationCategory(n.type) == _filter).toList();
    final groups = <String, List<NotificationDto>>{};
    for (final n in items) {
      groups.putIfAbsent(_group(context, n.createdAt), () => []).add(n);
    }
    // The Peer Navigator notifications design (product owner, 2026-11-20).
    return Scaffold(
        backgroundColor: Colors.white,
        body: SafeArea(
            bottom: false,
            child: s.loading && items.isEmpty
                ? const Center(child: CircularProgressIndicator())
                : s.error != null && items.isEmpty
                    ? ErrorPanel(l10n.notificationsLoadError,
                        onRetry: cubit.load)
                    : RefreshIndicator(
                        onRefresh: cubit.refresh,
                        child: ListView(
                            padding: const EdgeInsets.only(bottom: 24),
                            children: [
                              PeerHeader(
                                title: l10n.notificationsTitle,
                                subtitle: l10n.notificationsSubtitle,
                                onBack: Navigator.of(context).canPop()
                                    ? () => Navigator.of(context).maybePop()
                                    : null,
                              ),
                              if (s.unreadCount > 0)
                                Padding(
                                    padding:
                                        const EdgeInsets.fromLTRB(18, 0, 18, 6),
                                    child: Align(
                                        alignment: Alignment.centerRight,
                                        child: TextButton(
                                            key: const ValueKey(
                                                'notifications-mark-all-read'),
                                            style: TextButton.styleFrom(
                                                foregroundColor:
                                                    PeerTokens.green,
                                                backgroundColor:
                                                    PeerTokens.mint,
                                                shape: const StadiumBorder()),
                                            onPressed: s.busy
                                                ? null
                                                : cubit.markAllRead,
                                            child: s.busy
                                                ? const SizedBox(
                                                    width: 16,
                                                    height: 16,
                                                    child:
                                                        CircularProgressIndicator(
                                                            strokeWidth: 2))
                                                : Text(l10n.notificationsMarkAllRead,
                                                    style: const TextStyle(
                                                        fontSize: 13,
                                                        fontWeight:
                                                            FontWeight.w800))))),
                              CacheStatusBar(
                                  offline: false,
                                  refreshing: s.refreshing,
                                  lastUpdated: s.lastUpdatedAt),
                              if (s.error != null)
                                InlineError(l10n.notificationsRefreshError,
                                    onRetry: cubit.refresh),
                              PeerPillTabs(
                                  labels: [
                                    l10n.notificationsFilterAll,
                                    l10n.notificationsFilterAppointments,
                                    l10n.notificationsFilterMessages,
                                    l10n.notificationsFilterSystem,
                                  ],
                                  selected: filters.indexOf(_filter),
                                  counts: {
                                    if (s.unreadCount > 0) 0: s.unreadCount
                                  },
                                  onSelected: (i) =>
                                      setState(() => _filter = filters[i])),
                              if (items.isEmpty)
                                Padding(
                                    padding: const EdgeInsets.only(top: 70),
                                    child: PeerEmptyState(
                                        asset:
                                            'assets/peer/empty_notifications.webp',
                                        title: l10n.notificationsEmpty,
                                        body: l10n.notificationsSubtitle))
                              else
                                for (final g in groups.entries) ...[
                                  Padding(
                                      padding: const EdgeInsets.fromLTRB(
                                          18, 14, 18, 10),
                                      child: Row(children: [
                                        Text(g.key,
                                            style: const TextStyle(
                                                fontSize: 19,
                                                fontWeight: FontWeight.w800,
                                                color: PeerTokens.grey)),
                                        const SizedBox(width: 10),
                                        Container(
                                            constraints: const BoxConstraints(
                                                minWidth: 28),
                                            height: 28,
                                            alignment: Alignment.center,
                                            padding: const EdgeInsets.symmetric(
                                                horizontal: 8),
                                            decoration: BoxDecoration(
                                                color: PeerTokens.green,
                                                borderRadius:
                                                    BorderRadius.circular(14)),
                                            child: Text('${g.value.length}',
                                                style: const TextStyle(
                                                    color: Colors.white,
                                                    fontWeight:
                                                        FontWeight.w800))),
                                      ])),
                                  for (final n in g.value)
                                    Padding(
                                        padding: const EdgeInsets.fromLTRB(
                                            14, 0, 14, 12),
                                        child: _NotificationTile(
                                            key: ValueKey(n.id),
                                            notification: n,
                                            time: _ago(context, n.createdAt),
                                            onTap: () => _handleTap(n))),
                                ],
                              if (s.hasOlder)
                                TextButton(
                                    onPressed:
                                        s.loadingOlder ? null : cubit.loadOlder,
                                    child: Text(s.loadingOlder
                                        ? l10n.notificationsLoadingOlder
                                        : l10n.notificationsLoadOlder)),
                            ]))));
  }
}

/// One notification in the Peer Navigator notifications design: coloured
/// icon tile, title, body, when, unread dot and chevron.
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
    final (IconData icon, Color bg, Color fg) =
        switch (notificationCategory(n.type)) {
      'appointments' => n.type.contains('REMINDER')
          ? (
              Icons.alarm_rounded,
              const Color(0xFFFFF5E5),
              const Color(0xFFD98A00)
            )
          : n.type.contains('CANCEL') || n.type.contains('EXPIRED')
              ? (
                  Icons.event_busy_outlined,
                  const Color(0xFFFFF5E5),
                  const Color(0xFFD98A00)
                )
              : (
                  Icons.event_available_outlined,
                  PeerTokens.blue,
                  PeerTokens.blueText
                ),
      'messages' => (
          Icons.chat_bubble_outline_rounded,
          PeerTokens.purple,
          PeerTokens.purpleText
        ),
      _ => (
          Icons.notifications_none_rounded,
          const Color(0xFFE8F5E9),
          PeerTokens.green
        ),
    };
    return PeerCard(
        onTap: onTap,
        radius: 20,
        color: unread ? PeerTokens.mintSoft : Colors.white,
        padding: const EdgeInsets.fromLTRB(16, 16, 10, 16),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Container(
              width: 52,
              height: 52,
              decoration: BoxDecoration(
                  color: bg, borderRadius: BorderRadius.circular(16)),
              child: Icon(icon, color: fg, size: 30)),
          const SizedBox(width: 14),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(n.title,
                    style: TextStyle(
                        fontSize: 15.5,
                        fontWeight: unread ? FontWeight.w800 : FontWeight.w700,
                        color: PeerTokens.navy)),
                const SizedBox(height: 3),
                Text(n.body,
                    style: const TextStyle(
                        fontSize: 13.5, height: 1.35, color: PeerTokens.navy)),
                const SizedBox(height: 4),
                Text(time,
                    style: const TextStyle(
                        fontSize: 12.5, color: PeerTokens.grey)),
              ])),
          Column(children: [
            SizedBox(
                height: 14,
                child: unread
                    ? Container(
                        key: ValueKey('notification-unread-${n.id}'),
                        width: 10,
                        height: 10,
                        decoration: const BoxDecoration(
                            color: PeerTokens.green, shape: BoxShape.circle))
                    : null),
            const SizedBox(height: 8),
            const Icon(Icons.chevron_right_rounded, color: PeerTokens.navy),
          ]),
        ]));
  }
}
