import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';

const _slotJsonDay1Morning = {
  'id': 'slot-1',
  'doctor_id': 'doc-1',
  'start_time': '2026-11-02T07:00:00Z',
  'end_time': '2026-11-02T07:10:00Z',
};
const _slotJsonDay1Afternoon = {
  'id': 'slot-2',
  'doctor_id': 'doc-1',
  'start_time': '2026-11-02T11:00:00Z',
  'end_time': '2026-11-02T11:10:00Z',
};
const _slotJsonDay2 = {
  'id': 'slot-3',
  'doctor_id': 'doc-1',
  'start_time': '2026-11-03T07:00:00Z',
  'end_time': '2026-11-03T07:10:00Z',
};

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `family_profiles_cubit_test.dart`'s harness).
class FakeBookingRepository extends BookingRepository {
  FakeBookingRepository()
      : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  List<DoctorSlotDto>? slotsResult;
  Object? slotsError;
  BookingResultDto? bookResult;
  Object? bookError;
  Map<String, dynamic>? lastBookArgs;
  int uploadCallCount = 0;
  int? uploadResult;
  Object? uploadError;
  List<PatientPaymentMethodDto>? paymentMethodsResult;
  Object? paymentMethodsError;
  PatientPaymentMethodDto? addPaymentMethodResult;
  Object? addPaymentMethodError;
  String? lastAddedPhone;

  @override
  Future<List<DoctorSlotDto>> slots(String doctorId) async {
    if (slotsError != null) throw slotsError!;
    return slotsResult ?? const [];
  }

  @override
  Future<BookingResultDto> book({
    required String doctorId,
    required String slotId,
    required String modality,
    required String paymentMethodId,
    String? chiefComplaint,
    String? profileId,
  }) async {
    lastBookArgs = {
      'doctorId': doctorId,
      'slotId': slotId,
      'modality': modality,
      'paymentMethodId': paymentMethodId,
      'chiefComplaint': chiefComplaint,
      'profileId': profileId,
    };
    if (bookError != null) throw bookError!;
    return bookResult!;
  }

  @override
  Future<int> uploadIntake(
      String appointmentId, List<Map<String, String>> files) async {
    uploadCallCount++;
    if (uploadError != null) throw uploadError!;
    return uploadResult ?? files.length;
  }

  @override
  Future<List<PatientPaymentMethodDto>> paymentMethods(
      {String? profileId}) async {
    if (paymentMethodsError != null) throw paymentMethodsError!;
    return paymentMethodsResult ?? const [];
  }

  @override
  Future<PatientPaymentMethodDto> addMpesaPaymentMethod(String phoneNumber,
      {String? profileId}) async {
    lastAddedPhone = phoneNumber;
    if (addPaymentMethodError != null) throw addPaymentMethodError!;
    return addPaymentMethodResult!;
  }
}

const _paymentMethodJson = {
  'id': 'pm-1',
  'gateway': 'MPESA',
  'masked_destination': 'M-Pesa •••• 5678',
  'is_primary': true,
};

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

void main() {
  late FakeBookingRepository repo;
  late BookingCubit cubit;

  setUp(() {
    repo = FakeBookingRepository();
    cubit = BookingCubit(repo, doctorId: 'doc-1', modality: 'VIDEO');
  });

  tearDown(() => cubit.close());

  group('loadSlots', () {
    test('groups slots by date and defaults the earliest date selected',
        () async {
      repo.slotsResult = [
        DoctorSlotDto.fromJson(_slotJsonDay2),
        DoctorSlotDto.fromJson(_slotJsonDay1Morning),
        DoctorSlotDto.fromJson(_slotJsonDay1Afternoon),
      ];
      await cubit.loadSlots();
      expect(cubit.state.availableDates, hasLength(2));
      expect(cubit.state.selectedDate, cubit.state.availableDates.first);
      expect(cubit.state.slotsForSelectedDate, hasLength(2));
      expect(cubit.state.selectedSlotId, isNull);
    });

    test('surfaces a repository failure as state.slotsError', () async {
      repo.slotsError = const ApiException(500, 'Server error.');
      await cubit.loadSlots();
      expect(cubit.state.slotsError, isNotNull);
      expect(cubit.state.slots, isEmpty);
    });
  });

  group('canProceed — real date/slot/payment-method-required validation', () {
    test('false until a date, a slot, and a payment method are all selected',
        () async {
      repo.slotsResult = [DoctorSlotDto.fromJson(_slotJsonDay1Morning)];
      await cubit.loadSlots();
      // A date is auto-selected by loadSlots, but no slot yet.
      expect(cubit.state.selectedDate, isNotNull);
      expect(cubit.state.canProceed, isFalse);
      cubit.selectSlot('slot-1');
      // A slot is now selected, but there is still no payment method.
      expect(cubit.state.canProceed, isFalse);
      cubit.selectPaymentMethod('pm-1');
      expect(cubit.state.canProceed, isTrue);
    });

    test('selecting a new date clears the previously selected slot', () async {
      repo.slotsResult = [
        DoctorSlotDto.fromJson(_slotJsonDay1Morning),
        DoctorSlotDto.fromJson(_slotJsonDay2),
      ];
      await cubit.loadSlots();
      cubit.selectSlot('slot-1');
      cubit.selectPaymentMethod('pm-1');
      expect(cubit.state.canProceed, isTrue);
      cubit.selectDate(cubit.state.availableDates.last);
      expect(cubit.state.selectedSlotId, isNull);
      expect(cubit.state.canProceed, isFalse);
    });
  });

  group('submit', () {
    test('does nothing and returns false when canProceed is false', () async {
      final ok = await cubit.submit();
      expect(ok, isFalse);
      expect(repo.lastBookArgs, isNull);
    });

    test('does nothing when a slot is selected but no payment method is chosen',
        () async {
      repo.slotsResult = [DoctorSlotDto.fromJson(_slotJsonDay1Morning)];
      await cubit.loadSlots();
      cubit.selectSlot('slot-1');
      final ok = await cubit.submit();
      expect(ok, isFalse);
      expect(repo.lastBookArgs, isNull);
    });

    test(
        'books with the selected slot, modality, payment method, and profile id on success',
        () async {
      repo.slotsResult = [DoctorSlotDto.fromJson(_slotJsonDay1Morning)];
      await cubit.loadSlots();
      cubit.selectSlot('slot-1');
      cubit.selectPaymentMethod('pm-1');
      repo.bookResult = const BookingResultDto(
        appointmentId: 'apt-1',
        appointmentNumber: 'APT-20261102-ABC12345',
        status: 'PENDING_PAYMENT',
        amount: '630.00',
      );
      final ok = await cubit.submit(profileId: 'dep-1');
      expect(ok, isTrue);
      expect(cubit.state.result?.appointmentId, 'apt-1');
      expect(repo.lastBookArgs?['slotId'], 'slot-1');
      expect(repo.lastBookArgs?['modality'], 'VIDEO');
      expect(repo.lastBookArgs?['paymentMethodId'], 'pm-1');
      expect(repo.lastBookArgs?['profileId'], 'dep-1');
    });

    test('a slot-conflict failure surfaces as state.submitError, not thrown',
        () async {
      repo.slotsResult = [DoctorSlotDto.fromJson(_slotJsonDay1Morning)];
      await cubit.loadSlots();
      cubit.selectSlot('slot-1');
      cubit.selectPaymentMethod('pm-1');
      repo.bookError = const ApiException(
          409, 'Selected time slot is already locked or booked.');
      final ok = await cubit.submit();
      expect(ok, isFalse);
      expect(cubit.state.submitError, isNotNull);
      expect(cubit.state.result, isNull);
    });
  });

  group('loadPaymentMethods', () {
    test('auto-selects the primary method when one is marked primary',
        () async {
      repo.paymentMethodsResult = [
        PatientPaymentMethodDto.fromJson({
          ..._paymentMethodJson,
          'id': 'pm-1',
          'is_primary': false,
        }),
        PatientPaymentMethodDto.fromJson({
          ..._paymentMethodJson,
          'id': 'pm-2',
          'is_primary': true,
        }),
      ];
      await cubit.loadPaymentMethods();
      expect(cubit.state.paymentMethods, hasLength(2));
      expect(cubit.state.selectedPaymentMethodId, 'pm-2');
    });

    test('auto-selects the only method when none is marked primary', () async {
      repo.paymentMethodsResult = [
        PatientPaymentMethodDto.fromJson({
          ..._paymentMethodJson,
          'id': 'pm-1',
          'is_primary': false,
        }),
      ];
      await cubit.loadPaymentMethods();
      expect(cubit.state.selectedPaymentMethodId, 'pm-1');
    });

    test('selects nothing when the patient has no saved methods', () async {
      repo.paymentMethodsResult = const [];
      await cubit.loadPaymentMethods();
      expect(cubit.state.paymentMethods, isEmpty);
      expect(cubit.state.selectedPaymentMethodId, isNull);
    });

    test('surfaces a repository failure as state.paymentMethodsError',
        () async {
      repo.paymentMethodsError = const ApiException(500, 'Server error.');
      await cubit.loadPaymentMethods();
      expect(cubit.state.paymentMethodsError, isNotNull);
    });
  });

  group('addMpesaPaymentMethod', () {
    test('saves and immediately selects the new method on success', () async {
      repo.addPaymentMethodResult =
          PatientPaymentMethodDto.fromJson(_paymentMethodJson);
      final ok = await cubit.addMpesaPaymentMethod('+254712345678');
      expect(ok, isTrue);
      expect(repo.lastAddedPhone, '+254712345678');
      expect(cubit.state.paymentMethods, hasLength(1));
      expect(cubit.state.selectedPaymentMethodId, 'pm-1');
    });

    test('a provider-not-configured failure surfaces as addPaymentMethodError',
        () async {
      repo.addPaymentMethodError = const ApiException(
          409, 'The platform has not configured a MPESA payment provider yet.');
      final ok = await cubit.addMpesaPaymentMethod('+254712345678');
      expect(ok, isFalse);
      expect(cubit.state.addPaymentMethodError, isNotNull);
      expect(cubit.state.paymentMethods, isEmpty);
    });
  });

  group('uploadIntake', () {
    test('is a no-op before a booking result exists', () async {
      final ok = await cubit.uploadIntake(const [
        {'mime_type': 'image/jpeg', 'base64_content': 'abc'}
      ]);
      expect(ok, isFalse);
      expect(repo.uploadCallCount, 0);
    });

    test('uploads against the booked appointment id on success', () async {
      repo.slotsResult = [DoctorSlotDto.fromJson(_slotJsonDay1Morning)];
      await cubit.loadSlots();
      cubit.selectSlot('slot-1');
      cubit.selectPaymentMethod('pm-1');
      repo.bookResult = const BookingResultDto(
        appointmentId: 'apt-1',
        appointmentNumber: 'APT-20261102-ABC12345',
        status: 'PENDING_PAYMENT',
        amount: '630.00',
      );
      await cubit.submit();
      final ok = await cubit.uploadIntake(const [
        {'mime_type': 'image/jpeg', 'base64_content': 'abc'}
      ]);
      expect(ok, isTrue);
      expect(repo.uploadCallCount, 1);
      expect(cubit.state.uploadedCount, 1);
    });
  });
}
