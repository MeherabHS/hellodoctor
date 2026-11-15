/// Facilities & Services: published facilities from `GET /facilities`.
/// The server computes open/closed status from its own clock in the facility's
/// time zone; the app only formats it.
typedef Json = Map<String, dynamic>;

const facilityWeekdays = ['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN'];

class FacilityService {
  const FacilityService(
      {required this.code,
      required this.nameEn,
      required this.nameSw,
      required this.iconKey});
  final String code, nameEn, nameSw, iconKey;
  String nameFor(String language) =>
      language == 'sw' && nameSw.isNotEmpty ? nameSw : nameEn;
  factory FacilityService.fromJson(Json j) => FacilityService(
      code: '${j['code'] ?? ''}',
      nameEn: '${j['name_en'] ?? ''}',
      nameSw: '${j['name_sw'] ?? ''}',
      iconKey: '${j['icon_key'] ?? ''}');
}

class FacilityPeriod {
  const FacilityPeriod(this.open, this.close, {this.overnight = false});
  final String open, close;
  final bool overnight;
}

class FacilityDayHours {
  const FacilityDayHours({this.allDay = false, this.periods = const []});
  final bool allDay;
  final List<FacilityPeriod> periods;
  bool get closed => !allDay && periods.isEmpty;
}

enum FacilityOpenState { open, closed, open24Hours, hoursNotAvailable }

class FacilityStatus {
  const FacilityStatus({
    required this.state,
    required this.today,
    this.nextChangeAt,
    this.nextChangeLocalTime,
    this.nextChangeDayOffset,
    this.nextChangeWeekday,
  });
  final FacilityOpenState state;

  /// Facility-local weekday now (MON…SUN), for highlighting the schedule.
  final String today;
  final DateTime? nextChangeAt;

  /// Facility-local "HH:MM".
  final String? nextChangeLocalTime;
  final int? nextChangeDayOffset;
  final String? nextChangeWeekday;

  static FacilityStatus parse(dynamic raw) {
    final j = raw is Map ? Json.from(raw) : const <String, dynamic>{};
    final state = switch (j['state']) {
      'OPEN' => FacilityOpenState.open,
      'CLOSED' => FacilityOpenState.closed,
      'OPEN_24_HOURS' => FacilityOpenState.open24Hours,
      _ => FacilityOpenState.hoursNotAvailable,
    };
    final time = j['next_change_local_time'];
    return FacilityStatus(
      state: state,
      today: facilityWeekdays.contains(j['today']) ? j['today'] as String : '',
      nextChangeAt: DateTime.tryParse('${j['next_change_at'] ?? ''}'),
      nextChangeLocalTime:
          time is String && RegExp(r'^\d{2}:\d{2}$').hasMatch(time)
              ? time
              : null,
      nextChangeDayOffset: j['next_change_day_offset'] is int
          ? j['next_change_day_offset'] as int
          : null,
      nextChangeWeekday: facilityWeekdays.contains(j['next_change_weekday'])
          ? j['next_change_weekday'] as String
          : null,
    );
  }
}

/// Only the official Google Maps hosts the backend accepts; anything else is
/// dropped so the app can never open an unexpected site.
Uri? safeMapsUri(dynamic raw) {
  if (raw is! String) return null;
  final uri = Uri.tryParse(raw);
  if (uri == null || uri.scheme != 'https' || uri.userInfo.isNotEmpty) {
    return null;
  }
  if (uri.hasPort && uri.port != 443) return null;
  final host = uri.host.toLowerCase();
  final path = uri.path;
  final ok = switch (host) {
    'maps.app.goo.gl' => path.length > 1,
    'maps.google.com' || 'maps.google.co.ke' => true,
    'google.com' ||
    'www.google.com' ||
    'google.co.ke' ||
    'www.google.co.ke' =>
      path == '/maps' || path.startsWith('/maps/'),
    _ => false,
  };
  return ok ? uri : null;
}

class FacilityDto {
  const FacilityDto({
    required this.id,
    required this.name,
    required this.description,
    required this.county,
    required this.location,
    required this.phone,
    required this.mapsUri,
    required this.services,
    required this.youthFriendly,
    required this.hours,
    required this.status,
    required this.imagePath,
  });
  final String id, name, description, county, location, phone;
  final Uri? mapsUri;
  final List<FacilityService> services;
  final bool youthFriendly;

  /// Null = opening hours not provided.
  final Map<String, FacilityDayHours>? hours;
  final FacilityStatus status;

  /// API path of the photo (`/facilities/<id>/image?v=<media>`), or null.
  final String? imagePath;

  factory FacilityDto.fromJson(Json j) {
    Map<String, FacilityDayHours>? hours;
    final rawHours = j['opening_hours'];
    if (rawHours is Map) {
      hours = {};
      for (final day in facilityWeekdays) {
        final d = rawHours[day];
        if (d is! Map) continue;
        final periods = <FacilityPeriod>[];
        for (final p in (d['periods'] as List? ?? const [])) {
          if (p is Map && p['open'] is String && p['close'] is String) {
            periods.add(FacilityPeriod(
                p['open'] as String, p['close'] as String,
                overnight: p['overnight'] == true));
          }
        }
        hours[day] =
            FacilityDayHours(allDay: d['all_day'] == true, periods: periods);
      }
    }
    final image = j['image_url'];
    return FacilityDto(
      id: '${j['id'] ?? ''}',
      name: '${j['name'] ?? ''}',
      description: '${j['description'] ?? ''}',
      county: '${j['county'] ?? ''}',
      location: '${j['location_text'] ?? ''}',
      phone: '${j['contact_phone'] ?? ''}',
      mapsUri: safeMapsUri(j['google_maps_url']),
      services: [
        for (final s in (j['services'] as List? ?? const []))
          if (s is Map) FacilityService.fromJson(Json.from(s)),
      ],
      youthFriendly: j['youth_friendly'] == true,
      hours: hours,
      status: FacilityStatus.parse(j['status']),
      imagePath: image is String &&
              RegExp(r'^/facilities/[0-9a-fA-F-]{36}/image(\?v=[0-9a-fA-F-]{36})?$')
                  .hasMatch(image)
          ? image
          : null,
    );
  }
}

class FacilityPage {
  const FacilityPage(this.items, this.total, this.nextOffset);
  final List<FacilityDto> items;
  final int total;
  final int? nextOffset;
}

/// What the directory is filtered by: search text and the Testing,
/// PrEP/ART and Youth Friendly chips. No location filter (product owner,
/// 2026-11-13).
class FacilityFilter {
  const FacilityFilter(
      {this.search = '',
      this.testing = false,
      this.prepArt = false,
      this.youthFriendly = false});
  final String search;
  final bool testing, prepArt, youthFriendly;
  bool get isFiltered =>
      search.trim().isNotEmpty || testing || prepArt || youthFriendly;
  List<String> get serviceCodes => [
        if (testing) 'TESTING',
        if (prepArt) ...['PREP', 'ART'],
      ];
  FacilityFilter copyWith(
          {String? search,
          bool? testing,
          bool? prepArt,
          bool? youthFriendly}) =>
      FacilityFilter(
          search: search ?? this.search,
          testing: testing ?? this.testing,
          prepArt: prepArt ?? this.prepArt,
          youthFriendly: youthFriendly ?? this.youthFriendly);
}
