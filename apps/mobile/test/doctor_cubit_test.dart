import 'package:dio/dio.dart';
import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

class StubDoctorRepository extends DoctorRepository {
  StubDoctorRepository() : super(ApiClient(dio: Dio()));
  String slotStatus = 'AVAILABLE';
  String verificationStatus = 'VERIFIED';
  int clinicalReads = 0;
  Completer<List<SlotDto>>? slotResponse;
  bool dutyValue = false;
  bool failDuty = false;
  bool offlineFlag = false;
  CachedDoctorSnapshot? cachedSnapshotValue;
  Completer<List<AppointmentDto>>? queueResponse;
  bool bookingPolicyPaymentRequired = true;
  bool failBookingPolicy = false;
  @override
  bool get isOffline => offlineFlag;
  @override
  Future<CachedDoctorSnapshot> cachedSnapshot() async =>
      cachedSnapshotValue ?? const CachedDoctorSnapshot();

  @override
  Future<List<AppointmentDto>> queue({int limit = 25, int offset = 0}) async {
    clinicalReads++;
    return queueResponse == null
        ? [
            const AppointmentDto(
                id: 'a1', status: 'UPCOMING', lifecycle: 'UPCOMING')
          ]
        : await queueResponse!.future;
  }

  @override
  Future<List<SlotDto>> slots(
          {int limit = 25, int offset = 0, DateTime? day}) async =>
      slotResponse == null
          ? [SlotDto(id: 's1', status: slotStatus)]
          : await slotResponse!.future;
  @override
  Future<List<ChatDto>> chats({int limit = 25, int offset = 0}) async =>
      [const ChatDto(id: 'c1')];
  @override
  Future<List<QnaDto>> questions({int limit = 25, int offset = 0}) async =>
      [const QnaDto(id: 'q1', question: 'Question', status: 'OPEN')];
  @override
  Future<WalletDto> wallet() async =>
      const WalletDto(gross: '100', net: '80', pending: '80');
  @override
  Future<ProfileDto> profile() async => ProfileDto(
      name: 'Dr. Test Doctor',
      license: '12345',
      hospital: 'Test Hospital',
      specialty: 'General',
      verificationStatus: verificationStatus,
      qualifications: [],
      userId: 'u1');
  @override
  Future<BookingPolicyDto> bookingPolicy() async {
    if (failBookingPolicy) {
      throw const ApiException(null, 'Unable to load booking policy.');
    }
    return BookingPolicyDto(paymentRequired: bookingPolicyPaymentRequired);
  }

  @override
  Future<Json> duty() async => {'is_on_duty': dutyValue};
  @override
  Future<Json> setDuty(bool value) async {
    if (failDuty) throw const ApiException(409, 'Duty conflict');
    dutyValue = value;
    return {'is_on_duty': value};
  }
}

void main() {
  test(
      'pending and rejected review states load without requesting clinical data',
      () async {
    for (final status in ['PENDING', 'REJECTED']) {
      final repo = StubDoctorRepository()..verificationStatus = status;
      final cubit = DoctorCubit(repo);
      await cubit.loadAll();
      expect(cubit.state.profile?.verificationStatus, status);
      expect(cubit.state.error, isNull);
      expect(cubit.state.queue, isEmpty);
      expect(repo.clinicalReads, 0);
      await cubit.close();
    }
  });
  test('logout disposes account state and discards a late queue response',
      () async {
    final repo = StubDoctorRepository()
      ..queueResponse = Completer<List<AppointmentDto>>();
    final cubit = DoctorCubit(repo);
    final loading = cubit.loadQueue();
    await Future<void>.delayed(Duration.zero);
    await cubit.close();
    repo.queueResponse!.complete([
      const AppointmentDto(
          id: 'private-old-account', status: 'CONFIRMED', lifecycle: 'UPCOMING')
    ]);
    await loading;
    expect(cubit.state.queue, isEmpty);
    final newAccount = DoctorCubit(StubDoctorRepository());
    expect(newAccount.state.queue, isEmpty);
    expect(newAccount.state.wallet, isNull);
    await newAccount.close();
  });
  test(
      'schedule refresh observes server admin block and unblock without reloading other data',
      () async {
    final repo = StubDoctorRepository();
    final cubit = DoctorCubit(repo);
    await cubit.loadAll();
    repo.slotStatus = 'BLOCKED';
    await cubit.refreshSlots();
    expect((cubit.state.slots.single as SlotDto).status, 'BLOCKED');
    expect(cubit.state.queue.single.id, 'a1');
    repo.slotStatus = 'AVAILABLE';
    await cubit.refreshSlots();
    expect((cubit.state.slots.single as SlotDto).status, 'AVAILABLE');
    await cubit.close();
  });
  test('late schedule poll after logout is discarded', () async {
    final repo = StubDoctorRepository()
      ..slotResponse = Completer<List<SlotDto>>();
    final cubit = DoctorCubit(repo);
    final pending = cubit.refreshSlots();
    await Future<void>.delayed(Duration.zero);
    await cubit.close();
    repo.slotResponse!
        .complete([const SlotDto(id: 'old-account', status: 'BLOCKED')]);
    await pending;
    expect(cubit.state.slots, isEmpty);
  });
  test('loadAll populates every backend-driven collection', () async {
    final repo = StubDoctorRepository();
    final cubit = DoctorCubit(repo);
    await cubit.loadAll();
    expect(cubit.state.error, isNull);
    expect(cubit.state.queue.single.id, 'a1');
    expect(cubit.state.slots.single.id, 's1');
    expect(cubit.state.chats.single.id, 'c1');
    expect(cubit.state.questions.single.id, 'q1');
    expect((cubit.state.wallet as WalletDto).net, '80');
    expect(cubit.state.profile?.name, 'Dr. Test Doctor');
    await cubit.close();
  });

  test('duty remains server-confirmed and rolls back on conflict', () async {
    final repo = StubDoctorRepository();
    final cubit = DoctorCubit(repo);
    await cubit.loadQueue();
    expect(cubit.state.duty, isFalse);
    await cubit.toggleDuty();
    expect(cubit.state.duty, isTrue);
    repo.failDuty = true;
    await cubit.toggleDuty();
    expect(cubit.state.duty, isTrue);
    expect(cubit.state.dutyUpdating, isFalse);
    expect(cubit.state.error, contains('Duty conflict'));
    await cubit.close();
  });

  test('cached data is emitted before the fresh backend result', () async {
    final repo = StubDoctorRepository();
    repo.cachedSnapshotValue = const CachedDoctorSnapshot(queue: [
      AppointmentDto(id: 'cached', status: 'UPCOMING', lifecycle: 'UPCOMING')
    ]);
    final cubit = DoctorCubit(repo);
    final states = <DoctorState>[];
    final subscription = cubit.stream.listen(states.add);
    await cubit.loadAll();
    expect(states.first.refreshing, isTrue);
    expect(states.first.queue.single.id, 'cached');
    await Future<void>.delayed(Duration.zero);
    expect(states.last.refreshing, isFalse);
    expect(states.last.queue.single.id, 'a1');
    await subscription.cancel();
    await cubit.close();
  });

  test('known offline duty mutation is rejected without a local success',
      () async {
    final repo = StubDoctorRepository()..offlineFlag = true;
    final cubit = DoctorCubit(repo);
    await cubit.toggleDuty();
    expect(cubit.state.duty, isFalse);
    expect(cubit.state.offline, isTrue);
    expect(cubit.state.error, contains('Internet connection'));
    await cubit.close();
  });

  group('loadBookingPolicy (FINANCIAL_MODULE_MODE)', () {
    test('defaults to paymentRequired before any load', () async {
      final cubit = DoctorCubit(StubDoctorRepository());
      expect(cubit.state.paymentRequired, isTrue);
      await cubit.close();
    });

    test('a deferred server response hides financial actions', () async {
      final repo = StubDoctorRepository()
        ..bookingPolicyPaymentRequired = false;
      final cubit = DoctorCubit(repo);
      await cubit.loadBookingPolicy();
      expect(cubit.state.paymentRequired, isFalse);
      await cubit.close();
    });

    test('an enabled server response keeps financial actions visible',
        () async {
      final repo = StubDoctorRepository()
        ..bookingPolicyPaymentRequired = true;
      final cubit = DoctorCubit(repo);
      await cubit.loadBookingPolicy();
      expect(cubit.state.paymentRequired, isTrue);
      await cubit.close();
    });

    test(
        'a failed fetch never surfaces an error and keeps the safe default',
        () async {
      final repo = StubDoctorRepository()..failBookingPolicy = true;
      final cubit = DoctorCubit(repo);
      await cubit.loadBookingPolicy();
      expect(cubit.state.paymentRequired, isTrue);
      expect(cubit.state.error, isNull);
      await cubit.close();
    });
  });
}
