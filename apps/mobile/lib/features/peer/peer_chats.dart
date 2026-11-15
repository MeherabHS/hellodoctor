import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';

import '../../core/network/api_client.dart';
import '../doctor/agora_page.dart';
import 'peer_repository.dart';
import 'peer_ui.dart';

/// Voice/video for an accepted request, on the shared Agora screen with the
/// peer join-window and token endpoints (no doctor close-out).
void openPeerCall(
        BuildContext context, PeerRepository repo, String requestId) =>
    Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => AgoraPage(
            appointmentId: requestId,
            loadJoin: () => repo.joinEligibility(requestId),
            loadToken: () => repo.rtcToken(requestId))));

// ---------------------------------------------------------------------------
// Chats (references 05 and 05b)
// ---------------------------------------------------------------------------

class PeerChatsTab extends StatefulWidget {
  const PeerChatsTab(
      {super.key, required this.repo, required this.onViewRequests});
  final PeerRepository repo;
  final VoidCallback onViewRequests;
  @override
  State<PeerChatsTab> createState() => _PeerChatsTabState();
}

class _PeerChatsTabState extends State<PeerChatsTab> {
  late Future<List<PeerChatSummary>> _load = widget.repo.chats();
  int tab = 0; // All, Active, Archived (session marked complete)
  bool searching = false;
  String query = '', mode = 'ALL';
  void _reload() => setState(() {
        _load = widget.repo.chats();
      });

  Future<void> _filter() async {
    final picked = await showModalBottomSheet<String>(
        context: context,
        builder: (c) => SafeArea(
                child: Column(mainAxisSize: MainAxisSize.min, children: [
              const ListTile(
                  title: Text('Support type',
                      style: TextStyle(fontWeight: FontWeight.w800))),
              for (final (value, label) in [
                ('ALL', 'All'),
                ('CHAT', 'Chat support'),
                ('CALL', 'Call support'),
              ])
                ListTile(
                    title: Text(label),
                    trailing: mode == value
                        ? const Icon(Icons.check_rounded,
                            color: PeerTokens.green)
                        : null,
                    onTap: () => Navigator.pop(c, value)),
            ])));
    if (picked != null) setState(() => mode = picked);
  }

  void _open(PeerChatSummary c) => Navigator.of(context)
      .push(MaterialPageRoute(
          builder: (_) => PeerChatPage(
              repo: widget.repo,
              chatId: c.id,
              requestId: c.requestId,
              name: c.name,
              status: c.requestStatus)))
      .then((_) => _reload());

  @override
  Widget build(BuildContext context) => RefreshIndicator(
      onRefresh: () async {
        _reload();
        await _load;
      },
      child: PeerLoader<List<PeerChatSummary>>(
          future: _load,
          onRetry: _reload,
          builder: (chats) {
            final sorted = [...chats]..sort((a, b) =>
                (b.lastMessageAt ?? b.start)
                    .compareTo(a.lastMessageAt ?? a.start));
            final active =
                sorted.where((c) => c.requestStatus != 'COMPLETED').toList();
            final archived =
                sorted.where((c) => c.requestStatus == 'COMPLETED').toList();
            final q = query.trim().toLowerCase();
            final shown = [sorted, active, archived][tab]
                .where((c) =>
                    (mode == 'ALL' || c.supportType == mode) &&
                    (q.isEmpty ||
                        c.name.toLowerCase().contains(q) ||
                        (c.lastMessage ?? '').toLowerCase().contains(q)))
                .toList();
            return ListView(
                padding: const EdgeInsets.only(bottom: 24),
                children: [
                  PeerHeader(
                      title: 'Chats',
                      subtitle: chats.isEmpty
                          ? 'Your conversations and support sessions'
                          : 'Your peer support conversations',
                      actions: [
                        PeerCircleButton(
                            key: const ValueKey('peer-chats-search'),
                            icon: Icons.search_rounded,
                            tooltip: 'Search',
                            active: searching,
                            onTap: () => setState(() {
                                  searching = !searching;
                                  if (!searching) query = '';
                                })),
                        PeerCircleButton(
                            icon: Icons.filter_alt_outlined,
                            tooltip: 'Filter',
                            active: mode != 'ALL',
                            onTap: _filter),
                      ]),
                  if (searching)
                    Padding(
                        padding: const EdgeInsets.fromLTRB(18, 0, 18, 12),
                        child: TextField(
                            key: const ValueKey('peer-chats-query'),
                            autofocus: true,
                            onChanged: (v) => setState(() => query = v),
                            decoration: InputDecoration(
                                hintText: 'Search conversations',
                                prefixIcon: const Icon(Icons.search_rounded),
                                filled: true,
                                fillColor: Colors.white,
                                border: OutlineInputBorder(
                                    borderRadius: BorderRadius.circular(24),
                                    borderSide: const BorderSide(
                                        color: PeerTokens.line))))),
                  PeerPillTabs(
                      labels: const ['All Chats', 'Active', 'Archived'],
                      selected: tab,
                      counts: {if (chats.isNotEmpty) 0: chats.length},
                      icons: chats.isEmpty
                          ? const {
                              0: Icons.chat_bubble_outline_rounded,
                              1: Icons.bolt_rounded,
                              2: Icons.check_box_outlined
                            }
                          : const {},
                      onSelected: (i) => setState(() => tab = i)),
                  const SizedBox(height: 14),
                  if (chats.isEmpty)
                    PeerEmptyState(
                        asset: 'assets/peer/empty_chats.webp',
                        title: 'No conversations yet',
                        body:
                            'Your chats will appear here once you accept a support request or start a session.',
                        action: 'View Requests',
                        actionIcon: Icons.chat_bubble_outline_rounded,
                        onAction: widget.onViewRequests)
                  else if (shown.isEmpty)
                    Padding(
                        padding: const EdgeInsets.all(36),
                        child: Text(
                            q.isNotEmpty || mode != 'ALL'
                                ? 'No conversations match your search.'
                                : tab == 2
                                    ? 'Conversations move here when you mark a session complete. They stay open.'
                                    : 'No active conversations.',
                            textAlign: TextAlign.center,
                            style: const TextStyle(color: PeerTokens.grey)))
                  else
                    for (final c in shown)
                      Padding(
                          padding: const EdgeInsets.fromLTRB(14, 0, 14, 10),
                          child: _ChatRow(chat: c, onTap: () => _open(c))),
                ]);
          }));
}

class _ChatRow extends StatelessWidget {
  const _ChatRow({required this.chat, required this.onTap});
  final PeerChatSummary chat;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) {
    final unread = chat.unread > 0;
    final card = PeerCard(
        key: ValueKey('peer-chat-${chat.id}'),
        color: unread ? PeerTokens.mintSoft : Colors.white,
        border: unread ? const Color(0xFFD7EFDF) : PeerTokens.line,
        radius: 22,
        onTap: onTap,
        padding: const EdgeInsets.fromLTRB(18, 16, 16, 16),
        child: Row(children: [
          PeerAvatar(chat.name, size: 56),
          const SizedBox(width: 16),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(chat.name,
                    style: const TextStyle(
                        fontSize: 17,
                        fontWeight: FontWeight.w800,
                        color: PeerTokens.navy)),
                const SizedBox(height: 4),
                Text(chat.lastMessage ?? 'No messages yet',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 14.5, color: PeerTokens.grey)),
              ])),
          const SizedBox(width: 8),
          Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
            Text(peerListTime(chat.lastMessageAt ?? chat.start),
                style: TextStyle(
                    fontSize: 13,
                    color: unread ? PeerTokens.green : PeerTokens.grey)),
            if (unread) ...[
              const SizedBox(height: 8),
              Container(
                  width: 26,
                  height: 26,
                  alignment: Alignment.center,
                  decoration: const BoxDecoration(
                      color: PeerTokens.green, shape: BoxShape.circle),
                  child: Text('${chat.unread}',
                      style: const TextStyle(
                          color: Colors.white,
                          fontSize: 12.5,
                          fontWeight: FontWeight.w800))),
            ],
          ]),
        ]));
    if (!unread) return card;
    // Unread rows carry a green left edge.
    return Stack(children: [
      card,
      Positioned(
          left: 0,
          top: 12,
          bottom: 12,
          child: Container(
              width: 3,
              decoration: BoxDecoration(
                  color: PeerTokens.green,
                  borderRadius: BorderRadius.circular(2)))),
    ]);
  }
}

// ---------------------------------------------------------------------------
// Chat (reference 06). Never closes; images allowed as in doctor chat.
// ---------------------------------------------------------------------------

class PeerChatPage extends StatefulWidget {
  const PeerChatPage(
      {super.key,
      required this.repo,
      required this.chatId,
      required this.requestId,
      required this.name,
      required this.status,
      this.pickImage});
  final PeerRepository repo;
  final String chatId, requestId, name, status;

  /// Injectable picker for tests: returns (bytes, mime) or null.
  final Future<(Uint8List, String)?> Function(ImageSource source)? pickImage;
  @override
  State<PeerChatPage> createState() => _PeerChatPageState();
}

class _PeerChatPageState extends State<PeerChatPage> {
  final _text = TextEditingController();
  List<PeerChatMessage>? _messages;
  String? _error, _me;
  bool _sending = false;
  Timer? _poll;

  @override
  void initState() {
    super.initState();
    _load();
    _poll =
        Timer.periodic(const Duration(seconds: 10), (_) => _load(quiet: true));
  }

  @override
  void dispose() {
    _poll?.cancel();
    _text.dispose();
    super.dispose();
  }

  Future<void> _load({bool quiet = false}) async {
    try {
      final messages = await widget.repo.messages(widget.chatId);
      _me ??= peerUserIdFromToken(await widget.repo.api.read('access_token'));
      if (!mounted) return;
      final grew = messages.length != (_messages?.length ?? -1);
      setState(() {
        _messages = messages;
        _error = null;
      });
      // A failed read receipt must not hide the conversation.
      if (grew) {
        try {
          await widget.repo.markRead(widget.chatId);
        } catch (_) {}
      }
    } catch (e) {
      if (mounted && !quiet) setState(() => _error = peerErrorText(e));
    }
  }

  Future<void> _send() async {
    final content = _text.text.trim();
    if (content.isEmpty || _sending) return;
    setState(() => _sending = true);
    try {
      final message = await widget.repo.send(widget.chatId, content);
      _text.clear();
      if (mounted) setState(() => _messages = [...?_messages, message]);
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    } finally {
      if (mounted) setState(() => _sending = false);
    }
  }

  /// Camera or gallery; the picker's own copy is deleted once read so the
  /// photo is not left in a generic disk cache.
  Future<(Uint8List, String)?> _pick(ImageSource source) async {
    final file =
        await ImagePicker().pickImage(source: source, imageQuality: 85);
    if (file == null) return null;
    final name = file.name.toLowerCase();
    final png = name.endsWith('.png');
    try {
      if (!png && !name.endsWith('.jpg') && !name.endsWith('.jpeg')) {
        throw const ApiException(422, 'Select a JPEG or PNG image.');
      }
      if (await file.length() > 10 * 1024 * 1024) {
        throw const ApiException(413, 'Select an image smaller than 10 MB.');
      }
      return (await file.readAsBytes(), png ? 'image/png' : 'image/jpeg');
    } finally {
      try {
        final copy = File(file.path);
        if (await copy.exists()) await copy.delete();
      } catch (_) {/* best-effort cleanup */}
    }
  }

  Future<void> _attach() async {
    final source = await showModalBottomSheet<ImageSource>(
        context: context,
        builder: (c) => SafeArea(
                child: Column(mainAxisSize: MainAxisSize.min, children: [
              ListTile(
                  key: const ValueKey('peer-attach-camera'),
                  leading: const Icon(Icons.photo_camera_outlined),
                  title: const Text('Take a photo'),
                  onTap: () => Navigator.pop(c, ImageSource.camera)),
              ListTile(
                  key: const ValueKey('peer-attach-gallery'),
                  leading: const Icon(Icons.photo_library_outlined),
                  title: const Text('Choose from gallery'),
                  onTap: () => Navigator.pop(c, ImageSource.gallery)),
            ])));
    if (source == null || !mounted) return;
    setState(() => _sending = true);
    try {
      final picked = await (widget.pickImage ?? _pick)(source);
      if (picked == null) return;
      final key = List.generate(
          16,
          (_) => Random.secure()
              .nextInt(256)
              .toRadixString(16)
              .padLeft(2, '0')).join();
      final message =
          await widget.repo.sendImage(widget.chatId, picked.$1, picked.$2, key);
      if (mounted) setState(() => _messages = [...?_messages, message]);
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    } finally {
      if (mounted) setState(() => _sending = false);
    }
  }

  Future<void> _viewImage(PeerChatMessage m) async {
    try {
      final bytes = await widget.repo.image(widget.chatId, m.id);
      if (!mounted) return;
      await showDialog<void>(
          context: context,
          builder: (c) => Dialog(
              clipBehavior: Clip.antiAlias,
              child: Column(mainAxisSize: MainAxisSize.min, children: [
                InteractiveViewer(
                    child: Image.memory(bytes, gaplessPlayback: true)),
                TextButton(
                    onPressed: () => Navigator.pop(c),
                    child: const Text('Close')),
              ])));
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    }
  }

  @override
  Widget build(BuildContext context) {
    final messages = _messages;
    return Scaffold(
        backgroundColor: Colors.white,
        body: SafeArea(
            child: Column(children: [
          // Header: back, avatar, name, status.
          Padding(
              padding: const EdgeInsets.fromLTRB(14, 12, 14, 10),
              child: Row(children: [
                PeerCircleButton(
                    icon: Icons.arrow_back_rounded,
                    tooltip: 'Back',
                    filled: true,
                    onTap: () => Navigator.of(context).maybePop()),
                const SizedBox(width: 12),
                PeerAvatar(widget.name, size: 56),
                const SizedBox(width: 12),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      Text(widget.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 21,
                              fontWeight: FontWeight.w800,
                              color: PeerTokens.navy)),
                      const Text('Peer Support Chat',
                          style:
                              TextStyle(fontSize: 14, color: PeerTokens.grey)),
                    ])),
                PeerStatusPill(widget.status),
              ])),
          Padding(
              padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
              child: PeerCard(
                  color: const Color(0xFFEAF6EF),
                  wave: true,
                  padding: const EdgeInsets.fromLTRB(14, 14, 14, 14),
                  child: IntrinsicHeight(
                      child: Row(children: [
                    Container(
                        width: 46,
                        height: 46,
                        decoration: const BoxDecoration(
                            color: Color(0xFFD5F0E0), shape: BoxShape.circle),
                        child: const Icon(Icons.chat_outlined,
                            color: PeerTokens.green)),
                    const SizedBox(width: 10),
                    const VerticalDivider(
                        width: 1, thickness: 1.2, color: PeerTokens.mintLine),
                    const SizedBox(width: 12),
                    Expanded(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          const Text('Peer support conversation active.',
                              style: TextStyle(
                                  fontSize: 15,
                                  fontWeight: FontWeight.w800,
                                  color: PeerTokens.navy)),
                          const SizedBox(height: 3),
                          Text(
                              'You\'re connected with ${widget.name} for peer support, guidance, and follow-up.',
                              style: const TextStyle(
                                  fontSize: 13.5,
                                  height: 1.35,
                                  color: PeerTokens.grey)),
                        ])),
                  ])))),
          Expanded(
              child: _error != null && messages == null
                  ? Center(
                      child: TextButton(onPressed: _load, child: Text(_error!)))
                  : messages == null
                      ? const Center(child: CircularProgressIndicator())
                      : messages.isEmpty
                          ? const Center(
                              child: Text('No messages yet. Say hello.',
                                  style: TextStyle(color: PeerTokens.grey)))
                          : ListView(
                              reverse: true,
                              padding: const EdgeInsets.fromLTRB(12, 12, 12, 8),
                              children: [
                                  for (final m in messages.reversed)
                                    _Bubble(
                                        message: m,
                                        mine: m.senderId == _me,
                                        name: widget.name,
                                        onView: m.hasImage
                                            ? () => _viewImage(m)
                                            : null),
                                ])),
          if (widget.status == 'ACCEPTED')
            Padding(
                padding: const EdgeInsets.fromLTRB(16, 4, 16, 10),
                child: Center(
                    child: OutlinedButton.icon(
                        key: const ValueKey('peer-chat-call'),
                        style: OutlinedButton.styleFrom(
                            foregroundColor: PeerTokens.green,
                            minimumSize: const Size(270, 52),
                            side: const BorderSide(
                                color: PeerTokens.green, width: 1.4),
                            shape: const StadiumBorder(),
                            textStyle: const TextStyle(
                                fontSize: 17, fontWeight: FontWeight.w800)),
                        onPressed: () => openPeerCall(
                            context, widget.repo, widget.requestId),
                        icon: const Icon(Icons.call_rounded, size: 26),
                        label: const Text('Start Support Call')))),
          Padding(
              padding: const EdgeInsets.fromLTRB(12, 0, 12, 10),
              child: Row(children: [
                Expanded(
                    child: Container(
                        decoration: BoxDecoration(
                            color: Colors.white,
                            borderRadius: BorderRadius.circular(28),
                            border: Border.all(color: const Color(0xFFD5DADF))),
                        child: Row(children: [
                          IconButton(
                              key: const ValueKey('peer-attach'),
                              tooltip: 'Attach a photo',
                              onPressed: _sending ? null : _attach,
                              icon: Transform.rotate(
                                  angle: 0.75,
                                  child: const Icon(Icons.attach_file_rounded,
                                      color: PeerTokens.grey))),
                          Container(
                              width: 1,
                              height: 26,
                              color: const Color(0xFFD5DADF)),
                          const SizedBox(width: 10),
                          Expanded(
                              child: TextField(
                                  key: const ValueKey('peer-message-input'),
                                  controller: _text,
                                  maxLength: 2000,
                                  minLines: 1,
                                  maxLines: 4,
                                  decoration: const InputDecoration(
                                      counterText: '',
                                      border: InputBorder.none,
                                      enabledBorder: InputBorder.none,
                                      focusedBorder: InputBorder.none,
                                      filled: false,
                                      hintText: 'Type a message...'))),
                        ]))),
                const SizedBox(width: 10),
                Material(
                    color: PeerTokens.green,
                    shape: const CircleBorder(),
                    child: InkWell(
                        key: const ValueKey('peer-send'),
                        customBorder: const CircleBorder(),
                        onTap: _sending ? null : _send,
                        child: const SizedBox(
                            width: 56,
                            height: 56,
                            child: Icon(Icons.send_rounded,
                                color: Colors.white, size: 26)))),
              ])),
        ])));
  }
}

class _Bubble extends StatelessWidget {
  const _Bubble(
      {required this.message,
      required this.mine,
      required this.name,
      this.onView});
  final PeerChatMessage message;
  final bool mine;
  final String name;
  final VoidCallback? onView;
  @override
  Widget build(BuildContext context) {
    final bubble = Container(
        key: ValueKey('peer-message-${message.id}'),
        constraints:
            BoxConstraints(maxWidth: MediaQuery.sizeOf(context).width * 0.74),
        margin: const EdgeInsets.symmetric(vertical: 6),
        padding: const EdgeInsets.fromLTRB(16, 12, 14, 10),
        decoration: BoxDecoration(
            color: mine ? const Color(0xFFDDF3E5) : const Color(0xFFF1F2F5),
            borderRadius: BorderRadius.circular(20)),
        child: Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
          Align(
              alignment: Alignment.centerLeft,
              child: message.hasImage
                  ? TextButton.icon(
                      key: ValueKey('peer-view-image-${message.id}'),
                      style: TextButton.styleFrom(
                          foregroundColor: PeerTokens.navy,
                          padding: EdgeInsets.zero),
                      onPressed: onView,
                      icon: const Icon(Icons.image_outlined),
                      label: const Text('Photo · View image'))
                  : Text(message.content,
                      style: const TextStyle(
                          fontSize: 15.5,
                          height: 1.4,
                          color: PeerTokens.navy))),
          const SizedBox(height: 6),
          Row(mainAxisSize: MainAxisSize.min, children: [
            Text(peerTime(message.sentAt),
                style: const TextStyle(fontSize: 12.5, color: PeerTokens.grey)),
            if (mine) ...[
              const SizedBox(width: 6),
              Icon(message.isRead ? Icons.done_all_rounded : Icons.done_rounded,
                  size: 18,
                  color: message.isRead ? PeerTokens.green : PeerTokens.grey),
            ],
          ]),
        ]));
    return Align(
        alignment: mine ? Alignment.centerRight : Alignment.centerLeft,
        child: mine
            ? bubble
            : Row(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                    Padding(
                        padding: const EdgeInsets.only(bottom: 8, right: 8),
                        child: PeerAvatar(name, size: 38)),
                    Flexible(child: bubble),
                  ]));
  }
}

/// The signed-in user's id from the (unverified) access-token payload; used
/// only to tell own messages apart.
String? peerUserIdFromToken(String? token) {
  if (token == null) return null;
  final parts = token.split('.');
  if (parts.length != 3) return null;
  try {
    final payload =
        utf8.decode(base64Url.decode(base64Url.normalize(parts[1])));
    final match = RegExp(r'"user_id"\s*:\s*"([^"]+)"').firstMatch(payload);
    return match?.group(1);
  } catch (_) {
    return null;
  }
}
