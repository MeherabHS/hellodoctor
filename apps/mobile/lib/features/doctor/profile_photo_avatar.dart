import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show PlatformException;
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:image_picker/image_picker.dart';

import '../../core/theme/tokens.dart';
import 'doctor_cubit.dart';

/// Best-effort deletion of image_picker's own on-disk cache copy — same
/// rationale and shape as this app's existing `cleanupPickedRxFile`
/// (`doctor_pages.dart`): CLAUDE.md requires photos not be left sitting in
/// generic disk caches once this app has read their bytes into memory. Kept
/// as its own small copy rather than sharing code across features, matching
/// how `cleanupPickedRxFile` is itself independent of the Patient app's
/// identical-in-spirit cleanup helper.
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
/// [DoctorCubit]. The Doctor app has no localization infrastructure (see
/// `RegistrationPage`/`ProfilePage`'s own hardcoded-English convention), so
/// this widget's strings are plain literals, matching that convention
/// rather than introducing ARB-based localization as a side effect of this
/// feature. Entirely optional and non-blocking: a picker, permission, or
/// upload failure only ever surfaces as a transient snackbar.
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
      final cubit = context.read<DoctorCubit>();
      if (cubit.state.photoBytes == null && !cubit.state.photoLoading) {
        cubit.loadPhoto();
      }
    });
  }

  Future<void> _pick(ImageSource source) async {
    if (!mounted) return;
    final cubit = context.read<DoctorCubit>();
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
            content: Text(cubit.state.photoError ??
                'Unable to upload your photo. Please try again.')));
      }
    } on PlatformException {
      messenger.showSnackBar(const SnackBar(
          content: Text(
              'Camera access was denied. You can still choose a photo from your gallery.')));
    } catch (_) {
      messenger.showSnackBar(const SnackBar(
          content: Text('Unable to upload your photo. Please try again.')));
    }
  }

  Future<void> _remove() async {
    final cubit = context.read<DoctorCubit>();
    final messenger = ScaffoldMessenger.of(context);
    final ok = await cubit.removePhoto();
    if (!ok) {
      messenger.showSnackBar(SnackBar(
          content: Text(cubit.state.photoError ??
              'Unable to remove your photo. Please try again.')));
    }
  }

  void _openPicker() {
    final hasPhoto = context.read<DoctorCubit>().state.photoBytes != null;
    showModalBottomSheet<void>(
      context: context,
      builder: (sheetContext) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.photo_camera_outlined),
              title: const Text('Camera'),
              onTap: () {
                Navigator.of(sheetContext).pop();
                _pick(ImageSource.camera);
              },
            ),
            ListTile(
              leading: const Icon(Icons.photo_library_outlined),
              title: const Text('Choose Photo'),
              onTap: () {
                Navigator.of(sheetContext).pop();
                _pick(ImageSource.gallery);
              },
            ),
            if (hasPhoto)
              ListTile(
                leading: const Icon(Icons.delete_outline,
                    color: DoctorTokens.danger),
                title: const Text('Remove Photo',
                    style: TextStyle(color: DoctorTokens.danger)),
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
    return BlocBuilder<DoctorCubit, DoctorState>(
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
                backgroundColor: DoctorTokens.primaryLight,
                backgroundImage: state.photoBytes != null
                    ? MemoryImage(state.photoBytes!)
                    : null,
                child: state.photoBytes != null
                    ? null
                    : state.photoLoading
                        ? SizedBox(
                            width: iconSize,
                            height: iconSize,
                            child:
                                const CircularProgressIndicator(strokeWidth: 2))
                        : Icon(Icons.person,
                            size: iconSize, color: DoctorTokens.primaryDark),
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
                      color: DoctorTokens.primaryDark),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}
