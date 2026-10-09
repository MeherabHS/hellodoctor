import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../app.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/doctor_widgets.dart';
import 'notification_cubit.dart';
import 'notification_models.dart';

const _monthNames = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', //
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'
];

String _mediumDateTime(DateTime utc) {
  final t = utc.toLocal();
  final hour = t.hour % 12 == 0 ? 12 : t.hour % 12;
  final minute = t.minute.toString().padLeft(2, '0');
  final suffix = t.hour >= 12 ? 'PM' : 'AM';
  return '${_monthNames[t.month - 1]} ${t.day} · $hour:$minute $suffix';
}

/// Full notification-inbox screen. Reuses `DoctorPageHeader` (no bespoke
/// header widget) and matches `QnaPage`'s existing loading/empty/error/
/// populated state-handling shape.
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

  Future<void> _handleTap(NotificationDto n) async {
    final cubit = context.read<NotificationCubit>();
    await cubit.markRead(n.id);
    if (!mounted) return;
    // No dedicated single-appointment detail screen exists today — the only
    // recognized navigation target is the Schedule tab, for an appointment-
    // related notification (e.g. a reminder). Anything else is left as a
    // read-only inbox entry; there is nowhere else to invent a destination.
    if (n.relatedEntityType == 'APPOINTMENT') {
      Navigator.of(context).popUntil((r) => r.isFirst);
      DoctorShell.switchTab(context, 3);
    }
  }

  @override
  Widget build(BuildContext context) {
    final cubit = context.read<NotificationCubit>();
    final s = context.watch<NotificationCubit>().state;
    final items = [...s.items]
      ..sort((a, b) => b.createdAt.compareTo(a.createdAt));
    return Scaffold(
        backgroundColor: DoctorTokens.bg,
        body: SafeArea(
            bottom: false,
            child: Column(children: [
              DoctorPageHeader('Notifications', 'Account & activity updates',
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
                              : const Text('Mark all read',
                                  style: TextStyle(fontSize: 12))),
                  ]),
              CacheStatusBar(
                  offline: false,
                  refreshing: s.refreshing,
                  lastUpdated: s.lastUpdatedAt),
              Expanded(
                  child: s.loading && items.isEmpty
                      ? const Center(child: CircularProgressIndicator())
                      : s.error != null && items.isEmpty
                          ? ErrorPanel(
                              'Notifications could not be loaded. Please retry.',
                              onRetry: cubit.load)
                          : RefreshIndicator(
                              onRefresh: cubit.refresh,
                              child: ListView(
                                  padding:
                                      const EdgeInsets.fromLTRB(0, 4, 0, 24),
                                  children: [
                                    if (s.error != null)
                                      InlineError(
                                          'Notifications could not be refreshed. Please retry.',
                                          onRetry: cubit.refresh),
                                    if (items.isEmpty)
                                      const Padding(
                                          padding: EdgeInsets.all(32),
                                          child: Center(
                                              child:
                                                  Text('No notifications yet')))
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
    return InkWell(
        onTap: onTap,
        child: Container(
            margin: const EdgeInsets.fromLTRB(18, 0, 18, 10),
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
                color: unread
                    ? DoctorTokens.primaryLight.withValues(alpha: .35)
                    : DoctorTokens.surface,
                border: Border.all(color: DoctorTokens.border),
                borderRadius: BorderRadius.circular(DoctorTokens.cardRadius)),
            child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Padding(
                  padding: const EdgeInsets.only(top: 6, right: 8),
                  child: unread
                      ? Container(
                          width: 8,
                          height: 8,
                          decoration: const BoxDecoration(
                              color: DoctorTokens.primary,
                              shape: BoxShape.circle))
                      : const SizedBox(width: 8, height: 8)),
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
                    Text(_mediumDateTime(n.createdAt),
                        style: const TextStyle(
                            fontSize: 10, color: DoctorTokens.textSub)),
                  ]))
            ])));
  }
}
