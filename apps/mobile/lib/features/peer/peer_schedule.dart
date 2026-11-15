import 'package:flutter/material.dart';

import 'peer_repository.dart';
import 'peer_ui.dart';

/// Schedule & Availability (references 04 "Create Slots" and 04b "My
/// Schedule"). Slots are always 10 minutes, matching the backend.
const peerSlotMinutes = 10;

String _clock(int minutes) => minutes >= 24 * 60
    ? 'Midnight'
    : peerTime(DateTime(2000, 1, 1, minutes ~/ 60, minutes % 60));

class PeerScheduleTab extends StatefulWidget {
  const PeerScheduleTab(
      {super.key, required this.repo, this.initialMode = 0, this.clock});
  final PeerRepository repo;

  /// 0 Create Slots, 1 My Schedule.
  final int initialMode;

  /// Injectable "now" for tests.
  final DateTime Function()? clock;
  @override
  State<PeerScheduleTab> createState() => _PeerScheduleTabState();
}

class _PeerScheduleTabState extends State<PeerScheduleTab> {
  late int mode = widget.initialMode;
  DateTime _now() => (widget.clock ?? DateTime.now)();
  late DateTime day = peerDay(_now());
  late Future<(List<PeerSlot>, List<PeerInboxItem>)> _load = _fetch();
  int start = 8 * 60, end = 10 * 60;
  bool busy = false;

  Future<(List<PeerSlot>, List<PeerInboxItem>)> _fetch() async {
    final slots =
        await widget.repo.slots(day, day.add(const Duration(days: 1)));
    // Requests give "Booked by Amina M." / "Request from Peter K." labels.
    List<PeerInboxItem> requests = const [];
    try {
      requests = await widget.repo.requests();
    } catch (_) {}
    return (slots, requests);
  }

  void _reload() => setState(() {
        _load = _fetch();
      });
  void _setDay(DateTime d) => setState(() {
        day = peerDay(d);
        _load = _fetch();
      });

  Future<void> _pickDate() async {
    final today = peerDay(_now());
    final picked = await showDatePicker(
        context: context,
        initialDate: day.isBefore(today) ? today : day,
        firstDate: today,
        lastDate: today.add(const Duration(days: 90)));
    if (picked != null) _setDay(picked);
  }

  /// Sunday-first week containing [day], as in the reference.
  List<DateTime> get _week {
    final sunday = day.subtract(Duration(days: day.weekday % 7));
    return [for (var i = 0; i < 7; i++) sunday.add(Duration(days: i))];
  }

  /// New 10-minute starts in [start, end) that are in the future and not
  /// already slots.
  List<DateTime> _preview(List<PeerSlot> existing) {
    final now = _now();
    return [
      for (var m = start; m + peerSlotMinutes <= end; m += peerSlotMinutes)
        day.add(Duration(minutes: m))
    ]
        .where((t) =>
            t.isAfter(now) && !existing.any((s) => s.start.isAtSameMomentAs(t)))
        .toList();
  }

  Future<void> _create(List<DateTime> times) async {
    setState(() => busy = true);
    var made = 0;
    String? error;
    for (final t in times) {
      try {
        await widget.repo.addSlot(t);
        made++;
      } catch (e) {
        error ??= peerErrorText(e);
      }
    }
    if (!mounted) return;
    setState(() => busy = false);
    peerToast(
        context,
        made > 0
            ? '$made slot${made == 1 ? '' : 's'} created.${error == null ? '' : ' Some could not be created: $error'}'
            : error ?? 'No slots created.');
    _reload();
  }

  Future<void> _remove(PeerSlot s) async {
    try {
      await widget.repo.removeSlot(s.id);
      if (mounted) peerToast(context, 'Slot removed.');
      _reload();
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    }
  }

  @override
  Widget build(BuildContext context) =>
      ListView(padding: const EdgeInsets.only(bottom: 24), children: [
        PeerHeader(
            title: 'Schedule & Availability',
            subtitle: 'Manage your support slots and availability',
            actions: [
              PeerCircleButton(
                  key: const ValueKey('peer-schedule-calendar'),
                  icon: Icons.edit_calendar_outlined,
                  tooltip: 'Pick a date',
                  onTap: _pickDate),
            ]),
        PeerSegment(items: const [
          (Icons.add_box_outlined, 'Create Slots'),
          (Icons.event_note_outlined, 'My Schedule'),
        ], selected: mode, onSelected: (i) => setState(() => mode = i)),
        const SizedBox(height: 16),
        _Hero(create: mode == 0),
        _dateHeader(),
        _weekStrip(),
        PeerLoader<(List<PeerSlot>, List<PeerInboxItem>)>(
            future: _load,
            onRetry: _reload,
            builder: (data) =>
                mode == 0 ? _createBody(data.$1) : _scheduleBody(data)),
      ]);

  Widget _dateHeader() => Padding(
      padding: const EdgeInsets.fromLTRB(18, 20, 18, 12),
      child: Row(children: [
        const Expanded(
            child: Text('Select Date',
                style: TextStyle(
                    fontSize: 18,
                    fontWeight: FontWeight.w800,
                    color: PeerTokens.navy))),
        if (mode == 0) ...[
          _roundArrow(Icons.chevron_left_rounded, 'Previous week',
              () => _setDay(day.subtract(const Duration(days: 7)))),
          const SizedBox(width: 8),
        ],
        Material(
            color: Colors.white,
            shape:
                const StadiumBorder(side: BorderSide(color: PeerTokens.line)),
            child: InkWell(
                key: const ValueKey('peer-schedule-date'),
                customBorder: const StadiumBorder(),
                onTap: _pickDate,
                child: Padding(
                    padding:
                        const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
                    child: Row(mainAxisSize: MainAxisSize.min, children: [
                      const Icon(Icons.calendar_month_outlined,
                          size: 19, color: PeerTokens.navy),
                      const SizedBox(width: 8),
                      Text(peerLongDate(day),
                          style: const TextStyle(
                              fontSize: 13,
                              fontWeight: FontWeight.w600,
                              color: PeerTokens.navy)),
                      if (mode == 1) ...[
                        const SizedBox(width: 6),
                        const Icon(Icons.expand_more_rounded,
                            color: PeerTokens.navy),
                      ],
                    ])))),
        if (mode == 0) ...[
          const SizedBox(width: 8),
          _roundArrow(Icons.chevron_right_rounded, 'Next week',
              () => _setDay(day.add(const Duration(days: 7)))),
        ],
      ]));

  Widget _roundArrow(IconData icon, String tip, VoidCallback onTap) => Material(
      color: Colors.white,
      shape: const CircleBorder(side: BorderSide(color: PeerTokens.line)),
      child: InkWell(
          customBorder: const CircleBorder(),
          onTap: onTap,
          child: Tooltip(
              message: tip,
              child: SizedBox(
                  width: 36,
                  height: 36,
                  child: Icon(icon, color: PeerTokens.navy)))));

  Widget _weekStrip() {
    final today = peerDay(_now());
    return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14),
        child: Row(children: [
          for (final d in _week)
            Expanded(
                child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 3),
                    child: Material(
                        color: peerSameDay(d, day)
                            ? PeerTokens.green
                            : Colors.white,
                        shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(14),
                            side: BorderSide(
                                color: peerSameDay(d, day)
                                    ? PeerTokens.green
                                    : PeerTokens.line)),
                        child: InkWell(
                            key: ValueKey('peer-day-${d.day}'),
                            borderRadius: BorderRadius.circular(14),
                            onTap: d.isBefore(today) ? null : () => _setDay(d),
                            child: SizedBox(
                                height: 72,
                                child: Column(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: [
                                      Text(peerWeekdays[d.weekday - 1],
                                          style: TextStyle(
                                              fontSize: 13,
                                              color: peerSameDay(d, day)
                                                  ? Colors.white
                                                  : d.isBefore(today)
                                                      ? PeerTokens.line
                                                      : PeerTokens.grey)),
                                      const SizedBox(height: 2),
                                      Text('${d.day}',
                                          style: TextStyle(
                                              fontSize: 21,
                                              fontWeight: FontWeight.w800,
                                              color: peerSameDay(d, day)
                                                  ? Colors.white
                                                  : d.isBefore(today)
                                                      ? PeerTokens.line
                                                      : PeerTokens.navy)),
                                    ]))))))
        ]));
  }

  // -------------------------------------------------------------------------
  // Create Slots
  // -------------------------------------------------------------------------

  Widget _timeCard(String label, int value, List<int> options,
          ValueChanged<int> onChanged, String key) =>
      Expanded(
          child: PeerCard(
              padding: const EdgeInsets.fromLTRB(10, 10, 10, 10),
              child: Row(children: [
                Container(
                    width: 40,
                    height: 40,
                    decoration: const BoxDecoration(
                        color: PeerTokens.mint, shape: BoxShape.circle),
                    child: const Icon(Icons.schedule_rounded,
                        color: PeerTokens.green)),
                const SizedBox(width: 8),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      Text(label,
                          style: const TextStyle(
                              fontSize: 12.5, color: PeerTokens.grey)),
                      DropdownButton<int>(
                          key: ValueKey(key),
                          value: value,
                          isExpanded: true,
                          isDense: true,
                          menuMaxHeight: 320,
                          underline: const SizedBox(),
                          items: [
                            for (final o in options)
                              DropdownMenuItem(
                                  value: o,
                                  child: Text(_clock(o),
                                      style: const TextStyle(
                                          fontSize: 16,
                                          fontWeight: FontWeight.w800,
                                          color: PeerTokens.navy))),
                          ],
                          onChanged: (v) {
                            if (v != null) onChanged(v);
                          }),
                    ])),
              ])));

  Widget _createBody(List<PeerSlot> existing) {
    final preview = _preview(existing);
    final starts = [for (var m = 0; m < 24 * 60; m += peerSlotMinutes) m];
    final ends = [for (var m = start; m <= 24 * 60; m += peerSlotMinutes) m];
    final presetHours = (end - start) % 60 == 0 ? (end - start) ~/ 60 : -1;
    Widget preset(int hours) {
      final on = presetHours == hours;
      return Expanded(
          child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 3),
              child: Material(
                  color: on ? const Color(0xFFE5F7EA) : Colors.white,
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(14),
                      side: BorderSide(
                          color: on ? PeerTokens.mintLine : PeerTokens.line)),
                  child: InkWell(
                      key: ValueKey('peer-preset-$hours'),
                      borderRadius: BorderRadius.circular(14),
                      onTap: () => setState(
                          () => end = (start + hours * 60).clamp(0, 24 * 60)),
                      child: Padding(
                          padding: const EdgeInsets.symmetric(vertical: 8),
                          child: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                if (on) ...[
                                  const Icon(Icons.check_circle_rounded,
                                      size: 18, color: PeerTokens.green),
                                  const SizedBox(width: 4),
                                ],
                                Flexible(
                                    child: FittedBox(
                                        fit: BoxFit.scaleDown,
                                        child: Column(children: [
                                          Text(
                                              '$hours Hour${hours == 1 ? '' : 's'}',
                                              style: const TextStyle(
                                                  fontSize: 13.5,
                                                  fontWeight: FontWeight.w800,
                                                  color: PeerTokens.navy)),
                                          Text(
                                              '(${hours * 60 ~/ peerSlotMinutes} slots)',
                                              style: const TextStyle(
                                                  fontSize: 12,
                                                  color: PeerTokens.grey)),
                                        ]))),
                              ]))))));
    }

    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
          padding: const EdgeInsets.fromLTRB(18, 22, 18, 10),
          child: Row(children: [
            const Expanded(
                child: Text('Set Availability',
                    style: TextStyle(
                        fontSize: 20,
                        fontWeight: FontWeight.w800,
                        color: PeerTokens.navy))),
            const Text('Slot Duration',
                style: TextStyle(fontSize: 14, color: PeerTokens.grey)),
            const SizedBox(width: 10),
            Container(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                decoration: BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.circular(14),
                    border: Border.all(color: PeerTokens.line)),
                child: DropdownButton<int>(
                    key: const ValueKey('peer-slot-duration'),
                    value: peerSlotMinutes,
                    underline: const SizedBox(),
                    items: const [
                      DropdownMenuItem(
                          value: peerSlotMinutes,
                          child: Text('10 min',
                              style: TextStyle(
                                  fontWeight: FontWeight.w700,
                                  color: PeerTokens.navy))),
                    ],
                    onChanged: (_) {})),
          ])),
      Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Row(children: [
            _timeCard('Start Time', start, starts, (v) {
              setState(() {
                final length = end - start;
                start = v;
                end = (v + (length > 0 ? length : 60)).clamp(0, 24 * 60);
              });
            }, 'peer-start-time'),
            const Padding(
                padding: EdgeInsets.symmetric(horizontal: 6),
                child: Text('—',
                    style: TextStyle(fontSize: 18, color: PeerTokens.grey))),
            _timeCard('End Time', end, ends, (v) => setState(() => end = v),
                'peer-end-time'),
          ])),
      // The slot preview strip was removed (product owner, 2026-11-15);
      // only explain why Create is unavailable when no slots would be made.
      if (preview.isEmpty)
        Padding(
            padding: const EdgeInsets.fromLTRB(18, 14, 18, 0),
            child: Text(
                end <= start
                    ? 'Choose an end time after the start time.'
                    : 'No new slots in this range (past times and existing slots are skipped).',
                key: const ValueKey('peer-no-slots-hint'),
                style: const TextStyle(color: PeerTokens.grey))),
      Padding(
          padding: const EdgeInsets.fromLTRB(16, 18, 16, 0),
          child: PeerCard(
              padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
              child: Row(children: [
                const Icon(Icons.bolt_rounded,
                    color: PeerTokens.green, size: 26),
                const SizedBox(width: 4),
                const Text('Quick\nPresets',
                    style: TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w700,
                        color: PeerTokens.navy)),
                const SizedBox(width: 6),
                preset(1),
                preset(2),
                preset(3),
              ]))),
      Padding(
          padding: const EdgeInsets.fromLTRB(16, 22, 16, 0),
          child: Row(children: [
            Expanded(
                flex: 3,
                child: SizedBox(
                    height: 54,
                    child: OutlinedButton.icon(
                        key: const ValueKey('peer-slots-clear'),
                        style: OutlinedButton.styleFrom(
                            foregroundColor: PeerTokens.navy,
                            side: const BorderSide(color: Color(0xFFCDD2D9)),
                            shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(16))),
                        onPressed: () => setState(() => end = start),
                        icon: const Icon(Icons.delete_outline_rounded),
                        label: const Text('Clear',
                            style: TextStyle(fontSize: 16))))),
            const SizedBox(width: 12),
            Expanded(
                flex: 6,
                child: PeerPrimaryButton(
                    key: const ValueKey('peer-slots-create'),
                    label:
                        'Create ${preview.length} Slot${preview.length == 1 ? '' : 's'}',
                    icon: Icons.add_box_outlined,
                    busy: busy,
                    onPressed:
                        preview.isEmpty ? null : () => _create(preview))),
          ])),
    ]);
  }

  // -------------------------------------------------------------------------
  // My Schedule
  // -------------------------------------------------------------------------

  Widget _scheduleBody((List<PeerSlot>, List<PeerInboxItem>) data) {
    final (slots, requests) = data;
    final sorted = [...slots]..sort((a, b) => a.start.compareTo(b.start));
    PeerInboxItem? requestFor(PeerSlot s) {
      for (final r in requests) {
        if (r.start.isAtSameMomentAs(s.start) &&
            (r.status == 'PENDING' ||
                r.status == 'ACCEPTED' ||
                r.status == 'COMPLETED')) {
          return r;
        }
      }
      return null;
    }

    int count(String status) => slots.where((s) => s.status == status).length;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
          padding: const EdgeInsets.fromLTRB(18, 22, 18, 10),
          child: Row(children: [
            Expanded(
                child: Text(
                    'My Slots for ${peerWeekdays[day.weekday - 1]}, ${day.day} ${peerMonths[day.month - 1]}',
                    style: const TextStyle(
                        fontSize: 19,
                        fontWeight: FontWeight.w800,
                        color: PeerTokens.navy))),
            OutlinedButton.icon(
                key: const ValueKey('peer-edit-slots'),
                style: OutlinedButton.styleFrom(
                    foregroundColor: PeerTokens.green,
                    side: const BorderSide(color: PeerTokens.green, width: 1.3),
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(14))),
                onPressed: () => setState(() => mode = 0),
                icon: const Icon(Icons.edit_outlined, size: 19),
                label: const Text('Edit Slots',
                    style: TextStyle(fontWeight: FontWeight.w800))),
          ])),
      if (sorted.isEmpty)
        const Padding(
            padding: EdgeInsets.fromLTRB(28, 18, 28, 18),
            child: Text(
                'No slots on this day yet. Use Create Slots to open times for patients to book.',
                textAlign: TextAlign.center,
                style: TextStyle(color: PeerTokens.grey, height: 1.4))),
      for (final s in sorted)
        Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
            child: _SlotRow(
                slot: s, request: requestFor(s), onRemove: () => _remove(s))),
      Padding(
          padding: const EdgeInsets.fromLTRB(16, 10, 16, 0),
          child: PeerCard(
              padding: const EdgeInsets.symmetric(vertical: 14, horizontal: 6),
              child: IntrinsicHeight(
                  child: Row(children: [
                _Stat(Icons.calendar_today_outlined, slots.length,
                    'Total Slots', PeerTokens.green),
                const VerticalDivider(color: PeerTokens.line),
                _Stat(Icons.check_circle_outline_rounded, count('AVAILABLE'),
                    'Available', PeerTokens.green),
                const VerticalDivider(color: PeerTokens.line),
                _Stat(Icons.event_available_outlined, count('BOOKED'), 'Booked',
                    PeerTokens.blueText),
                const VerticalDivider(color: PeerTokens.line),
                _Stat(Icons.people_outline_rounded, count('REQUESTED'),
                    'Requested', PeerTokens.amberText),
              ])))),
    ]);
  }
}

class _Hero extends StatelessWidget {
  const _Hero({required this.create});
  final bool create;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Container(
          height: 140,
          clipBehavior: Clip.antiAlias,
          decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(22),
              gradient: const LinearGradient(
                  colors: [Color(0xFFEDF8F1), Color(0xFFE2F4E8)])),
          child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Expanded(
                flex: 13,
                child: Padding(
                    padding: const EdgeInsets.fromLTRB(18, 12, 4, 12),
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Text(
                              create
                                  ? 'Create Your Support Slots'
                                  : 'Your Schedule',
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                  fontSize: 17,
                                  fontWeight: FontWeight.w800,
                                  color: PeerTokens.navy)),
                          const SizedBox(height: 6),
                          Flexible(
                              child: Text(
                                  create
                                      ? 'Set your availability for peer support sessions. Patients can book a slot and connect with you.'
                                      : 'View, manage and update your support slots. Patients can book from your available slots.',
                                  overflow: TextOverflow.fade,
                                  style: const TextStyle(
                                      fontSize: 12.5,
                                      height: 1.35,
                                      color: PeerTokens.grey))),
                        ]))),
            Padding(
                padding: const EdgeInsets.fromLTRB(0, 10, 10, 6),
                child: SizedBox(
                    width: 140,
                    child: Image.asset(
                        create
                            ? 'assets/peer/calendar_create.webp'
                            : 'assets/peer/calendar_schedule.webp',
                        fit: BoxFit.contain,
                        alignment: Alignment.bottomRight))),
          ])));
}

class _SlotRow extends StatelessWidget {
  const _SlotRow(
      {required this.slot, required this.request, required this.onRemove});
  final PeerSlot slot;
  final PeerInboxItem? request;
  final VoidCallback onRemove;
  @override
  Widget build(BuildContext context) {
    final (bg, tint, fg, chip, chipIcon, sub) = switch (slot.status) {
      'BOOKED' => (
          Colors.white,
          PeerTokens.blue,
          PeerTokens.blueText,
          'Booked',
          Icons.event_available_outlined,
          request == null ? 'Booked' : 'Booked by ${request!.patientName}'
        ),
      'REQUESTED' => (
          Colors.white,
          PeerTokens.amber,
          PeerTokens.amberText,
          'Requested',
          Icons.people_outline_rounded,
          request == null
              ? 'Waiting for your reply'
              : 'Request from ${request!.patientName}'
        ),
      _ => (
          const Color(0xFFF1FAF4),
          PeerTokens.mint,
          PeerTokens.green,
          'Available',
          Icons.circle,
          'Open for booking'
        ),
    };
    final available = slot.status == 'AVAILABLE';
    return PeerCard(
        key: ValueKey('peer-slot-${slot.id}'),
        color: bg,
        border: available ? const Color(0xFFD7EFDF) : PeerTokens.line,
        radius: 18,
        padding: const EdgeInsets.fromLTRB(14, 12, 10, 12),
        child: Row(children: [
          Container(
              width: 42,
              height: 42,
              decoration: BoxDecoration(color: tint, shape: BoxShape.circle),
              child: Icon(Icons.schedule_rounded, color: fg, size: 26)),
          const SizedBox(width: 14),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                FittedBox(
                    fit: BoxFit.scaleDown,
                    alignment: Alignment.centerLeft,
                    child: Text(
                        '${peerTime(slot.start)} – ${peerTime(slot.end)}',
                        maxLines: 1,
                        softWrap: false,
                        style: const TextStyle(
                            fontSize: 14.5,
                            fontWeight: FontWeight.w800,
                            color: PeerTokens.navy))),
                const SizedBox(height: 2),
                Text(sub,
                    style:
                        const TextStyle(fontSize: 13, color: PeerTokens.grey)),
              ])),
          Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
              decoration: BoxDecoration(
                  color: tint, borderRadius: BorderRadius.circular(18)),
              child: Row(mainAxisSize: MainAxisSize.min, children: [
                Icon(chipIcon, size: available ? 10 : 17, color: fg),
                const SizedBox(width: 6),
                Text(chip,
                    style: TextStyle(
                        fontSize: 12.5,
                        fontWeight: FontWeight.w800,
                        color: fg)),
              ])),
          const SizedBox(width: 10),
          available
              ? Material(
                  color: PeerTokens.redSoft,
                  borderRadius: BorderRadius.circular(12),
                  child: InkWell(
                      key: ValueKey('peer-remove-${slot.id}'),
                      borderRadius: BorderRadius.circular(12),
                      onTap: onRemove,
                      child: const Tooltip(
                          message: 'Remove slot',
                          child: SizedBox(
                              width: 42,
                              height: 42,
                              child: Icon(Icons.delete_outline_rounded,
                                  color: PeerTokens.red)))))
              : Container(
                  width: 42,
                  height: 42,
                  decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(color: PeerTokens.line)),
                  child: const Tooltip(
                      message: 'Booked or requested slots cannot be removed',
                      child: Icon(Icons.lock_outline_rounded,
                          color: PeerTokens.navy))),
        ]));
  }
}

class _Stat extends StatelessWidget {
  const _Stat(this.icon, this.value, this.label, this.color);
  final IconData icon;
  final int value;
  final String label;
  final Color color;
  @override
  Widget build(BuildContext context) => Expanded(
          child: Row(mainAxisAlignment: MainAxisAlignment.center, children: [
        Icon(icon, color: color, size: 24),
        const SizedBox(width: 6),
        Flexible(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('$value',
              style: const TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.w800,
                  color: PeerTokens.navy)),
          Text(label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 12, color: PeerTokens.grey)),
        ])),
      ]));
}
