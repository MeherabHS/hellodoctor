import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/theme/tokens.dart';
import '../../core/widgets/afya_auth_widgets.dart';
import '../auth/auth_cubit.dart';
import '../notifications/notification_repository.dart';
import 'peer_chats.dart';
import 'peer_notifications.dart';
import 'peer_repository.dart';
import 'peer_requests.dart';
import 'peer_schedule.dart';
import 'peer_ui.dart';

export 'peer_chats.dart';
export 'peer_notifications.dart';
export 'peer_requests.dart';
export 'peer_schedule.dart';

/// Peer Navigator side of the Care app, built to the Care-app peer
/// references in `docs/design/peer-navigator/care-app/` (Home, Support
/// Requests, Request Details, Schedule & Availability, Chats, Chat, Profile,
/// Notifications) and backed by the Peer Navigator API. A peer sees a patient
/// only as "Amina M.", an age band and the topic they wrote.

// ---------------------------------------------------------------------------
// Entry: approval gate, then the shell
// ---------------------------------------------------------------------------

/// Signed-in Peer Navigator: shows the approval state until an Admin
/// approves the account, then the five tabs.
class PeerApp extends StatefulWidget {
  const PeerApp({super.key, required this.repo, this.notifications});
  final PeerRepository repo;

  /// The own-inbox notifications (defaults to the shared repository).
  final NotificationRepository? notifications;
  @override
  State<PeerApp> createState() => _PeerAppState();
}

class _PeerAppState extends State<PeerApp> {
  late Future<PeerProfile> _profile = widget.repo.profile();
  late final NotificationRepository _notifications =
      widget.notifications ?? NotificationRepository(widget.repo.api);
  void _reload() => setState(() => _profile = widget.repo.profile());

  @override
  Widget build(BuildContext context) => FutureBuilder<PeerProfile>(
      future: _profile,
      builder: (context, snap) {
        if (snap.connectionState != ConnectionState.done) {
          return const Scaffold(
              body: Center(child: CircularProgressIndicator()));
        }
        if (snap.hasError) {
          return _Message(
              icon: Icons.cloud_off_outlined,
              title: 'Unable to load your account',
              body: peerErrorText(snap.error!),
              action: 'Retry',
              onAction: _reload);
        }
        final profile = snap.data!;
        if (!profile.approved) {
          return _ApprovalGate(profile: profile, onRefresh: _reload);
        }
        return PeerShell(
            repo: widget.repo, profile: profile, notifications: _notifications);
      });
}

class _ApprovalGate extends StatelessWidget {
  const _ApprovalGate({required this.profile, required this.onRefresh});
  final PeerProfile profile;
  final VoidCallback onRefresh;
  @override
  Widget build(BuildContext context) {
    final (title, body) = switch (profile.approvalStatus) {
      'REJECTED' => (
          'Application not approved',
          profile.approvalNote ??
              'Your Peer Navigator application was not approved.'
        ),
      'SUSPENDED' => (
          'Account paused',
          profile.approvalNote ??
              'Your Peer Navigator account is paused. Contact AfyaMkononi support.'
        ),
      _ => (
          'Application under review',
          'Thank you for registering as a Peer Navigator. An AfyaMkononi Admin '
              'will review your application. You can start supporting people '
              'once it is approved.'
        ),
    };
    return _Message(
        icon: profile.approvalStatus == 'PENDING'
            ? Icons.hourglass_top_rounded
            : Icons.info_outline,
        title: title,
        body: body,
        action: 'Refresh status',
        onAction: onRefresh,
        secondary: 'Sign out',
        onSecondary: () => context.read<AuthCubit>().logout());
  }
}

class _Message extends StatelessWidget {
  const _Message(
      {required this.icon,
      required this.title,
      required this.body,
      required this.action,
      required this.onAction,
      this.secondary,
      this.onSecondary});
  final IconData icon;
  final String title, body, action;
  final VoidCallback onAction;
  final String? secondary;
  final VoidCallback? onSecondary;
  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.screen,
      body: SafeArea(
          child: Center(
              child: Padding(
                  padding: const EdgeInsets.all(28),
                  child: Column(mainAxisSize: MainAxisSize.min, children: [
                    const AfyaLogoMark(size: 58),
                    const SizedBox(height: 22),
                    Icon(icon, size: 42, color: PeerTokens.green),
                    const SizedBox(height: 12),
                    Text(title,
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                            fontSize: 21,
                            fontWeight: FontWeight.w800,
                            color: PeerTokens.navy)),
                    const SizedBox(height: 8),
                    Text(body,
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                            fontSize: 14.5,
                            height: 1.45,
                            color: PeerTokens.grey)),
                    const SizedBox(height: 22),
                    AfyaPrimaryButton(label: action, onPressed: onAction),
                    if (secondary != null)
                      TextButton(
                          onPressed: onSecondary, child: Text(secondary!)),
                  ])))));
}

// ---------------------------------------------------------------------------
// Shell
// ---------------------------------------------------------------------------

class PeerShell extends StatefulWidget {
  const PeerShell(
      {super.key,
      required this.repo,
      required this.profile,
      required this.notifications});
  final PeerRepository repo;
  final PeerProfile profile;
  final NotificationRepository notifications;
  @override
  State<PeerShell> createState() => _PeerShellState();
}

class _PeerShellState extends State<PeerShell> {
  int tab = 0;

  /// Bumped to make the Schedule tab open on "Create Slots" or "My Schedule".
  int scheduleMode = 0, scheduleVersion = 0;
  static const _items = <(IconData, IconData, String)>[
    (Icons.home_outlined, Icons.home_rounded, 'Home'),
    (Icons.mail_outline_rounded, Icons.mail_rounded, 'Requests'),
    (Icons.calendar_month_outlined, Icons.calendar_month_rounded, 'Schedule'),
    (Icons.chat_bubble_outline_rounded, Icons.chat_bubble_rounded, 'Chats'),
    (Icons.person_outline_rounded, Icons.person_rounded, 'Profile'),
  ];

  void go(int i) => setState(() => tab = i);
  void openSchedule(int mode) => setState(() {
        tab = 2;
        scheduleMode = mode;
        scheduleVersion++;
      });

  Future<void> openNotifications() =>
      Navigator.of(context).push(MaterialPageRoute(
          builder: (_) => PeerNotificationsPage(
              repo: widget.repo, notifications: widget.notifications)));

  @override
  Widget build(BuildContext context) {
    final pages = [
      PeerHomeTab(
          repo: widget.repo,
          profile: widget.profile,
          notifications: widget.notifications,
          onOpenTab: go,
          onOpenSchedule: openSchedule,
          onOpenNotifications: openNotifications),
      PeerRequestsTab(
          repo: widget.repo, onSetAvailability: () => openSchedule(0)),
      PeerScheduleTab(
          key: ValueKey('schedule-$scheduleVersion'),
          repo: widget.repo,
          initialMode: scheduleMode),
      PeerChatsTab(repo: widget.repo, onViewRequests: () => go(1)),
      PeerProfileTab(
          repo: widget.repo,
          profile: widget.profile,
          onManageSlots: () => openSchedule(1)),
    ];
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(bottom: false, child: pages[tab]),
        bottomNavigationBar: SafeArea(
            top: false,
            child: Container(
                height: 84,
                decoration: const BoxDecoration(
                    color: Colors.white,
                    border: Border(top: BorderSide(color: PeerTokens.line))),
                padding: const EdgeInsets.fromLTRB(8, 8, 8, 8),
                child: Row(children: [
                  for (var i = 0; i < _items.length; i++)
                    Expanded(
                        child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 3),
                            child: Material(
                                color: tab == i
                                    ? PeerTokens.mint
                                    : Colors.transparent,
                                borderRadius: BorderRadius.circular(20),
                                child: InkWell(
                                    key: ValueKey('peer-tab-${_items[i].$3}'),
                                    onTap: () => go(i),
                                    borderRadius: BorderRadius.circular(20),
                                    child: Column(
                                        mainAxisAlignment:
                                            MainAxisAlignment.center,
                                        children: [
                                          Icon(
                                              tab == i
                                                  ? _items[i].$2
                                                  : _items[i].$1,
                                              size: 27,
                                              color: tab == i
                                                  ? PeerTokens.green
                                                  : PeerTokens.navy),
                                          const SizedBox(height: 3),
                                          Text(_items[i].$3,
                                              maxLines: 1,
                                              style: TextStyle(
                                                  fontSize: 12.5,
                                                  fontWeight: tab == i
                                                      ? FontWeight.w800
                                                      : FontWeight.w500,
                                                  color: tab == i
                                                      ? PeerTokens.green
                                                      : PeerTokens.grey)),
                                        ])))))
                ]))));
  }
}

// ---------------------------------------------------------------------------
// Home (reference 01)
// ---------------------------------------------------------------------------

class PeerHomeTab extends StatefulWidget {
  const PeerHomeTab(
      {super.key,
      required this.repo,
      required this.profile,
      required this.notifications,
      required this.onOpenTab,
      required this.onOpenSchedule,
      required this.onOpenNotifications});
  final PeerRepository repo;
  final PeerProfile profile;
  final NotificationRepository notifications;
  final ValueChanged<int> onOpenTab;
  final ValueChanged<int> onOpenSchedule;
  final Future<void> Function() onOpenNotifications;
  @override
  State<PeerHomeTab> createState() => _PeerHomeTabState();
}

class _PeerHomeTabState extends State<PeerHomeTab> {
  late Future<(List<PeerInboxItem>, List<PeerChatSummary>)> _load = _fetch();
  int _unreadNotices = 0;

  @override
  void initState() {
    super.initState();
    _loadUnread();
  }

  Future<void> _loadUnread() async {
    try {
      final n = await widget.notifications.unreadCount();
      if (mounted) setState(() => _unreadNotices = n);
    } catch (_) {/* the bell simply shows no dot */}
  }

  Future<(List<PeerInboxItem>, List<PeerChatSummary>)> _fetch() async =>
      (await widget.repo.requests(), await widget.repo.chats());

  Future<void> _refresh() async {
    setState(() {
      _load = _fetch();
    });
    _loadUnread();
    await _load;
  }

  @override
  Widget build(BuildContext context) {
    final first = widget.profile.fullName.split(' ').first;
    return RefreshIndicator(
        onRefresh: _refresh,
        child: ListView(padding: const EdgeInsets.only(bottom: 24), children: [
          // Brand bar: logo, bell with unread dot, initials.
          Padding(
              padding: const EdgeInsets.fromLTRB(18, 12, 18, 10),
              child: Row(children: [
                const AfyaLogoMark(size: 44),
                const SizedBox(width: 10),
                const Expanded(
                    child: Text('AfyaMkononi',
                        style: TextStyle(
                            fontSize: 25,
                            fontWeight: FontWeight.w800,
                            color: PeerTokens.navy))),
                IconButton(
                    key: const ValueKey('peer-bell'),
                    tooltip: 'Notifications',
                    onPressed: () async {
                      await widget.onOpenNotifications();
                      _loadUnread();
                    },
                    icon: Stack(clipBehavior: Clip.none, children: [
                      const Icon(Icons.notifications_none_rounded,
                          size: 30, color: PeerTokens.navy),
                      if (_unreadNotices > 0)
                        Positioned(
                            right: 0,
                            top: 0,
                            child: Container(
                                key: const ValueKey('peer-bell-dot'),
                                width: 10,
                                height: 10,
                                decoration: BoxDecoration(
                                    color: const Color(0xFF16A34A),
                                    shape: BoxShape.circle,
                                    border: Border.all(
                                        color: Colors.white, width: 1.5)))),
                    ])),
                const SizedBox(width: 6),
                CircleAvatar(
                    radius: 23,
                    backgroundColor: PeerTokens.green,
                    child: Text(peerInitials(widget.profile.fullName),
                        style: const TextStyle(
                            color: Colors.white,
                            fontWeight: FontWeight.w800,
                            fontSize: 16))),
              ])),
          // Welcome hero with the three-friends illustration.
          Padding(
              padding: const EdgeInsets.fromLTRB(14, 4, 14, 0),
              child: Container(
                  height: 168,
                  clipBehavior: Clip.antiAlias,
                  decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(26),
                      gradient: const LinearGradient(
                          colors: [Color(0xFFE9F7EE), Color(0xFFDDF3E5)])),
                  child: Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Expanded(
                            flex: 11,
                            child: Padding(
                                padding:
                                    const EdgeInsets.fromLTRB(20, 18, 4, 14),
                                child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: [
                                      FittedBox(
                                          key: const ValueKey(
                                              'peer-home-welcome'),
                                          fit: BoxFit.scaleDown,
                                          alignment: Alignment.centerLeft,
                                          child: Text('Welcome,\n$first 👋',
                                              style: const TextStyle(
                                                  fontSize: 23,
                                                  height: 1.2,
                                                  fontWeight: FontWeight.w800,
                                                  color: PeerTokens.navy))),
                                      const SizedBox(height: 8),
                                      Container(
                                          width: 34,
                                          height: 4,
                                          decoration: BoxDecoration(
                                              color: const Color(0xFF14AD5D),
                                              borderRadius:
                                                  BorderRadius.circular(2))),
                                      const SizedBox(height: 8),
                                      const Text(
                                          'Thank you for supporting others on their health journey.',
                                          maxLines: 3,
                                          overflow: TextOverflow.ellipsis,
                                          style: TextStyle(
                                              fontSize: 12.5,
                                              height: 1.4,
                                              color: PeerTokens.grey)),
                                    ]))),
                        Expanded(
                            flex: 9,
                            child: Image.asset('assets/peer/hero_people.webp',
                                fit: BoxFit.cover,
                                alignment: Alignment.centerLeft)),
                      ]))),
          PeerLoader<(List<PeerInboxItem>, List<PeerChatSummary>)>(
              future: _load,
              onRetry: () => setState(() {
                    _load = _fetch();
                  }),
              builder: (data) {
                final (requests, chats) = data;
                final now = DateTime.now();
                final pending = requests.where((r) => r.status == 'PENDING');
                final today = requests.where(
                    (r) => r.status == 'ACCEPTED' && peerSameDay(r.start, now));
                final unread = chats.fold<int>(0, (n, c) => n + c.unread);
                Widget kpi(IconData icon, int value, String label, int tab) =>
                    Expanded(
                        child: PeerCard(
                            key: ValueKey('peer-kpi-$tab'),
                            wave: true,
                            padding: const EdgeInsets.fromLTRB(6, 16, 6, 18),
                            onTap: () => widget.onOpenTab(tab),
                            child: Column(children: [
                              Container(
                                  width: 52,
                                  height: 48,
                                  decoration: BoxDecoration(
                                      color: PeerTokens.mint,
                                      borderRadius: BorderRadius.circular(14)),
                                  child: Icon(icon,
                                      size: 26, color: PeerTokens.green)),
                              const SizedBox(height: 10),
                              Text('$value',
                                  style: const TextStyle(
                                      fontSize: 32,
                                      height: 1.1,
                                      fontWeight: FontWeight.w800,
                                      color: PeerTokens.navy)),
                              const SizedBox(height: 4),
                              Text(label,
                                  textAlign: TextAlign.center,
                                  style: const TextStyle(
                                      fontSize: 13.5,
                                      height: 1.25,
                                      color: PeerTokens.grey)),
                            ])));
                return Padding(
                    padding: const EdgeInsets.fromLTRB(16, 18, 16, 0),
                    child: IntrinsicHeight(
                        child: Row(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                          kpi(Icons.mail_outline_rounded, pending.length,
                              'Pending\nRequests', 1),
                          const SizedBox(width: 10),
                          kpi(Icons.calendar_today_outlined, today.length,
                              'Accepted\nSessions\nToday', 2),
                          const SizedBox(width: 10),
                          kpi(Icons.chat_bubble_outline_rounded, unread,
                              'Unread\nChats', 3),
                        ])));
              }),
          // Mission card.
          Padding(
              padding: const EdgeInsets.fromLTRB(14, 18, 14, 0),
              child: PeerCard(
                  color: const Color(0xFFE8F7EE),
                  wave: true,
                  padding: const EdgeInsets.fromLTRB(14, 16, 16, 18),
                  child: IntrinsicHeight(
                      child: Row(children: [
                    Image.asset('assets/peer/mission_people.webp',
                        width: 62, fit: BoxFit.contain),
                    const SizedBox(width: 10),
                    const VerticalDivider(
                        width: 1, thickness: 1.2, color: PeerTokens.mintLine),
                    const SizedBox(width: 14),
                    const Expanded(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          Text('Support. Guide. Connect. Follow Up.',
                              style: TextStyle(
                                  fontSize: 14,
                                  fontWeight: FontWeight.w800,
                                  color: PeerTokens.navy)),
                          SizedBox(height: 6),
                          Text(
                              'Be a trusted peer, share experiences, and help others stay on their health journey.',
                              style: TextStyle(
                                  fontSize: 12.5,
                                  height: 1.4,
                                  color: PeerTokens.grey)),
                        ])),
                  ])))),
          const Padding(
              padding: EdgeInsets.fromLTRB(18, 24, 18, 12),
              child: Text('Quick Actions',
                  style: TextStyle(
                      fontSize: 21,
                      fontWeight: FontWeight.w800,
                      color: PeerTokens.navy))),
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 14),
              child: IntrinsicHeight(
                  child: Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                    Expanded(
                        child: _QuickAction(
                            key: const ValueKey('peer-quick-availability'),
                            icon: Icons.event_available_outlined,
                            label: 'Manage\nAvailability',
                            onTap: () => widget.onOpenSchedule(1))),
                    const SizedBox(width: 10),
                    Expanded(
                        child: _QuickAction(
                            key: const ValueKey('peer-quick-slots'),
                            icon: Icons.add_circle_outline_rounded,
                            label: 'Create\nSupport Slots',
                            onTap: () => widget.onOpenSchedule(0))),
                  ]))),
        ]));
  }
}

class _QuickAction extends StatelessWidget {
  const _QuickAction(
      {super.key,
      required this.icon,
      required this.label,
      required this.onTap});
  final IconData icon;
  final String label;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => PeerCard(
      wave: true,
      onTap: onTap,
      padding: const EdgeInsets.fromLTRB(14, 18, 10, 18),
      child: Row(children: [
        Icon(icon, size: 34, color: PeerTokens.green),
        const SizedBox(width: 10),
        Expanded(
            child: Text(label,
                style: const TextStyle(
                    fontSize: 15,
                    height: 1.25,
                    fontWeight: FontWeight.w700,
                    color: PeerTokens.navy))),
        Container(
            width: 30,
            height: 30,
            decoration: const BoxDecoration(
                color: PeerTokens.mint, shape: BoxShape.circle),
            child: const Icon(Icons.chevron_right_rounded,
                size: 22, color: PeerTokens.navy)),
      ]));
}

// ---------------------------------------------------------------------------
// Profile (reference 07)
// ---------------------------------------------------------------------------

class PeerProfileTab extends StatefulWidget {
  const PeerProfileTab(
      {super.key,
      required this.repo,
      required this.profile,
      required this.onManageSlots});
  final PeerRepository repo;
  final PeerProfile profile;
  final VoidCallback onManageSlots;
  @override
  State<PeerProfileTab> createState() => _PeerProfileTabState();
}

class _PeerProfileTabState extends State<PeerProfileTab> {
  late PeerProfile p = widget.profile;

  /// Edits one field in a dialog and saves it.
  Future<void> _edit(
      {required String title,
      required String initial,
      required int maxLength,
      int lines = 1,
      required Future<PeerProfile> Function(String value) save}) async {
    final value = await peerPromptText(context,
        title: title,
        initial: initial,
        maxLength: maxLength,
        lines: lines,
        fieldKey: const ValueKey('peer-profile-edit-field'));
    if (value == null) return;
    try {
      final updated = await save(value.trim());
      if (mounted) setState(() => p = updated);
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    }
  }

  Widget _row(
          {required Key key,
          required IconData icon,
          required String title,
          required String value,
          Widget? trailing,
          VoidCallback? onTap,
          Color color = Colors.white}) =>
      Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: PeerCard(
              key: key,
              color: color,
              radius: 20,
              onTap: onTap,
              padding: const EdgeInsets.fromLTRB(14, 16, 14, 16),
              child: Row(children: [
                Container(
                    width: 50,
                    height: 50,
                    decoration: BoxDecoration(
                        color: color == Colors.white
                            ? PeerTokens.mintSoft
                            : PeerTokens.mint,
                        borderRadius: BorderRadius.circular(14)),
                    child: Icon(icon, size: 26, color: PeerTokens.green)),
                const SizedBox(width: 14),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      Text(title,
                          style: const TextStyle(
                              fontSize: 16.5,
                              fontWeight: FontWeight.w800,
                              color: PeerTokens.navy)),
                      const SizedBox(height: 3),
                      Text(value,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 14, color: PeerTokens.grey)),
                    ])),
                if (trailing != null) trailing,
              ])));

  Widget _pillButton(String label, IconData icon, VoidCallback onTap,
          {Key? key}) =>
      OutlinedButton.icon(
          key: key,
          style: OutlinedButton.styleFrom(
              foregroundColor: PeerTokens.green,
              backgroundColor: Colors.white,
              side: const BorderSide(color: PeerTokens.green, width: 1.3),
              shape: const StadiumBorder(),
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              textStyle:
                  const TextStyle(fontSize: 14.5, fontWeight: FontWeight.w800)),
          onPressed: onTap,
          icon: Icon(icon, size: 19),
          label: Text(label));

  @override
  Widget build(BuildContext context) =>
      ListView(padding: const EdgeInsets.only(bottom: 24), children: [
        const PeerHeader(
            title: 'My Profile',
            subtitle: 'View and manage your peer navigator profile'),
        Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 14),
            child: PeerCard(
                color: const Color(0xFFEAF7EF),
                wave: true,
                padding: const EdgeInsets.fromLTRB(18, 20, 18, 20),
                child: Row(children: [
                  Container(
                      width: 72,
                      height: 72,
                      alignment: Alignment.center,
                      decoration: const BoxDecoration(
                          color: Color(0xFFD6F1E1), shape: BoxShape.circle),
                      child: Text(peerInitials(p.fullName),
                          style: const TextStyle(
                              fontSize: 27,
                              fontWeight: FontWeight.w800,
                              color: PeerTokens.green))),
                  const SizedBox(width: 16),
                  Expanded(
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                        Text(p.fullName,
                            style: const TextStyle(
                                fontSize: 20,
                                fontWeight: FontWeight.w800,
                                color: PeerTokens.navy)),
                        const SizedBox(height: 2),
                        const Text('Peer Navigator',
                            style: TextStyle(
                                fontSize: 15, color: PeerTokens.grey)),
                        const SizedBox(height: 8),
                        const PeerStatusPill('ACCEPTED', label: 'Approved'),
                      ])),
                ]))),
        _row(
            key: const ValueKey('peer-profile-location'),
            icon: Icons.location_on_outlined,
            title: 'Location',
            value: p.county,
            trailing:
                const Icon(Icons.chevron_right_rounded, color: PeerTokens.navy),
            onTap: () => _edit(
                title: 'Location (county)',
                initial: p.county,
                maxLength: 80,
                save: (v) => widget.repo.updateProfile(county: v))),
        _row(
            key: const ValueKey('peer-profile-organization'),
            icon: Icons.apartment_rounded,
            title: 'Facility / Organization',
            value: p.organization ?? 'Not set',
            trailing:
                const Icon(Icons.chevron_right_rounded, color: PeerTokens.navy),
            onTap: () => _edit(
                title: 'Facility / Organization',
                initial: p.organization ?? '',
                maxLength: 150,
                save: (v) => widget.repo.updateProfile(organization: v))),
        _row(
            key: const ValueKey('peer-profile-about'),
            icon: Icons.person_outline_rounded,
            title: 'About You',
            value: (p.bio ?? '').trim().isEmpty ? 'Not set' : p.bio!,
            trailing: _pillButton(
                'Edit',
                Icons.edit_outlined,
                key: const ValueKey('peer-profile-edit-about'),
                () => _edit(
                    title: 'About you (shown to patients)',
                    initial: p.bio ?? '',
                    maxLength: 500,
                    lines: 4,
                    save: (v) => widget.repo.updateProfile(bio: v)))),
        _row(
            key: const ValueKey('peer-profile-availability'),
            icon: Icons.calendar_month_outlined,
            title: 'Availability',
            value: 'Create and manage your support slots.',
            color: const Color(0xFFEDF8F1),
            trailing: _pillButton(
                'Manage Slots',
                Icons.event_note_outlined,
                key: const ValueKey('peer-profile-manage-slots'),
                widget.onManageSlots)),
        Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
            child: Material(
                color: PeerTokens.redSoft,
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(18),
                    side: const BorderSide(color: Color(0xFFF3B7B7))),
                child: InkWell(
                    key: const ValueKey('peer-sign-out'),
                    borderRadius: BorderRadius.circular(18),
                    onTap: () => context.read<AuthCubit>().logout(),
                    child: const Padding(
                        padding:
                            EdgeInsets.symmetric(horizontal: 20, vertical: 20),
                        child: Row(children: [
                          Icon(Icons.logout_rounded,
                              color: PeerTokens.red, size: 26),
                          SizedBox(width: 16),
                          Text('Sign Out',
                              style: TextStyle(
                                  fontSize: 17,
                                  fontWeight: FontWeight.w800,
                                  color: PeerTokens.red)),
                        ]))))),
      ]);
}
