import '../patient/chat/chat_repository.dart';
import '../patient/chat/patient_chat_page.dart';
import '../patient/profile/profile_cubit.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import '../../l10n/app_localizations.dart';
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

/// Full notification-inbox screen — mirrors `apps/mobile`'s
/// `NotificationCenterPage` structurally (same loading/empty/error/populated
/// shape). No dedicated single-appointment detail screen exists for Patient
/// either, so tapping an appointment-related notification marks it read and
/// returns to Home rather than inventing a destination.
class NotificationCenterPage extends StatefulWidget {
  const NotificationCenterPage({super.key, required this.cubit});
  final NotificationCubit cubit;

  @override
  State<NotificationCenterPage> createState() => _NotificationCenterPageState();
}

class _NotificationCenterPageState extends State<NotificationCenterPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => widget.cubit.load());
  }

  Future<void> _handleTap(NotificationDto n) async {
    await widget.cubit.markRead(n.id);
    if (!mounted) return;
    if (n.relatedEntityType == 'CHAT' && n.relatedEntityId != null) {
      try {
        final repo = ChatRepository(widget.cubit.repo.api);
        final appointments = await repo.consultations();
        final appointmentId = n.metadata?['appointment_id']?.toString();
        final appointment =
            appointments.where((a) => a.id == appointmentId).firstOrNull;
        if (appointment == null) throw StateError('Consultation unavailable');
        final conversation =
            await repo.conversationForAppointment(appointment.id);
        if (conversation.id != n.relatedEntityId) {
          throw StateError('Conversation mismatch');
        }
        if (mounted) {
          await Navigator.push(
              context,
              MaterialPageRoute(
                  builder: (_) => PatientChatPage(
                      conversation: conversation,
                      doctorDisplayName: appointment.doctorDisplayName,
                      repo: repo,
                      myUserId:
                          context.read<ProfileCubit>().state.profile?.userId)));
        }
      } catch (_) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
              content:
                  Text('Unable to open this conversation. Please retry.')));
        }
      }
    } else if (n.relatedEntityType == 'APPOINTMENT') {
      Navigator.of(context).popUntil((r) => r.isFirst);
    }
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
          return Scaffold(
            backgroundColor: PatientTokens.bg,
            body: SafeArea(
              bottom: false,
              child: Column(children: [
                PatientPageHeader(
                  l10n.notificationsTitle,
                  l10n.notificationsSubtitle,
                  onRefresh: widget.cubit.refresh,
                  actions: [
                    if (s.unreadCount > 0)
                      TextButton(
                        onPressed: s.busy ? null : widget.cubit.markAllRead,
                        child: s.busy
                            ? const SizedBox(
                                width: 16,
                                height: 16,
                                child:
                                    CircularProgressIndicator(strokeWidth: 2))
                            : Text(l10n.notificationsMarkAllRead,
                                style: const TextStyle(fontSize: 12)),
                      ),
                  ],
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
                              child: ListView(
                                padding: const EdgeInsets.fromLTRB(0, 4, 0, 24),
                                children: [
                                  if (s.error != null)
                                    InlineError(l10n.notificationsRefreshError,
                                        onRetry: widget.cubit.refresh),
                                  if (s.hasOlder)
                                    TextButton(
                                        onPressed: s.loadingOlder
                                            ? null
                                            : widget.cubit.loadOlder,
                                        child: Text(s.loadingOlder
                                            ? 'Loading earlier notifications...'
                                            : 'Load earlier notifications')),
                                  if (items.isEmpty)
                                    Padding(
                                      padding: const EdgeInsets.all(32),
                                      child: Center(
                                          child: Text(l10n.notificationsEmpty)),
                                    )
                                  else
                                    ...items.map((n) => _NotificationTile(
                                        key: ValueKey(n.id),
                                        notification: n,
                                        onTap: () => _handleTap(n))),
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
              ? PatientTokens.primaryLight.withValues(alpha: .35)
              : PatientTokens.surface,
          border: Border.all(color: PatientTokens.border),
          borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
        ),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Padding(
            padding: const EdgeInsets.only(top: 6, right: 8),
            child: unread
                ? Container(
                    width: 8,
                    height: 8,
                    decoration: const BoxDecoration(
                        color: PatientTokens.primary, shape: BoxShape.circle))
                : const SizedBox(width: 8, height: 8),
          ),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(n.title,
                  style: TextStyle(
                      fontSize: 13,
                      fontWeight: unread ? FontWeight.w800 : FontWeight.w600,
                      color: PatientTokens.text)),
              const SizedBox(height: 4),
              Text(n.body,
                  style: const TextStyle(
                      fontSize: 12, color: PatientTokens.textSub, height: 1.4)),
              const SizedBox(height: 6),
              Text(_mediumDateTime(n.createdAt),
                  style: const TextStyle(
                      fontSize: 10, color: PatientTokens.textSub)),
            ]),
          ),
        ]),
      ),
    );
  }
}
