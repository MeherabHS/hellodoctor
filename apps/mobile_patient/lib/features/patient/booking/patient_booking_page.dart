import '../../../core/network/availability_refresh.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../directory/directory_models.dart';
import '../profile/family_profile_chip_row.dart';
import '../profile/family_profiles_cubit.dart';
import 'add_payment_method_sheet.dart';
import 'booking_cubit.dart';
import 'booking_models.dart';

import 'booking_repository.dart';

import '../chat/consultation_status_page.dart';
import '../home/home_models.dart';

/// `prototype/index.html` view-4 — "Booking Flow & Slots". Real date/slot
/// selection replaces the prototype's six hardcoded placeholder days and
/// pre-selected slot (plan §4's view-4 row: "add real date/slot-required
/// validation the prototype lacks") — [BookingState.canProceed] only turns
/// true once the patient has actually picked both, and the payment amount
/// shown is always the real `consultation_fee_video`/`consultation_fee_chat`
/// already confirmed on [doctor], never invented.
///
/// Carries the M3.5 "Booking for: [Self ▾]" profile selector from the start
/// (plan §5b) via [familyProfilesCubit], passed in directly rather than
/// resolved from `BuildContext` — this page is reached via `Navigator.push`
/// from Directory/Doctor Profile, a sibling route that does not inherit the
/// `BlocProvider`s declared inside the Home/Account tabs' own route.
class PatientBookingPage extends StatefulWidget {
  const PatientBookingPage({
    super.key,
    required this.doctor,
    required this.modality,
    required this.repo,
    required this.familyProfilesCubit,
  });
  final PublicDoctorDto doctor;
  final String modality;
  final BookingRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;

  @override
  State<PatientBookingPage> createState() => _PatientBookingPageState();
}

class _PatientBookingPageState extends State<PatientBookingPage> {
  late final BookingCubit _cubit;
  late final AvailabilityRefresh _availabilityRefresh;
  final _complaintController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _cubit = BookingCubit(widget.repo,
        doctorId: widget.doctor.id, modality: widget.modality)
      ..loadSlots()
      ..loadBookingPolicy()
      ..loadPaymentMethods(
          profileId: widget.familyProfilesCubit.state.activeProfileId);
    _availabilityRefresh = AvailabilityRefresh(() {
      if (mounted && !_cubit.state.submitting && _cubit.state.result == null) {
        _cubit.loadSlots();
      }
    }, widget.repo.connectivity);
  }

  @override
  void dispose() {
    _availabilityRefresh.dispose();
    _cubit.close();
    _complaintController.dispose();
    super.dispose();
  }

  String get _fee => widget.modality == 'VIDEO'
      ? widget.doctor.consultationFeeVideo
      : widget.doctor.consultationFeeChat;

  Future<void> _submit() async {
    final activeProfileId = widget.familyProfilesCubit.state.activeProfileId;
    final ok = await _cubit.submit(profileId: activeProfileId);
    if (!mounted || !ok) return;
    final result = _cubit.state.result!;
    final selfProfiles = widget.familyProfilesCubit.state.profiles
        .where((p) => p.isSelf)
        .toList();
    final myUserId = selfProfiles.isEmpty ? null : selfProfiles.first.userId;
    final slot = _cubit.state.slots
        .firstWhere((s) => s.id == _cubit.state.selectedSlotId);
    await Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => ConsultationStatusPage(
              appointment: AppointmentSummaryDto(
                  id: result.appointmentId,
                  appointmentNumber: result.appointmentNumber,
                  doctorId: widget.doctor.id,
                  doctorDisplayName: widget.doctor.displayName,
                  primarySpecialty: widget.doctor.primarySpecialty,
                  modality: widget.modality,
                  status: result.status,
                  startTime: slot.startTime,
                  endTime: slot.endTime,
                  fee: result.amount,
                  paymentPolicy: result.paymentPolicy),
              repo: widget.repo,
              myUserId: myUserId,
              familyProfilesCubit: widget.familyProfilesCubit,
            )));
    if (mounted) Navigator.of(context).popUntil((route) => route.isFirst);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _availabilityRefresh.attach(context);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return BlocProvider.value(
      value: _cubit,
      child: BlocListener<FamilyProfilesCubit, FamilyProfilesState>(
        bloc: widget.familyProfilesCubit,
        listenWhen: (previous, current) =>
            previous.activeProfileId != current.activeProfileId,
        listener: (context, state) =>
            _cubit.loadPaymentMethods(profileId: state.activeProfileId),
        child: Scaffold(
          backgroundColor: PatientTokens.bg,
          body: SafeArea(
            child: Column(children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(8, 8, 8, 0),
                child: Row(children: [
                  IconButton(
                    onPressed: () => Navigator.of(context).maybePop(),
                    icon: const Icon(Icons.arrow_back, size: 20),
                  ),
                  Text(
                      widget.modality == 'VIDEO'
                          ? l10n.bookingVideoTitle
                          : l10n.bookingChatTitle,
                      style: const TextStyle(
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text)),
                ]),
              ),
              _DoctorBanner(doctor: widget.doctor),
              Expanded(
                child: BlocBuilder<BookingCubit, BookingState>(
                  builder: (context, state) {
                    return ListView(
                      padding: const EdgeInsets.fromLTRB(18, 10, 18, 110),
                      children: [
                        _BookingForBar(cubit: widget.familyProfilesCubit),
                        const SizedBox(height: 14),
                        if (state.loadingSlots)
                          const SkeletonBlock(height: 140)
                        else if (state.slotsError != null)
                          InlineError(state.slotsError!,
                              onRetry: _cubit.loadSlots)
                        else if (state.availableDates.isEmpty)
                          PatientCard(
                            child: Text(l10n.bookingNoSlots,
                                style: const TextStyle(
                                    color: PatientTokens.textSub)),
                          )
                        else ...[
                          Text(l10n.bookingSelectDate,
                              style: const TextStyle(
                                  fontWeight: FontWeight.w800,
                                  color: PatientTokens.text)),
                          const SizedBox(height: 8),
                          _DateSlider(state: state, cubit: _cubit),
                          const SizedBox(height: 14),
                          Text(l10n.bookingAvailableSlots,
                              style: const TextStyle(
                                  fontWeight: FontWeight.w800,
                                  color: PatientTokens.text)),
                          const SizedBox(height: 2),
                          Text(l10n.bookingTimesEat,
                              style: const TextStyle(
                                  fontSize: 11, color: PatientTokens.textSub)),
                          const SizedBox(height: 8),
                          _SlotGrid(state: state, cubit: _cubit),
                        ],
                        const SizedBox(height: 16),
                        Text(l10n.bookingReasonLabel,
                            style: const TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w700,
                                color: PatientTokens.textSub)),
                        const SizedBox(height: 6),
                        TextField(
                          controller: _complaintController,
                          maxLines: 2,
                          maxLength: 1000,
                          onChanged: _cubit.setChiefComplaint,
                          decoration: InputDecoration(
                            hintText: l10n.bookingReasonHint,
                          ),
                        ),
                        const SizedBox(height: 10),
                        if (state.paymentRequired) ...[
                          _FeeSummary(
                              doctor: widget.doctor, modality: widget.modality),
                          const SizedBox(height: 14),
                          _PaymentMethodPicker(
                            state: state,
                            cubit: _cubit,
                            profileId: widget
                                .familyProfilesCubit.state.activeProfileId,
                          ),
                        ] else
                          PatientCard(
                            child: Row(children: [
                              const Icon(Icons.check_circle_outline,
                                  color: PatientTokens.primary, size: 18),
                              const SizedBox(width: 8),
                              Expanded(
                                  child: Text(
                                      l10n.bookingNoPaymentRequiredNotice,
                                      style: const TextStyle(
                                          fontWeight: FontWeight.w700,
                                          color: PatientTokens.text))),
                            ]),
                          ),
                        if (state.submitError != null) ...[
                          const SizedBox(height: 6),
                          InlineError(state.submitError!),
                        ],
                      ],
                    );
                  },
                ),
              ),
            ]),
          ),
          bottomNavigationBar: SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: BlocBuilder<BookingCubit, BookingState>(
                builder: (context, state) => SizedBox(
                  width: double.infinity,
                  child: FilledButton(
                    onPressed: state.canProceed ? _submit : null,
                    style: FilledButton.styleFrom(
                        backgroundColor: PatientTokens.primary,
                        padding: const EdgeInsets.symmetric(vertical: 14)),
                    child: state.submitting
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(
                                strokeWidth: 2, color: Colors.white))
                        : Text(
                            state.paymentRequired
                                ? l10n.bookingProceedToPayment(_fee)
                                : l10n.bookingConfirmNoPayment,
                            textAlign: TextAlign.center,
                            style:
                                const TextStyle(fontWeight: FontWeight.w800)),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _DoctorBanner extends StatelessWidget {
  const _DoctorBanner({required this.doctor});
  final PublicDoctorDto doctor;
  @override
  Widget build(BuildContext context) => Container(
        margin: const EdgeInsets.fromLTRB(18, 10, 18, 0),
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
            color: PatientTokens.surface,
            borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
            border: Border.all(color: PatientTokens.border)),
        child: Row(children: [
          CircleAvatar(
            radius: 20,
            backgroundColor: PatientTokens.primaryLight,
            child: Text(doctor.initials,
                style: const TextStyle(
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.primaryDark)),
          ),
          const SizedBox(width: 10),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(doctor.displayName,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontWeight: FontWeight.w800, color: PatientTokens.text)),
              Text(
                  '${doctor.primarySpecialty} • ${doctor.licenseAuthority} #${doctor.licenseNumber}',
                  style: const TextStyle(
                      fontSize: 11,
                      color: PatientTokens.primary,
                      fontWeight: FontWeight.w700)),
              Text(
                  '${doctor.currentHospital} • ${doctor.experienceYears}+ yrs clinical experience',
                  style: const TextStyle(
                      fontSize: 10.5, color: PatientTokens.textSub)),
            ]),
          ),
        ]),
      );
}

/// Compact entry point to the Family Profiles switcher, reused from Home
/// (§5b) — [cubit] is supplied directly since this page lives on a separate
/// pushed route from the tab that normally provides it via `BlocProvider`.
class _BookingForBar extends StatelessWidget {
  const _BookingForBar({required this.cubit});
  final FamilyProfilesCubit cubit;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return BlocBuilder<FamilyProfilesCubit, FamilyProfilesState>(
      bloc: cubit,
      builder: (context, state) {
        if (state.profiles.length <= 1) return const SizedBox.shrink();
        final active = state.activeProfile;
        final label = active == null || active.isSelf
            ? l10n.bookingForSelf
            : active.fullName;
        return InkWell(
          onTap: () => _open(context),
          borderRadius: BorderRadius.circular(PatientTokens.chipRadius),
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            decoration: BoxDecoration(
              color: PatientTokens.surface,
              borderRadius: BorderRadius.circular(PatientTokens.chipRadius),
              border: Border.all(color: PatientTokens.border),
            ),
            child: Row(children: [
              const Icon(Icons.person_outline,
                  size: 16, color: PatientTokens.primaryDark),
              const SizedBox(width: 8),
              Expanded(
                child: Text(l10n.bookingForBar(label),
                    style: const TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w700,
                        color: PatientTokens.text)),
              ),
              const Icon(Icons.unfold_more,
                  size: 16, color: PatientTokens.textSub),
            ]),
          ),
        );
      },
    );
  }

  void _open(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (sheetContext) => Padding(
        padding: const EdgeInsets.fromLTRB(0, 20, 0, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20),
              child: Text(l10n.bookingForLabel,
                  style: const TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.text)),
            ),
            const SizedBox(height: 14),
            BlocBuilder<FamilyProfilesCubit, FamilyProfilesState>(
              bloc: cubit,
              builder: (context, state) => FamilyProfileChipRow(
                profiles: state.profiles,
                activeProfileId: state.activeProfileId,
                onSelect: (id) {
                  cubit.setActiveProfile(id);
                  Navigator.of(sheetContext).pop();
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _DateSlider extends StatelessWidget {
  const _DateSlider({required this.state, required this.cubit});
  final BookingState state;
  final BookingCubit cubit;
  static const _days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

  @override
  Widget build(BuildContext context) => SizedBox(
        height: 62,
        child: ListView(
          scrollDirection: Axis.horizontal,
          children: [
            for (final date in state.availableDates)
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: _DateCard(
                  label: _days[date.weekday % 7],
                  day: date.day,
                  selected: state.selectedDate == date,
                  onTap: () => cubit.selectDate(date),
                ),
              ),
          ],
        ),
      );
}

class _DateCard extends StatelessWidget {
  const _DateCard(
      {required this.label,
      required this.day,
      required this.selected,
      required this.onTap});
  final String label;
  final int day;
  final bool selected;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(12),
        child: Container(
          width: 52,
          padding: const EdgeInsets.symmetric(vertical: 8),
          decoration: BoxDecoration(
            color: selected ? PatientTokens.primary : PatientTokens.surface,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
                color: selected ? PatientTokens.primary : PatientTokens.border),
          ),
          child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
            Text(label,
                style: TextStyle(
                    fontSize: 10,
                    color: selected ? Colors.white70 : PatientTokens.textSub)),
            Text('$day',
                style: TextStyle(
                    fontWeight: FontWeight.w800,
                    fontSize: 15,
                    color: selected ? Colors.white : PatientTokens.text)),
          ]),
        ),
      );
}

class _SlotGrid extends StatelessWidget {
  const _SlotGrid({required this.state, required this.cubit});
  final BookingState state;
  final BookingCubit cubit;

  @override
  Widget build(BuildContext context) {
    final slots = state.slotsForSelectedDate;
    if (slots.isEmpty) {
      return Text(AppLocalizations.of(context).bookingNoSlotsOnDate,
          style: const TextStyle(color: PatientTokens.textSub));
    }
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final slot in slots)
          InkWell(
            onTap: () => cubit.selectSlot(slot.id),
            borderRadius: BorderRadius.circular(10),
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 9),
              decoration: BoxDecoration(
                color: state.selectedSlotId == slot.id
                    ? PatientTokens.primary
                    : PatientTokens.surface,
                borderRadius: BorderRadius.circular(10),
                border: Border.all(
                    color: state.selectedSlotId == slot.id
                        ? PatientTokens.primary
                        : PatientTokens.border),
              ),
              child: Text(_format(slot.startTime),
                  style: TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w700,
                      color: state.selectedSlotId == slot.id
                          ? Colors.white
                          : PatientTokens.text)),
            ),
          ),
      ],
    );
  }

  String _format(DateTime t) {
    final hour12 = t.hour % 12 == 0 ? 12 : t.hour % 12;
    final suffix = t.hour >= 12 ? 'PM' : 'AM';
    return '${hour12.toString().padLeft(2, '0')}:${t.minute.toString().padLeft(2, '0')} $suffix';
  }
}

class _FeeSummary extends StatelessWidget {
  const _FeeSummary({required this.doctor, required this.modality});
  final PublicDoctorDto doctor;
  final String modality;
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final fee = modality == 'VIDEO'
        ? doctor.consultationFeeVideo
        : doctor.consultationFeeChat;
    return PatientCard(
      child: Column(children: [
        Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
          Text(modality == 'VIDEO' ? l10n.bookingFeeVideo : l10n.bookingFeeChat,
              style:
                  const TextStyle(fontSize: 12, color: PatientTokens.textSub)),
          Text(l10n.bookingFeeAmount(fee),
              style: const TextStyle(fontWeight: FontWeight.w700)),
        ]),
        const SizedBox(height: 6),
        Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
          Text(l10n.bookingPlatformGatewayFee,
              style:
                  const TextStyle(fontSize: 12, color: PatientTokens.textSub)),
          Text(l10n.bookingFree,
              style: const TextStyle(
                  color: PatientTokens.primary, fontWeight: FontWeight.w700)),
        ]),
        const Divider(height: 20),
        Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
          Text(l10n.bookingTotalPayable,
              style: const TextStyle(fontWeight: FontWeight.w800)),
          Text(l10n.bookingFeeAmount(fee),
              style: const TextStyle(
                  fontWeight: FontWeight.w800,
                  color: PatientTokens.primary,
                  fontSize: 16)),
        ]),
      ]),
    );
  }
}

/// Real checkout payment-method picker — replaces the old hardcoded
/// "M-PESA" chip this section used to show regardless of what the patient
/// actually had saved. Lists every saved `patient_payment_methods` row as a
/// selectable chip (mirrors `_SlotGrid`'s tap-to-select style, deliberately
/// not a `FilledButton` so the bottom nav's own submit button remains the
/// only `FilledButton` on this page), with an inline "+ Add" action when the
/// patient has none yet, or wants another.
class _PaymentMethodPicker extends StatelessWidget {
  const _PaymentMethodPicker(
      {required this.state, required this.cubit, required this.profileId});
  final BookingState state;
  final BookingCubit cubit;
  final String? profileId;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return PatientCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l10n.bookingPaymentMethodLabel,
              style: const TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w800,
                  color: PatientTokens.text)),
          const SizedBox(height: 10),
          if (state.loadingPaymentMethods)
            Text(l10n.bookingPaymentMethodLoading,
                style:
                    const TextStyle(fontSize: 12, color: PatientTokens.textSub))
          else if (state.paymentMethodsError != null)
            Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(state.paymentMethodsError!,
                  style: const TextStyle(
                      fontSize: 12, color: PatientTokens.danger)),
              TextButton(
                onPressed: () => cubit.loadPaymentMethods(profileId: profileId),
                child: Text(AppLocalizations.of(context).retryButton),
              ),
            ])
          else ...[
            if (state.paymentMethods.isEmpty)
              Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: Text(l10n.bookingPaymentMethodNone,
                    style: const TextStyle(
                        fontSize: 12, color: PatientTokens.textSub)),
              )
            else
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final method in state.paymentMethods)
                    _PaymentMethodChip(
                      method: method,
                      selected: state.selectedPaymentMethodId == method.id,
                      defaultLabel: l10n.bookingPaymentMethodDefaultBadge,
                      onTap: () => cubit.selectPaymentMethod(method.id),
                    ),
                ],
              ),
            const SizedBox(height: 10),
            OutlinedButton(
              onPressed: () =>
                  showAddPaymentMethodSheet(context, profileId: profileId),
              child: Text(state.paymentMethods.isEmpty
                  ? l10n.bookingAddMpesaNumber
                  : l10n.bookingAddAnotherMpesaNumber),
            ),
          ],
        ],
      ),
    );
  }
}

class _PaymentMethodChip extends StatelessWidget {
  const _PaymentMethodChip({
    required this.method,
    required this.selected,
    required this.defaultLabel,
    required this.onTap,
  });
  final PatientPaymentMethodDto method;
  final bool selected;
  final String defaultLabel;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(10),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
          decoration: BoxDecoration(
            color: selected ? PatientTokens.primary : PatientTokens.surface,
            borderRadius: BorderRadius.circular(10),
            border: Border.all(
                color: selected ? PatientTokens.primary : PatientTokens.border),
          ),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            Icon(selected ? Icons.check_circle : Icons.radio_button_unchecked,
                size: 16,
                color: selected ? Colors.white : PatientTokens.textSub),
            const SizedBox(width: 6),
            Text(method.maskedDestination,
                style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w700,
                    color: selected ? Colors.white : PatientTokens.text)),
            if (method.isPrimary) ...[
              const SizedBox(width: 6),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: selected
                      ? Colors.white.withValues(alpha: 0.2)
                      : PatientTokens.primaryLight,
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Text(defaultLabel,
                    style: TextStyle(
                        fontSize: 9,
                        fontWeight: FontWeight.w800,
                        color: selected
                            ? Colors.white
                            : PatientTokens.primaryDark)),
              ),
            ],
          ]),
        ),
      );
}
