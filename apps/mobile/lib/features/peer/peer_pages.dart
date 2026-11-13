import 'package:flutter/material.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/afya_auth_widgets.dart';
import '../auth/welcome_pages.dart';
import '../doctor/doctor_models.dart';
import '../doctor/doctor_pages.dart';
import '../home/home_pages.dart';

/// Peer Navigator screens, built to the designs in docs/design/
/// (afyamkononi-peer-*-reference.png).
///
/// STATUS: presentation only. The server has no Peer Navigator role,
/// directory, request, conversation or call support yet, so nothing here
/// loads or saves anything. Every screen is fed a [PeerData] value; the only
/// place that builds one today is the design preview, with labelled sample
/// content. The signed-in app cannot reach these screens.

const _mint = Color(0xFFE6F5EE);
const _blueTint = Color(0xFFE3EFFB);
const _amberTint = Color(0xFFFDF3DC);
const _violetTint = Color(0xFFEDE9FB);

class PeerRequest {
  const PeerRequest(this.name, this.detail, this.age);
  final String name, detail, age;
}

class PeerClient {
  const PeerClient(
      {required this.name,
      required this.id,
      required this.phone,
      required this.status,
      required this.next,
      this.age,
      this.sex,
      this.location});
  final String name, id, phone, status, next;
  final String? age, sex, location;
}

class PeerThread {
  const PeerThread(
      {required this.name,
      required this.state,
      required this.topic,
      required this.preview,
      required this.when,
      required this.chat,
      this.unread = 0});
  final String name, state, topic, preview, when;
  final int unread;

  /// Opened with the doctor app's own chat thread screen.
  final ChatDto chat;
}

class PeerData {
  const PeerData(
      {required this.name,
      required this.location,
      required this.peerId,
      required this.bio,
      required this.approved,
      required this.requests,
      required this.clients,
      required this.threads,
      required this.stats});
  final String name, location, peerId, bio;
  final bool approved;
  final List<PeerRequest> requests;
  final List<PeerClient> clients;
  final List<PeerThread> threads;

  /// Counters keyed by the label drawn under them.
  final Map<String, String> stats;
}

void _notConnected(BuildContext context) =>
    ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
        content: Text('Not connected yet — Peer Navigator design preview.')));

void _open(BuildContext context, Widget page) => Navigator.push(
    context,
    MaterialPageRoute(
        builder: (_) => ColoredBox(color: DoctorTokens.screen, child: page)));

class PeerShell extends StatefulWidget {
  const PeerShell({super.key, required this.data});
  final PeerData data;
  @override
  State<PeerShell> createState() => _PeerShellState();
}

class _PeerShellState extends State<PeerShell> {
  int tab = 0;
  static const _items = <(IconData, String)>[
    (Icons.home_outlined, 'Home'),
    (Icons.groups_outlined, 'Clients'),
    (Icons.chat_bubble_outline, 'Messages'),
    (Icons.notifications_none, 'Notifications'),
    (Icons.person_outline, 'Profile'),
  ];
  @override
  Widget build(BuildContext context) {
    final d = widget.data;
    void go(int i) => setState(() => tab = i);
    final pages = [
      PeerHomePage(data: d, onOpenTab: go),
      PeerClientsPage(data: d, onOpenTab: go),
      PeerMessagesPage(data: d, onOpenTab: go),
      _PeerNotifications(onOpenTab: go),
      PeerProfilePage(data: d, onOpenTab: go),
    ];
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: pages[tab],
        bottomNavigationBar: SafeArea(
            top: false,
            child: Container(
                height: 76,
                decoration: const BoxDecoration(
                    color: Colors.white,
                    border: Border(
                        top: BorderSide(color: DoctorTokens.fieldBorder))),
                padding: const EdgeInsets.fromLTRB(6, 7, 6, 5),
                child: Row(children: [
                  for (var i = 0; i < _items.length; i++)
                    Expanded(
                        child: InkWell(
                            onTap: () => go(i),
                            borderRadius: BorderRadius.circular(14),
                            child: Container(
                                decoration: BoxDecoration(
                                    color: tab == i ? _mint : null,
                                    borderRadius: BorderRadius.circular(14)),
                                child: Column(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: [
                                      Icon(_items[i].$1,
                                          size: 25,
                                          color: tab == i
                                              ? DoctorTokens.primary
                                              : DoctorTokens.fieldText),
                                      const SizedBox(height: 3),
                                      FittedBox(
                                          fit: BoxFit.scaleDown,
                                          child: Text(_items[i].$2,
                                              style: TextStyle(
                                                  fontSize: 11,
                                                  fontWeight: tab == i
                                                      ? FontWeight.w800
                                                      : FontWeight.w500,
                                                  color: tab == i
                                                      ? DoctorTokens.primary
                                                      : DoctorTokens
                                                          .fieldText)))
                                    ]))))
                ]))));
  }
}

class _BrandBar extends StatelessWidget {
  const _BrandBar({required this.onProfile});
  final VoidCallback onProfile;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.fromLTRB(14, 6, 8, 2),
      child: Row(children: [
        const Flexible(
            child: Align(
                alignment: Alignment.centerLeft,
                child: AfyaBrandLockup(scale: .62))),
        const Spacer(),
        IconButton(
            onPressed: () => _notConnected(context),
            tooltip: 'Notifications',
            icon: const Icon(Icons.notifications_none,
                size: 28, color: DoctorTokens.primaryDark)),
        IconButton(
            onPressed: onProfile,
            tooltip: 'Profile',
            icon: const Icon(Icons.account_circle_outlined,
                size: 30, color: DoctorTokens.primaryDark)),
      ]));
}

class _Hero extends StatelessWidget {
  const _Hero({required this.children});
  final List<Widget> children;
  @override
  Widget build(BuildContext context) => Container(
      decoration: const BoxDecoration(
          gradient: LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: [Colors.white, Color(0xFFDFF2EA)])),
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 14),
      child: Row(children: [
        ClipOval(
            child: Image.asset('assets/peer/peer_portrait.jpg',
                width: 96,
                height: 96,
                fit: BoxFit.cover,
                excludeFromSemantics: true)),
        const SizedBox(width: 14),
        Expanded(
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: children))
      ]));
}

const _heroBody =
    TextStyle(fontSize: 14, height: 1.3, color: DoctorTokens.slate);

class _RolePill extends StatelessWidget {
  const _RolePill(this.text);
  final String text;
  @override
  Widget build(BuildContext context) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
      decoration:
          BoxDecoration(color: _mint, borderRadius: BorderRadius.circular(20)),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        const Icon(Icons.groups_outlined,
            size: 18, color: DoctorTokens.primaryDark),
        const SizedBox(width: 6),
        Flexible(
            child: Text(text,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                    fontSize: 13.5,
                    fontWeight: FontWeight.w800,
                    color: DoctorTokens.primaryDark)))
      ]));
}

class _Card extends StatelessWidget {
  const _Card({required this.child, this.padding = const EdgeInsets.all(14)});
  final Widget child;
  final EdgeInsets padding;
  @override
  Widget build(BuildContext context) => Container(
      padding: padding,
      decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: DoctorTokens.fieldBorder)),
      child: child);
}

/// One counter. Every counter in the Peer screens is this widget, laid out
/// by [_KpiRow], so sizes, padding and alignment are the same everywhere.
class _Kpi extends StatelessWidget {
  const _Kpi(this.icon, this.value, this.label,
      {this.tint = Colors.white,
      this.color = DoctorTokens.primary,
      this.outlined = false,
      this.lines = 0});
  final IconData icon;
  final String value;

  /// One line per "\n". A line that is too wide shrinks instead of being
  /// cut off, so four counters still fit across a narrow phone.
  final String label;
  final Color tint, color;
  final bool outlined;

  /// Label lines to reserve; set by [_KpiRow] so a row's tiles end level.
  final int lines;

  int get labelLines => label.split('\n').length;

  _Kpi reserving(int count) => _Kpi(icon, value, label,
      tint: tint, color: color, outlined: outlined, lines: count);

  static const _lineHeight = 16.0;

  @override
  Widget build(BuildContext context) {
    final parts = label.split('\n');
    final reserved = lines > parts.length ? lines : parts.length;
    return Container(
        padding: const EdgeInsets.fromLTRB(10, 10, 8, 10),
        decoration: BoxDecoration(
            color: tint,
            borderRadius: BorderRadius.circular(14),
            border:
                outlined ? Border.all(color: DoctorTokens.fieldBorder) : null),
        child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                  height: 26,
                  child: Row(children: [
                    Icon(icon, size: 22, color: color),
                    const SizedBox(width: 6),
                    Expanded(
                        child: FittedBox(
                            fit: BoxFit.scaleDown,
                            alignment: Alignment.centerLeft,
                            child: Text(value,
                                style: const TextStyle(
                                    fontSize: 21,
                                    height: 1.1,
                                    fontWeight: FontWeight.w800,
                                    color: DoctorTokens.navy))))
                  ])),
              const SizedBox(height: 4),
              // Fixed-height lines: the tile is exactly as tall as its
              // content, with no stretching.
              for (var i = 0; i < reserved; i++)
                SizedBox(
                    height: _lineHeight,
                    width: double.infinity,
                    child: i < parts.length
                        ? FittedBox(
                            fit: BoxFit.scaleDown,
                            alignment: Alignment.centerLeft,
                            child: Text(parts[i],
                                style: const TextStyle(
                                    fontSize: 12.5,
                                    height: 1.2,
                                    color: DoctorTokens.fieldText)))
                        : null),
            ]));
  }
}

/// Counters in one row: equal widths, one gap, and the same number of label
/// lines reserved in each so they finish level.
class _KpiRow extends StatelessWidget {
  const _KpiRow(this.tiles);
  final List<_Kpi> tiles;
  @override
  Widget build(BuildContext context) {
    final lines =
        tiles.map((t) => t.labelLines).reduce((a, b) => a > b ? a : b);
    return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
      for (var i = 0; i < tiles.length; i++) ...[
        if (i > 0) const SizedBox(width: 8),
        Expanded(child: tiles[i].reserving(lines))
      ]
    ]);
  }
}

class _SectionTitle extends StatelessWidget {
  const _SectionTitle(this.title, {this.action, this.onAction});
  final String title;
  final String? action;
  final VoidCallback? onAction;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.fromLTRB(16, 18, 6, 8),
      child: Row(children: [
        Expanded(
            child: Text(title,
                style: const TextStyle(
                    fontSize: 18,
                    fontWeight: FontWeight.w800,
                    color: DoctorTokens.navy))),
        if (action != null)
          TextButton(onPressed: onAction, child: Text(action!))
        else
          const SizedBox(height: 40)
      ]));
}

class _Chips extends StatelessWidget {
  const _Chips(this.labels);
  final List<String> labels;
  @override
  Widget build(BuildContext context) => SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.fromLTRB(16, 4, 16, 4),
      child: Row(children: [
        for (var i = 0; i < labels.length; i++)
          Container(
              margin: const EdgeInsets.only(right: 8),
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 9),
              decoration: BoxDecoration(
                  color: i == 0 ? DoctorTokens.primaryDark : Colors.white,
                  borderRadius: BorderRadius.circular(22),
                  border: Border.all(
                      color: i == 0
                          ? DoctorTokens.primaryDark
                          : DoctorTokens.fieldBorder)),
              child: Text(labels[i],
                  style: TextStyle(
                      fontSize: 13.5,
                      fontWeight: FontWeight.w600,
                      color: i == 0 ? Colors.white : DoctorTokens.fieldText)))
      ]));
}

class _Search extends StatelessWidget {
  const _Search(this.hint);
  final String hint;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.fromLTRB(16, 10, 16, 6),
      child: TextField(
          readOnly: true,
          onTap: () => _notConnected(context),
          decoration: InputDecoration(
              hintText: hint,
              hintStyle:
                  const TextStyle(fontSize: 14.5, color: DoctorTokens.slate),
              prefixIcon: const Icon(Icons.search, color: DoctorTokens.navy),
              isDense: true,
              filled: true,
              fillColor: Colors.white,
              enabledBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(14),
                  borderSide:
                      const BorderSide(color: DoctorTokens.fieldBorder)))));
}

// ---------------------------------------------------------------- Home

class PeerHomePage extends StatelessWidget {
  const PeerHomePage({super.key, required this.data, required this.onOpenTab});
  final PeerData data;
  final ValueChanged<int> onOpenTab;
  @override
  Widget build(BuildContext context) {
    final d = data;
    Widget action(IconData icon, String title, String sub, VoidCallback tap) =>
        Expanded(
            child: Material(
                color: Colors.white,
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(16),
                    side: const BorderSide(color: DoctorTokens.fieldBorder)),
                clipBehavior: Clip.antiAlias,
                child: InkWell(
                    onTap: tap,
                    child: Padding(
                        padding: const EdgeInsets.fromLTRB(8, 12, 4, 12),
                        child: Row(children: [
                          Container(
                              width: 40,
                              height: 40,
                              decoration: BoxDecoration(
                                  color: _mint,
                                  borderRadius: BorderRadius.circular(10)),
                              child: Icon(icon,
                                  size: 24, color: DoctorTokens.primary)),
                          const SizedBox(width: 8),
                          Expanded(
                              child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                Text(title,
                                    style: const TextStyle(
                                        fontSize: 13,
                                        fontWeight: FontWeight.w800,
                                        color: DoctorTokens.navy)),
                                Text(sub,
                                    style: const TextStyle(
                                        fontSize: 11.5,
                                        height: 1.25,
                                        color: DoctorTokens.fieldText)),
                              ])),
                          const Icon(Icons.chevron_right,
                              size: 20, color: DoctorTokens.primary)
                        ])))));
    return SafeArea(
        bottom: false,
        child: ListView(padding: EdgeInsets.zero, children: [
          _BrandBar(onProfile: () => onOpenTab(4)),
          const _Hero(children: [
            Text('Welcome back,',
                style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: DoctorTokens.navy)),
            Text('Peer Navigator',
                style: TextStyle(
                    fontSize: 24,
                    height: 1.15,
                    fontWeight: FontWeight.w800,
                    color: DoctorTokens.primaryDark)),
            SizedBox(height: 4),
            Text('Offer guidance, support and link patients to care services.',
                style: _heroBody),
          ]),
          Transform.translate(
              offset: const Offset(0, -8),
              child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: _Card(
                      child: Row(children: [
                    Expanded(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          Text(d.name,
                              style: const TextStyle(
                                  fontSize: 19,
                                  fontWeight: FontWeight.w800,
                                  color: DoctorTokens.navy)),
                          const Text('Peer Navigator',
                              style: TextStyle(
                                  fontSize: 14, color: DoctorTokens.fieldText)),
                          const SizedBox(height: 4),
                          Row(children: [
                            const Icon(Icons.location_on_outlined,
                                size: 18, color: DoctorTokens.primaryDark),
                            const SizedBox(width: 4),
                            Flexible(
                                child: Text(d.location,
                                    style: const TextStyle(
                                        fontSize: 14,
                                        color: DoctorTokens.navy)))
                          ])
                        ])),
                    Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 12, vertical: 8),
                        decoration: BoxDecoration(
                            color:
                                d.approved ? _mint : DoctorTokens.warningLight,
                            borderRadius: BorderRadius.circular(12)),
                        child: Row(mainAxisSize: MainAxisSize.min, children: [
                          Icon(
                              d.approved
                                  ? Icons.check_circle
                                  : Icons.hourglass_top,
                              size: 18,
                              color: d.approved
                                  ? DoctorTokens.primary
                                  : DoctorTokens.warning),
                          const SizedBox(width: 6),
                          Text(d.approved ? 'Approved' : 'Pending review',
                              style: const TextStyle(
                                  fontSize: 14,
                                  fontWeight: FontWeight.w700,
                                  color: DoctorTokens.primaryDark))
                        ]))
                  ])))),
          const _SectionTitle('Your Support Overview'),
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: _KpiRow([
                _Kpi(Icons.forum_outlined, d.stats['Support Requests'] ?? '–',
                    'Support\nRequests',
                    tint: _mint),
                _Kpi(
                    Icons.groups_outlined,
                    d.stats['Active Conversations'] ?? '–',
                    'Active\nConversations',
                    tint: _blueTint,
                    color: DoctorTokens.info),
                _Kpi(Icons.videocam_outlined, d.stats['Calls Completed'] ?? '–',
                    'Calls\nCompleted',
                    tint: _amberTint, color: DoctorTokens.warning),
              ])),
          _SectionTitle('New Support Requests',
              action: 'View All →', onAction: () => _notConnected(context)),
          for (final r in d.requests)
            Padding(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
                child: _Card(
                    padding: const EdgeInsets.all(10),
                    child: Row(children: [
                      InitialsAvatar(r.name, size: 46),
                      const SizedBox(width: 10),
                      Expanded(
                          child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                            Text(r.name,
                                style: const TextStyle(
                                    fontSize: 15,
                                    fontWeight: FontWeight.w800,
                                    color: DoctorTokens.navy)),
                            Text(r.detail,
                                style: const TextStyle(
                                    fontSize: 12.5,
                                    color: DoctorTokens.fieldText)),
                            Text(r.age,
                                style: const TextStyle(
                                    fontSize: 12, color: DoctorTokens.slate)),
                          ])),
                      OutlinedButton(
                          onPressed: () => _notConnected(context),
                          style: OutlinedButton.styleFrom(
                              foregroundColor: DoctorTokens.navy,
                              padding:
                                  const EdgeInsets.symmetric(horizontal: 12)),
                          child: const Text('Decline')),
                      const SizedBox(width: 6),
                      FilledButton(
                          onPressed: () => _notConnected(context),
                          style: FilledButton.styleFrom(
                              minimumSize: const Size(0, 40),
                              padding:
                                  const EdgeInsets.symmetric(horizontal: 16),
                              textStyle: const TextStyle(
                                  fontSize: 14, fontWeight: FontWeight.w700)),
                          child: const Text('Accept')),
                    ]))),
          const _SectionTitle('Quick Actions'),
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Column(children: [
                Row(children: [
                  action(
                      Icons.mark_chat_unread_outlined,
                      'Support Requests',
                      'View and respond to new requests',
                      () => _notConnected(context)),
                  const SizedBox(width: 8),
                  action(Icons.forum_outlined, 'My Conversations',
                      'Continue ongoing support chats', () => onOpenTab(2)),
                ]),
                const SizedBox(height: 8),
                Row(children: [
                  action(
                      Icons.videocam_outlined,
                      'Start a Call',
                      'Request or join an audio/video call',
                      () => _notConnected(context)),
                  const SizedBox(width: 8),
                  action(Icons.person_outline, 'My Profile',
                      'Update your details and location', () => onOpenTab(4)),
                ]),
              ])),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 20),
              child: Container(
                  clipBehavior: Clip.antiAlias,
                  decoration: BoxDecoration(
                      color: _mint, borderRadius: BorderRadius.circular(18)),
                  child: Row(children: [
                    const Expanded(
                        child: Padding(
                            padding: EdgeInsets.fromLTRB(16, 14, 4, 14),
                            child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text('Support, Guide, Connect',
                                      style: TextStyle(
                                          fontSize: 18,
                                          height: 1.15,
                                          fontWeight: FontWeight.w800,
                                          color: DoctorTokens.navy)),
                                  SizedBox(height: 6),
                                  Text(
                                      'Be a trusted companion on the patient’s health journey.',
                                      style: TextStyle(
                                          fontSize: 12.5,
                                          color: DoctorTokens.fieldText)),
                                ]))),
                    Image.asset('assets/peer/peer_banner.jpg',
                        width: 190,
                        height: 100,
                        fit: BoxFit.cover,
                        excludeFromSemantics: true)
                  ]))),
        ]));
  }
}

// ------------------------------------------------------------- Clients

(String, Color, Color, IconData) _clientPill(String status) => switch (status) {
      'Active' => (
          'Active',
          _mint,
          DoctorTokens.primaryDark,
          Icons.check_circle
        ),
      'Needs Follow-up' => (
          'Needs Follow-up',
          _amberTint,
          const Color(0xFFB45309),
          Icons.event_note
        ),
      'In Support' => (
          'In Support',
          _blueTint,
          DoctorTokens.info,
          Icons.schedule
        ),
      _ => (
          status,
          DoctorTokens.surfaceSubtle,
          DoctorTokens.fieldText,
          Icons.pause_circle_outline
        ),
    };

class PeerClientsPage extends StatelessWidget {
  const PeerClientsPage(
      {super.key, required this.data, required this.onOpenTab});
  final PeerData data;
  final ValueChanged<int> onOpenTab;
  @override
  Widget build(BuildContext context) {
    final clients = data.clients;
    final active = clients.where((c) => c.status == 'Active').length;
    final follow = clients.where((c) => c.status == 'Needs Follow-up').length;
    return SafeArea(
        bottom: false,
        child: ListView(padding: EdgeInsets.zero, children: [
          _BrandBar(onProfile: () => onOpenTab(4)),
          const _Hero(children: [
            _RolePill('Peer Navigator'),
            SizedBox(height: 4),
            Text('My Clients',
                style: TextStyle(
                    fontSize: 24,
                    fontWeight: FontWeight.w800,
                    color: DoctorTokens.navy)),
            Text(
                'Manage your clients, provide support and follow up on their care journey.',
                style: _heroBody),
          ]),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 2),
              child: _KpiRow([
                _Kpi(Icons.groups_outlined, '${clients.length}',
                    'Total\nClients',
                    outlined: true),
                _Kpi(Icons.check_circle_outline, '$active', 'Active',
                    outlined: true),
                _Kpi(Icons.event_outlined, '$follow', 'Follow-ups',
                    outlined: true),
                _Kpi(Icons.person_add_alt, data.stats['New (This Week)'] ?? '–',
                    'New\n(This Week)',
                    outlined: true),
              ])),
          const _Search('Search clients by name, phone or client ID'),
          _Chips([
            'All Clients (${clients.length})',
            'Active ($active)',
            'Needs Follow-up ($follow)',
            'More'
          ]),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
              child: Row(children: [
                Expanded(
                    child: Text('Clients (${clients.length})',
                        style: const TextStyle(
                            fontSize: 20,
                            fontWeight: FontWeight.w800,
                            color: DoctorTokens.navy))),
                OutlinedButton.icon(
                    onPressed: () => _notConnected(context),
                    style: OutlinedButton.styleFrom(
                        foregroundColor: DoctorTokens.primaryDark,
                        side: const BorderSide(color: DoctorTokens.primary),
                        shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(12))),
                    icon: const Icon(Icons.person_add_alt),
                    label: const Text('Add Client',
                        style: TextStyle(fontWeight: FontWeight.w800)))
              ])),
          for (final cl in clients)
            Padding(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
                child: Material(
                    color: Colors.white,
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(16),
                        side:
                            const BorderSide(color: DoctorTokens.fieldBorder)),
                    clipBehavior: Clip.antiAlias,
                    child: InkWell(
                        onTap: () =>
                            _open(context, PeerClientProfilePage(client: cl)),
                        child: Padding(
                            padding: const EdgeInsets.fromLTRB(10, 10, 4, 10),
                            child: Row(children: [
                              InitialsAvatar(cl.name, size: 46),
                              const SizedBox(width: 10),
                              Expanded(
                                  child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                    Text(cl.name,
                                        style: const TextStyle(
                                            fontSize: 15.5,
                                            fontWeight: FontWeight.w800,
                                            color: DoctorTokens.navy)),
                                    Text('ID: ${cl.id}  |  ${cl.phone}',
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: const TextStyle(
                                            fontSize: 12,
                                            color: DoctorTokens.fieldText)),
                                    const SizedBox(height: 4),
                                    Row(children: [
                                      _StatusPill(cl.status),
                                      const SizedBox(width: 8),
                                      Flexible(
                                          child: Text(cl.next,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: const TextStyle(
                                                  fontSize: 12,
                                                  color:
                                                      DoctorTokens.fieldText)))
                                    ])
                                  ])),
                              const Icon(Icons.chevron_right,
                                  color: DoctorTokens.primary)
                            ]))))),
          const SizedBox(height: 16),
        ]));
  }
}

class _StatusPill extends StatelessWidget {
  const _StatusPill(this.status);
  final String status;
  @override
  Widget build(BuildContext context) {
    final (label, bg, fg, icon) = _clientPill(status);
    return Container(
        padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
        decoration:
            BoxDecoration(color: bg, borderRadius: BorderRadius.circular(14)),
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          Icon(icon, size: 15, color: fg),
          const SizedBox(width: 4),
          Text(label,
              style: TextStyle(
                  fontSize: 12, fontWeight: FontWeight.w700, color: fg))
        ]));
  }
}

// ------------------------------------------------------ Client profile

class PeerClientProfilePage extends StatelessWidget {
  const PeerClientProfilePage({super.key, required this.client});
  final PeerClient client;
  @override
  Widget build(BuildContext context) {
    final cl = client;
    Widget fact(IconData icon, String text) =>
        Row(mainAxisSize: MainAxisSize.min, children: [
          Icon(icon, size: 17, color: DoctorTokens.fieldText),
          const SizedBox(width: 5),
          Text(text,
              style: const TextStyle(fontSize: 13.5, color: DoctorTokens.navy))
        ]);
    Widget act(IconData icon, String label, {bool primary = false}) => Expanded(
        child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 3),
            child: Material(
                color: primary ? DoctorTokens.primaryDark : _mint,
                borderRadius: BorderRadius.circular(14),
                clipBehavior: Clip.antiAlias,
                child: InkWell(
                    onTap: () => _notConnected(context),
                    child: SizedBox(
                        height: 66,
                        child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Icon(icon,
                                  size: 24,
                                  color: primary
                                      ? Colors.white
                                      : DoctorTokens.primaryDark),
                              const SizedBox(height: 3),
                              Text(label,
                                  textAlign: TextAlign.center,
                                  style: TextStyle(
                                      fontSize: 11.5,
                                      height: 1.1,
                                      fontWeight: FontWeight.w700,
                                      color: primary
                                          ? Colors.white
                                          : DoctorTokens.navy))
                            ]))))));
    Widget empty(IconData icon, String title, String text) => Padding(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
        child: _Card(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Icon(icon, size: 22, color: DoctorTokens.primary),
            const SizedBox(width: 8),
            Text(title,
                style: const TextStyle(
                    fontSize: 16.5,
                    fontWeight: FontWeight.w800,
                    color: DoctorTokens.navy))
          ]),
          const SizedBox(height: 8),
          Text(text,
              style: const TextStyle(
                  fontSize: 14, height: 1.4, color: DoctorTokens.fieldText))
        ])));
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            child: ListView(padding: EdgeInsets.zero, children: [
          SizedBox(
              height: 54,
              child: Stack(alignment: Alignment.center, children: [
                Align(
                    alignment: Alignment.centerLeft,
                    child: IconButton(
                        onPressed: () => Navigator.pop(context),
                        tooltip: 'Back',
                        icon: const Icon(Icons.arrow_back,
                            color: DoctorTokens.navy))),
                const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 60),
                    child: AfyaBrandLockup(scale: .58)),
              ])),
          const Center(child: _RolePill('Peer Navigator • Client Support')),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 10),
              child: _Card(
                  child: Row(children: [
                InitialsAvatar(cl.name, size: 76),
                const SizedBox(width: 12),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      Text(cl.name,
                          style: const TextStyle(
                              fontSize: 19,
                              fontWeight: FontWeight.w800,
                              color: DoctorTokens.navy)),
                      Text('Client ID: ${cl.id}',
                          style: const TextStyle(
                              fontSize: 13, color: DoctorTokens.fieldText)),
                      const SizedBox(height: 6),
                      Wrap(spacing: 14, runSpacing: 4, children: [
                        if (cl.age != null)
                          fact(Icons.calendar_today_outlined, cl.age!),
                        if (cl.sex != null) fact(Icons.person_outline, cl.sex!),
                        if (cl.location != null)
                          fact(Icons.location_on_outlined, cl.location!),
                        fact(Icons.phone_outlined, cl.phone),
                      ]),
                      const SizedBox(height: 6),
                      _StatusPill(cl.status),
                    ]))
              ]))),
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 13),
              child: Row(children: [
                act(Icons.phone_outlined, 'Call', primary: true),
                act(Icons.chat_outlined, 'Message'),
                act(Icons.event_outlined, 'Schedule\nFollow-up'),
                act(Icons.edit_outlined, 'Update\nSupport Status'),
              ])),
          const SizedBox(height: 6),
          const _Chips(['Overview', 'Interactions', 'Notes', 'Care Plan']),
          const SizedBox(height: 10),
          // The design fills these cards with a care-progress checklist,
          // session history and a written note. None of that exists to show
          // yet, and sample clinical detail is not invented for a client.
          empty(Icons.track_changes, 'Care Progress',
              'Care-progress tracking will appear here once Peer Navigator support is connected.'),
          empty(Icons.chat_outlined, 'Recent Interactions',
              'No interactions recorded yet.'),
          empty(Icons.note_alt_outlined, 'Latest Note', 'No notes yet.'),
          const SizedBox(height: 10),
        ])));
  }
}

// ------------------------------------------------------------ Messages

class PeerMessagesPage extends StatelessWidget {
  const PeerMessagesPage(
      {super.key, required this.data, required this.onOpenTab});
  final PeerData data;
  final ValueChanged<int> onOpenTab;
  @override
  Widget build(BuildContext context) {
    final threads = data.threads;
    final unread = threads.where((t) => t.unread > 0).length;
    return Stack(children: [
      SafeArea(
          bottom: false,
          child: ListView(padding: EdgeInsets.zero, children: [
            _BrandBar(onProfile: () => onOpenTab(4)),
            const _Hero(children: [
              Text('Peer Navigator Messages',
                  style: TextStyle(
                      fontSize: 20,
                      fontWeight: FontWeight.w800,
                      color: DoctorTokens.navy)),
              SizedBox(height: 2),
              Text('Support conversations with patients in your care.',
                  style: _heroBody),
            ]),
            const _Search('Search conversations, patients…'),
            _Chips([
              'All Messages  ${threads.length}',
              'Unread  $unread',
              'Active',
              'Closed'
            ]),
            const SizedBox(height: 6),
            for (final t in threads)
              InkWell(
                  // The conversation itself is the doctor app's chat thread.
                  onTap: () => _open(context, ChatThreadPage(chat: t.chat)),
                  child: Container(
                      padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
                      decoration: BoxDecoration(
                          color: t.unread > 1 ? _mint : null,
                          border: const Border(
                              bottom:
                                  BorderSide(color: DoctorTokens.fieldBorder))),
                      child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            InitialsAvatar(t.name, size: 48),
                            const SizedBox(width: 12),
                            Expanded(
                                child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: [
                                  Text(t.name,
                                      style: const TextStyle(
                                          fontSize: 15.5,
                                          fontWeight: FontWeight.w800,
                                          color: DoctorTokens.navy)),
                                  Row(children: [
                                    Icon(Icons.circle,
                                        size: 9,
                                        color: t.state == 'Active'
                                            ? DoctorTokens.primary
                                            : DoctorTokens.slate),
                                    const SizedBox(width: 6),
                                    Text('${t.state}  ·  ${t.topic}',
                                        style: const TextStyle(
                                            fontSize: 12.5,
                                            color: DoctorTokens.fieldText))
                                  ]),
                                  const SizedBox(height: 2),
                                  Text(t.preview,
                                      maxLines: 2,
                                      overflow: TextOverflow.ellipsis,
                                      style: const TextStyle(
                                          fontSize: 13.5,
                                          height: 1.3,
                                          color: DoctorTokens.fieldText)),
                                ])),
                            const SizedBox(width: 8),
                            Column(children: [
                              Text(t.when,
                                  style: const TextStyle(
                                      fontSize: 12,
                                      color: DoctorTokens.fieldText)),
                              const SizedBox(height: 8),
                              if (t.unread > 0)
                                Container(
                                    width: 24,
                                    height: 24,
                                    alignment: Alignment.center,
                                    decoration: const BoxDecoration(
                                        color: DoctorTokens.primaryDark,
                                        shape: BoxShape.circle),
                                    child: Text('${t.unread}',
                                        style: const TextStyle(
                                            fontSize: 12,
                                            fontWeight: FontWeight.w800,
                                            color: Colors.white)))
                            ])
                          ]))),
            const Padding(
                padding: EdgeInsets.fromLTRB(16, 14, 16, 96),
                child: AfyaSecurityNote(
                    'Secure & Confidential. Conversations follow AfyaMkononi’s data protection standards.')),
          ])),
      Positioned(
          right: 14,
          bottom: 14,
          child: FilledButton.icon(
              onPressed: () => _notConnected(context),
              style: FilledButton.styleFrom(
                  backgroundColor: DoctorTokens.primaryDark,
                  shape: const StadiumBorder(),
                  padding:
                      const EdgeInsets.symmetric(horizontal: 22, vertical: 14)),
              icon: const Icon(Icons.edit_outlined),
              label: const Text('Compose Message')))
    ]);
  }
}

class _PeerNotifications extends StatelessWidget {
  const _PeerNotifications({required this.onOpenTab});
  final ValueChanged<int> onOpenTab;
  @override
  Widget build(BuildContext context) => SafeArea(
      bottom: false,
      child: Column(children: [
        _BrandBar(onProfile: () => onOpenTab(4)),
        const Expanded(
            child: Center(
                child: Padding(
                    padding: EdgeInsets.all(32),
                    child: Text(
                        'No design was supplied for Peer Navigator notifications yet.',
                        textAlign: TextAlign.center,
                        style: TextStyle(
                            fontSize: 15, color: DoctorTokens.slate)))))
      ]));
}

// ------------------------------------------------------------- Profile

class PeerProfilePage extends StatelessWidget {
  const PeerProfilePage(
      {super.key, required this.data, required this.onOpenTab});
  final PeerData data;
  final ValueChanged<int> onOpenTab;
  @override
  Widget build(BuildContext context) {
    final d = data;
    Widget row(IconData icon, String title, String sub, {bool last = false}) =>
        InkWell(
            onTap: () => _notConnected(context),
            child: Container(
                padding: const EdgeInsets.symmetric(vertical: 14),
                decoration: BoxDecoration(
                    border: last
                        ? null
                        : const Border(
                            bottom:
                                BorderSide(color: DoctorTokens.fieldBorder))),
                child: Row(children: [
                  Icon(icon, size: 30, color: DoctorTokens.primary),
                  const SizedBox(width: 14),
                  Expanded(
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                        Text(title,
                            style: const TextStyle(
                                fontSize: 16,
                                fontWeight: FontWeight.w800,
                                color: DoctorTokens.navy)),
                        Text(sub,
                            style: const TextStyle(
                                fontSize: 13, color: DoctorTokens.fieldText)),
                      ])),
                  const Icon(Icons.chevron_right, color: DoctorTokens.primary)
                ])));
    return SafeArea(
        bottom: false,
        child: ListView(padding: EdgeInsets.zero, children: [
          _BrandBar(onProfile: () {}),
          BoardTitleBar('My Profile', onBack: () => onOpenTab(0)),
          Container(
              decoration: const BoxDecoration(
                  gradient: LinearGradient(
                      begin: Alignment.topCenter,
                      end: Alignment.bottomCenter,
                      colors: [Colors.white, Color(0xFFDFF2EA)])),
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 18),
              child:
                  Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                ClipOval(
                    child: Image.asset('assets/peer/peer_profile_photo.jpg',
                        width: 96,
                        height: 96,
                        fit: BoxFit.cover,
                        excludeFromSemantics: true)),
                const SizedBox(width: 14),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      Text(d.name,
                          style: const TextStyle(
                              fontSize: 21,
                              fontWeight: FontWeight.w800,
                              color: DoctorTokens.navy)),
                      const SizedBox(height: 4),
                      const _RolePill('Peer Navigator'),
                      const SizedBox(height: 6),
                      Row(children: [
                        const Icon(Icons.badge_outlined,
                            size: 17, color: DoctorTokens.fieldText),
                        const SizedBox(width: 5),
                        Text(d.peerId,
                            style: const TextStyle(
                                fontSize: 13.5, color: DoctorTokens.fieldText))
                      ]),
                      const SizedBox(height: 4),
                      Text(d.bio,
                          style: const TextStyle(
                              fontSize: 13.5,
                              height: 1.35,
                              color: DoctorTokens.fieldText)),
                      const SizedBox(height: 8),
                      OutlinedButton.icon(
                          onPressed: () => _notConnected(context),
                          style: OutlinedButton.styleFrom(
                              foregroundColor: DoctorTokens.primaryDark,
                              side: const BorderSide(
                                  color: DoctorTokens.primary)),
                          icon: const Icon(Icons.edit_outlined, size: 18),
                          label: const Text('Edit Profile',
                              style: TextStyle(fontWeight: FontWeight.w800)))
                    ]))
              ])),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 14, 16, 0),
              child: _Card(
                  padding: const EdgeInsets.all(8),
                  child: _KpiRow([
                    _Kpi(Icons.groups_outlined,
                        d.stats['People Supported'] ?? '–', 'People\nSupported',
                        tint: _mint),
                    _Kpi(
                        Icons.chat_bubble_outline,
                        d.stats['Support Conversations'] ?? '–',
                        'Support\nConversations',
                        tint: _blueTint,
                        color: DoctorTokens.info),
                    _Kpi(Icons.reply_outlined, d.stats['Referrals Made'] ?? '–',
                        'Referrals\nMade',
                        tint: _amberTint, color: DoctorTokens.warning),
                    _Kpi(
                        Icons.task_alt,
                        d.stats['Client Follow-up Rate'] ?? '–',
                        'Client\nFollow-up Rate',
                        tint: _violetTint,
                        color: const Color(0xFF6D4AE0)),
                  ]))),
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 14, 16, 24),
              child: _Card(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Column(children: [
                    row(Icons.person_outline, 'Account Information',
                        'View and update your personal and work details'),
                    row(Icons.verified_user_outlined, 'Verification & ID',
                        'Your peer navigator verification and project affiliation'),
                    row(Icons.settings_outlined, 'Settings',
                        'Notifications, app preferences and security',
                        last: true),
                  ]))),
        ]));
  }
}
