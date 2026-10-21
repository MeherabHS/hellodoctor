import '../../../core/network/availability_refresh.dart';
// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';

import '../../../core/cache/patient_cache_store.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/connectivity_service.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../core/l10n/patient_labels.dart';
import '../../../l10n/app_localizations.dart';
import '../booking/booking_models.dart';
import '../booking/booking_repository.dart';
import '../booking/paid_chat_booking_modal.dart';
import '../booking/patient_booking_page.dart';
import '../profile/family_profiles_cubit.dart';
import 'directory_models.dart';
import 'directory_repository.dart';

/// `prototype/index.html` view-9 — Doctor Profile. The "Availability" tab
/// deliberately does NOT reproduce the prototype's static
/// "Saturday–Thursday 09:00–20:00" weekly-schedule copy: no backend contract
/// exposes a doctor's recurring weekly template (only ad-hoc
/// `doctor_schedule_slots` rows), and inventing one would violate CLAUDE.md's
/// "No Unilateral Feature Inventions" rule. It instead shows the real next
/// few bookable slots, fetched from `GET /doctors/:id/slots`.
///
/// The favorite-star toggle is local-only UI state (plan §4, view-9 row
/// confirms no backend support exists for it) — not persisted, not faked as
/// synced.
class DoctorProfilePage extends StatefulWidget {
  const DoctorProfilePage({
    super.key,
    required this.doctorId,
    required this.api,
    required this.cache,
    required this.connectivity,
    required this.familyProfilesCubit,
  });
  final String doctorId;
  final ApiClient api;
  final PatientCacheStore? cache;
  final ConnectivityService connectivity;
  final FamilyProfilesCubit familyProfilesCubit;

  @override
  State<DoctorProfilePage> createState() => _DoctorProfilePageState();
}

class _DoctorProfilePageState extends State<DoctorProfilePage> {
  late final DirectoryRepository _repo;
  late final BookingRepository _bookingRepo;
  late Future<PublicDoctorDto> _future;
  Future<List<DoctorSlotDto>>? _slotsFuture;
  String _tab = 'about';
  bool _favorited = false;
  late final AvailabilityRefresh _availabilityRefresh;

  @override
  void dispose() {
    _availabilityRefresh.dispose();
    super.dispose();
  }

  @override
  void initState() {
    super.initState();
    _repo = DirectoryRepository(widget.api,
        cache: widget.cache, connectivity: widget.connectivity);
    _bookingRepo =
        BookingRepository(widget.api, connectivity: widget.connectivity);
    _future = _repo.doctor(widget.doctorId);
    _availabilityRefresh = AvailabilityRefresh(() {
      if (mounted && _tab == 'availability') {
        setState(() => _slotsFuture = _bookingRepo.slots(widget.doctorId));
      }
    }, widget.connectivity);
  }

  void _openBooking(PublicDoctorDto doctor, String modality) {
    final repo =
        BookingRepository(widget.api, connectivity: widget.connectivity);
    if (modality == 'CHAT') {
      showPaidChatBookingModal(context,
          doctor: doctor,
          repo: repo,
          familyProfilesCubit: widget.familyProfilesCubit);
      return;
    }
    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => PatientBookingPage(
        doctor: doctor,
        modality: modality,
        repo: repo,
        familyProfilesCubit: widget.familyProfilesCubit,
      ),
    ));
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _availabilityRefresh.attach(context);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 0),
            child: Row(children: [
              IconButton(
                onPressed: () => Navigator.of(context).maybePop(),
                icon: const Icon(Icons.arrow_back, size: 20),
              ),
              Expanded(
                child: Column(children: [
                  Text(AppLocalizations.of(context).doctorProfileTitle,
                      style: TextStyle(
                          fontSize: 16,
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text)),
                  Text(AppLocalizations.of(context).doctorProfileSubtitle,
                      style: TextStyle(
                          fontSize: 11, color: PatientTokens.textSub)),
                ]),
              ),
              IconButton(
                onPressed: () => setState(() => _favorited = !_favorited),
                icon: Icon(_favorited ? Icons.favorite : Icons.favorite_border,
                    color: _favorited
                        ? PatientTokens.rose
                        : PatientTokens.textSub),
              ),
            ]),
          ),
          Expanded(
            child: FutureBuilder<PublicDoctorDto>(
              future: _future,
              builder: (context, snapshot) {
                if (snapshot.connectionState != ConnectionState.done) {
                  return ListView(
                    padding: const EdgeInsets.all(18),
                    children: const [SkeletonBlock(height: 220)],
                  );
                }
                if (snapshot.hasError) {
                  return ErrorPanel(
                      AppLocalizations.of(context).doctorProfileLoadError,
                      onRetry: () => setState(
                          () => _future = _repo.doctor(widget.doctorId)));
                }
                final doctor = snapshot.data!;
                return ListView(
                  padding: const EdgeInsets.fromLTRB(18, 8, 18, 24),
                  children: [
                    DoctorProfileSummary(doctor: doctor),
                    const SizedBox(height: 14),
                    Wrap(spacing: 8, runSpacing: 8, children: [
                      _TabButton(
                          label: AppLocalizations.of(context).doctorAboutTab,
                          active: _tab == 'about',
                          onTap: () => setState(() => _tab = 'about')),
                      const SizedBox(width: 8),
                      _TabButton(
                          label: AppLocalizations.of(context)
                              .doctorAvailabilityTab,
                          active: _tab == 'availability',
                          onTap: () {
                            setState(() {
                              _tab = 'availability';
                              _slotsFuture =
                                  _bookingRepo.slots(widget.doctorId);
                            });
                          }),
                    ]),
                    const SizedBox(height: 10),
                    if (_tab == 'about')
                      _AboutCard(doctor: doctor)
                    else
                      _AvailabilityCard(future: _slotsFuture!),
                    const SizedBox(height: 16),
                    if (doctor.supportsVideo) ...[
                      Text(
                          '${AppLocalizations.of(context).consultVideo} - KES ${doctor.consultationFeeVideo}'),
                      const SizedBox(height: 8),
                      FilledButton(
                          onPressed: () => _openBooking(doctor, 'VIDEO'),
                          child: Text(
                              AppLocalizations.of(context).consultBookVideo,
                              textAlign: TextAlign.center)),
                      const SizedBox(height: 16),
                    ],
                    if (doctor.supportsChat) ...[
                      Text(
                          '${AppLocalizations.of(context).consultChat} - KES ${doctor.consultationFeeChat}'),
                      const SizedBox(height: 8),
                      OutlinedButton(
                          onPressed: () => _openBooking(doctor, 'CHAT'),
                          child: Text(
                              AppLocalizations.of(context).consultBookChat,
                              textAlign: TextAlign.center)),
                    ],
                  ],
                );
              },
            ),
          ),
        ]),
      ),
    );
  }
}

class DoctorProfileSummary extends StatelessWidget {
  const DoctorProfileSummary({super.key, required this.doctor});
  final PublicDoctorDto doctor;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Column(children: [
      CircleAvatar(
          radius: 36,
          backgroundColor: PatientTokens.surfaceSubtle,
          child: Text(doctor.initials,
              style: const TextStyle(
                  fontSize: 22,
                  fontWeight: FontWeight.w600,
                  color: PatientTokens.primaryDark))),
      const SizedBox(height: 16),
      Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Text(doctor.displayName,
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.titleLarge)),
      const SizedBox(height: 8),
      Text(patientSpecialtyLabel(l, doctor.primarySpecialty),
          textAlign: TextAlign.center),
      const SizedBox(height: 12),
      Wrap(
          alignment: WrapAlignment.center,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 6,
          children: [
            Icon(Icons.verified_outlined,
                color: PatientTokens.primary,
                size: 18,
                semanticLabel: l.profileVerified),
            Text(l.profileVerified,
                style: const TextStyle(color: PatientTokens.primary))
          ]),
      const SizedBox(height: 16),
      Text(doctor.currentHospital, textAlign: TextAlign.center),
      const SizedBox(height: 4),
      Text(l.profileExperienceLine(doctor.experienceYears.toString())),
    ]);
  }
}

class _TabButton extends StatelessWidget {
  const _TabButton(
      {required this.label, required this.active, required this.onTap});
  final String label;
  final bool active;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(10),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
          decoration: BoxDecoration(
            color: active ? PatientTokens.primaryLight : Colors.transparent,
            borderRadius: BorderRadius.circular(10),
          ),
          child: Text(label,
              style: TextStyle(
                  fontWeight: FontWeight.w700,
                  color: active
                      ? PatientTokens.primaryDark
                      : PatientTokens.textSub)),
        ),
      );
}

class _AboutCard extends StatelessWidget {
  const _AboutCard({required this.doctor});
  final PublicDoctorDto doctor;
  @override
  Widget build(BuildContext context) => PatientCard(
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(
              AppLocalizations.of(context)
                  .doctorAboutTitle(_shortName(doctor.displayName)),
              style: const TextStyle(
                  fontWeight: FontWeight.w800, color: PatientTokens.text)),
          const SizedBox(height: 6),
          Text(
              '${doctor.qualifications.join(', ')}${doctor.qualifications.isNotEmpty ? ' — ' : ''}'
              '${AppLocalizations.of(context).doctorPractice(doctor.currentHospital, doctor.licenseCountry)}',
              style: const TextStyle(
                  fontSize: 11.5, color: PatientTokens.textSub, height: 1.5)),
          const Divider(height: 24),
          Text(AppLocalizations.of(context).profileCredentials,
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          Text(doctor.licenseNumber.startsWith(doctor.licenseAuthority)
              ? doctor.licenseNumber
              : '${doctor.licenseAuthority} - ${doctor.licenseNumber}'),
        ]),
      );

  String _shortName(String fullName) {
    final parts = fullName.replaceAll('Dr.', '').trim().split(RegExp(r'\s+'));
    return parts.isEmpty ? fullName : parts.first;
  }
}

class _AvailabilityCard extends StatelessWidget {
  const _AvailabilityCard({required this.future});
  final Future<List<DoctorSlotDto>> future;
  @override
  Widget build(BuildContext context) => PatientCard(
        child: FutureBuilder<List<DoctorSlotDto>>(
          future: future,
          builder: (context, snapshot) {
            if (snapshot.connectionState != ConnectionState.done) {
              return const SkeletonBlock(height: 60);
            }
            final slots = snapshot.data ?? const <DoctorSlotDto>[];
            if (slots.isEmpty) {
              return Text(AppLocalizations.of(context).doctorNoSlots,
                  style: TextStyle(color: PatientTokens.textSub));
            }
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                    AppLocalizations.of(context)
                        .doctorSlotCount(slots.length.toString()),
                    style: const TextStyle(
                        fontWeight: FontWeight.w800,
                        color: PatientTokens.text)),
                const SizedBox(height: 8),
                for (final slot in slots.take(5))
                  Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: Text(_format(slot.startTime),
                        style: const TextStyle(
                            fontSize: 12, color: PatientTokens.textSub)),
                  ),
              ],
            );
          },
        ),
      );

  String _format(DateTime t) =>
      '${t.day}/${t.month} • ${t.hour.toString().padLeft(2, '0')}:${t.minute.toString().padLeft(2, '0')}';
}
