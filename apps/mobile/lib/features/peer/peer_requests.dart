import 'package:flutter/material.dart';

import 'peer_chats.dart';
import 'peer_repository.dart';
import 'peer_ui.dart';

/// "Today, 2:12 AM".
String _when(DateTime t) => '${peerDayLabel(t)}, ${peerTime(t)}';
String _mode(PeerInboxItem r) => r.isCall ? 'Call' : 'Chat';

/// Asks for an optional note, then declines. Returns true when declined.
Future<bool> peerDecline(
    BuildContext context, PeerRepository repo, PeerInboxItem r) async {
  final note = await peerPromptText(context,
      title: 'Decline this request?',
      hint: 'Optional note, e.g. a better time',
      confirm: 'Decline',
      fieldKey: const ValueKey('peer-decline-note'),
      confirmKey: const ValueKey('peer-decline-confirm'));
  if (note == null) return false;
  try {
    await repo.decide(r.id, 'decline', note: note);
    if (context.mounted) peerToast(context, 'Request declined.');
    return true;
  } catch (e) {
    if (context.mounted) peerToast(context, peerErrorText(e));
    return false;
  }
}

// ---------------------------------------------------------------------------
// Support Requests (references 02 and 02b)
// ---------------------------------------------------------------------------

class PeerRequestsTab extends StatefulWidget {
  const PeerRequestsTab(
      {super.key, required this.repo, required this.onSetAvailability});
  final PeerRepository repo;
  final VoidCallback onSetAvailability;
  @override
  State<PeerRequestsTab> createState() => _PeerRequestsTabState();
}

class _PeerRequestsTabState extends State<PeerRequestsTab> {
  late Future<List<PeerInboxItem>> _load = widget.repo.requests();
  int tab = 0; // 0 Pending, 1 Accepted, 2 All Requests
  bool banner = true, searching = false;
  String query = '', mode = 'ALL';
  String? _busy;
  void _reload() => setState(() {
        _load = widget.repo.requests();
      });

  Future<void> _accept(PeerInboxItem r) async {
    setState(() => _busy = r.id);
    try {
      await widget.repo.decide(r.id, 'accept');
      if (mounted) peerToast(context, 'Request accepted. The chat is open.');
      _reload();
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    } finally {
      if (mounted) setState(() => _busy = null);
    }
  }

  Future<void> _decline(PeerInboxItem r) async {
    setState(() => _busy = r.id);
    final done = await peerDecline(context, widget.repo, r);
    if (!mounted) return;
    setState(() => _busy = null);
    if (done) _reload();
  }

  Future<void> _open(PeerInboxItem r) async {
    await Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => PeerRequestDetailPage(repo: widget.repo, request: r)));
    _reload();
  }

  void _openChat(PeerInboxItem r) => Navigator.of(context)
      .push(MaterialPageRoute(
          builder: (_) => PeerChatPage(
              repo: widget.repo,
              chatId: r.conversationId!,
              requestId: r.id,
              name: r.patientName,
              status: r.status,
              callClosed: r.callClosed)))
      .then((_) => _reload());

  Future<void> _filter() async {
    final picked = await showModalBottomSheet<String>(
        context: context,
        builder: (c) => SafeArea(
                child: Column(mainAxisSize: MainAxisSize.min, children: [
              const ListTile(
                  title: Text('Requested mode',
                      style: TextStyle(fontWeight: FontWeight.w800))),
              for (final (value, label) in [
                ('ALL', 'All modes'),
                ('CHAT', 'Chat'),
                ('CALL', 'Call'),
              ])
                ListTile(
                    key: ValueKey('peer-filter-$value'),
                    title: Text(label),
                    trailing: mode == value
                        ? const Icon(Icons.check_rounded,
                            color: PeerTokens.green)
                        : null,
                    onTap: () => Navigator.pop(c, value)),
            ])));
    if (picked != null) setState(() => mode = picked);
  }

  bool _matches(PeerInboxItem r) {
    final q = query.trim().toLowerCase();
    return (mode == 'ALL' || r.supportType == mode) &&
        (q.isEmpty ||
            r.patientName.toLowerCase().contains(q) ||
            r.topic.toLowerCase().contains(q));
  }

  @override
  Widget build(BuildContext context) => RefreshIndicator(
      onRefresh: () async {
        _reload();
        await _load;
      },
      child: PeerLoader<List<PeerInboxItem>>(
          future: _load,
          onRetry: _reload,
          builder: (items) {
            final pending = items.where((r) => r.status == 'PENDING').toList();
            final accepted = items
                .where((r) => r.status == 'ACCEPTED' || r.status == 'COMPLETED')
                .toList();
            final shown =
                [pending, accepted, items][tab].where(_matches).toList();
            return ListView(
                padding: const EdgeInsets.only(bottom: 24),
                children: [
                  PeerHeader(
                      title: 'Support Requests',
                      subtitle: 'Review and respond to peer support requests',
                      actions: [
                        PeerCircleButton(
                            key: const ValueKey('peer-requests-search'),
                            icon: Icons.search_rounded,
                            tooltip: 'Search',
                            active: searching,
                            onTap: () => setState(() {
                                  searching = !searching;
                                  if (!searching) query = '';
                                })),
                        PeerCircleButton(
                            key: const ValueKey('peer-requests-filter'),
                            icon: Icons.filter_alt_outlined,
                            tooltip: 'Filter',
                            active: mode != 'ALL',
                            onTap: _filter),
                      ]),
                  if (searching)
                    Padding(
                        padding: const EdgeInsets.fromLTRB(18, 0, 18, 12),
                        child: TextField(
                            key: const ValueKey('peer-requests-query'),
                            autofocus: true,
                            onChanged: (v) => setState(() => query = v),
                            decoration: InputDecoration(
                                hintText: 'Search by name or topic',
                                prefixIcon: const Icon(Icons.search_rounded),
                                filled: true,
                                fillColor: Colors.white,
                                border: OutlineInputBorder(
                                    borderRadius: BorderRadius.circular(24),
                                    borderSide: const BorderSide(
                                        color: PeerTokens.line))))),
                  PeerPillTabs(
                      labels: const ['Pending', 'Accepted', 'All Requests'],
                      selected: tab,
                      counts: {0: pending.length, 1: accepted.length},
                      onSelected: (i) => setState(() => tab = i)),
                  if (items.isEmpty)
                    PeerEmptyState(
                        asset: 'assets/peer/empty_requests.png',
                        title: 'No support requests yet',
                        body:
                            'When patients send you a support request, it will appear here for your review.',
                        action: 'Set Availability',
                        actionIcon: Icons.notifications_none_rounded,
                        onAction: widget.onSetAvailability,
                        footer: const Row(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Icon(Icons.edit_calendar_outlined,
                                  color: PeerTokens.grey),
                              SizedBox(width: 10),
                              Flexible(
                                  child: Text(
                                      'Or create support slots from Schedule',
                                      style: TextStyle(
                                          fontSize: 14,
                                          color: PeerTokens.grey))),
                            ]))
                  else ...[
                    if (banner && tab == 0)
                      Padding(
                          padding: const EdgeInsets.fromLTRB(16, 14, 16, 0),
                          child: PeerCard(
                              color: const Color(0xFFEAF6EF),
                              padding: const EdgeInsets.fromLTRB(14, 14, 6, 14),
                              child: Row(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Container(
                                        width: 52,
                                        height: 46,
                                        decoration: const BoxDecoration(
                                            color: Color(0xFFD5F0E0),
                                            shape: BoxShape.circle),
                                        child: const Icon(
                                            Icons.handshake_outlined,
                                            color: PeerTokens.green,
                                            size: 25)),
                                    const SizedBox(width: 12),
                                    const Expanded(
                                        child: Column(
                                            crossAxisAlignment:
                                                CrossAxisAlignment.start,
                                            children: [
                                          Text('Pending Requests',
                                              style: TextStyle(
                                                  fontSize: 15,
                                                  fontWeight: FontWeight.w800,
                                                  color: PeerTokens.navy)),
                                          SizedBox(height: 3),
                                          Text(
                                              'Review request details and accept to start a support conversation.',
                                              style: TextStyle(
                                                  fontSize: 12.5,
                                                  height: 1.35,
                                                  color: PeerTokens.grey)),
                                        ])),
                                    IconButton(
                                        key: const ValueKey(
                                            'peer-requests-banner-close'),
                                        tooltip: 'Dismiss',
                                        onPressed: () =>
                                            setState(() => banner = false),
                                        icon: const Icon(Icons.close_rounded,
                                            color: PeerTokens.grey)),
                                  ]))),
                    if (shown.isEmpty)
                      Padding(
                          padding: const EdgeInsets.all(36),
                          child: Text(
                              query.isNotEmpty || mode != 'ALL'
                                  ? 'No requests match your search.'
                                  : tab == 0
                                      ? 'No pending requests right now.'
                                      : 'No accepted requests yet.',
                              textAlign: TextAlign.center,
                              style: const TextStyle(color: PeerTokens.grey))),
                    for (final r in shown)
                      Padding(
                          padding: const EdgeInsets.fromLTRB(16, 14, 16, 0),
                          child: _RequestCard(
                              request: r,
                              busy: _busy != null,
                              onOpen: () => _open(r),
                              onAccept: () => _accept(r),
                              onDecline: () => _decline(r),
                              onChat: r.conversationId == null
                                  ? null
                                  : () => _openChat(r))),
                  ],
                ]);
          }));
}

class _AgeChip extends StatelessWidget {
  const _AgeChip(this.age);
  final String age;
  @override
  Widget build(BuildContext context) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(20),
          border: Border.all(color: PeerTokens.line)),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        const Icon(Icons.person_outline_rounded,
            size: 15, color: PeerTokens.grey),
        const SizedBox(width: 4),
        Flexible(
            child: Text('Age ${age.replaceAll('-', '–')}',
                overflow: TextOverflow.ellipsis,
                style:
                    const TextStyle(fontSize: 11.5, color: PeerTokens.grey))),
      ]));
}

class _RequestCard extends StatelessWidget {
  const _RequestCard(
      {required this.request,
      required this.busy,
      required this.onOpen,
      required this.onAccept,
      required this.onDecline,
      this.onChat});
  final PeerInboxItem request;
  final bool busy;
  final VoidCallback onOpen, onAccept, onDecline;
  final VoidCallback? onChat;
  @override
  Widget build(BuildContext context) {
    final r = request;
    Widget meta(IconData icon, String label, String value) => Expanded(
            child: Row(children: [
          Icon(icon, size: 20, color: PeerTokens.navy),
          const SizedBox(width: 12),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(label,
                    style:
                        const TextStyle(fontSize: 11, color: PeerTokens.grey)),
                Text(value,
                    style: const TextStyle(
                        fontSize: 12.5,
                        fontWeight: FontWeight.w800,
                        color: PeerTokens.navy)),
              ])),
        ]));
    return PeerCard(
        key: ValueKey('peer-request-${r.id}'),
        onTap: onOpen,
        padding: const EdgeInsets.fromLTRB(14, 14, 14, 14),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            PeerAvatar(r.patientName, size: 40),
            const SizedBox(width: 12),
            Expanded(
                child: Wrap(
                    spacing: 10,
                    runSpacing: 4,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                  Text(r.patientName,
                      style: const TextStyle(
                          fontSize: 16.5,
                          fontWeight: FontWeight.w800,
                          color: PeerTokens.navy)),
                  if (r.ageRange != null) _AgeChip(r.ageRange!),
                ])),
            const SizedBox(width: 8),
            PeerStatusPill(r.status),
          ]),
          const SizedBox(height: 10),
          Container(
              padding: const EdgeInsets.fromLTRB(14, 8, 14, 10),
              decoration: BoxDecoration(
                  color: const Color(0xFFF5F6F8),
                  borderRadius: BorderRadius.circular(12)),
              child: Row(children: [
                const Icon(Icons.chat_bubble_outline_rounded,
                    size: 20, color: PeerTokens.navy),
                const SizedBox(width: 14),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      const Text('Topic',
                          style:
                              TextStyle(fontSize: 11, color: PeerTokens.grey)),
                      Text(r.topic,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 13.5, color: PeerTokens.navy)),
                    ])),
              ])),
          const SizedBox(height: 10),
          IntrinsicHeight(
              child: Row(children: [
            meta(r.isCall ? Icons.call_outlined : Icons.chat_bubble_outline,
                'Requested Mode', _mode(r)),
            const VerticalDivider(color: PeerTokens.line, width: 20),
            meta(Icons.calendar_month_outlined, 'Requested Time',
                _when(r.start)),
          ])),
          const SizedBox(height: 12),
          if (r.status == 'PENDING')
            Row(children: [
              Expanded(
                  flex: 11,
                  child: PeerPrimaryButton(
                      key: ValueKey('accept-${r.id}'),
                      height: 44,
                      label: 'Accept Request',
                      icon: Icons.check_rounded,
                      onPressed: busy ? null : onAccept)),
              const SizedBox(width: 12),
              Expanded(
                  flex: 9,
                  child: PeerDangerOutlineButton(
                      key: ValueKey('decline-${r.id}'),
                      height: 44,
                      label: 'Decline',
                      onPressed: busy ? null : onDecline)),
            ])
          else if (onChat != null)
            OutlinedButton(
                key: ValueKey('open-conversation-${r.id}'),
                style: OutlinedButton.styleFrom(
                    foregroundColor: PeerTokens.green,
                    minimumSize: const Size.fromHeight(50),
                    side: const BorderSide(color: PeerTokens.line),
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(14))),
                onPressed: onChat,
                child: const Row(children: [
                  Icon(Icons.chat_bubble_outline_rounded),
                  SizedBox(width: 14),
                  Expanded(
                      child: Text('Open Conversation',
                          style: TextStyle(
                              fontSize: 14, fontWeight: FontWeight.w800))),
                  Icon(Icons.chevron_right_rounded),
                ])),
        ]));
  }
}

// ---------------------------------------------------------------------------
// Support Request Details (reference 03) + accepted-session actions
// ---------------------------------------------------------------------------

class PeerRequestDetailPage extends StatefulWidget {
  const PeerRequestDetailPage(
      {super.key, required this.repo, required this.request});
  final PeerRepository repo;
  final PeerInboxItem request;
  @override
  State<PeerRequestDetailPage> createState() => _PeerRequestDetailPageState();
}

class _PeerRequestDetailPageState extends State<PeerRequestDetailPage> {
  late PeerInboxItem r = widget.request;
  bool busy = false;

  Future<void> _act(String action, String done) async {
    setState(() => busy = true);
    try {
      final updated = await widget.repo.decide(r.id, action);
      if (!mounted) return;
      setState(() => r = updated);
      peerToast(context, done);
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> _decline() async {
    setState(() => busy = true);
    final done = await peerDecline(context, widget.repo, r);
    if (!mounted) return;
    setState(() => busy = false);
    if (done) Navigator.of(context).pop();
  }

  void _chat() => Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => PeerChatPage(
          repo: widget.repo,
          chatId: r.conversationId!,
          requestId: r.id,
          name: r.patientName,
          status: r.status,
          callClosed: r.callClosed)));

  @override
  Widget build(BuildContext context) {
    Widget row(IconData icon, String label, String value,
            {bool last = false}) =>
        Container(
            padding: const EdgeInsets.symmetric(vertical: 14),
            decoration: BoxDecoration(
                border: last
                    ? null
                    : const Border(bottom: BorderSide(color: PeerTokens.line))),
            child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Icon(icon, size: 24, color: PeerTokens.grey),
              const SizedBox(width: 14),
              SizedBox(
                  width: 124,
                  child: Text(label,
                      style: const TextStyle(
                          fontSize: 14, color: PeerTokens.grey))),
              Expanded(
                  child: Text(value,
                      style: const TextStyle(
                          fontSize: 14,
                          fontWeight: FontWeight.w600,
                          color: PeerTokens.navy))),
            ]));
    final pending = r.status == 'PENDING';
    final accepted = r.status == 'ACCEPTED';
    final age = r.ageRange?.replaceAll('-', '–');
    return Scaffold(
        backgroundColor: Colors.white,
        body: SafeArea(
            child:
                ListView(padding: const EdgeInsets.only(bottom: 24), children: [
          PeerHeader(
              title: 'Support Request Details',
              subtitle:
                  'Review the request and decide to start a support session',
              onBack: () => Navigator.of(context).maybePop()),
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: PeerCard(
                  color: const Color(0xFFEDF7F1),
                  wave: true,
                  padding: const EdgeInsets.fromLTRB(16, 18, 16, 18),
                  child: Row(children: [
                    Container(
                        width: 66,
                        height: 66,
                        alignment: Alignment.center,
                        decoration: const BoxDecoration(
                            color: Color(0xFFD6F1E1), shape: BoxShape.circle),
                        child: Text(peerInitials(r.patientName),
                            style: const TextStyle(
                                fontSize: 24,
                                fontWeight: FontWeight.w800,
                                color: PeerTokens.green))),
                    const SizedBox(width: 14),
                    Expanded(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          Text(r.patientName,
                              style: const TextStyle(
                                  fontSize: 21,
                                  fontWeight: FontWeight.w800,
                                  color: PeerTokens.navy)),
                          const Text('Peer Support Request',
                              style: TextStyle(
                                  fontSize: 14.5, color: PeerTokens.grey)),
                          if (r.ageRange != null) ...[
                            const SizedBox(height: 6),
                            _AgeChip(r.ageRange!),
                          ],
                        ])),
                    PeerStatusPill(r.status, pendingLabel: 'Waiting'),
                  ]))),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
              child: PeerCard(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Column(children: [
                    row(Icons.sms_outlined, 'Topic', r.topic),
                    row(Icons.calendar_month_outlined, 'Date & Time',
                        '${_when(r.start)} – ${peerTime(r.end)}'),
                    row(
                        r.isCall ? Icons.call_outlined : Icons.chat_outlined,
                        'Requested Mode',
                        r.isCall ? 'Call Support' : 'Chat Support',
                        last: age == null),
                    if (age != null)
                      row(Icons.people_outline_rounded, 'Age Group', age,
                          last: true),
                  ]))),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
              child: PeerCard(
                  color: const Color(0xFFEDF7F1),
                  child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Icon(Icons.info_outline_rounded,
                            color: PeerTokens.green, size: 28),
                        const SizedBox(width: 14),
                        Expanded(
                            child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                              Text(
                                  pending
                                      ? 'About the Request'
                                      : accepted
                                          ? 'Session Status'
                                          : 'About this Session',
                                  style: const TextStyle(
                                      fontSize: 16,
                                      fontWeight: FontWeight.w800,
                                      color: PeerTokens.navy)),
                              const SizedBox(height: 4),
                              Text(
                                  switch (r.status) {
                                    'PENDING' =>
                                      'The user has requested a peer support ${r.isCall ? 'call' : 'chat'} session on the topic above. You can accept to start a conversation or decline if you\'re not available.',
                                    'ACCEPTED' =>
                                      'Support session accepted and ready. After your conversation, mark the session as complete. The chat stays open.',
                                    'COMPLETED' =>
                                      'Session complete. You can still chat with them.',
                                    _ =>
                                      'This request is ${r.status.toLowerCase()}.',
                                  },
                                  style: const TextStyle(
                                      fontSize: 14,
                                      height: 1.4,
                                      color: PeerTokens.grey)),
                            ])),
                      ]))),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 20, 16, 0),
              child: pending
                  ? Row(children: [
                      Expanded(
                          flex: 11,
                          child: PeerPrimaryButton(
                              key: const ValueKey('peer-detail-accept'),
                              label: 'Accept Request',
                              icon: Icons.check_rounded,
                              busy: busy,
                              onPressed: () => _act('accept',
                                  'Request accepted. The chat is open.'))),
                      const SizedBox(width: 14),
                      Expanded(
                          flex: 9,
                          child: PeerDangerOutlineButton(
                              key: const ValueKey('peer-detail-decline'),
                              label: 'Decline',
                              onPressed: busy ? null : _decline)),
                    ])
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                          if (accepted && !r.callClosed)
                            PeerPrimaryButton(
                                key: const ValueKey('peer-detail-call'),
                                label: 'Start Support Call',
                                icon: Icons.call_rounded,
                                onPressed: () =>
                                    openPeerCall(context, widget.repo, r.id)),
                          if (r.conversationId != null) ...[
                            const SizedBox(height: 12),
                            SizedBox(
                                height: 52,
                                child: OutlinedButton.icon(
                                    key: const ValueKey('peer-detail-chat'),
                                    style: OutlinedButton.styleFrom(
                                        foregroundColor: PeerTokens.navy,
                                        side: const BorderSide(
                                            color: PeerTokens.line),
                                        shape: RoundedRectangleBorder(
                                            borderRadius:
                                                BorderRadius.circular(16))),
                                    onPressed: _chat,
                                    icon: const Icon(
                                        Icons.chat_bubble_outline_rounded),
                                    label: const Text('Open Chat',
                                        style: TextStyle(
                                            fontSize: 16,
                                            fontWeight: FontWeight.w700)))),
                          ],
                          if (accepted) ...[
                            const SizedBox(height: 12),
                            SizedBox(
                                height: 52,
                                child: OutlinedButton.icon(
                                    key: const ValueKey('mark-complete'),
                                    style: OutlinedButton.styleFrom(
                                        foregroundColor: PeerTokens.green,
                                        side: const BorderSide(
                                            color: PeerTokens.green,
                                            width: 1.3),
                                        shape: RoundedRectangleBorder(
                                            borderRadius:
                                                BorderRadius.circular(16))),
                                    onPressed: busy
                                        ? null
                                        : () => _act('complete',
                                            'Session marked complete. The chat stays open.'),
                                    icon: const Icon(Icons.check_rounded),
                                    label: const Text('Mark Session Complete',
                                        style: TextStyle(
                                            fontSize: 16,
                                            fontWeight: FontWeight.w800)))),
                          ],
                        ])),
        ])));
  }
}
