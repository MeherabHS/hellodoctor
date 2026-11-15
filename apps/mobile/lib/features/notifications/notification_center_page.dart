import '../../core/l10n/doctor_locale.dart';
import '../../core/notifications/notification_taps.dart';
import 'notification_navigation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/time/kenya_time.dart';
import '../../core/widgets/doctor_widgets.dart';
import 'notification_cubit.dart';
import 'notification_models.dart';

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
      type == 'PRESCRIPTION_AVAILABLE') {
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

  @override
  Widget build(BuildContext context) {
    final l10n = doctorL10n(context);
    final cubit = context.read<NotificationCubit>();
    final s = context.watch<NotificationCubit>().state;
    final all = [...s.items]
      ..sort((a, b) => b.createdAt.compareTo(a.createdAt));
    final items = _filter == 'all'
        ? all
        : all.where((n) => notificationCategory(n.type) == _filter).toList();
    return Scaffold(
        backgroundColor: DoctorTokens.bg,
        body: SafeArea(
            bottom: false,
            child: Column(children: [
              DoctorPageHeader(
                  l10n.notificationsTitle, l10n.notificationsSubtitle,
                  onRefresh: cubit.refresh,
                  actions: [
                    if (s.unreadCount > 0)
                      TextButton(
                          onPressed: s.busy ? null : cubit.markAllRead,
                          child: s.busy
                              ? const SizedBox(
                                  width: 16,
                                  height: 16,
                                  child:
                                      CircularProgressIndicator(strokeWidth: 2))
                              : Text(l10n.notificationsMarkAllRead,
                                  style: const TextStyle(fontSize: 12))),
                  ]),
              SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  padding: const EdgeInsets.fromLTRB(18, 10, 18, 6),
                  child: Row(children: [
                    for (final (key, label) in const [
                      ('all', 'All'),
                      ('appointments', 'Appointments'),
                      ('messages', 'Messages'),
                      ('system', 'System'),
                    ])
                      Padding(
                          padding: const EdgeInsets.only(right: 8),
                          child: ChoiceChip(
                              label: Text(label),
                              selected: _filter == key,
                              showCheckmark: false,
                              selectedColor: DoctorTokens.primary,
                              backgroundColor: Colors.white,
                              side: BorderSide(
                                  color: _filter == key
                                      ? DoctorTokens.primary
                                      : DoctorTokens.fieldBorder),
                              labelStyle: TextStyle(
                                  fontSize: 13,
                                  fontWeight: FontWeight.w700,
                                  color: _filter == key
                                      ? Colors.white
                                      : DoctorTokens.fieldText),
                              onSelected: (_) => setState(() => _filter = key)))
                  ])),
              CacheStatusBar(
                  offline: false,
                  refreshing: s.refreshing,
                  lastUpdated: s.lastUpdatedAt),
              Expanded(
                  child: s.loading && items.isEmpty
                      ? const Center(child: CircularProgressIndicator())
                      : s.error != null && items.isEmpty
                          ? ErrorPanel(l10n.notificationsLoadError,
                              onRetry: cubit.load)
                          : RefreshIndicator(
                              onRefresh: cubit.refresh,
                              child: ListView(
                                  padding:
                                      const EdgeInsets.fromLTRB(0, 4, 0, 24),
                                  children: [
                                    if (s.error != null)
                                      InlineError(
                                          l10n.notificationsRefreshError,
                                          onRetry: cubit.refresh),
                                    if (s.hasOlder)
                                      TextButton(
                                          onPressed: s.loadingOlder
                                              ? null
                                              : cubit.loadOlder,
                                          child: Text(s.loadingOlder
                                              ? l10n.notificationsLoadingOlder
                                              : l10n.notificationsLoadOlder)),
                                    if (items.isEmpty)
                                      Padding(
                                          padding: const EdgeInsets.all(32),
                                          child: Center(
                                              child: Text(
                                                  l10n.notificationsEmpty)))
                                    else
                                      ...items.map((n) => _NotificationTile(
                                          key: ValueKey(n.id),
                                          notification: n,
                                          onTap: () => _handleTap(n))),
                                  ])))
            ])));
  }
}

class _NotificationTile extends StatelessWidget {
  const _NotificationTile(
      {super.key, required this.notification, required this.onTap});
  final NotificationDto notification;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final n = notification;
    final unread = !n.isRead;
    final (IconData icon, Color iconBg, Color iconColor) =
        switch (notificationCategory(n.type)) {
      'appointments' => n.type.contains('REMINDER')
          ? (
              Icons.notifications,
              DoctorTokens.warningLight,
              DoctorTokens.warning
            )
          : (
              Icons.calendar_month_outlined,
              DoctorTokens.infoLight,
              DoctorTokens.info
            ),
      'messages' => (
          Icons.chat_bubble_outline,
          DoctorTokens.infoLight,
          DoctorTokens.info
        ),
      _ => (Icons.info_outline, DoctorTokens.infoLight, DoctorTokens.info),
    };
    return InkWell(
        onTap: onTap,
        child: Container(
            margin: const EdgeInsets.fromLTRB(18, 0, 18, 10),
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
                color: DoctorTokens.surface,
                border: Border.all(color: DoctorTokens.fieldBorder),
                borderRadius: BorderRadius.circular(14)),
            child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Container(
                  width: 42,
                  height: 42,
                  margin: const EdgeInsets.only(right: 12),
                  decoration: BoxDecoration(
                      color: iconBg, borderRadius: BorderRadius.circular(21)),
                  child: Icon(icon, size: 22, color: iconColor)),
              Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                    Text(n.title,
                        style: TextStyle(
                            fontSize: 13,
                            fontWeight:
                                unread ? FontWeight.w800 : FontWeight.w600,
                            color: DoctorTokens.text)),
                    const SizedBox(height: 4),
                    Text(n.body,
                        style: const TextStyle(
                            fontSize: 12,
                            color: DoctorTokens.textSub,
                            height: 1.4)),
                    const SizedBox(height: 6),
                    Text(_mediumDateTime(context, n.createdAt),
                        style: const TextStyle(
                            fontSize: 10, color: DoctorTokens.textSub)),
                  ])),
              if (unread)
                Padding(
                    padding: const EdgeInsets.only(left: 8, top: 4),
                    child: Container(
                        width: 9,
                        height: 9,
                        decoration: const BoxDecoration(
                            color: DoctorTokens.primary,
                            shape: BoxShape.circle)))
            ])));
  }
}
