import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';
import 'package:intl/intl.dart';

import '../../../core/l10n/kenya_time.dart';
import '../../../core/network/api_client.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/utils/initials.dart';
import '../../../core/utils/time_format.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../core/widgets/person_photo.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_repository.dart';
import '../booking/pre_consult_intake_modal.dart' show deletePickedSourceFile;
import '../chat/clinical_image_viewer.dart';
import '../modules/afya_module_widgets.dart';
import '../modules/patient_modules.dart';
import '../video/agora_call_page.dart';
import 'peer_repository.dart';

/// Peer Navigator workflow, patient journey (board
/// `docs/design/peer-navigator/peer-workflow-patient-journey.jpeg`, panels
/// 2-3), in the Patient app's design language. Peer support is non-clinical:
/// no prescriptions, referrals or records.

const _pad = EdgeInsets.fromLTRB(20, 12, 20, 28);
const int peerTopicMaxLength = 200;

String _errorText(AppLocalizations l, Object e) =>
    e is ApiException ? e.userMessage : l.peerGenericError;

String peerStatusLabel(AppLocalizations l, String status) => switch (status) {
      'PENDING' => l.peerStatusPending,
      'ACCEPTED' => l.peerStatusAccepted,
      'DECLINED' => l.peerStatusDeclined,
      'CANCELLED' => l.peerStatusCancelled,
      'COMPLETED' => l.peerStatusCompleted,
      _ => status.toLowerCase(),
    };

String _supportLabel(AppLocalizations l, String type) =>
    type == 'CALL' ? l.peerSupportCall : l.peerSupportChat;

/// "Tomorrow, Thu 23 Jan" in Kenya time.
String _dayLabel(AppLocalizations l, DateTime instant) {
  final day = kenyaTime(instant);
  final today = kenyaTime(DateTime.now());
  final d0 = DateTime(today.year, today.month, today.day);
  final d1 = DateTime(day.year, day.month, day.day);
  final date = DateFormat('EEE d MMM', l.localeName).format(day);
  return switch (d1.difference(d0).inDays) {
    0 => '${l.peerToday}, $date',
    1 => '${l.peerTomorrow}, $date',
    _ => date,
  };
}

String _slotTime(DateTime t) => formatTime12h(kenyaTime(t));

class _TrainedBadge extends StatelessWidget {
  const _TrainedBadge();
  @override
  Widget build(BuildContext context) =>
      Row(mainAxisSize: MainAxisSize.min, children: [
        const Icon(Icons.verified_rounded,
            size: 15, color: PatientTokens.primaryDark),
        const SizedBox(width: 4),
        Flexible(
          child: Text(AppLocalizations.of(context).peerTrained,
              style: const TextStyle(fontSize: 12, color: PatientTokens.ink)),
        ),
      ]);
}

/// Peer card used on the list, Book and Request Sent screens.
class _PeerCard extends StatelessWidget {
  const _PeerCard(
      {super.key,
      required this.name,
      this.subtitle,
      this.onTap,
      this.trailing});
  final String name;
  final String? subtitle;
  final VoidCallback? onTap;
  final Widget? trailing;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AfyaCard(
      onTap: onTap,
      child: Row(children: [
        AfyaInitialsAvatar(name: name, size: 52),
        const SizedBox(width: 12),
        Expanded(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(name,
                style: const TextStyle(
                    fontSize: 15.5,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.navy)),
            const SizedBox(height: 2),
            Text(l.peerRole,
                style:
                    const TextStyle(fontSize: 12.5, color: PatientTokens.ink)),
            const SizedBox(height: 4),
            const _TrainedBadge(),
            if (subtitle != null) ...[
              const SizedBox(height: 4),
              Text(subtitle!,
                  style: const TextStyle(
                      fontSize: 12, color: PatientTokens.textSub)),
            ],
          ]),
        ),
        if (trailing != null) trailing!,
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// Choose a Peer Navigator
// ---------------------------------------------------------------------------

class PeerNavigatorListPage extends StatefulWidget {
  const PeerNavigatorListPage(
      {super.key,
      required this.repo,
      required this.nav,
      required this.bookingRepo});
  final PatientPeerRepository repo;
  final BookingRepository bookingRepo;
  final PatientShellNav nav;
  @override
  State<PeerNavigatorListPage> createState() => _PeerNavigatorListPageState();
}

class _PeerNavigatorListPageState extends State<PeerNavigatorListPage> {
  List<PeerNavigatorDto>? peers;
  Object? error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => error = null);
    try {
      final list = await widget.repo.peers();
      if (mounted) setState(() => peers = list);
    } catch (e) {
      if (mounted) setState(() => error = e);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AfyaModuleScaffold(
      title: l.peerChooseTitle,
      nav: widget.nav,
      child: RefreshIndicator(
        onRefresh: _load,
        child: ListView(padding: _pad, children: [
          if (error != null)
            InlineError(l.peerLoadError, onRetry: _load)
          else if (peers == null)
            const Column(children: [
              SkeletonBlock(height: 96),
              SizedBox(height: 12),
              SkeletonBlock(height: 96),
            ])
          else if (peers!.isEmpty)
            AfyaUnavailableNote(
                icon: Icons.people_alt_rounded,
                title: l.peerNoneTitle,
                body: l.peerNoneBody)
          else
            for (final p in peers!)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: _PeerCard(
                  key: ValueKey('peer-${p.id}'),
                  name: p.fullName,
                  subtitle: p.nextAvailable == null
                      ? l.peerNoSlotsYet
                      : l.peerNextAvailable(
                          formatEatDateTime(p.nextAvailable!)),
                  trailing: const Icon(Icons.chevron_right_rounded,
                      color: PatientTokens.navy),
                  onTap: () => Navigator.of(context).push(MaterialPageRoute(
                      builder: (_) => BookPeerSupportPage(
                          repo: widget.repo,
                          bookingRepo: widget.bookingRepo,
                          nav: widget.nav,
                          peer: p))),
                ),
              ),
        ]),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Book Peer Support (board panel 2)
// ---------------------------------------------------------------------------

class BookPeerSupportPage extends StatefulWidget {
  const BookPeerSupportPage(
      {super.key,
      required this.repo,
      required this.bookingRepo,
      required this.nav,
      required this.peer});
  final PatientPeerRepository repo;
  final BookingRepository bookingRepo;
  final PatientShellNav nav;
  final PeerNavigatorDto peer;
  @override
  State<BookPeerSupportPage> createState() => _BookPeerSupportPageState();
}

class _BookPeerSupportPageState extends State<BookPeerSupportPage> {
  final topic = TextEditingController();
  List<PeerOpenSlotDto>? slots;
  Object? loadError;
  String supportType = 'CHAT';
  String? slotId;
  String? error;
  bool busy = false;

  @override
  void initState() {
    super.initState();
    topic.addListener(() => setState(() {}));
    _load();
  }

  @override
  void dispose() {
    topic.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() => loadError = null);
    try {
      final list = await widget.repo.openSlots(widget.peer.id);
      list.sort((a, b) => a.start.compareTo(b.start));
      if (mounted) {
        setState(() {
          slots = list;
          if (!list.any((s) => s.id == slotId)) slotId = null;
        });
      }
    } catch (e) {
      if (mounted) setState(() => loadError = e);
    }
  }

  Future<void> _submit() async {
    final l = AppLocalizations.of(context);
    final problem = slotId == null
        ? l.peerChooseSlot
        : topic.text.trim().length < 3
            ? l.peerTopicTooShort
            : null;
    setState(() => error = problem);
    if (problem != null || busy) return;
    setState(() => busy = true);
    try {
      final created = await widget.repo.request(
          slotId: slotId!, supportType: supportType, topic: topic.text);
      if (!mounted) return;
      await Navigator.of(context).pushReplacement(MaterialPageRoute(
          builder: (_) => PeerRequestSentPage(
              nav: widget.nav,
              request: created,
              peerName: widget.peer.fullName)));
    } catch (e) {
      if (!mounted) return;
      setState(() => error = _errorText(l, e));
      // A slot taken meanwhile disappears from the refreshed list.
      unawaited(_load());
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Widget _typeOption(String value, IconData icon, String label) {
    final selected = supportType == value;
    return Expanded(
      child: InkWell(
        key: ValueKey('support-$value'),
        borderRadius: BorderRadius.circular(12),
        onTap: () => setState(() => supportType = value),
        child: Container(
          height: 64,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          decoration: BoxDecoration(
              color: PatientTokens.surface,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                  color: selected
                      ? PatientTokens.primaryDark
                      : PatientTokens.border,
                  width: selected ? 1.6 : 1)),
          child: Row(children: [
            Icon(icon, color: PatientTokens.navy, size: 22),
            const SizedBox(width: 8),
            Expanded(
                child: Text(label,
                    style: const TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                        color: PatientTokens.navy))),
            if (selected)
              const Icon(Icons.check_circle_rounded,
                  size: 18, color: PatientTokens.primaryDark),
          ]),
        ),
      ),
    );
  }

  Widget _sectionTitle(IconData icon, String text) => Padding(
        padding: const EdgeInsets.only(top: 18, bottom: 10),
        child: Row(children: [
          Icon(icon, size: 19, color: PatientTokens.navy),
          const SizedBox(width: 8),
          Flexible(
            child: Text(text,
                style: const TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.navy)),
          ),
        ]),
      );

  List<Widget> _slotGroups(AppLocalizations l) {
    final groups = <String, List<PeerOpenSlotDto>>{};
    for (final s in slots!) {
      groups.putIfAbsent(_dayLabel(l, s.start), () => []).add(s);
    }
    return [
      for (final entry in groups.entries) ...[
        Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: Text(entry.key,
              style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: PatientTokens.navy)),
        ),
        GridView.count(
          crossAxisCount: 3,
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          mainAxisSpacing: 8,
          crossAxisSpacing: 8,
          childAspectRatio: 2.6,
          children: [
            for (final s in entry.value)
              InkWell(
                key: ValueKey('slot-${s.id}'),
                borderRadius: BorderRadius.circular(10),
                onTap: () => setState(() => slotId = s.id),
                child: Container(
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                      color: slotId == s.id
                          ? PatientTokens.primaryDark
                          : PatientTokens.surface,
                      borderRadius: BorderRadius.circular(10),
                      border: Border.all(
                          color: slotId == s.id
                              ? PatientTokens.primaryDark
                              : PatientTokens.border)),
                  child: Text(_slotTime(s.start),
                      style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: slotId == s.id
                              ? Colors.white
                              : PatientTokens.navy)),
                ),
              ),
          ],
        ),
        const SizedBox(height: 12),
      ]
    ];
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return AfyaModuleScaffold(
      title: l.peerBookTitle,
      nav: widget.nav,
      child: ListView(padding: _pad, children: [
        AfyaCard(
          color: PatientTokens.mintCard,
          shadow: false,
          child: Row(children: [
            const Icon(Icons.groups_rounded,
                size: 40, color: PatientTokens.primaryDark),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(l.peerBookBannerTitle,
                        style: const TextStyle(
                            fontSize: 14,
                            fontWeight: FontWeight.w800,
                            color: PatientTokens.navy)),
                    const SizedBox(height: 2),
                    Text(l.peerBookBannerBody,
                        style: const TextStyle(
                            fontSize: 12, color: PatientTokens.ink)),
                  ]),
            ),
          ]),
        ),
        const SizedBox(height: 12),
        _PeerCard(name: widget.peer.fullName),
        _sectionTitle(Icons.support_agent_rounded, l.peerSupportType),
        Row(children: [
          _typeOption(
              'CHAT', Icons.chat_bubble_outline_rounded, l.peerSupportChat),
          const SizedBox(width: 10),
          _typeOption('CALL', Icons.videocam_outlined, l.peerSupportCall),
        ]),
        _sectionTitle(Icons.calendar_month_outlined, l.peerAvailableSlots),
        if (loadError != null)
          InlineError(_errorText(l, loadError!), onRetry: _load)
        else if (slots == null)
          const SkeletonBlock(height: 90)
        else if (slots!.isEmpty)
          Text(l.peerNoOpenSlots,
              style:
                  const TextStyle(fontSize: 13, color: PatientTokens.textSub))
        else
          ..._slotGroups(l),
        _sectionTitle(Icons.chat_outlined, l.peerTopicLabel),
        TextField(
          key: const ValueKey('peer-topic'),
          controller: topic,
          minLines: 3,
          maxLines: 4,
          maxLength: peerTopicMaxLength,
          decoration: InputDecoration(
            hintText: l.peerTopicHint,
            filled: true,
            fillColor: PatientTokens.surface,
            border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(12),
                borderSide: const BorderSide(color: PatientTokens.border)),
            enabledBorder: OutlineInputBorder(
                borderRadius: BorderRadius.circular(12),
                borderSide: const BorderSide(color: PatientTokens.border)),
          ),
        ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Text(error!,
                key: const ValueKey('peer-book-error'),
                style:
                    const TextStyle(color: PatientTokens.danger, fontSize: 13)),
          ),
        const SizedBox(height: 4),
        SizedBox(
          height: 50,
          child: FilledButton(
            key: const ValueKey('peer-request-support'),
            style: FilledButton.styleFrom(
                backgroundColor: PatientTokens.primaryDark,
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(25))),
            onPressed: busy ? null : _submit,
            child: busy
                ? const SizedBox(
                    width: 22,
                    height: 22,
                    child: CircularProgressIndicator(
                        strokeWidth: 2.4, color: Colors.white))
                : Text(l.peerRequestSupport,
                    style: const TextStyle(
                        fontSize: 15, fontWeight: FontWeight.w700)),
          ),
        ),
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// Request Sent (board panel 3)
// ---------------------------------------------------------------------------

class PeerRequestSentPage extends StatelessWidget {
  const PeerRequestSentPage(
      {super.key,
      required this.nav,
      required this.request,
      required this.peerName});
  final PatientShellNav nav;
  final PatientPeerRequestDto request;
  final String peerName;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    Widget row(IconData icon, String label, Widget value) => Padding(
          padding: const EdgeInsets.symmetric(vertical: 7),
          child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Icon(icon, size: 18, color: PatientTokens.navy),
            const SizedBox(width: 8),
            SizedBox(
                width: 112,
                child: Text(label,
                    style: const TextStyle(
                        fontSize: 12.5,
                        fontWeight: FontWeight.w600,
                        color: PatientTokens.navy))),
            Expanded(child: value),
          ]),
        );
    const valueStyle = TextStyle(fontSize: 12.5, color: PatientTokens.ink);
    return AfyaModuleScaffold(
      title: l.peerSentTitle,
      nav: nav,
      child: ListView(padding: _pad, children: [
        const SizedBox(height: 8),
        Center(
          child: Container(
            width: 120,
            height: 120,
            decoration: const BoxDecoration(
                color: PatientTokens.mintCard, shape: BoxShape.circle),
            child: const Icon(Icons.event_available_rounded,
                size: 64, color: PatientTokens.primaryDark),
          ),
        ),
        const SizedBox(height: 14),
        Text(l.peerSentHeadline,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 20,
                fontWeight: FontWeight.w800,
                color: PatientTokens.navy)),
        const SizedBox(height: 8),
        Text(l.peerSentBody,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 13.5, height: 1.45, color: PatientTokens.ink)),
        const SizedBox(height: 16),
        AfyaCard(
          color: PatientTokens.amberCard,
          shadow: false,
          child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            const Icon(Icons.schedule_rounded,
                size: 28, color: PatientTokens.tileAmber),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(l.peerWaitingTitle,
                        style: const TextStyle(
                            fontSize: 14.5,
                            fontWeight: FontWeight.w800,
                            color: PatientTokens.navy)),
                    const SizedBox(height: 2),
                    Text(l.peerWaitingBody(peerName),
                        style: const TextStyle(
                            fontSize: 12.5, color: PatientTokens.ink)),
                  ]),
            ),
          ]),
        ),
        const SizedBox(height: 16),
        Row(children: [
          const Icon(Icons.calendar_month_outlined,
              size: 19, color: PatientTokens.navy),
          const SizedBox(width: 8),
          Text(l.peerDetails,
              style: const TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w800,
                  color: PatientTokens.navy)),
        ]),
        const SizedBox(height: 8),
        AfyaCard(
          child: Column(children: [
            row(
                Icons.person_outline_rounded,
                l.peerRole,
                Row(children: [
                  AfyaInitialsAvatar(name: peerName, size: 30),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(peerName,
                              style: const TextStyle(
                                  fontSize: 13,
                                  fontWeight: FontWeight.w700,
                                  color: PatientTokens.navy)),
                          const _TrainedBadge(),
                        ]),
                  ),
                ])),
            row(Icons.event_outlined, l.peerRequestedSlot,
                Text(formatEatDateTime(request.start), style: valueStyle)),
            row(Icons.support_agent_rounded, l.peerSupportType,
                Text(_supportLabel(l, request.supportType), style: valueStyle)),
            row(
                Icons.notes_rounded,
                l.peerYourTopic,
                Text(request.topic,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: valueStyle)),
          ]),
        ),
        const SizedBox(height: 12),
        AfyaCard(
          color: PatientTokens.infoCard,
          shadow: false,
          child: Row(children: [
            const Icon(Icons.notifications_none_rounded,
                color: PatientTokens.navy),
            const SizedBox(width: 10),
            Expanded(
                child: Text(l.peerNotifyNote,
                    style: const TextStyle(
                        fontSize: 12.5, color: PatientTokens.ink))),
          ]),
        ),
        const SizedBox(height: 16),
        SizedBox(
          height: 48,
          child: OutlinedButton(
            key: const ValueKey('peer-back-to-services'),
            style: OutlinedButton.styleFrom(
                foregroundColor: PatientTokens.navy,
                side: const BorderSide(color: PatientTokens.primaryDark),
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(24))),
            onPressed: () {
              Navigator.of(context).popUntil((r) => r.isFirst);
              nav.openServices();
            },
            child: Text(l.peerBackToServices,
                style: const TextStyle(fontWeight: FontWeight.w700)),
          ),
        ),
      ]),
    );
  }
}

// ---------------------------------------------------------------------------
// The patient's own requests (on the Peer Navigator page)
// ---------------------------------------------------------------------------

class PatientPeerRequestsSection extends StatefulWidget {
  const PatientPeerRequestsSection(
      {super.key,
      required this.repo,
      required this.bookingRepo,
      this.myUserId,
      this.refreshToken = 0});
  final PatientPeerRepository repo;
  final BookingRepository bookingRepo;
  final String? myUserId;

  /// Bumped by the parent to reload (e.g. after a new request).
  final int refreshToken;
  @override
  State<PatientPeerRequestsSection> createState() =>
      _PatientPeerRequestsSectionState();
}

class _PatientPeerRequestsSectionState
    extends State<PatientPeerRequestsSection> {
  List<PatientPeerRequestDto>? requests;
  bool failed = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(covariant PatientPeerRequestsSection old) {
    super.didUpdateWidget(old);
    if (old.refreshToken != widget.refreshToken) _load();
  }

  Future<void> _load() async {
    try {
      final list = await widget.repo.myRequests();
      if (mounted) {
        setState(() {
          requests = list
              .where((r) =>
                  r.status == 'PENDING' ||
                  r.status == 'ACCEPTED' ||
                  r.status == 'COMPLETED' ||
                  r.status == 'DECLINED')
              .take(10)
              .toList();
          failed = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => failed = true);
    }
  }

  Future<void> _cancel(PatientPeerRequestDto r) async {
    final l = AppLocalizations.of(context);
    final ok = await showDialog<bool>(
        context: context,
        builder: (c) => AlertDialog(
              title: Text(l.peerCancelConfirmTitle),
              content: Text(l.peerCancelConfirmBody),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(c, false),
                    child: Text(l.peerKeepRequest)),
                TextButton(
                    onPressed: () => Navigator.pop(c, true),
                    child: Text(l.peerCancelRequest)),
              ],
            ));
    if (ok != true || !mounted) return;
    try {
      await widget.repo.cancel(r.id);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(_errorText(l, e))));
      }
    }
    unawaited(_load());
  }

  void _openChat(PatientPeerRequestDto r) =>
      Navigator.of(context).push(MaterialPageRoute(
          builder: (_) => PatientPeerChatPage(
              repo: widget.repo,
              bookingRepo: widget.bookingRepo,
              chatId: r.conversationId!,
              requestId: r.id,
              peerName: r.peerName,
              requestStatus: r.status,
              callClosed: r.callClosed,
              myUserId: widget.myUserId)));

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final list = requests;
    if (failed || list == null || list.isEmpty) return const SizedBox.shrink();
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      const SizedBox(height: 16),
      Text(l.peerMyRequests,
          style: const TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w800,
              color: PatientTokens.navy)),
      const SizedBox(height: 10),
      for (final r in list)
        Padding(
          padding: const EdgeInsets.only(bottom: 10),
          child: AfyaCard(
            key: ValueKey('peer-request-${r.id}'),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Row(children: [
                AfyaInitialsAvatar(name: r.peerName, size: 40),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(r.peerName,
                            style: const TextStyle(
                                fontSize: 14,
                                fontWeight: FontWeight.w700,
                                color: PatientTokens.navy)),
                        Text(
                            '${_supportLabel(l, r.supportType)} · ${formatEatDateTime(r.start)}',
                            style: const TextStyle(
                                fontSize: 12, color: PatientTokens.ink)),
                      ]),
                ),
                StatusChip(peerStatusLabel(l, r.status)),
              ]),
              if (r.status == 'DECLINED' &&
                  (r.declineNote ?? '').trim().isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(r.declineNote!,
                      style: const TextStyle(
                          fontSize: 12.5, color: PatientTokens.textSub)),
                ),
              if (r.isPending || r.conversationId != null)
                Align(
                  alignment: Alignment.centerRight,
                  child: r.isPending
                      ? TextButton(
                          key: ValueKey('peer-cancel-${r.id}'),
                          onPressed: () => _cancel(r),
                          child: Text(l.peerCancelRequest))
                      : TextButton.icon(
                          key: ValueKey('peer-open-chat-${r.id}'),
                          onPressed: () => _openChat(r),
                          icon: const Icon(Icons.chat_bubble_outline_rounded,
                              size: 18),
                          label: Text(l.peerOpenChat)),
                ),
            ]),
          ),
        ),
    ]);
  }
}

// ---------------------------------------------------------------------------
// Peer chat (patient side), reference
// `docs/design/peer-navigator/patient-peer-chat.png`. Never closes; "Mark
// complete" is the peer's. Photos travel like doctor-chat images: sent as
// JPEG/PNG, stored encrypted by the server, viewed from memory only.
// ---------------------------------------------------------------------------

const _peerAvatarGreen = Color(0xFF0E5C46);
const _dateChipBg = Color(0xFFEEF1F3);
const _bubbleMine = Color(0xFFE6F6EE);
const _bannerIconBg = Color(0xFFD5EFE2);

String _chatDayLabel(AppLocalizations l, DateTime instant) {
  final day = kenyaTime(instant);
  final today = kenyaTime(DateTime.now());
  final diff = DateTime(today.year, today.month, today.day)
      .difference(DateTime(day.year, day.month, day.day))
      .inDays;
  return switch (diff) {
    0 => l.peerToday,
    1 => l.peerYesterday,
    _ => DateFormat('EEE d MMM', l.localeName).format(day),
  };
}

String _initial(String name) {
  final words = initialWords(name);
  return words.isEmpty ? '?' : words.first[0].toUpperCase();
}

/// The peer's initial, or their saved photo when [api] and [photoPath] are
/// given and the peer has one.
class _PeerLetterAvatar extends StatelessWidget {
  const _PeerLetterAvatar(
      {required this.name, required this.size, this.api, this.photoPath});
  final String name;
  final double size;
  final ApiClient? api;
  final String? photoPath;
  @override
  Widget build(BuildContext context) {
    final client = api;
    if (client == null || photoPath == null) return _letter();
    return PersonPhoto(
        api: client, path: photoPath, size: size, fallback: _letter());
  }

  Widget _letter() => Container(
        width: size,
        height: size,
        alignment: Alignment.center,
        decoration: const BoxDecoration(
            color: _peerAvatarGreen, shape: BoxShape.circle),
        child: Text(_initial(name),
            style: TextStyle(
                color: Colors.white,
                fontSize: size * .42,
                fontWeight: FontWeight.w600)),
      );
}

class _CircleOutlineButton extends StatelessWidget {
  const _CircleOutlineButton(
      {super.key,
      required this.icon,
      required this.tooltip,
      required this.onPressed,
      this.color = PatientTokens.navy});
  final IconData icon;
  final String tooltip;
  final VoidCallback onPressed;
  final Color color;
  @override
  Widget build(BuildContext context) => Tooltip(
        message: tooltip,
        child: Material(
          color: Colors.white,
          shape: const CircleBorder(
              side: BorderSide(color: Color(0xFFDCE3E8), width: 1.2)),
          child: InkWell(
            customBorder: const CircleBorder(),
            onTap: onPressed,
            child: SizedBox(
                width: 44,
                height: 44,
                child: Icon(icon, color: color, size: 22)),
          ),
        ),
      );
}

class PatientPeerChatPage extends StatefulWidget {
  const PatientPeerChatPage(
      {super.key,
      required this.repo,
      required this.bookingRepo,
      required this.chatId,
      required this.requestId,
      required this.peerName,
      required this.requestStatus,
      this.callClosed = false,
      this.myUserId,
      this.pickImage});
  final PatientPeerRepository repo;
  final BookingRepository bookingRepo;
  final String chatId, requestId, peerName, requestStatus;

  /// The session's call can no longer be started: no call button.
  final bool callClosed;
  final String? myUserId;

  /// Injectable picker for tests: returns (bytes, mime) or null.
  final Future<(Uint8List, String)?> Function(ImageSource source)? pickImage;
  @override
  State<PatientPeerChatPage> createState() => _PatientPeerChatPageState();
}

class _PatientPeerChatPageState extends State<PatientPeerChatPage> {
  final _text = TextEditingController();
  final _scroll = ScrollController();
  List<PeerChatMessageDto>? _messages;
  Object? _error;
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
    _scroll.dispose();
    super.dispose();
  }

  void _toBottom() => WidgetsBinding.instance.addPostFrameCallback((_) {
        if (_scroll.hasClients) {
          _scroll.jumpTo(_scroll.position.maxScrollExtent);
        }
      });

  Future<void> _load({bool quiet = false}) async {
    try {
      final list = await widget.repo.messages(widget.chatId);
      if (!mounted) return;
      final grew = list.length != (_messages?.length ?? -1);
      setState(() {
        _messages = list;
        _error = null;
      });
      if (grew) {
        _toBottom();
        try {
          await widget.repo.markRead(widget.chatId);
        } catch (_) {}
      }
    } catch (e) {
      if (mounted && !quiet) setState(() => _error = e);
    }
  }

  void _snack(String message) => ScaffoldMessenger.of(context)
      .showSnackBar(SnackBar(content: Text(message)));

  Future<void> _send() async {
    final l = AppLocalizations.of(context);
    final content = _text.text.trim();
    if (content.isEmpty || _sending) return;
    setState(() => _sending = true);
    try {
      final m = await widget.repo.send(widget.chatId, content);
      if (!mounted) return;
      _text.clear();
      setState(() => _messages = [...?_messages, m]);
      _toBottom();
    } catch (e) {
      if (mounted) _snack(e is ApiException ? e.userMessage : l.peerSendError);
    } finally {
      if (mounted) setState(() => _sending = false);
    }
  }

  /// Camera or gallery; the picker's own copy is deleted once read.
  Future<(Uint8List, String)?> _pick(ImageSource source) async {
    final l = AppLocalizations.of(context);
    final file = await ImagePicker().pickImage(
        source: source, imageQuality: 85, maxWidth: 2000, maxHeight: 2000);
    if (file == null) return null;
    final name = file.name.toLowerCase();
    final mime = name.endsWith('.png')
        ? 'image/png'
        : name.endsWith('.jpg') || name.endsWith('.jpeg')
            ? 'image/jpeg'
            : null;
    if (mime == null) {
      unawaited(deletePickedSourceFile(file.path));
      throw ApiException(422, l.chatAttachInvalid);
    }
    if (await file.length() > 10 * 1024 * 1024) {
      unawaited(deletePickedSourceFile(file.path));
      throw ApiException(413, l.chatAttachTooLarge);
    }
    final bytes = await file.readAsBytes();
    unawaited(deletePickedSourceFile(file.path));
    return (bytes, mime);
  }

  Future<void> _attach() async {
    final l = AppLocalizations.of(context);
    if (_sending) return;
    final source = await showModalBottomSheet<ImageSource>(
        context: context,
        builder: (sheet) => SafeArea(
            child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 18, 20, 12),
                child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(l.peerAttachTitle,
                          style: const TextStyle(
                              fontSize: 18, fontWeight: FontWeight.w700)),
                      const SizedBox(height: 6),
                      Text(l.peerAttachSubtitle,
                          style: const TextStyle(
                              fontSize: 13, color: PatientTokens.textSub)),
                      const SizedBox(height: 8),
                      ListTile(
                          key: const ValueKey('peer-attach-camera'),
                          contentPadding: EdgeInsets.zero,
                          leading: const Icon(Icons.photo_camera_outlined),
                          title: Text(l.chatAttachCamera),
                          onTap: () =>
                              Navigator.pop(sheet, ImageSource.camera)),
                      ListTile(
                          key: const ValueKey('peer-attach-gallery'),
                          contentPadding: EdgeInsets.zero,
                          leading: const Icon(Icons.photo_library_outlined),
                          title: Text(l.chatAttachGallery),
                          onTap: () =>
                              Navigator.pop(sheet, ImageSource.gallery)),
                    ]))));
    if (source == null || !mounted) return;
    setState(() => _sending = true);
    try {
      final picked = await (widget.pickImage ?? _pick)(source);
      if (picked == null || !mounted) return;
      final key = List.generate(
          16,
          (_) => Random.secure()
              .nextInt(256)
              .toRadixString(16)
              .padLeft(2, '0')).join();
      final m =
          await widget.repo.sendImage(widget.chatId, picked.$1, picked.$2, key);
      if (!mounted) return;
      setState(() => _messages = [...?_messages, m]);
      _toBottom();
    } catch (e) {
      if (mounted) {
        _snack(e is ApiException ? e.userMessage : l.peerSendPhotoError);
      }
    } finally {
      if (mounted) setState(() => _sending = false);
    }
  }

  void _viewPhoto(PeerChatMessageDto m) {
    final l = AppLocalizations.of(context);
    Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => ClinicalImageViewer(
            title: l.peerPhoto,
            load: () => widget.repo.image(widget.chatId, m.id))));
  }

  void _call() => Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => AgoraCallPage(
          appointmentId: widget.requestId,
          repo: widget.bookingRepo,
          loadToken: () => widget.repo.rtcToken(widget.requestId),
          loadOpenAt: () => widget.repo.callOpensAt(widget.requestId),
          // The same chat over the call, without its own call button.
          loadChat: () async => PatientPeerChatPage(
              repo: widget.repo,
              bookingRepo: widget.bookingRepo,
              chatId: widget.chatId,
              requestId: widget.requestId,
              peerName: widget.peerName,
              requestStatus: widget.requestStatus,
              callClosed: true,
              myUserId: widget.myUserId))));

  void _details() {
    final l = AppLocalizations.of(context);
    final load = widget.repo.myRequests();
    showModalBottomSheet<void>(
        context: context,
        builder: (sheet) => SafeArea(
            child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 18, 20, 20),
                child: FutureBuilder<List<PatientPeerRequestDto>>(
                    future: load,
                    builder: (_, snap) {
                      PatientPeerRequestDto? r;
                      for (final x in snap.data ?? const []) {
                        if (x.id == widget.requestId) r = x;
                      }
                      Widget row(String label, String value) => Padding(
                            padding: const EdgeInsets.only(top: 12),
                            child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(label,
                                      style: const TextStyle(
                                          fontSize: 12.5,
                                          color: PatientTokens.textSub)),
                                  const SizedBox(height: 2),
                                  Text(value,
                                      style: const TextStyle(
                                          fontSize: 15,
                                          color: PatientTokens.navy,
                                          fontWeight: FontWeight.w600)),
                                ]),
                          );
                      return Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(l.peerSessionDetails,
                                style: const TextStyle(
                                    fontSize: 18, fontWeight: FontWeight.w700)),
                            if (snap.hasError)
                              Padding(
                                  padding: const EdgeInsets.only(top: 12),
                                  child: Text(l.peerGenericError))
                            else if (r == null)
                              const Padding(
                                  padding: EdgeInsets.all(20),
                                  child: Center(
                                      child: CircularProgressIndicator()))
                            else ...[
                              row(l.peerSessionWhen,
                                  '${_dayLabel(l, r.start)} · ${_slotTime(r.start)} – ${_slotTime(r.end)}'),
                              row(l.peerSupportType,
                                  _supportLabel(l, r.supportType)),
                              row(l.peerTopicLabel, r.topic),
                              const SizedBox(height: 14),
                              _StatusPill(peerStatusLabel(l, r.status)),
                            ],
                          ]);
                    }))));
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final messages = _messages;
    return Scaffold(
      backgroundColor: Colors.white,
      body: SafeArea(
        child: Column(children: [
          _header(l),
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 6, 14, 4),
            child: Container(
              key: const ValueKey('peer-chat-banner'),
              padding: const EdgeInsets.fromLTRB(12, 12, 14, 12),
              decoration: BoxDecoration(
                  color: PatientTokens.mintCard,
                  borderRadius: BorderRadius.circular(14)),
              child: Row(children: [
                Container(
                    width: 46,
                    height: 46,
                    decoration: const BoxDecoration(
                        color: _bannerIconBg, shape: BoxShape.circle),
                    child: const Icon(Icons.chat_outlined,
                        color: PatientTokens.primaryDark, size: 24)),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(l.peerChatActiveTitle,
                            style: const TextStyle(
                                fontSize: 14.5,
                                fontWeight: FontWeight.w800,
                                color: PatientTokens.navy)),
                        const SizedBox(height: 2),
                        Text(l.peerChatActiveBody(widget.peerName),
                            style: const TextStyle(
                                fontSize: 12.5,
                                height: 1.35,
                                color: PatientTokens.ink)),
                      ]),
                ),
              ]),
            ),
          ),
          Expanded(
            child: _error != null && messages == null
                ? Padding(
                    padding: const EdgeInsets.all(16),
                    child: InlineError(l.peerChatLoadError, onRetry: _load))
                : messages == null
                    ? const Center(child: CircularProgressIndicator())
                    : messages.isEmpty
                        ? Center(
                            child: Text(l.peerChatEmpty,
                                style: const TextStyle(
                                    color: PatientTokens.textSub)))
                        : ListView.builder(
                            controller: _scroll,
                            padding: const EdgeInsets.fromLTRB(14, 6, 14, 8),
                            itemCount: messages.length,
                            itemBuilder: (_, i) {
                              final m = messages[i];
                              final day = _chatDayLabel(l, m.sentAt);
                              final newDay = i == 0 ||
                                  _chatDayLabel(l, messages[i - 1].sentAt) !=
                                      day;
                              return Column(children: [
                                if (newDay) _DateChip(day),
                                _PeerBubble(
                                    message: m,
                                    mine: m.senderId == widget.myUserId,
                                    peerName: widget.peerName,
                                    api: widget.repo.api,
                                    photoPath: peerRequestCounterpartPhotoPath(
                                        widget.requestId),
                                    onView: m.hasImage
                                        ? () => _viewPhoto(m)
                                        : null),
                              ]);
                            }),
          ),
          _composer(l),
        ]),
      ),
    );
  }

  Widget _header(AppLocalizations l) => Padding(
        padding: const EdgeInsets.fromLTRB(10, 8, 2, 8),
        child: Row(children: [
          _CircleOutlineButton(
              key: const ValueKey('peer-chat-back'),
              icon: Icons.arrow_back_rounded,
              tooltip: MaterialLocalizations.of(context).backButtonTooltip,
              onPressed: () => Navigator.of(context).maybePop()),
          const SizedBox(width: 8),
          _PeerLetterAvatar(
              name: widget.peerName,
              size: 42,
              api: widget.repo.api,
              photoPath: peerRequestCounterpartPhotoPath(widget.requestId)),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text.rich(
                      TextSpan(children: [
                        TextSpan(
                            text: widget.peerName,
                            style:
                                const TextStyle(fontWeight: FontWeight.w800)),
                        TextSpan(
                            // " (Peer Navigator)" in the active language.
                            text: l.peerNameWithRole('')),
                      ]),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          fontSize: 13.5,
                          height: 1.2,
                          color: PatientTokens.navy)),
                  const SizedBox(height: 3),
                  Row(children: [
                    const Icon(Icons.check_circle_rounded,
                        size: 14, color: PatientTokens.primary),
                    const SizedBox(width: 3),
                    Flexible(
                      child: Text(l.peerVerified,
                          key: const ValueKey('peer-chat-verified'),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 11.5, color: PatientTokens.ink)),
                    ),
                  ]),
                ]),
          ),
          const SizedBox(width: 4),
          _StatusPill(peerStatusLabel(l, widget.requestStatus)),
          const SizedBox(width: 6),
          if (!widget.callClosed)
            _CircleOutlineButton(
                key: const ValueKey('peer-start-call'),
                icon: Icons.call_outlined,
                color: PatientTokens.primaryDark,
                tooltip: l.peerStartCall,
                onPressed: _call),
          Container(
              width: 1,
              height: 26,
              margin: const EdgeInsets.only(left: 6),
              color: const Color(0xFFDCE3E8)),
          PopupMenuButton<String>(
            key: const ValueKey('peer-chat-more'),
            tooltip: l.peerMoreOptions,
            padding: EdgeInsets.zero,
            child: const SizedBox(
                width: 30,
                height: 44,
                child:
                    Icon(Icons.more_vert_rounded, color: PatientTokens.navy)),
            onSelected: (v) => v == 'details' ? _details() : _load(),
            itemBuilder: (_) => [
              PopupMenuItem(
                  value: 'details', child: Text(l.peerSessionDetails)),
              PopupMenuItem(value: 'refresh', child: Text(l.peerRefresh)),
            ],
          ),
        ]),
      );

  Widget _composer(AppLocalizations l) => Padding(
        padding: const EdgeInsets.fromLTRB(14, 6, 14, 12),
        child: Container(
          padding: const EdgeInsets.fromLTRB(6, 5, 5, 5),
          decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(32),
            border: Border.all(color: const Color(0xFFDCE3E8), width: 1.2),
          ),
          child: Row(children: [
            IconButton(
                key: const ValueKey('peer-attach'),
                tooltip: l.peerAttachTooltip,
                onPressed: _sending ? null : _attach,
                icon: Transform.rotate(
                    angle: .7,
                    child: const Icon(Icons.attach_file_rounded,
                        color: PatientTokens.navy, size: 26))),
            Container(width: 1, height: 26, color: const Color(0xFFDCE3E8)),
            const SizedBox(width: 12),
            Expanded(
              child: TextField(
                key: const ValueKey('peer-message-input'),
                controller: _text,
                minLines: 1,
                maxLines: 4,
                maxLength: 2000,
                textCapitalization: TextCapitalization.sentences,
                buildCounter: (_,
                        {required currentLength,
                        required isFocused,
                        maxLength}) =>
                    null,
                style:
                    const TextStyle(fontSize: 15.5, color: PatientTokens.text),
                decoration: InputDecoration(
                  hintText: l.peerTypeMessage,
                  hintStyle: const TextStyle(
                      fontSize: 15.5, color: PatientTokens.textSub),
                  border: InputBorder.none,
                  enabledBorder: InputBorder.none,
                  focusedBorder: InputBorder.none,
                  filled: false,
                  isCollapsed: true,
                ),
              ),
            ),
            const SizedBox(width: 8),
            Tooltip(
              message: l.peerSendTooltip,
              child: Material(
                color: _sending
                    ? PatientTokens.primary.withValues(alpha: .5)
                    : PatientTokens.primaryDark,
                shape: const CircleBorder(),
                child: InkWell(
                  key: const ValueKey('peer-send'),
                  customBorder: const CircleBorder(),
                  onTap: _sending ? null : _send,
                  child: const SizedBox(
                      width: 48,
                      height: 48,
                      child: Icon(Icons.send_rounded,
                          color: Colors.white, size: 24)),
                ),
              ),
            ),
          ]),
        ),
      );
}

class _StatusPill extends StatelessWidget {
  const _StatusPill(this.label);
  final String label;
  @override
  Widget build(BuildContext context) => Container(
        key: const ValueKey('peer-chat-status'),
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        decoration: BoxDecoration(
            color: PatientTokens.mintCard,
            borderRadius: BorderRadius.circular(18)),
        child: Text(label,
            style: const TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w500,
                color: PatientTokens.primaryDark)),
      );
}

class _DateChip extends StatelessWidget {
  const _DateChip(this.label);
  final String label;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(top: 6, bottom: 14),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 5),
          decoration: BoxDecoration(
              color: _dateChipBg, borderRadius: BorderRadius.circular(16)),
          child: Text(label,
              style: const TextStyle(fontSize: 13, color: PatientTokens.ink)),
        ),
      );
}

class _PeerBubble extends StatelessWidget {
  const _PeerBubble(
      {required this.message,
      required this.mine,
      required this.peerName,
      required this.api,
      required this.photoPath,
      this.onView});
  final PeerChatMessageDto message;
  final bool mine;
  final String peerName;
  final ApiClient api;
  final String photoPath;
  final VoidCallback? onView;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final time = formatTime12h(kenyaTime(message.sentAt));
    final bubble = Container(
      key: ValueKey('peer-message-${message.id}'),
      constraints:
          BoxConstraints(maxWidth: MediaQuery.sizeOf(context).width * .7),
      padding: const EdgeInsets.fromLTRB(14, 11, 14, 9),
      decoration: BoxDecoration(
        color: mine ? _bubbleMine : Colors.white,
        border: mine ? null : Border.all(color: const Color(0xFFE1E7EB)),
        borderRadius: BorderRadius.circular(16),
      ),
      child: Column(
          crossAxisAlignment:
              mine ? CrossAxisAlignment.end : CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            if (message.content.isNotEmpty)
              Align(
                alignment: Alignment.centerLeft,
                widthFactor: 1,
                child: Text(message.content,
                    style: const TextStyle(
                        fontSize: 15, height: 1.38, color: PatientTokens.text)),
              ),
            if (message.hasImage)
              InkWell(
                key: ValueKey('peer-view-image-${message.id}'),
                onTap: onView,
                borderRadius: BorderRadius.circular(10),
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: Row(mainAxisSize: MainAxisSize.min, children: [
                    const Icon(Icons.image_outlined,
                        color: PatientTokens.primaryDark, size: 22),
                    const SizedBox(width: 8),
                    Text('${l.peerPhoto} · ${l.peerViewPhoto}',
                        style: const TextStyle(
                            fontSize: 14.5,
                            fontWeight: FontWeight.w600,
                            color: PatientTokens.primaryDark)),
                  ]),
                ),
              ),
            const SizedBox(height: 5),
            Row(mainAxisSize: MainAxisSize.min, children: [
              Text(time,
                  style: TextStyle(
                      fontSize: 12,
                      color: mine
                          ? PatientTokens.primaryDark
                          : PatientTokens.textSub)),
              if (mine) ...[
                const SizedBox(width: 6),
                Icon(
                    message.isRead
                        ? Icons.done_all_rounded
                        : Icons.done_rounded,
                    key: ValueKey(
                        'peer-ticks-${message.id}-${message.isRead ? 'read' : 'sent'}'),
                    size: 17,
                    semanticLabel:
                        message.isRead ? l.peerMessageRead : l.peerMessageSent,
                    color: message.isRead
                        ? PatientTokens.primary
                        : PatientTokens.textSub),
              ],
            ]),
          ]),
    );
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Row(
          mainAxisAlignment:
              mine ? MainAxisAlignment.end : MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (!mine) ...[
              Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: _PeerLetterAvatar(
                      name: peerName,
                      size: 30,
                      api: api,
                      photoPath: photoPath)),
              const SizedBox(width: 8),
            ],
            Flexible(child: bubble),
          ]),
    );
  }
}
