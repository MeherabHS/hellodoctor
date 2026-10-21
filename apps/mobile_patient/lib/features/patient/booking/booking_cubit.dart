import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/network/api_client.dart';
import 'booking_models.dart';
import 'booking_repository.dart';

const _unset = Object();

class BookingState {
  const BookingState({
    this.loadingSlots = false,
    this.slotsError,
    this.slots = const [],
    this.selectedDate,
    this.selectedSlotId,
    this.chiefComplaint = '',
    this.submitting = false,
    this.submitError,
    this.result,
    this.uploading = false,
    this.uploadError,
    this.uploadedCount,
    this.loadingPaymentMethods = false,
    this.paymentMethodsError,
    this.paymentMethods = const [],
    this.selectedPaymentMethodId,
    this.addingPaymentMethod = false,
    this.addPaymentMethodError,
  });
  final bool loadingSlots, submitting, uploading;
  final String? slotsError, submitError, uploadError, selectedSlotId;
  final List<DoctorSlotDto> slots;
  final DateTime? selectedDate;
  final String chiefComplaint;
  final BookingResultDto? result;
  final int? uploadedCount;
  final bool loadingPaymentMethods, addingPaymentMethod;
  final String? paymentMethodsError, addPaymentMethodError;
  final List<PatientPaymentMethodDto> paymentMethods;
  final String? selectedPaymentMethodId;

  /// Slots grouped by calendar day — the prototype's date-slider cards
  /// (`view-4`'s `.date-slider`), but built from real distinct dates present
  /// in the actual slot list rather than six hardcoded placeholder days.
  List<DateTime> get availableDates {
    final seen = <DateTime>{};
    for (final s in slots) {
      seen.add(DateTime(s.startTime.year, s.startTime.month, s.startTime.day));
    }
    final list = seen.toList()..sort();
    return list;
  }

  List<DoctorSlotDto> get slotsForSelectedDate {
    final date = selectedDate;
    if (date == null) return const [];
    return slots
        .where((s) =>
            s.startTime.year == date.year &&
            s.startTime.month == date.month &&
            s.startTime.day == date.day)
        .toList();
  }

  /// Real validation the prototype's static demo lacks (plan §4, view-4
  /// row): "Proceed" must stay disabled until both a date and a slot on that
  /// date are actually selected, not merely pre-highlighted by default. Also
  /// requires a real saved payment method to be selected — the backend
  /// requires one (`payment_method_id`) for every booking, and a patient
  /// picking which saved instrument to pay with is the whole point of this
  /// screen's payment section, not an optional extra.
  bool get canProceed =>
      selectedDate != null &&
      selectedSlotId != null &&
      selectedPaymentMethodId != null &&
      !loadingSlots &&
      slotsError == null &&
      !submitting &&
      result == null;

  BookingState copyWith({
    bool? loadingSlots,
    Object? slotsError = _unset,
    List<DoctorSlotDto>? slots,
    Object? selectedDate = _unset,
    Object? selectedSlotId = _unset,
    String? chiefComplaint,
    bool? submitting,
    Object? submitError = _unset,
    Object? result = _unset,
    bool? uploading,
    Object? uploadError = _unset,
    Object? uploadedCount = _unset,
    bool? loadingPaymentMethods,
    Object? paymentMethodsError = _unset,
    List<PatientPaymentMethodDto>? paymentMethods,
    Object? selectedPaymentMethodId = _unset,
    bool? addingPaymentMethod,
    Object? addPaymentMethodError = _unset,
  }) =>
      BookingState(
        loadingSlots: loadingSlots ?? this.loadingSlots,
        slotsError: identical(slotsError, _unset)
            ? this.slotsError
            : slotsError as String?,
        slots: slots ?? this.slots,
        selectedDate: identical(selectedDate, _unset)
            ? this.selectedDate
            : selectedDate as DateTime?,
        selectedSlotId: identical(selectedSlotId, _unset)
            ? this.selectedSlotId
            : selectedSlotId as String?,
        chiefComplaint: chiefComplaint ?? this.chiefComplaint,
        submitting: submitting ?? this.submitting,
        submitError: identical(submitError, _unset)
            ? this.submitError
            : submitError as String?,
        result: identical(result, _unset)
            ? this.result
            : result as BookingResultDto?,
        uploading: uploading ?? this.uploading,
        uploadError: identical(uploadError, _unset)
            ? this.uploadError
            : uploadError as String?,
        uploadedCount: identical(uploadedCount, _unset)
            ? this.uploadedCount
            : uploadedCount as int?,
        loadingPaymentMethods:
            loadingPaymentMethods ?? this.loadingPaymentMethods,
        paymentMethodsError: identical(paymentMethodsError, _unset)
            ? this.paymentMethodsError
            : paymentMethodsError as String?,
        paymentMethods: paymentMethods ?? this.paymentMethods,
        selectedPaymentMethodId: identical(selectedPaymentMethodId, _unset)
            ? this.selectedPaymentMethodId
            : selectedPaymentMethodId as String?,
        addingPaymentMethod: addingPaymentMethod ?? this.addingPaymentMethod,
        addPaymentMethodError: identical(addPaymentMethodError, _unset)
            ? this.addPaymentMethodError
            : addPaymentMethodError as String?,
      );
}

/// Owns one booking attempt for one (doctor, modality) pair — constructed
/// fresh per push of [PatientBookingPage], not a root-level cubit, since a
/// booking-in-progress has no reason to survive leaving that screen.
class BookingCubit extends Cubit<BookingState> {
  BookingCubit(this.repo, {required this.doctorId, required this.modality})
      : super(const BookingState());
  final BookingRepository repo;
  final String doctorId;
  final String modality;

  int _slotsGeneration = 0;
  Future<void> loadSlots() async {
    final generation = ++_slotsGeneration;
    final previousId = state.selectedSlotId;
    emit(state.copyWith(
        loadingSlots: true,
        slotsError: null,
        slots: const [],
        selectedSlotId: null));
    try {
      final slots = await repo.slots(doctorId);
      if (isClosed || generation != _slotsGeneration) return;
      final dates = <DateTime>{};
      for (final s in slots) {
        dates.add(
            DateTime(s.startTime.year, s.startTime.month, s.startTime.day));
      }
      final firstDate = dates.isEmpty ? null : (dates.toList()..sort()).first;
      DoctorSlotDto? preserved;
      for (final slot in slots) {
        if (slot.id == previousId) {
          preserved = slot;
          break;
        }
      }
      final selectedDate = preserved == null
          ? firstDate
          : DateTime(preserved.startTime.year, preserved.startTime.month,
              preserved.startTime.day);
      emit(state.copyWith(
        loadingSlots: false,
        slots: slots,
        selectedDate: selectedDate,
        selectedSlotId:
            slots.any((s) => s.id == previousId) ? previousId : null,
      ));
    } catch (e) {
      if (isClosed || generation != _slotsGeneration) return;
      emit(state.copyWith(
        loadingSlots: false,
        slotsError: e is ApiException
            ? e.userMessage
            : 'Unable to load available slots.',
      ));
    }
  }

  void selectDate(DateTime date) {
    if (state.selectedDate == date) return;
    emit(state.copyWith(selectedDate: date, selectedSlotId: null));
  }

  void selectSlot(String slotId) =>
      emit(state.copyWith(selectedSlotId: slotId));
  void setChiefComplaint(String value) =>
      emit(state.copyWith(chiefComplaint: value));

  /// [profileId]: whose saved payment methods to list — `null` = the
  /// caller's own self profile, matching [submit]'s own `profileId`
  /// semantics. Auto-selects the primary method (or the first one, if none
  /// is marked primary) so a returning patient with exactly one saved method
  /// never has to tap anything extra to proceed.
  int _paymentMethodsGeneration = 0;
  Future<void> loadPaymentMethods({String? profileId}) async {
    final generation = ++_paymentMethodsGeneration;
    emit(
        state.copyWith(loadingPaymentMethods: true, paymentMethodsError: null));
    try {
      final methods = await repo.paymentMethods(profileId: profileId);
      if (isClosed || generation != _paymentMethodsGeneration) return;
      final primary = methods.where((m) => m.isPrimary);
      final selected = primary.isNotEmpty
          ? primary.first.id
          : (methods.isNotEmpty ? methods.first.id : null);
      emit(state.copyWith(
        loadingPaymentMethods: false,
        paymentMethods: methods,
        selectedPaymentMethodId: selected,
      ));
    } catch (e) {
      if (isClosed || generation != _paymentMethodsGeneration) return;
      emit(state.copyWith(
        loadingPaymentMethods: false,
        paymentMethodsError: e is ApiException
            ? e.userMessage
            : 'Unable to load saved payment methods.',
      ));
    }
  }

  void selectPaymentMethod(String id) =>
      emit(state.copyWith(selectedPaymentMethodId: id));

  /// Saves a new M-Pesa number and immediately selects it — a patient who
  /// just added their first method should not have to tap it again to pick
  /// it. [profileId] mirrors [loadPaymentMethods]'s own scoping.
  Future<bool> addMpesaPaymentMethod(String phoneNumber,
      {String? profileId}) async {
    if (state.addingPaymentMethod) return false;
    emit(
        state.copyWith(addingPaymentMethod: true, addPaymentMethodError: null));
    try {
      final method =
          await repo.addMpesaPaymentMethod(phoneNumber, profileId: profileId);
      if (isClosed) return true;
      emit(state.copyWith(
        addingPaymentMethod: false,
        paymentMethods: [...state.paymentMethods, method],
        selectedPaymentMethodId: method.id,
      ));
      return true;
    } catch (e) {
      if (isClosed) return false;
      emit(state.copyWith(
        addingPaymentMethod: false,
        addPaymentMethodError: e is ApiException
            ? e.userMessage
            : 'Unable to save this payment method.',
      ));
      return false;
    }
  }

  /// [profileId]: the id to book for (`null` = self) — resolved by the
  /// caller from the active Family Profiles selection (§5b), threaded
  /// through rather than owned here, since this cubit has no visibility into
  /// `FamilyProfilesCubit`.
  Future<bool> submit({String? profileId}) async {
    if (!state.canProceed) return false;
    emit(state.copyWith(submitting: true, submitError: null));
    try {
      final result = await repo.book(
        doctorId: doctorId,
        slotId: state.selectedSlotId!,
        modality: modality,
        paymentMethodId: state.selectedPaymentMethodId!,
        chiefComplaint: state.chiefComplaint,
        profileId: profileId,
      );
      if (isClosed) return true;
      emit(state.copyWith(submitting: false, result: result));
      return true;
    } catch (e) {
      if (isClosed) return false;
      emit(state.copyWith(
        submitting: false,
        submitError: e is ApiException
            ? e.userMessage
            : 'Unable to complete this booking.',
      ));
      return false;
    }
  }

  Future<bool> uploadIntake(List<Map<String, String>> files) async {
    final appointmentId = state.result?.appointmentId;
    if (appointmentId == null || state.uploading) return false;
    emit(state.copyWith(uploading: true, uploadError: null));
    try {
      final count = await repo.uploadIntake(appointmentId, files);
      if (isClosed) return true;
      emit(state.copyWith(uploading: false, uploadedCount: count));
      return true;
    } catch (e) {
      if (isClosed) return false;
      emit(state.copyWith(
        uploading: false,
        uploadError:
            e is ApiException ? e.userMessage : 'Unable to upload these files.',
      ));
      return false;
    }
  }
}
