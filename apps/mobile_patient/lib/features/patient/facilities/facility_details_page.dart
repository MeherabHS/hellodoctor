import 'dart:async';

import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';
import '../modules/patient_modules.dart';
import 'facility_models.dart';
import 'facility_repository.dart';
import 'facility_widgets.dart';

/// Facility Details (reference: right panel). Shows only Admin-published data:
/// photo, name, Youth Friendly, location, server-computed status, services,
/// call (dialer), directions (the Admin's Google Maps link), weekly hours and
/// About. No distance, rating or invented hours. Bookmark and Share from the
/// reference are not shown (no bookmark store; facilities need sign-in).
class FacilityDetailsPage extends StatefulWidget {
  const FacilityDetailsPage(
      {super.key,
      required this.facility,
      required this.repo,
      required this.nav});
  final FacilityDto facility;
  final FacilityRepository repo;
  final PatientShellNav nav;
  @override
  State<FacilityDetailsPage> createState() => _FacilityDetailsPageState();
}

class _FacilityDetailsPageState extends State<FacilityDetailsPage> {
  late FacilityDto _f = widget.facility;
  Timer? _statusTimer;

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  @override
  void dispose() {
    _statusTimer?.cancel();
    super.dispose();
  }

  Future<void> _refresh() async {
    try {
      final fresh = await widget.repo.detail(_f.id);
      if (!mounted) return;
      setState(() => _f = fresh);
    } catch (_) {
      // Keep the list's copy; it is still published data.
    }
    _schedule();
  }

  void _schedule() {
    _statusTimer?.cancel();
    final at = _f.status.nextChangeAt;
    final now = DateTime.now().toUtc();
    if (at == null || !at.isAfter(now)) return;
    var wait = at.difference(now) + const Duration(seconds: 5);
    if (wait > const Duration(hours: 1)) wait = const Duration(hours: 1);
    _statusTimer = Timer(wait, _refresh);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final language = Localizations.localeOf(context).languageCode;
    final f = _f;
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(6, 6, 16, 4),
            child: Row(children: [
              IconButton(
                  onPressed: () => Navigator.of(context).maybePop(),
                  tooltip: MaterialLocalizations.of(context).backButtonTooltip,
                  icon: const Icon(Icons.arrow_back_rounded,
                      size: 24, color: FacColors.ink)),
              const SizedBox(width: 4),
              Expanded(
                child: Text(l.facilitiesDetailsTitle,
                    style: const TextStyle(
                        fontSize: 20,
                        fontWeight: FontWeight.w800,
                        color: FacColors.ink)),
              ),
            ]),
          ),
          Expanded(
            child: RefreshIndicator(
              onRefresh: _refresh,
              child: ListView(
                physics: const AlwaysScrollableScrollPhysics(),
                padding: const EdgeInsets.fromLTRB(18, 4, 18, 28),
                children: [
                  LayoutBuilder(
                      builder: (context, c) => FacilityPhoto(
                          facility: f,
                          repo: widget.repo,
                          width: c.maxWidth,
                          height: c.maxWidth * 9 / 20,
                          radius: 16)),
                  const SizedBox(height: 14),
                  Text(f.name,
                      style: const TextStyle(
                          fontSize: 24,
                          height: 1.15,
                          fontWeight: FontWeight.w800,
                          color: FacColors.ink)),
                  if (f.youthFriendly) ...[
                    const SizedBox(height: 8),
                    Align(
                      alignment: Alignment.centerLeft,
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 12, vertical: 4),
                        decoration: BoxDecoration(
                            color: FacColors.youthBg,
                            borderRadius: BorderRadius.circular(20)),
                        child: Text(l.facilitiesYouthFriendly,
                            style: const TextStyle(
                                fontSize: 13,
                                fontWeight: FontWeight.w700,
                                color: FacColors.green)),
                      ),
                    ),
                  ],
                  const SizedBox(height: 10),
                  Wrap(
                      spacing: 14,
                      runSpacing: 6,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        Row(mainAxisSize: MainAxisSize.min, children: [
                          const Icon(Icons.location_on_outlined,
                              size: 18, color: FacColors.body),
                          const SizedBox(width: 4),
                          Flexible(
                            child: Text(facilityPlace(f),
                                style: const TextStyle(
                                    fontSize: 14, color: FacColors.body)),
                          ),
                        ]),
                        FacilityStatusLine(status: f.status, size: 14),
                      ]),
                  const SizedBox(height: 16),
                  if (f.services.isNotEmpty)
                    _Section(
                      title: l.facilitiesServicesAvailable,
                      tinted: true,
                      child: Wrap(
                        spacing: 6,
                        runSpacing: 12,
                        alignment: WrapAlignment.spaceAround,
                        children: [
                          for (final s in f.services)
                            SizedBox(
                              // Grows with the text size so labels such as
                              // "ART (Treatment)" break between words.
                              width: MediaQuery.textScalerOf(context)
                                  .scale(76)
                                  .clamp(76.0, 124.0),
                              child: Column(children: [
                                Builder(builder: (context) {
                                  final i = facilityServiceIcon(s.iconKey);
                                  return Container(
                                    width: 48,
                                    height: 48,
                                    decoration: BoxDecoration(
                                        color: i.bg,
                                        shape: BoxShape.circle,
                                        border: Border.all(
                                            color: Colors.white, width: 2)),
                                    child:
                                        Icon(i.icon, color: i.color, size: 24),
                                  );
                                }),
                                const SizedBox(height: 6),
                                Text(s.nameFor(language),
                                    textAlign: TextAlign.center,
                                    style: const TextStyle(
                                        fontSize: 12.5,
                                        height: 1.25,
                                        color: FacColors.ink)),
                              ]),
                            ),
                        ],
                      ),
                    ),
                  const SizedBox(height: 12),
                  Container(
                    decoration: BoxDecoration(
                        color: Colors.white,
                        borderRadius: BorderRadius.circular(14),
                        border: Border.all(color: FacColors.divider)),
                    child: Column(children: [
                      if (f.phone.isNotEmpty)
                        _ActionRow(
                            icon: Icons.call_outlined,
                            title: l.facilitiesCallFacility,
                            subtitle: facilityPhoneDisplay(f.phone),
                            onTap: () => facilityCall(context, f.phone)),
                      if (f.phone.isNotEmpty && f.mapsUri != null)
                        const Divider(
                            height: 1, indent: 70, color: FacColors.divider),
                      if (f.mapsUri != null)
                        _ActionRow(
                            icon: Icons.place_outlined,
                            title: l.facilitiesGetDirections,
                            subtitle: '',
                            // The Admin's directions link, shown as given.
                            below:
                                FacilityLinkText(uri: f.mapsUri!, maxLines: 2),
                            onTap: () => facilityLaunch(context, f.mapsUri!)),
                    ]),
                  ),
                  const SizedBox(height: 12),
                  _Section(
                    title: l.facilitiesOpeningHours,
                    child: f.hours == null
                        ? Text(l.facilitiesHoursNotAvailable,
                            style: const TextStyle(color: FacColors.muted))
                        : Column(children: [
                            for (final day in facilityWeekdays)
                              _HoursRow(
                                  day: day,
                                  hours: f.hours![day],
                                  today: f.status.today == day),
                          ]),
                  ),
                  if (f.description.trim().isNotEmpty) ...[
                    const SizedBox(height: 12),
                    _Section(
                      title: l.facilitiesAbout,
                      child: Text(f.description,
                          style: const TextStyle(
                              fontSize: 14.5,
                              height: 1.45,
                              color: FacColors.body)),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ]),
      ),
    );
  }
}

class _Section extends StatelessWidget {
  const _Section(
      {required this.title, required this.child, this.tinted = false});
  final String title;
  final Widget child;
  final bool tinted;
  @override
  Widget build(BuildContext context) => Container(
        width: double.infinity,
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
        decoration: BoxDecoration(
            color: tinted ? FacColors.card : Colors.white,
            borderRadius: BorderRadius.circular(14),
            border: Border.all(color: FacColors.divider)),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              style: const TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.w800,
                  color: FacColors.ink)),
          const SizedBox(height: 12),
          child,
        ]),
      );
}

class _ActionRow extends StatelessWidget {
  const _ActionRow(
      {required this.icon,
      required this.title,
      required this.subtitle,
      required this.onTap,
      this.below});
  final IconData icon;
  final String title, subtitle;
  final VoidCallback onTap;
  final Widget? below;
  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(14),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 12, 8, 12),
          child: Row(children: [
            Container(
              width: 44,
              height: 44,
              decoration: const BoxDecoration(
                  color: FacColors.pillBg, shape: BoxShape.circle),
              child: Icon(icon, color: FacColors.green),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: const TextStyle(
                            fontSize: 15.5,
                            fontWeight: FontWeight.w800,
                            color: FacColors.ink)),
                    if (subtitle.isNotEmpty) ...[
                      const SizedBox(height: 2),
                      Text(subtitle,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 14, color: FacColors.body)),
                    ],
                    if (below != null) ...[
                      const SizedBox(height: 2),
                      below!,
                    ],
                  ]),
            ),
            const Icon(Icons.chevron_right_rounded,
                size: 26, color: FacColors.ink),
          ]),
        ),
      );
}

class _HoursRow extends StatelessWidget {
  const _HoursRow(
      {required this.day, required this.hours, required this.today});
  final String day;
  final FacilityDayHours? hours;
  final bool today;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final h = hours;
    final text = h == null || h.closed
        ? l.facilitiesDayClosed
        : h.allDay
            ? l.facilitiesDayAllDay
            : h.periods
                .map((p) =>
                    '${facilityTime(context, p.open)} – ${facilityTime(context, p.close)}'
                    '${p.overnight ? ' ${l.facilitiesNextDay}' : ''}')
                .join('\n');
    final style = TextStyle(
        fontSize: 14,
        height: 1.35,
        fontWeight: today ? FontWeight.w800 : FontWeight.w500,
        color: today ? FacColors.green : FacColors.body);
    return Container(
      margin: const EdgeInsets.only(bottom: 4),
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      decoration: today
          ? BoxDecoration(
              color: FacColors.pillBg, borderRadius: BorderRadius.circular(8))
          : null,
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Expanded(
          flex: 4,
          child: Text(
              today
                  ? '${facilityWeekdayName(context, day)} · ${l.facilitiesToday}'
                  : facilityWeekdayName(context, day),
              style: style),
        ),
        Expanded(
            flex: 5, child: Text(text, textAlign: TextAlign.end, style: style)),
      ]),
    );
  }
}
