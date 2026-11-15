import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../l10n/app_localizations.dart';
import 'facility_models.dart';
import 'facility_repository.dart';

/// Colours sampled from `docs/design/facilities/facility-services-flow.png`.
class FacColors {
  static const ink = Color(0xFF0E1A36); // titles, names
  static const body = Color(0xFF3B465C);
  static const muted = Color(0xFF6B7287); // "Testing · PrEP · ART"
  static const green = Color(0xFF0A7A2A); // pill and badge text
  static const chipOn = Color(0xFF0B9344); // selected chip
  static const pillBg = Color(0xFFE2F8F0); // Get Directions pill, row icons
  static const youthBg = Color(0xFFCEF8E0);
  static const field = Color(0xFFF1F4F8); // search field
  static const card = Color(0xFFF5F7F9); // Services Available card
  static const divider = Color(0xFFE6EAEE);
  static const open = Color(0xFF0B8A3B);
  static const closed = Color(0xFFB4232F);
}

/// "8:00 AM" from a facility-local "HH:MM" (no time-zone conversion: the
/// value is already in the facility's time zone).
String facilityTime(BuildContext context, String hhmm) {
  final parts = hhmm.split(':');
  final h = int.tryParse(parts.first) ?? 0;
  final m = int.tryParse(parts.length > 1 ? parts[1] : '0') ?? 0;
  // intl puts a narrow no-break space before AM/PM; use a plain space so the
  // label wraps and reads like the rest of the app.
  return DateFormat.jm(Localizations.localeOf(context).toLanguageTag())
      .format(DateTime(2024, 1, 1, h % 24, m))
      .replaceAll(' ', ' ');
}

/// Localized weekday name for MON…SUN (2024-01-01 is a Monday).
String facilityWeekdayName(BuildContext context, String code) {
  final index = facilityWeekdays.indexOf(code);
  if (index < 0) return code;
  return DateFormat.EEEE(Localizations.localeOf(context).toLanguageTag())
      .format(DateTime(2024, 1, 1 + index));
}

String facilityStatusText(BuildContext context, FacilityStatus s) {
  final l = AppLocalizations.of(context);
  final time = s.nextChangeLocalTime == null
      ? null
      : facilityTime(context, s.nextChangeLocalTime!);
  switch (s.state) {
    case FacilityOpenState.open24Hours:
      return l.facilitiesOpen24;
    case FacilityOpenState.hoursNotAvailable:
      return l.facilitiesHoursNotAvailable;
    case FacilityOpenState.open:
      if (time == null) return l.facilitiesOpen;
      if ((s.nextChangeDayOffset ?? 0) == 0) {
        return l.facilitiesOpenCloses(time);
      }
      return l.facilitiesOpenClosesOn(
          facilityWeekdayName(context, s.nextChangeWeekday ?? ''), time);
    case FacilityOpenState.closed:
      if (time == null) return l.facilitiesClosed;
      return switch (s.nextChangeDayOffset) {
        0 => l.facilitiesClosedOpensToday(time),
        1 => l.facilitiesClosedOpensTomorrow(time),
        _ => l.facilitiesClosedOpensOn(
            facilityWeekdayName(context, s.nextChangeWeekday ?? ''), time),
      };
  }
}

class FacilityStatusLine extends StatelessWidget {
  const FacilityStatusLine({super.key, required this.status, this.size = 12.5});
  final FacilityStatus status;
  final double size;
  @override
  Widget build(BuildContext context) {
    final color = switch (status.state) {
      FacilityOpenState.open || FacilityOpenState.open24Hours => FacColors.open,
      FacilityOpenState.closed => FacColors.closed,
      FacilityOpenState.hoursNotAvailable => FacColors.muted,
    };
    return Row(mainAxisSize: MainAxisSize.min, children: [
      Icon(Icons.schedule_rounded, size: size + 3, color: color),
      const SizedBox(width: 5),
      Flexible(
        child: Text(facilityStatusText(context, status),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
                fontSize: size, color: color, fontWeight: FontWeight.w600)),
      ),
    ]);
  }
}

/// Admin-uploaded photo; a quiet placeholder (never a broken-image icon)
/// while loading, when absent, or on error.
class FacilityPhoto extends StatefulWidget {
  const FacilityPhoto(
      {super.key,
      required this.facility,
      required this.repo,
      required this.width,
      required this.height,
      this.radius = 12});
  final FacilityDto facility;
  final FacilityRepository repo;
  final double width, height, radius;
  @override
  State<FacilityPhoto> createState() => _FacilityPhotoState();
}

class _FacilityPhotoState extends State<FacilityPhoto> {
  Future<Uint8List>? _bytes;
  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(covariant FacilityPhoto old) {
    super.didUpdateWidget(old);
    if (old.facility.imagePath != widget.facility.imagePath) _load();
  }

  void _load() {
    final path = widget.facility.imagePath;
    _bytes = path == null ? null : widget.repo.photo(path);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final placeholder = Container(
      width: widget.width,
      height: widget.height,
      color: const Color(0xFFE8F3EE),
      alignment: Alignment.center,
      child: Icon(Icons.local_hospital_outlined,
          color: FacColors.open.withValues(alpha: .45),
          size: widget.height * .36),
    );
    final cached = widget.facility.imagePath == null
        ? null
        : widget.repo.cachedPhoto(widget.facility.imagePath!);
    Widget image(Uint8List bytes) => Image.memory(bytes,
        width: widget.width,
        height: widget.height,
        fit: BoxFit.cover,
        gaplessPlayback: true,
        semanticLabel: l.facilitiesPhotoOf(widget.facility.name),
        errorBuilder: (_, __, ___) => placeholder);
    return ClipRRect(
      borderRadius: BorderRadius.circular(widget.radius),
      child: cached != null
          ? image(cached)
          : _bytes == null
              ? placeholder
              : FutureBuilder<Uint8List>(
                  future: _bytes,
                  builder: (context, s) =>
                      s.hasData ? image(s.data!) : placeholder),
    );
  }
}

/// Icon for a service category key; unknown keys get a generic icon so new
/// categories added by Admin need no app update.
({IconData icon, Color color, Color bg}) facilityServiceIcon(String key) =>
    switch (key) {
      'testing' => (
          icon: Icons.science_outlined,
          color: const Color(0xFFE5484D),
          bg: const Color(0xFFFDECEC)
        ),
      'prep' => (
          icon: Icons.medication_outlined,
          color: const Color(0xFF2F7DE1),
          bg: const Color(0xFFE8F1FD)
        ),
      'art' => (
          icon: Icons.medical_services_outlined,
          color: const Color(0xFFF07A2B),
          bg: const Color(0xFFFFF0E5)
        ),
      'counselling' => (
          icon: Icons.groups_outlined,
          color: const Color(0xFF6E56CF),
          bg: const Color(0xFFF0EDFC)
        ),
      _ => (
          icon: Icons.health_and_safety_outlined,
          color: FacColors.open,
          bg: FacColors.pillBg
        ),
    };

/// "+254 712 345 678" for display; dialling uses the raw E.164 number.
String facilityPhoneDisplay(String e164) {
  final m = RegExp(r'^\+254(\d{3})(\d{3})(\d{3})$').firstMatch(e164);
  return m == null ? e164 : '+254 ${m[1]} ${m[2]} ${m[3]}';
}

/// Location, plus the county only when the location text does not already
/// name it ("Siaya town, Kenya" rather than "Siaya town, Kenya, Siaya").
String facilityPlace(FacilityDto f) {
  if (f.county.isEmpty ||
      f.location.toLowerCase().contains(f.county.toLowerCase())) {
    return f.location;
  }
  return f.location.isEmpty ? f.county : '${f.location}, ${f.county}';
}

/// The Admin-entered Google Maps link, shown under Get Directions.
class FacilityLinkText extends StatelessWidget {
  const FacilityLinkText(
      {super.key, required this.uri, this.size = 13, this.maxLines = 1});
  final Uri uri;
  final double size;
  final int maxLines;
  @override
  Widget build(BuildContext context) => Text(uri.toString(),
      maxLines: maxLines,
      overflow: TextOverflow.ellipsis,
      style: TextStyle(
          fontSize: size,
          color: FacColors.green,
          decoration: TextDecoration.underline,
          decorationColor: FacColors.green));
}

/// Short service label for cards: "Testing · PrEP · ART".
String facilityServiceSummary(BuildContext context, FacilityDto f) {
  final language = Localizations.localeOf(context).languageCode;
  return f.services
      .map((s) => s.nameFor(language).split(' (').first)
      .join(' · ');
}

Future<void> facilityLaunch(BuildContext context, Uri uri) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final failed = AppLocalizations.of(context).facilitiesActionFailed;
  var ok = false;
  try {
    ok = await launchUrl(uri, mode: LaunchMode.externalApplication);
    // No maps or browser app answered: let the platform choose (browser).
    if (!ok && uri.scheme == 'https') {
      ok = await launchUrl(uri, mode: LaunchMode.platformDefault);
    }
  } catch (_) {}
  if (!ok) messenger?.showSnackBar(SnackBar(content: Text(failed)));
}

/// Opens the device dialer with the number filled in; never places the call.
Future<void> facilityCall(BuildContext context, String phone) =>
    facilityLaunch(context, Uri(scheme: 'tel', path: phone));

class FacilityCard extends StatelessWidget {
  const FacilityCard(
      {super.key,
      required this.facility,
      required this.repo,
      required this.onOpen});
  final FacilityDto facility;
  final FacilityRepository repo;
  final VoidCallback onOpen;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final f = facility;
    return Semantics(
      button: true,
      label: l.facilitiesOpenDetails(f.name),
      child: InkWell(
        onTap: onOpen,
        borderRadius: BorderRadius.circular(14),
        child: Container(
          padding: const EdgeInsets.symmetric(vertical: 12),
          decoration: const BoxDecoration(
              border: Border(bottom: BorderSide(color: FacColors.divider))),
          child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            FacilityPhoto(facility: f, repo: repo, width: 92, height: 82),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(f.name,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            fontSize: 15.5,
                            height: 1.2,
                            fontWeight: FontWeight.w800,
                            color: FacColors.ink)),
                    const SizedBox(height: 4),
                    Text(facilityServiceSummary(context, f),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            fontSize: 13, color: FacColors.muted)),
                    if (f.youthFriendly) ...[
                      const SizedBox(height: 4),
                      Text(l.facilitiesYouthFriendly,
                          style: const TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w700,
                              color: FacColors.green)),
                    ],
                    const SizedBox(height: 6),
                    FacilityStatusLine(status: f.status),
                    const SizedBox(height: 8),
                    Wrap(
                        spacing: 8,
                        runSpacing: 6,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          Row(mainAxisSize: MainAxisSize.min, children: [
                            const Icon(Icons.location_on_outlined,
                                size: 15, color: FacColors.body),
                            const SizedBox(width: 3),
                            ConstrainedBox(
                              constraints: const BoxConstraints(maxWidth: 150),
                              child: Text(f.location,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: const TextStyle(
                                      fontSize: 12.5, color: FacColors.body)),
                            ),
                          ]),
                        ]),
                    if (f.mapsUri != null) ...[
                      const SizedBox(height: 8),
                      Material(
                        color: FacColors.pillBg,
                        borderRadius: BorderRadius.circular(20),
                        child: InkWell(
                          borderRadius: BorderRadius.circular(20),
                          onTap: () => facilityLaunch(context, f.mapsUri!),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 12, vertical: 6),
                            child: Text(l.facilitiesGetDirections,
                                style: const TextStyle(
                                    fontSize: 12.5,
                                    fontWeight: FontWeight.w700,
                                    color: FacColors.green)),
                          ),
                        ),
                      ),
                      const SizedBox(height: 4),
                      // The Admin's directions link, shown as given.
                      FacilityLinkText(uri: f.mapsUri!, size: 11.5),
                    ],
                  ]),
            ),
            const Padding(
              padding: EdgeInsets.only(top: 22),
              child: Icon(Icons.chevron_right_rounded,
                  size: 26, color: FacColors.ink),
            ),
          ]),
        ),
      ),
    );
  }
}

class FacilityEmptyState extends StatelessWidget {
  const FacilityEmptyState({super.key, required this.filtered});

  /// True when a search or filter produced no results.
  final bool filtered;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 18),
      child: Column(children: [
        Image.asset('assets/facilities/facilities_empty.webp',
            height: 170,
            fit: BoxFit.contain,
            filterQuality: FilterQuality.medium,
            excludeFromSemantics: true),
        const SizedBox(height: 14),
        Text(filtered ? l.facilitiesNoMatchTitle : l.facilitiesEmptyTitle,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 21,
                fontWeight: FontWeight.w800,
                color: FacColors.ink)),
        const SizedBox(height: 8),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12),
          child: Text(
              filtered ? l.facilitiesNoMatchBody : l.facilitiesEmptyBody,
              textAlign: TextAlign.center,
              style: const TextStyle(
                  fontSize: 14.5, height: 1.45, color: FacColors.body)),
        ),
      ]),
    );
  }
}
