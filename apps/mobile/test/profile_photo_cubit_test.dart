// Cubit-level coverage for DoctorCubit's photo methods (loadPhoto /
// uploadPhoto / removePhoto). Core guarantee under test: a profile-picture
// failure must never contaminate or block the rest of the doctor's state
// (`error`/`loading`/`saving`-adjacent fields), matching the independent
// photo-state design documented on DoctorState itself.
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `doctor_cubit_test.dart`'s `StubDoctorRepository`).
class StubPhotoRepository extends DoctorRepository {
  StubPhotoRepository() : super(ApiClient(dio: Dio()));

  Uint8List? fetchResult;
  Object? fetchError;
  Object? uploadError;
  Object? deleteError;
  int fetchCalls = 0;
  int uploadCalls = 0;
  int deleteCalls = 0;
  Uint8List? afterUploadBytes;

  @override
  Future<Uint8List?> fetchPhotoBytes() async {
    fetchCalls++;
    if (fetchError != null) throw fetchError!;
    return fetchResult;
  }

  @override
  Future<void> uploadPhoto({
    required Uint8List bytes,
    required String mimeType,
  }) async {
    uploadCalls++;
    if (uploadError != null) throw uploadError!;
    fetchResult = afterUploadBytes ?? bytes;
  }

  @override
  Future<void> deletePhoto() async {
    deleteCalls++;
    if (deleteError != null) throw deleteError!;
    fetchResult = null;
  }
}

void main() {
  late StubPhotoRepository repo;
  late DoctorCubit cubit;

  setUp(() {
    repo = StubPhotoRepository();
    cubit = DoctorCubit(repo);
  });

  tearDown(() => cubit.close());

  group('loadPhoto', () {
    test('success sets photoBytes and clears photoLoading', () async {
      final bytes = Uint8List.fromList([1, 2, 3]);
      repo.fetchResult = bytes;

      await cubit.loadPhoto();

      expect(cubit.state.photoBytes, bytes);
      expect(cubit.state.photoLoading, isFalse);
      expect(cubit.state.photoError, isNull);
    });

    test(
        'failure clears photoLoading and leaves photoBytes alone, never sets '
        'photoError or error', () async {
      final existing = Uint8List.fromList([9, 9]);
      repo.fetchResult = existing;
      await cubit.loadPhoto();
      expect(cubit.state.photoBytes, existing);

      repo.fetchError = const ApiException(500, 'Server error.');
      await cubit.loadPhoto();

      expect(cubit.state.photoLoading, isFalse);
      expect(cubit.state.photoBytes, existing,
          reason: 'a failed reload must not clear a previously-loaded photo');
      expect(cubit.state.photoError, isNull);
      expect(cubit.state.error, isNull);
    });
  });

  group('uploadPhoto', () {
    test(
        'success sets photoBytes to the freshly-refetched value and returns '
        'true', () async {
      final refetched = Uint8List.fromList([5, 5, 5]);
      repo.afterUploadBytes = refetched;

      final ok = await cubit.uploadPhoto(Uint8List.fromList([1]), 'image/jpeg');

      expect(ok, isTrue);
      expect(cubit.state.photoBytes, refetched);
      expect(cubit.state.photoSaving, isFalse);
      expect(cubit.state.photoError, isNull);
      expect(repo.uploadCalls, 1);
      expect(repo.fetchCalls, 1,
          reason: 'must re-fetch after a successful upload');
    });

    test(
        'failure sets photoError (not error) and returns false, leaving any '
        'pre-existing photoBytes untouched', () async {
      final existing = Uint8List.fromList([7, 7]);
      repo.fetchResult = existing;
      await cubit.loadPhoto();
      expect(cubit.state.photoBytes, existing);

      repo.uploadError = const ApiException(422, 'Malformed image.');
      final ok = await cubit.uploadPhoto(Uint8List.fromList([2]), 'image/png');

      expect(ok, isFalse);
      expect(cubit.state.photoError, isNotNull);
      expect(cubit.state.error, isNull);
      expect(cubit.state.photoBytes, existing);
      expect(cubit.state.photoSaving, isFalse);
    });

    test('never touches error/loading/duty state', () async {
      repo.uploadError = const ApiException(500, 'Server error.');
      await cubit.uploadPhoto(Uint8List.fromList([1]), 'image/jpeg');

      expect(cubit.state.error, isNull);
      expect(cubit.state.loading, isFalse);
      expect(cubit.state.dutyUpdating, isFalse);
    });
  });

  group('removePhoto', () {
    test('success clears photoBytes and returns true', () async {
      repo.fetchResult = Uint8List.fromList([3, 3]);
      await cubit.loadPhoto();
      expect(cubit.state.photoBytes, isNotNull);

      final ok = await cubit.removePhoto();

      expect(ok, isTrue);
      expect(cubit.state.photoBytes, isNull);
      expect(cubit.state.photoSaving, isFalse);
      expect(cubit.state.photoError, isNull);
      expect(repo.deleteCalls, 1);
    });

    test(
        'failure sets only photoError and returns false, leaving photoBytes '
        'untouched', () async {
      final existing = Uint8List.fromList([4, 4]);
      repo.fetchResult = existing;
      await cubit.loadPhoto();

      repo.deleteError = const ApiException(500, 'Server error.');
      final ok = await cubit.removePhoto();

      expect(ok, isFalse);
      expect(cubit.state.photoError, isNotNull);
      expect(cubit.state.error, isNull);
      expect(cubit.state.photoBytes, existing);
      expect(cubit.state.loading, isFalse);
    });
  });
}
