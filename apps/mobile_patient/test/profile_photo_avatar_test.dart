// Widget-level coverage for ProfilePhotoAvatar (Patient) plus its two
// standalone, plugin-channel-free helper functions. Platform-channel-backed
// image_picker calls are deliberately never driven here — this codebase's own
// established convention (see `patient_intake_cleanup_test.dart`'s coverage
// of `pre_consult_intake_modal.dart`'s equivalent helpers) keeps those out of
// widget tests entirely.
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_photo_avatar.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

/// A real, minimal 1x1 transparent PNG — not garbage bytes. `MemoryImage`
/// actually decodes its bytes during the render pipeline (the task's "no
/// decoded-pixel assertions needed" note doesn't exempt it from needing to
/// be a *decodable* image; invalid bytes throw an async image-codec error
/// that flutter_test surfaces as a test failure), so every test below that
/// seeds `photoBytes` uses this rather than arbitrary integers.
final Uint8List _tinyPngBytes = base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY'
    '42YAAAAASUVORK5CYII=');

class _NoopStore implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

/// Never actually hits the network in these tests — every state is seeded
/// directly via `cubit.emit`, matching `doctor_widgets_test.dart`'s harness
/// convention. `loadPhoto()` IS exercised (the widget calls it from a
/// post-frame callback), so it must resolve harmlessly rather than throw.
class _QuietProfileRepository extends ProfileRepository {
  _QuietProfileRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  @override
  Future<Uint8List?> fetchPhotoBytes({String? profileId}) async => null;
}

Widget harness(ProfileState seed) => BlocProvider<ProfileCubit>(
      create: (_) => ProfileCubit(_QuietProfileRepository())..emit(seed),
      child: const MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(body: Center(child: ProfilePhotoAvatar())),
      ),
    );

void main() {
  group('mimeTypeForPickedPhoto', () {
    test('.png (any case) maps to image/png', () {
      expect(mimeTypeForPickedPhoto('/tmp/photo.png'), 'image/png');
      expect(mimeTypeForPickedPhoto('/tmp/PHOTO.PNG'), 'image/png');
    });

    test('.jpg/.jpeg and no-extension all map to image/jpeg', () {
      expect(mimeTypeForPickedPhoto('/tmp/photo.jpg'), 'image/jpeg');
      expect(mimeTypeForPickedPhoto('/tmp/photo.jpeg'), 'image/jpeg');
      expect(mimeTypeForPickedPhoto('/tmp/photo'), 'image/jpeg');
    });
  });

  group('deletePickedPhotoSourceFile', () {
    late Directory tempDir;

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('profile_photo_test_');
    });

    tearDown(() async {
      if (await tempDir.exists()) await tempDir.delete(recursive: true);
    });

    test('deletes an existing file', () async {
      final f = File('${tempDir.path}/picked.jpg');
      await f.writeAsBytes([1, 2, 3]);
      expect(await f.exists(), isTrue);

      await deletePickedPhotoSourceFile(f.path);

      expect(await f.exists(), isFalse);
    });

    test('a missing path does not throw', () async {
      final f = File('${tempDir.path}/does_not_exist.jpg');
      expect(await f.exists(), isFalse);
      await expectLater(deletePickedPhotoSourceFile(f.path), completes);
    });
  });

  group('ProfilePhotoAvatar', () {
    testWidgets('shows the person placeholder when no photo and not loading',
        (tester) async {
      await tester.pumpWidget(
          harness(const ProfileState(photoBytes: null, photoLoading: false)));
      // Two pumps: the widget's own post-frame callback fires loadPhoto() when
      // no bytes are seeded yet, and its (stubbed, no-op) result needs an
      // extra pump to be reflected back into the rebuilt tree.
      await tester.pump();
      await tester.pump();

      expect(find.byIcon(Icons.person), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsNothing);
      expect(
          find.byType(CircleAvatar).evaluate().first.widget,
          isA<CircleAvatar>()
              .having((c) => c.backgroundImage, 'backgroundImage', isNull));
    });

    testWidgets('shows a spinner while photoLoading and no bytes yet',
        (tester) async {
      await tester.pumpWidget(
          harness(const ProfileState(photoBytes: null, photoLoading: true)));
      await tester.pump();

      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      expect(find.byIcon(Icons.person), findsNothing);
    });

    testWidgets('renders a CircleAvatar with a MemoryImage when bytes are set',
        (tester) async {
      final bytes = _tinyPngBytes;
      await tester.pumpWidget(harness(ProfileState(photoBytes: bytes)));
      await tester.pump();

      final avatar = find
          .byType(CircleAvatar)
          .evaluate()
          .map((e) => e.widget)
          .cast<CircleAvatar>()
          .firstWhere((c) => c.backgroundImage != null);
      expect(avatar.backgroundImage, isA<MemoryImage>());
      expect((avatar.backgroundImage as MemoryImage).bytes, bytes);
      expect(find.byIcon(Icons.person), findsNothing);
    });

    testWidgets(
        'photoSaving shows the saving overlay and disables tap (no sheet opens)',
        (tester) async {
      await tester.pumpWidget(harness(const ProfileState(photoSaving: true)));
      // Two pumps so the widget's auto-triggered loadPhoto() (photoBytes is
      // null and photoLoading defaults to false) fully settles first — only
      // the saving overlay's spinner should remain afterward.
      await tester.pump();
      await tester.pump();

      // The saving overlay's own small spinner, distinguishable from the
      // "no bytes yet" placeholder spinner because here photoLoading is false
      // and bytes are null — both spinners would look similar, so assert via
      // tap-is-disabled instead, which is the behavior that actually matters.
      expect(find.byType(CircularProgressIndicator), findsOneWidget);

      await tester.tap(find.byType(GestureDetector));
      // Not pumpAndSettle: photoSaving's overlay spinner animates for as
      // long as this seeded state holds (there is no transient completion to
      // wait out), so pumpAndSettle would time out here by design. onTap is
      // null while saving, so nothing should happen from a plain pump either.
      await tester.pump();

      expect(find.byType(ListTile), findsNothing,
          reason: 'tapping while photoSaving must not open the picker sheet');
    });

    testWidgets(
        'tapping when idle opens a sheet with Camera and Choose Photo, no '
        'Remove option when there is no photo', (tester) async {
      await tester.pumpWidget(
          harness(const ProfileState(photoBytes: null, photoLoading: false)));
      // Let the auto-triggered loadPhoto() settle before opening the modal —
      // pumpAndSettle cannot be used while its placeholder spinner is still
      // animating (an indeterminate CircularProgressIndicator never settles).
      await tester.pump();
      await tester.pump();

      await tester.tap(find.byType(GestureDetector));
      await tester.pumpAndSettle();

      expect(find.text('Camera'), findsOneWidget);
      expect(find.text('Choose Photo'), findsOneWidget);
      expect(find.text('Remove photo'), findsNothing);
    });

    testWidgets('tapping when a photo exists also offers Remove photo',
        (tester) async {
      final bytes = _tinyPngBytes;
      await tester.pumpWidget(harness(ProfileState(photoBytes: bytes)));
      await tester.pump();

      await tester.tap(find.byType(GestureDetector));
      await tester.pumpAndSettle();

      expect(find.text('Camera'), findsOneWidget);
      expect(find.text('Choose Photo'), findsOneWidget);
      expect(find.text('Remove photo'), findsOneWidget);
    });

    testWidgets(
        'tapping a non-destructive tile dismisses the sheet without crashing '
        '(the underlying image_picker platform-channel call is never driven '
        'here; a MissingPluginException is a caught, already-handled outcome)',
        (tester) async {
      await tester.pumpWidget(
          harness(const ProfileState(photoBytes: null, photoLoading: false)));
      await tester.pump();
      await tester.pump();

      await tester.tap(find.byType(GestureDetector));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Camera'));
      await tester.pumpAndSettle();

      expect(find.text('Camera'), findsNothing,
          reason: 'the bottom sheet must have closed');
    });
  });
}
