import 'dart:async';
import 'package:flutter/material.dart';
import '../../core/notifications/notification_taps.dart';
import '../../l10n/app_localizations.dart';
import '../patient/blogs/blog_detail_page.dart';
import '../patient/blogs/blogs_repository.dart';
import '../patient/chat/chat_models.dart';
import '../patient/chat/chat_repository.dart';
import '../patient/chat/patient_chat_page.dart';
import '../patient/profile/profile_repository.dart';
import 'notification_appointment_page.dart';
import 'notification_center_page.dart';
import 'notification_cubit.dart';
import 'notification_destination.dart';

Future<void> _recordOpened(NotificationCubit cubit, String id) async {
  if (cubit.isClosed) return;
  if (cubit.state.items.any((n) => n.id == id)) {
    await cubit.markRead(id);
  } else {
    // Cold-start lookup can finish before the paged inbox refresh does.
    // Record only the explicit, authorized open; receipt itself stays unread.
    try {
      await cubit.repo.markRead(id);
      if (!cubit.isClosed) await cubit.refresh();
    } catch (_) {/* Read acknowledgment remains retryable from the inbox. */}
  }
}

Future<void> openNotificationDestination(BuildContext context,
        NotificationCubit cubit, String? id, NotificationTapGate gate,
        {bool fromInbox = false}) =>
    gate.run(id, () async {
      void inbox() {
        if (context.mounted && gate.current && !fromInbox) {
          unawaited(Navigator.of(context).push(MaterialPageRoute<void>(
              builder: (_) => NotificationCenterPage(cubit: cubit))));
        }
      }

      try {
        final destination =
            await NotificationDestination.load(cubit.repo, id, doctor: false);
        if (!context.mounted || !gate.current) return;
        Widget? page;
        switch (destination.kind) {
          case NotificationDestinationKind.appointment:
            page = NotificationAppointmentPage(
                api: cubit.repo.api, id: destination.id!);
          case NotificationDestinationKind.chat:
            final profile = await ProfileRepository(cubit.repo.api).profile();
            if (!context.mounted || !gate.current) return;
            page = PatientChatPage(
                conversation: ChatConversationDto.fromJson(destination.data),
                doctorDisplayName: AppLocalizations.of(context).doctorLabel,
                repo: ChatRepository(cubit.repo.api),
                myUserId: profile.userId);
          case NotificationDestinationKind.article:
            page = BlogDetailPage(
                articleId: destination.id!,
                repo: BlogsRepository(cubit.repo.api));
          case NotificationDestinationKind.inbox:
          case NotificationDestinationKind.doctorArticle:
            inbox();
        }
        if (page != null && context.mounted && gate.current) {
          unawaited(Navigator.of(context)
              .push(MaterialPageRoute<void>(builder: (_) => page!)));
          // Only explicit user interaction after authorization marks a notice read.
          if (notificationUuid(id)) unawaited(_recordOpened(cubit, id!));
        } else if (fromInbox &&
            notificationUuid(id) &&
            context.mounted &&
            gate.current) {
          unawaited(_recordOpened(cubit, id!));
        }
      } catch (_) {
        if (context.mounted && gate.current) {
          inbox();
          ScaffoldMessenger.of(context).showSnackBar(SnackBar(
              content: Text(
                  AppLocalizations.of(context).notificationDestinationError)));
        }
        rethrow;
      }
    });

/// Mounted only inside the authenticated, profile-complete app subtree.
class NotificationNavigationHost extends StatefulWidget {
  const NotificationNavigationHost(
      {super.key, required this.cubit, required this.child});
  final NotificationCubit cubit;
  final Widget child;
  @override
  State<NotificationNavigationHost> createState() => _HostState();
}

class _HostState extends State<NotificationNavigationHost> {
  final _gate = NotificationTapGate();
  StreamSubscription<String?>? _subscription;
  void _open(String? id) {
    unawaited(openNotificationDestination(context, widget.cubit, id, _gate)
        .catchError((_) {}));
  }

  @override
  void initState() {
    super.initState();
    _subscription = NotificationTaps.events.listen(_open);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        for (final id in NotificationTaps.drain()) {
          _open(id);
        }
      }
    });
  }

  @override
  void dispose() {
    _gate.dispose();
    _subscription?.cancel();
    NotificationTaps.clear();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
