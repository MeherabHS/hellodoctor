import '../../../core/network/screen_refresh.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'clinical_image_viewer.dart';
import 'dart:async';

import 'package:flutter/material.dart';
import '../../../core/l10n/patient_display.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../grievance/grievance_modal.dart';
import '../grievance/grievance_repository.dart';
import 'chat_cubit.dart';
import 'chat_models.dart';
import 'chat_repository.dart';

/// `prototype/index.html` view-6 — "Consultation Chat & Digital Rx". Scoped
/// to plain-text messaging plus a real "Report Issue" grievance shortcut
/// (M8): the prototype's digital-Rx and patient-history shortcuts in the
/// chat header remain out of scope — those are Doctor-authored actions
/// (`apps/mobile`'s Smart Rx / patient-history screens have no patient-side
/// equivalent contract), so this screen doesn't fabricate buttons for them.
class PatientChatPage extends StatefulWidget {
  const PatientChatPage({
    super.key,
    required this.conversation,
    required this.doctorDisplayName,
    required this.repo,
    this.myUserId,
  });
  final ChatConversationDto conversation;
  final String doctorDisplayName;
  final ChatRepository repo;
  final String? myUserId;

  @override
  State<PatientChatPage> createState() => _PatientChatPageState();
}

class _PatientChatPageState extends State<PatientChatPage> {
  late final ChatCubit _cubit;
  final _input = TextEditingController();
  late final ScreenRefresh _screenRefresh;
  final _acknowledged = <String>{};
  final _displayed = <String>{};
  bool _ackBusy = false;
  DateTime? _refreshFailure;
  StreamSubscription<ChatState>? _stateSubscription;

  @override
  void initState() {
    super.initState();
    _input.addListener(() {
      if (mounted) setState(() {});
    });
    _cubit = ChatCubit(widget.repo, conversationId: widget.conversation.id)
      ..onRefreshOutcome = _refreshObserved
      ..load();
    unawaited(_ack('CHAT_OPENED'));
    unawaited(_ack('CONSULTATION_OPENED'));
    var serverLocked = false;
    _stateSubscription = _cubit.stream.listen((state) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _ackRead();
      });
      if (mounted && state.serverLocked != serverLocked) {
        serverLocked = state.serverLocked;
        setState(() {});
      }
    });
    _screenRefresh = ScreenRefresh(() async {
      await _cubit.poll();
      if (mounted) setState(() {});
    },
        interval: const Duration(seconds: 10),
        networkChanges: widget.repo.connectivity?.changes,
        enabled: () =>
            !widget.conversation.isExpired && !_cubit.state.serverLocked);
  }

  void _refreshObserved(bool succeeded) {
    if (!succeeded) {
      _refreshFailure ??= DateTime.now().toUtc();
      return;
    }
    final failedAt = _refreshFailure;
    if (failedAt == null) return;
    _refreshFailure = null;
    unawaited(_ackRefreshRecovery(failedAt));
  }

  Future<void> _ackRefreshRecovery(DateTime failedAt) async {
    try {
      await widget.repo.acknowledge(widget.conversation.appointmentId,
          widget.conversation.id, 'CHAT_REFRESH_FAILED',
          clientOccurredAt: failedAt);
      await _ack('CHAT_REFRESH_SUCCEEDED');
    } catch (_) {
      // Retain the client observation until a successful backend acknowledgement.
      _refreshFailure ??= failedAt;
    }
  }

  Future<void> _ack(String event, {List<String> messages = const []}) async {
    try {
      await widget.repo.acknowledge(
          widget.conversation.appointmentId, widget.conversation.id, event,
          messages: messages);
    } catch (_) {
      /* Acknowledgement is retried by the next successful refresh. */
    }
  }

  Future<void> _ackRead() async {
    if (_ackBusy) return;
    final ids = _displayed.difference(_acknowledged).take(50).toList();
    if (ids.isEmpty) return;
    _ackBusy = true;
    try {
      await widget.repo.acknowledge(widget.conversation.appointmentId,
          widget.conversation.id, 'CHAT_READ_ACKNOWLEDGED',
          messages: ids);
      _acknowledged.addAll(ids);
    } catch (_) {/* Retry after the next displayed refresh. */} finally {
      _ackBusy = false;
    }
  }

  @override
  void dispose() {
    _screenRefresh.dispose();
    _stateSubscription?.cancel();
    _cubit.close();
    _input.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _input.text;
    if (text.trim().isEmpty || widget.conversation.isExpired) return;
    await _cubit.send(text);
    if (mounted && !_cubit.state.notSent) _input.clear();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _screenRefresh.attach(context);
  }

  @override
  Widget build(BuildContext context) {
    final locked = widget.conversation.isExpired || _cubit.state.serverLocked;
    final l10n = AppLocalizations.of(context);
    return BlocProvider.value(
      value: _cubit,
      child: Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
          child: Column(children: [
            _ChatTopBar(
                name: widget.doctorDisplayName,
                expiresAt: widget.conversation.expiresAt,
                appointmentId: widget.conversation.appointmentId,
                grievanceRepo: GrievanceRepository(widget.repo.api)),
            if (!locked) _ExpiryBar(expiresAt: widget.conversation.expiresAt),
            Expanded(
              child: BlocBuilder<ChatCubit, ChatState>(
                builder: (context, state) {
                  if (state.loading && state.messages.isEmpty) {
                    return const Center(child: CircularProgressIndicator());
                  }
                  if (state.error != null && state.messages.isEmpty) {
                    return ErrorPanel(state.error!, onRetry: _cubit.load);
                  }
                  if (state.messages.isEmpty) {
                    return Center(
                        child: Text(l10n.chatNoMessages,
                            style:
                                const TextStyle(color: PatientTokens.textSub)));
                  }
                  return ListView.builder(
                    reverse: true,
                    scrollCacheExtent: const ScrollCacheExtent.pixels(0),
                    padding: const EdgeInsets.all(16),
                    itemCount: state.messages.length + 1,
                    itemBuilder: (_, i) {
                      if (i == state.messages.length) {
                        return state.hasOlder
                            ? TextButton(
                                onPressed: state.loadingOlder
                                    ? null
                                    : _cubit.loadOlder,
                                child: Text(state.loadingOlder
                                    ? 'Loading history...'
                                    : 'Load earlier messages'))
                            : const SizedBox.shrink();
                      }
                      final m = state.messages[state.messages.length - 1 - i];
                      final fromMe = widget.myUserId != null &&
                          m.senderId == widget.myUserId;
                      if (!fromMe) {
                        _displayed.add(m.id);
                        WidgetsBinding.instance.addPostFrameCallback((_) {
                          if (mounted) _ackRead();
                        });
                      }
                      return _ChatBubble(
                          message: m,
                          fromMe: fromMe,
                          onViewAttachment: () => Navigator.push(
                              context,
                              MaterialPageRoute(
                                  builder: (_) => ClinicalImageViewer(
                                      load: () => widget.repo.clinicalImage(
                                          widget.conversation.id, m.id)))));
                    },
                  );
                },
              ),
            ),
            if (locked)
              Padding(
                padding: const EdgeInsets.all(14),
                child: Text(l10n.chatWindowClosed,
                    textAlign: TextAlign.center,
                    style: const TextStyle(
                        color: PatientTokens.danger, fontSize: 12)),
              )
            else
              BlocBuilder<ChatCubit, ChatState>(
                builder: (context, state) => Container(
                  padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
                  decoration: const BoxDecoration(
                      color: PatientTokens.surface,
                      border:
                          Border(top: BorderSide(color: PatientTokens.border))),
                  child: Column(children: [
                    if (state.notSent)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 6),
                        child: Align(
                          alignment: Alignment.centerLeft,
                          child: Text(l10n.chatNotSent,
                              style: const TextStyle(
                                  color: PatientTokens.danger, fontSize: 11)),
                        ),
                      ),
                    Row(children: [
                      Expanded(
                        child: Container(
                          decoration: BoxDecoration(
                              color: PatientTokens.surfaceSubtle,
                              borderRadius: BorderRadius.circular(20),
                              border: Border.all(color: PatientTokens.border)),
                          padding: const EdgeInsets.symmetric(horizontal: 14),
                          child: TextField(
                            controller: _input,
                            textInputAction: TextInputAction.newline,
                            minLines: 1,
                            maxLines: 4,
                            onSubmitted: (_) => _send(),
                            maxLength: 2000,
                            style: const TextStyle(fontSize: 12.5),
                            decoration: InputDecoration(
                                filled: false,
                                border: InputBorder.none,
                                hintText: l10n.chatInputHint,
                                isDense: true,
                                counterText: '',
                                contentPadding:
                                    const EdgeInsets.symmetric(vertical: 10)),
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      SizedBox(
                        width: 48,
                        height: 48,
                        child: Material(
                          color: PatientTokens.primary,
                          shape: const CircleBorder(),
                          child: InkWell(
                            customBorder: const CircleBorder(),
                            onTap: state.sending || _input.text.trim().isEmpty
                                ? null
                                : _send,
                            child: state.sending
                                ? const Padding(
                                    padding: EdgeInsets.all(9),
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2, color: Colors.white))
                                : const Icon(Icons.send,
                                    size: 17, color: Colors.white),
                          ),
                        ),
                      ),
                    ]),
                  ]),
                ),
              ),
          ]),
        ),
      ),
    );
  }
}

class _ChatTopBar extends StatelessWidget {
  const _ChatTopBar({
    required this.name,
    required this.expiresAt,
    required this.appointmentId,
    required this.grievanceRepo,
  });
  final String name;
  final DateTime expiresAt;
  final String appointmentId;
  final GrievanceRepository grievanceRepo;

  @override
  Widget build(BuildContext context) {
    final initials = _initialsOf(name);
    final l10n = AppLocalizations.of(context);
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 8, 14, 10),
      decoration: const BoxDecoration(
          color: PatientTokens.surface,
          border: Border(bottom: BorderSide(color: PatientTokens.border))),
      child: Row(children: [
        InkWell(
          onTap: () => Navigator.of(context).maybePop(),
          borderRadius: BorderRadius.circular(11),
          child: Container(
            width: 48,
            height: 48,
            decoration: BoxDecoration(
                color: PatientTokens.surfaceSubtle,
                borderRadius: BorderRadius.circular(11),
                border: Border.all(color: PatientTokens.border)),
            alignment: Alignment.center,
            child: const Icon(Icons.arrow_back, size: 18),
          ),
        ),
        const SizedBox(width: 8),
        CircleAvatar(
          radius: 18,
          backgroundColor: PatientTokens.primaryLight,
          child: Text(initials,
              style: const TextStyle(
                  fontWeight: FontWeight.w800,
                  fontSize: 12,
                  color: PatientTokens.primaryDark)),
        ),
        const SizedBox(width: 10),
        Expanded(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(patientDoctorName(name),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                    fontWeight: FontWeight.w800,
                    fontSize: 13,
                    color: PatientTokens.text)),
            Text(l10n.waitingRoomChatTitle,
                style: const TextStyle(
                    fontSize: 10, color: PatientTokens.textSub)),
          ]),
        ),
        IconButton(
          onPressed: () => showGrievanceModal(
            context,
            appointmentId: appointmentId,
            doctorName: name,
            repo: grievanceRepo,
          ),
          icon: const Icon(Icons.report_problem_outlined,
              size: 18, color: PatientTokens.danger),
          tooltip: l10n.waitingRoomReportIssue,
        ),
      ]),
    );
  }

  String _initialsOf(String name) {
    final words = name
        .replaceAll('Dr.', '')
        .trim()
        .split(RegExp(r'\s+'))
        .where((w) => w.isNotEmpty)
        .toList();
    if (words.isEmpty) return '?';
    if (words.length == 1) return words.first.substring(0, 1).toUpperCase();
    return (words.first.substring(0, 1) + words.last.substring(0, 1))
        .toUpperCase();
  }
}

class _ExpiryBar extends StatelessWidget {
  const _ExpiryBar({required this.expiresAt});
  final DateTime expiresAt;

  @override
  Widget build(BuildContext context) {
    final remaining = expiresAt.difference(DateTime.now().toUtc());
    final hours = remaining.inHours.clamp(0, 999);
    final minutes = (remaining.inMinutes % 60).clamp(0, 59);
    return Container(
      margin: const EdgeInsets.fromLTRB(14, 8, 14, 0),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(
          color: PatientTokens.infoLight,
          borderRadius: BorderRadius.circular(10)),
      child: Wrap(
          spacing: 12,
          runSpacing: 6,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(
                AppLocalizations.of(context)
                    .chatActiveRemaining(hours, minutes),
                style:
                    const TextStyle(fontSize: 11, color: PatientTokens.info)),
            StatusChip(AppLocalizations.of(context).chat24hSession,
                color: PatientTokens.infoLight, textColor: PatientTokens.info),
          ]),
    );
  }
}

class _ChatBubble extends StatelessWidget {
  const _ChatBubble(
      {required this.message, required this.fromMe, this.onViewAttachment});
  final ChatMessageDto message;
  final bool fromMe;
  final VoidCallback? onViewAttachment;

  @override
  Widget build(BuildContext context) {
    final time =
        '${message.sentAt.hour.toString().padLeft(2, '0')}:${message.sentAt.minute.toString().padLeft(2, '0')}';
    return Align(
      alignment: fromMe ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.only(bottom: 10),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 9),
        constraints: const BoxConstraints(maxWidth: 280),
        decoration: BoxDecoration(
          color: fromMe ? PatientTokens.primary : PatientTokens.surface,
          borderRadius: BorderRadius.circular(14),
          border: fromMe ? null : Border.all(color: PatientTokens.border),
        ),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          if (message.content.isNotEmpty)
            Text(message.content,
                style: TextStyle(
                    fontSize: 13,
                    color: fromMe ? Colors.white : PatientTokens.text)),
          if (message.attachmentId != null) ...[
            Text('Clinical image',
                style: TextStyle(
                    color: fromMe ? Colors.white : PatientTokens.text,
                    fontWeight: FontWeight.w600)),
            if (message.attachmentSize != null)
              Text('${(message.attachmentSize! / 1024).toStringAsFixed(1)} KiB',
                  style: TextStyle(
                      color: fromMe ? Colors.white : PatientTokens.text)),
            TextButton(
                onPressed: onViewAttachment,
                style: TextButton.styleFrom(
                    foregroundColor:
                        fromMe ? Colors.white : PatientTokens.primary),
                child: const Text('View image')),
          ],
          const SizedBox(height: 3),
          Text(time,
              style: TextStyle(
                  fontSize: 9.5,
                  color: fromMe ? Colors.white70 : PatientTokens.textSub)),
        ]),
      ),
    );
  }
}
