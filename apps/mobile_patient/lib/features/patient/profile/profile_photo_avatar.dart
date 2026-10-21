import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show PlatformException;
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:image_picker/image_picker.dart';

import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';
import 'profile_cubit.dart';

/// Best-effort deletion of image_picker's own on-disk cache copy — same
/// rationale and shape as `pre_consult_intake_modal.dart`'s
/// `deletePickedSourceFile` (CLAUDE.md requires photos not be left sitting
/// in generic disk caches once this app has read their bytes into memory);
/// kept as its own small copy rather than importing across features, the
/// same independence the Doctor app's own Rx-scanner cleanup already keeps.
Future<void> deletePickedPhotoSourceFile(String path) async {
  try {
    final f = File(path);
    if (await f.exists()) await f.delete();
  } catch (_) {
    // best-effort only
  }
}

String mimeTypeForPickedPhoto(String path) {
  final lower = path.toLowerCase();
  if (lower.endsWith('.png')) return 'image/png';
  return 'image/jpeg';
}

/// Circular profile-photo display + tap-to-change affordance, backed by
/// [ProfileCubit] so every place this widget appears shares the same
/// loaded/cached bytes. Entirely optional and non-blocking by design: a
/// picker, permission, or upload failure only ever surfaces as a transient
/// snackbar — it never throws up to a caller or blocks anything else on the
/// screen (registration completion in particular must succeed regardless).
/// Compression and the 40KB ceiling are enforced server-side only; this
/// widget sends the picked image's bytes unmodified, matching the
/// established prescription-intake precedent (`upload_validation.dart`).
class ProfilePhotoAvatar extends StatefulWidget {
  const ProfilePhotoAvatar({super.key, this.radius = 28});
  final double radius;

  @override
  State<ProfilePhotoAvatar> createState() => _ProfilePhotoAvatarState();
}

class _ProfilePhotoAvatarState extends State<ProfilePhotoAvatar> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final cubit = context.read<ProfileCubit>();
      if (cubit.state.photoBytes == null && !cubit.state.photoLoading) {
        cubit.loadPhoto();
      }
    });
  }

  Future<void> _pick(ImageSource source) async {
    if (!mounted) return;
    final l10n = AppLocalizations.of(context);
    final cubit = context.read<ProfileCubit>();
    final messenger = ScaffoldMessenger.of(context);
    try {
      final picked =
          await ImagePicker().pickImage(source: source, imageQuality: 85);
      if (picked == null) return;
      final bytes = await picked.readAsBytes();
      final mimeType = mimeTypeForPickedPhoto(picked.path);
      await deletePickedPhotoSourceFile(picked.path);
      final ok = await cubit.uploadPhoto(bytes, mimeType);
      if (!ok) {
        messenger.showSnackBar(SnackBar(
            content:
                Text(cubit.state.photoError ?? l10n.profilePhotoUploadError)));
      }
    } on PlatformException {
      // Camera/gallery access denied or unavailable — report inline, never
      // throw: this feature is entirely optional.
      messenger.showSnackBar(
          SnackBar(content: Text(l10n.profilePhotoPermissionDenied)));
    } catch (_) {
      messenger
          .showSnackBar(SnackBar(content: Text(l10n.profilePhotoUploadError)));
    }
  }

  Future<void> _remove() async {
    final l10n = AppLocalizations.of(context);
    final cubit = context.read<ProfileCubit>();
    final messenger = ScaffoldMessenger.of(context);
    final ok = await cubit.removePhoto();
    if (!ok) {
      messenger.showSnackBar(SnackBar(
          content: Text(cubit.state.photoError ?? l10n.profilePhotoRemoveError)));
    }
  }

  void _openPicker() {
    final l10n = AppLocalizations.of(context);
    final hasPhoto = context.read<ProfileCubit>().state.photoBytes != null;
    showModalBottomSheet<void>(
      context: context,
      builder: (sheetContext) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.photo_camera_outlined),
              title: Text(l10n.cameraButton),
              onTap: () {
                Navigator.of(sheetContext).pop();
                _pick(ImageSource.camera);
              },
            ),
            ListTile(
              leading: const Icon(Icons.photo_library_outlined),
              title: Text(l10n.galleryButton),
              onTap: () {
                Navigator.of(sheetContext).pop();
                _pick(ImageSource.gallery);
              },
            ),
            if (hasPhoto)
              ListTile(
                leading:
                    const Icon(Icons.delete_outline, color: PatientTokens.danger),
                title: Text(l10n.profilePhotoRemove,
                    style: const TextStyle(color: PatientTokens.danger)),
                onTap: () {
                  Navigator.of(sheetContext).pop();
                  _remove();
                },
              ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return BlocBuilder<ProfileCubit, ProfileState>(
      buildWhen: (a, b) =>
          a.photoBytes != b.photoBytes ||
          a.photoSaving != b.photoSaving ||
          a.photoLoading != b.photoLoading,
      builder: (context, state) {
        final iconSize = widget.radius;
        return GestureDetector(
          onTap: state.photoSaving ? null : _openPicker,
          child: Stack(
            clipBehavior: Clip.none,
            children: [
              CircleAvatar(
                radius: widget.radius,
                backgroundColor: PatientTokens.primaryLight,
                backgroundImage: state.photoBytes != null
                    ? MemoryImage(state.photoBytes!)
                    : null,
                child: state.photoBytes != null
                    ? null
                    : state.photoLoading
                        ? SizedBox(
                            width: iconSize,
                            height: iconSize,
                            child: const CircularProgressIndicator(
                                strokeWidth: 2))
                        : Icon(Icons.person,
                            size: iconSize, color: PatientTokens.primaryDark),
              ),
              if (state.photoSaving)
                Positioned.fill(
                  child: CircleAvatar(
                    radius: widget.radius,
                    backgroundColor: Colors.black38,
                    child: const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white)),
                  ),
                ),
              Positioned(
                bottom: -2,
                right: -2,
                child: Container(
                  padding: const EdgeInsets.all(3),
                  decoration: const BoxDecoration(
                      color: Colors.white, shape: BoxShape.circle),
                  child: Icon(Icons.camera_alt,
                      size: widget.radius * 0.5,
                      color: PatientTokens.primaryDark),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}
