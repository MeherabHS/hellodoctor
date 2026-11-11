import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import 'directory_models.dart';
import 'directory_repository.dart';

/// Read-only circular avatar for a doctor's photo, shown to patients on the
/// home screen, doctor directory listings, and the doctor profile page.
/// Takes primitives rather than a full [PublicDoctorDto] so screens that
/// only have an appointment summary (home's upcoming-consultation card) can
/// use it too, not only screens with the full directory DTO. Falls back to
/// initials only when the doctor genuinely has no photo set
/// ([hasPhoto] == false) or the fetch fails — never while a real photo is
/// still loading, to avoid an initials-then-photo flash. Keyed by
/// `doctorId:photoUpdatedAt` in a small process-lifetime in-memory cache so
/// a doctor's photo change (new [photoUpdatedAt]) is always re-fetched, and
/// so the same photo isn't re-downloaded every time this widget appears
/// across different screens in one session.
class DoctorPhotoAvatar extends StatefulWidget {
  const DoctorPhotoAvatar({
    super.key,
    required this.doctorId,
    required this.initials,
    required this.hasPhoto,
    required this.photoUpdatedAt,
    required this.repo,
    this.radius = 28,
  });

  factory DoctorPhotoAvatar.forDoctor({
    Key? key,
    required PublicDoctorDto doctor,
    required DirectoryRepository repo,
    double radius = 28,
  }) =>
      DoctorPhotoAvatar(
        key: key,
        doctorId: doctor.id,
        initials: doctor.initials,
        hasPhoto: doctor.hasPhoto,
        photoUpdatedAt: doctor.photoUpdatedAt,
        repo: repo,
        radius: radius,
      );

  final String doctorId;
  final String initials;
  final bool hasPhoto;
  final DateTime? photoUpdatedAt;
  final DirectoryRepository repo;
  final double radius;

  @override
  State<DoctorPhotoAvatar> createState() => _DoctorPhotoAvatarState();
}

class _DoctorPhotoAvatarState extends State<DoctorPhotoAvatar> {
  static final Map<String, Uint8List> _cache = {};

  Uint8List? _bytes;

  String get _cacheKey =>
      '${widget.doctorId}:${widget.photoUpdatedAt?.toIso8601String() ?? ''}';

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(covariant DoctorPhotoAvatar old) {
    super.didUpdateWidget(old);
    if (old.doctorId != widget.doctorId ||
        old.photoUpdatedAt != widget.photoUpdatedAt) {
      _bytes = null;
      _load();
    }
  }

  Future<void> _load() async {
    if (!widget.hasPhoto) return;
    final cached = _cache[_cacheKey];
    if (cached != null) {
      if (mounted) setState(() => _bytes = cached);
      return;
    }
    try {
      final bytes = await widget.repo.fetchDoctorPhotoBytes(widget.doctorId);
      if (bytes != null) _cache[_cacheKey] = bytes;
      if (mounted) setState(() => _bytes = bytes);
    } catch (_) {
      // Best-effort only — initials remain the fallback.
    }
  }

  @override
  Widget build(BuildContext context) => CircleAvatar(
        radius: widget.radius,
        backgroundColor: PatientTokens.surfaceSubtle,
        backgroundImage: _bytes != null ? MemoryImage(_bytes!) : null,
        child: _bytes != null
            ? null
            : Text(widget.initials,
                style: TextStyle(
                    fontSize: widget.radius * 0.6,
                    fontWeight: FontWeight.w600,
                    color: PatientTokens.primaryDark)),
      );
}
