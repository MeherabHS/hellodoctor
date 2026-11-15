import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/facilities/facility_details_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/facilities/facility_directory_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/facilities/facility_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/facilities/facility_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/facilities/facility_widgets.dart';
import 'package:hellodoctor_mobile_patient/features/patient/modules/patient_modules.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class _Tokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

const _id1 = '11111111-1111-4111-8111-111111111111';
const _id2 = '22222222-2222-4222-8222-222222222222';

Map<String, dynamic> _json(String id, String name,
        {Map<String, dynamic>? status,
        bool youth = true,
        String maps = 'https://www.google.com/maps/place/Siaya',
        Object? hours = const {
          'MON': {
            'periods': [
              {'open': '08:00', 'close': '17:00'}
            ]
          },
          'SAT': {'all_day': true}
        }}) =>
    {
      'id': id,
      'name': name,
      'description': 'A county hospital offering youth-friendly HIV testing.',
      'county': 'Siaya',
      'location_text': 'Siaya town',
      'contact_phone': '+254712345678',
      'google_maps_url': maps,
      'services': [
        {
          'code': 'TESTING',
          'name_en': 'Testing (HIV/STIs)',
          'name_sw': 'Upimaji',
          'icon_key': 'testing'
        },
        {
          'code': 'PREP',
          'name_en': 'PrEP',
          'name_sw': 'PrEP',
          'icon_key': 'prep'
        },
        {
          'code': 'ART',
          'name_en': 'ART (Treatment)',
          'name_sw': 'ART',
          'icon_key': 'art'
        },
      ],
      'youth_friendly': youth,
      'opening_hours': hours,
      'status': status ??
          {
            'state': 'OPEN',
            'today': 'MON',
            'next_change_at': '2099-01-01T14:00:00Z',
            'next_change_local_time': '17:00',
            'next_change_day_offset': 0,
            'next_change_weekday': 'MON'
          },
      'image_url': null,
    };

class _Repo extends FacilityRepository {
  _Repo(this.pages, {this.fail = false})
      : super(ApiClient(
            dio: Dio(BaseOptions(baseUrl: 'http://127.0.0.1:1/api/v1')),
            tokenStore: _Tokens()));
  final List<Map<String, dynamic>> pages;
  bool fail;
  final filters = <FacilityFilter>[];
  @override
  Future<FacilityPage> list(FacilityFilter filter,
      {int offset = 0, int limit = 20}) async {
    filters.add(filter);
    if (fail) throw Exception('offline');
    final rows = filter.isFiltered && filter.search == 'nowhere'
        ? <Map<String, dynamic>>[]
        : pages;
    return FacilityPage(
        rows.map(FacilityDto.fromJson).toList(), rows.length, null);
  }

  @override
  Future<FacilityDto> detail(String id) async =>
      FacilityDto.fromJson(pages.firstWhere((p) => p['id'] == id));
  @override
  Future<Uint8List> photo(String path) async => Uint8List(0);
}

const _nav = PatientShellNav(
    openServices: _noop, openMessages: _noop, openProfile: _noop, select: _sel);
void _noop() {}
void _sel(int _) {}

Widget _host(Widget child, {String language = 'en'}) => MaterialApp(
    locale: Locale(language),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: child);

void main() {
  group('model', () {
    test('only official Google Maps https links survive parsing', () {
      expect(
          safeMapsUri('https://maps.app.goo.gl/abc')?.host, 'maps.app.goo.gl');
      for (final bad in [
        'http://www.google.com/maps/x',
        'https://evil.example/maps',
        'https://www.google.com.evil.example/maps',
        'https://user@www.google.com/maps',
        'javascript:alert(1)',
        'https://www.google.com/search?q=maps',
      ]) {
        expect(safeMapsUri(bad), isNull, reason: bad);
      }
    });

    test('image path must be the API path, never a storage URL', () {
      final ok = FacilityDto.fromJson({
        ..._json(_id1, 'A'),
        'image_url': '/facilities/$_id1/image?v=$_id2'
      });
      expect(ok.imagePath, '/facilities/$_id1/image?v=$_id2');
      final bad = FacilityDto.fromJson(
          {..._json(_id1, 'A'), 'image_url': 'https://storage.example/x.jpg'});
      expect(bad.imagePath, isNull);
    });

    test('hours: missing means not provided; days parse', () {
      expect(FacilityDto.fromJson(_json(_id1, 'A', hours: null)).hours, isNull);
      final h = FacilityDto.fromJson(_json(_id1, 'A')).hours!;
      expect(h['SAT']!.allDay, isTrue);
      expect(h['MON']!.periods.single.close, '17:00');
      expect(h['TUE'], isNull);
    });
  });

  group('status text', () {
    Future<String> textFor(
        WidgetTester tester, Map<String, dynamic> status) async {
      late String out;
      await tester.pumpWidget(_host(Builder(builder: (context) {
        out = facilityStatusText(context, FacilityStatus.parse(status));
        return const SizedBox();
      })));
      return out;
    }

    testWidgets('each state reads as designed', (tester) async {
      expect(
          await textFor(tester, {
            'state': 'OPEN',
            'next_change_local_time': '17:00',
            'next_change_day_offset': 0
          }),
          'Open · Closes 5:00 PM');
      expect(
          await textFor(tester, {
            'state': 'CLOSED',
            'next_change_local_time': '08:00',
            'next_change_day_offset': 1,
            'next_change_weekday': 'TUE'
          }),
          'Closed · Opens tomorrow at 8:00 AM');
      expect(
          await textFor(tester, {
            'state': 'CLOSED',
            'next_change_local_time': '08:00',
            'next_change_day_offset': 3,
            'next_change_weekday': 'MON'
          }),
          'Closed · Opens Monday at 8:00 AM');
      expect(
          await textFor(tester, {
            'state': 'CLOSED',
            'next_change_local_time': '14:00',
            'next_change_day_offset': 0
          }),
          'Closed · Opens today at 2:00 PM');
      expect(
          await textFor(tester, {'state': 'OPEN_24_HOURS'}), 'Open 24 hours');
      expect(await textFor(tester, {'state': 'HOURS_NOT_AVAILABLE'}),
          'Opening hours not available');
      expect(await textFor(tester, {'state': 'CLOSED'}), 'Closed');
    });
  });

  group('directory', () {
    testWidgets('empty directory shows the "no facilities" state',
        (tester) async {
      await tester
          .pumpWidget(_host(FacilityDirectoryPage(repo: _Repo([]), nav: _nav)));
      await tester.pumpAndSettle();
      expect(find.text('Facility Services'), findsOneWidget);
      expect(find.text('No facilities listed yet'), findsOneWidget);
      expect(find.textContaining("We haven't added any facilities"),
          findsOneWidget);
    });

    testWidgets('populated cards, no distance, and search with no match',
        (tester) async {
      final repo = _Repo([
        _json(_id1, 'Siaya County Hospital'),
        _json(_id2, 'Kisumu County Hospital', youth: false),
      ]);
      await tester
          .pumpWidget(_host(FacilityDirectoryPage(repo: repo, nav: _nav)));
      await tester.pumpAndSettle();
      expect(find.byType(FacilityCard), findsNWidgets(2));
      expect(find.text('Testing · PrEP · ART'), findsNWidgets(2));
      expect(find.text('Open · Closes 5:00 PM'), findsNWidgets(2));
      expect(find.text('Get Directions'), findsNWidgets(2));
      expect(find.textContaining(' km'), findsNothing);
      expect(find.text('View on Map'), findsNothing);
      // Search is debounced, then shows the "no match" state.
      await tester.enterText(find.byType(TextField), 'nowhere');
      await tester.pump(const Duration(milliseconds: 200));
      expect(repo.filters.length, 1);
      await tester.pump(const Duration(milliseconds: 400));
      await tester.pumpAndSettle();
      expect(repo.filters.last.search, 'nowhere');
      expect(find.text('No matching facilities'), findsOneWidget);
      expect(find.text('Try a different location or service.'), findsOneWidget);
    });

    testWidgets('chips map to server filters; no Nearby; link under Directions',
        (tester) async {
      final repo = _Repo([_json(_id1, 'Siaya County Hospital')]);
      await tester
          .pumpWidget(_host(FacilityDirectoryPage(repo: repo, nav: _nav)));
      await tester.pumpAndSettle();
      // Product owner: no location filter at all.
      expect(find.text('Nearby'), findsNothing);
      // The Admin's link is shown under Get Directions.
      expect(
          find.text('https://www.google.com/maps/place/Siaya'), findsOneWidget);
      await tester.tap(find.text('PrEP/ART'));
      await tester.pumpAndSettle();
      expect(repo.filters.last.serviceCodes, ['PREP', 'ART']);
      // The chip (the card's badge reads the same).
      await tester.tap(find.text('Youth Friendly').first);
      await tester.pumpAndSettle();
      expect(repo.filters.last.youthFriendly, isTrue);
    });

    testWidgets('network error offers Retry', (tester) async {
      final repo = _Repo([_json(_id1, 'Siaya County Hospital')], fail: true);
      await tester
          .pumpWidget(_host(FacilityDirectoryPage(repo: repo, nav: _nav)));
      await tester.pumpAndSettle();
      expect(find.text('Try again'), findsOneWidget);
      repo.fail = false;
      await tester.tap(find.text('Try again'));
      await tester.pumpAndSettle();
      expect(find.byType(FacilityCard), findsOneWidget);
    });

    testWidgets('360 dp, text scale 1.3 and Swahili do not overflow',
        (tester) async {
      tester.view.physicalSize = const Size(360, 780);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      for (final language in ['en', 'sw']) {
        for (final rows in [
          <Map<String, dynamic>>[],
          [_json(_id1, 'Siaya County Hospital')]
        ]) {
          await tester.pumpWidget(MediaQuery(
              data: const MediaQueryData(textScaler: TextScaler.linear(1.3)),
              child: _host(FacilityDirectoryPage(repo: _Repo(rows), nav: _nav),
                  language: language)));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull,
              reason: '$language ${rows.length}');
        }
      }
    });
  });

  group('details', () {
    testWidgets('shows admin data, services, actions and the weekly hours',
        (tester) async {
      tester.view.physicalSize = const Size(420, 2400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final repo = _Repo([_json(_id1, 'Siaya County Hospital')]);
      await tester.pumpWidget(_host(FacilityDetailsPage(
          facility: FacilityDto.fromJson(_json(_id1, 'Siaya County Hospital')),
          repo: repo,
          nav: _nav)));
      await tester.pumpAndSettle();
      expect(find.text('Facility Details'), findsOneWidget);
      expect(find.text('Siaya County Hospital'), findsOneWidget);
      expect(find.text('Youth Friendly'), findsOneWidget);
      expect(find.text('Services Available'), findsOneWidget);
      expect(find.text('Testing (HIV/STIs)'), findsOneWidget);
      expect(find.text('Call Facility'), findsOneWidget);
      expect(find.text('+254 712 345 678'), findsOneWidget);
      expect(find.text('Get Directions'), findsOneWidget);
      expect(
          find.text('https://www.google.com/maps/place/Siaya'), findsOneWidget);
      expect(find.text('Opening Hours'), findsOneWidget);
      expect(find.text('Monday · Today'), findsOneWidget);
      expect(find.text('8:00 AM – 5:00 PM'), findsOneWidget);
      expect(find.text('Open 24 hours'), findsOneWidget); // Saturday
      expect(find.text('Closed'), findsNWidgets(5));
      expect(find.textContaining(' km'), findsNothing);
      expect(find.byIcon(Icons.bookmark_border), findsNothing);
    });

    testWidgets('hours not provided', (tester) async {
      tester.view.physicalSize = const Size(420, 2400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final json = _json(_id1, 'Clinic',
          hours: null,
          status: {'state': 'HOURS_NOT_AVAILABLE', 'today': 'MON'});
      await tester.pumpWidget(_host(FacilityDetailsPage(
          facility: FacilityDto.fromJson(json),
          repo: _Repo([json]),
          nav: _nav)));
      await tester.pumpAndSettle();
      expect(find.text('Opening hours not available'), findsNWidgets(2));
    });

    testWidgets('360 dp, 1.3 text scale and Swahili do not overflow',
        (tester) async {
      tester.view.physicalSize = const Size(360, 780);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final json =
          _json(_id1, 'Siaya County Referral Hospital and Training Centre');
      for (final language in ['en', 'sw']) {
        await tester.pumpWidget(MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(1.3)),
            child: _host(
                FacilityDetailsPage(
                    facility: FacilityDto.fromJson(json),
                    repo: _Repo([json]),
                    nav: _nav),
                language: language)));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull, reason: language);
      }
    });
  });
}
