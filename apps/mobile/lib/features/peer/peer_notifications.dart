import 'package:flutter/material.dart';

import '../notifications/notification_models.dart';
import '../notifications/notification_repository.dart';
import 'peer_chats.dart';
import 'peer_repository.dart';
import 'peer_requests.dart';
import 'peer_ui.dart';

/// Notifications (references 08 and 08b): the peer's own inbox grouped by
/// day. Texts come from the server and carry no names, topics or messages.
class PeerNotificationsPage extends StatefulWidget {
  const PeerNotificationsPage(
      {super.key, required this.repo, required this.notifications});
  final PeerRepository repo;
  final NotificationRepository notifications;
  @override
  State<PeerNotificationsPage> createState() => _PeerNotificationsPageState();
}

class _PeerNotificationsPageState extends State<PeerNotificationsPage> {
  late Future<(List<NotificationDto>, List<PeerInboxItem>)> _load = _fetch();
  final _read = <String>{};

  Future<(List<NotificationDto>, List<PeerInboxItem>)> _fetch() async {
    final items = await widget.notifications.list(limit: 50);
    // Used to show Decline / View Request only while a request is pending.
    List<PeerInboxItem> requests = const [];
    try {
      requests = await widget.repo.requests();
    } catch (_) {}
    return (items, requests);
  }

  void _reload() => setState(() {
        _load = _fetch();
      });

  Future<void> _markRead(NotificationDto n) async {
    if (n.isRead || _read.contains(n.id)) return;
    setState(() => _read.add(n.id));
    try {
      await widget.notifications.markRead(n.id);
    } catch (_) {}
  }

  Future<void> _open(NotificationDto n, List<PeerInboxItem> requests) async {
    _markRead(n);
    final target = n.relatedEntityId;
    if (n.relatedEntityType == 'PEER_REQUEST') {
      PeerInboxItem? request;
      for (final r in requests) {
        if (r.id == target) request = r;
      }
      if (request == null) {
        peerToast(context, 'This request is no longer available.');
        return;
      }
      await Navigator.of(context).push(MaterialPageRoute(
          builder: (_) =>
              PeerRequestDetailPage(repo: widget.repo, request: request!)));
      _reload();
    } else if (n.relatedEntityType == 'PEER_CHAT') {
      try {
        final chats = await widget.repo.chats();
        final chat = chats.where((c) => c.id == target).firstOrNull;
        if (!mounted) return;
        if (chat == null) {
          peerToast(context, 'This conversation is no longer available.');
          return;
        }
        await Navigator.of(context).push(MaterialPageRoute(
            builder: (_) => PeerChatPage(
                repo: widget.repo,
                chatId: chat.id,
                requestId: chat.requestId,
                name: chat.name,
                status: chat.requestStatus)));
      } catch (e) {
        if (mounted) peerToast(context, peerErrorText(e));
      }
    }
  }

  Future<void> _decline(NotificationDto n, PeerInboxItem r) async {
    _markRead(n);
    if (await peerDecline(context, widget.repo, r)) _reload();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: Colors.white,
      body: SafeArea(
          child: RefreshIndicator(
              onRefresh: () async {
                _reload();
                await _load;
              },
              child: ListView(
                  padding: const EdgeInsets.only(bottom: 24),
                  children: [
                    PeerHeader(
                        title: 'Notifications',
                        subtitle:
                            'Updates on your requests, sessions and messages',
                        onBack: () => Navigator.of(context).maybePop()),
                    PeerLoader<(List<NotificationDto>, List<PeerInboxItem>)>(
                        future: _load,
                        onRetry: _reload,
                        builder: (data) {
                          final (items, requests) = data;
                          if (items.isEmpty) {
                            return const Padding(
                                padding: EdgeInsets.only(top: 90),
                                child: PeerEmptyState(
                                    asset:
                                        'assets/peer/empty_notifications.png',
                                    title: 'No notifications yet',
                                    body:
                                        'You\'ll see updates here for your support requests, sessions and messages.'));
                          }
                          final groups = <String, List<NotificationDto>>{};
                          for (final n in items) {
                            final label = peerDayLabel(n.createdAt.toLocal());
                            groups
                                .putIfAbsent(
                                    label == 'Today' || label == 'Yesterday'
                                        ? label
                                        : 'Earlier',
                                    () => [])
                                .add(n);
                          }
                          return Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
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
                                        child: _NoticeCard(
                                            notice: n,
                                            unread: !n.isRead &&
                                                !_read.contains(n.id),
                                            pendingRequest: n.type ==
                                                    'PEER_REQUEST_NEW'
                                                ? requests
                                                    .where((r) =>
                                                        r.id ==
                                                            n.relatedEntityId &&
                                                        r.status == 'PENDING')
                                                    .firstOrNull
                                                : null,
                                            onOpen: () => _open(n, requests),
                                            onDecline: (r) => _decline(n, r))),
                                ],
                              ]);
                        }),
                  ]))));
}

class _NoticeCard extends StatelessWidget {
  const _NoticeCard(
      {required this.notice,
      required this.unread,
      required this.pendingRequest,
      required this.onOpen,
      required this.onDecline});
  final NotificationDto notice;
  final bool unread;
  final PeerInboxItem? pendingRequest;
  final VoidCallback onOpen;
  final ValueChanged<PeerInboxItem> onDecline;

  @override
  Widget build(BuildContext context) {
    final n = notice;
    final (icon, bg, fg) = switch (n.type) {
      'PEER_REQUEST_NEW' => (
          Icons.person_add_alt_outlined,
          const Color(0xFFE8F5E9),
          PeerTokens.green
        ),
      'NEW_CHAT_MESSAGE' => (
          Icons.chat_bubble_outline_rounded,
          PeerTokens.purple,
          PeerTokens.purpleText
        ),
      'PEER_REQUEST_CANCELLED' => (
          Icons.event_busy_outlined,
          const Color(0xFFFFF5E5),
          const Color(0xFFD98A00)
        ),
      'PEER_REQUEST_ACCEPTED' => (
          Icons.event_available_outlined,
          PeerTokens.blue,
          PeerTokens.blueText
        ),
      _ => (
          Icons.notifications_none_rounded,
          PeerTokens.blue,
          PeerTokens.blueText
        ),
    };
    final card = PeerCard(
        key: ValueKey('peer-notice-${n.id}'),
        onTap: onOpen,
        radius: 20,
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
                    style: const TextStyle(
                        fontSize: 15.5,
                        fontWeight: FontWeight.w800,
                        color: PeerTokens.navy)),
                const SizedBox(height: 3),
                Text(n.body,
                    style: const TextStyle(
                        fontSize: 13.5, height: 1.35, color: PeerTokens.navy)),
                const SizedBox(height: 4),
                Text(peerAgo(n.createdAt.toLocal()),
                    style: const TextStyle(
                        fontSize: 12.5, color: PeerTokens.grey)),
                if (pendingRequest != null) ...[
                  const SizedBox(height: 10),
                  Align(
                      alignment: Alignment.centerRight,
                      child: Wrap(spacing: 8, runSpacing: 6, children: [
                        OutlinedButton(
                            key: ValueKey('peer-notice-decline-${n.id}'),
                            style: OutlinedButton.styleFrom(
                                foregroundColor: PeerTokens.navy,
                                side:
                                    const BorderSide(color: Color(0xFFCDD2D9)),
                                visualDensity: VisualDensity.compact,
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 14),
                                textStyle: const TextStyle(fontSize: 13.5),
                                shape: const StadiumBorder()),
                            onPressed: () => onDecline(pendingRequest!),
                            child: const Text('Decline',
                                style: TextStyle(fontWeight: FontWeight.w700))),
                        FilledButton(
                            key: ValueKey('peer-notice-view-${n.id}'),
                            style: FilledButton.styleFrom(
                                backgroundColor: PeerTokens.green,
                                visualDensity: VisualDensity.compact,
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 14),
                                textStyle: const TextStyle(fontSize: 13.5),
                                shape: const StadiumBorder()),
                            onPressed: onOpen,
                            child: const Text('View Request',
                                style: TextStyle(fontWeight: FontWeight.w800))),
                      ])),
                ],
              ])),
          Column(children: [
            SizedBox(
                height: 14,
                child: unread
                    ? Container(
                        key: ValueKey('peer-notice-unread-${n.id}'),
                        width: 10,
                        height: 10,
                        decoration: const BoxDecoration(
                            color: PeerTokens.green, shape: BoxShape.circle))
                    : null),
            const SizedBox(height: 8),
            const Icon(Icons.chevron_right_rounded, color: PeerTokens.navy),
          ]),
        ]));
    if (!(unread && pendingRequest != null)) return card;
    return Stack(children: [
      card,
      Positioned(
          left: 0,
          top: 10,
          bottom: 10,
          child: Container(
              width: 3,
              decoration: BoxDecoration(
                  color: PeerTokens.green,
                  borderRadius: BorderRadius.circular(2)))),
    ]);
  }
}
