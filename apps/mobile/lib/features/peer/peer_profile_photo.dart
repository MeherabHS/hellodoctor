import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show PlatformException;
import 'package:image_picker/image_picker.dart';

import '../../core/widgets/person_photo.dart';
import 'peer_repository.dart';
import 'peer_ui.dart';

/// The signed-in peer's saved photo, or [fallback] (their initials). Used by
/// the Home header and the Profile card; both read the same in-memory entry,
/// so a newly saved photo shows in both at once.
class PeerOwnPhoto extends StatelessWidget {
  const PeerOwnPhoto(
      {super.key,
      required this.repo,
      required this.size,
      required this.fallback});
  final PeerRepository repo;
  final double size;
  final Widget fallback;
  @override
  Widget build(BuildContext context) => PersonPhoto(
      key: const ValueKey('peer-own-photo'),
      api: repo.api,
      path: ownPeerPhotoPath,
      size: size,
      fallback: fallback);
}

/// The Profile card's photo: tap to take or choose a photo, or remove it.
/// Optional and non-blocking, like the Doctor profile photo: a failure only
/// shows a message.
class PeerProfilePhotoPicker extends StatefulWidget {
  const PeerProfilePhotoPicker(
      {super.key,
      required this.repo,
      required this.size,
      required this.fallback,
      this.pickImage});
  final PeerRepository repo;
  final double size;
  final Widget fallback;

  /// Injectable picker for tests: returns (bytes, mime) or null.
  final Future<(Uint8List, String)?> Function(ImageSource source)? pickImage;
  @override
  State<PeerProfilePhotoPicker> createState() => _PeerProfilePhotoPickerState();
}

class _PeerProfilePhotoPickerState extends State<PeerProfilePhotoPicker> {
  bool _saving = false;

  /// Camera or gallery; the picker's own copy is deleted once read so the
  /// photo is not left in a generic disk cache.
  Future<(Uint8List, String)?> _pick(ImageSource source) async {
    final file = await ImagePicker().pickImage(
        source: source, imageQuality: 85, maxWidth: 2000, maxHeight: 2000);
    if (file == null) return null;
    final png = file.name.toLowerCase().endsWith('.png');
    try {
      return (await file.readAsBytes(), png ? 'image/png' : 'image/jpeg');
    } finally {
      try {
        final copy = File(file.path);
        if (await copy.exists()) await copy.delete();
      } catch (_) {/* best-effort cleanup */}
    }
  }

  Future<void> _change(ImageSource source) async {
    try {
      final picked = await (widget.pickImage ?? _pick)(source);
      if (picked == null || !mounted) return;
      setState(() => _saving = true);
      await widget.repo.uploadPhoto(picked.$1, picked.$2);
      // Show what the server stored (it re-encodes and compresses).
      Uint8List saved = picked.$1;
      try {
        saved = Uint8List.fromList(
            await widget.repo.api.getBytes(ownPeerPhotoPath));
      } catch (_) {}
      PersonPhotoCache.put(ownPeerPhotoPath, saved);
    } on PlatformException {
      if (mounted) {
        peerToast(context,
            'Camera access was denied. You can still choose a photo from your gallery.');
      }
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _remove() async {
    setState(() => _saving = true);
    try {
      await widget.repo.removePhoto();
      PersonPhotoCache.put(ownPeerPhotoPath, null);
    } catch (e) {
      if (mounted) peerToast(context, peerErrorText(e));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  void _open() {
    final hasPhoto = PersonPhotoCache.peek(ownPeerPhotoPath) != null;
    showModalBottomSheet<void>(
        context: context,
        builder: (sheet) => SafeArea(
                child: Column(mainAxisSize: MainAxisSize.min, children: [
              ListTile(
                  key: const ValueKey('peer-photo-camera'),
                  leading: const Icon(Icons.photo_camera_outlined),
                  title: const Text('Take a photo'),
                  onTap: () {
                    Navigator.pop(sheet);
                    _change(ImageSource.camera);
                  }),
              ListTile(
                  key: const ValueKey('peer-photo-gallery'),
                  leading: const Icon(Icons.photo_library_outlined),
                  title: const Text('Choose from gallery'),
                  onTap: () {
                    Navigator.pop(sheet);
                    _change(ImageSource.gallery);
                  }),
              if (hasPhoto)
                ListTile(
                    key: const ValueKey('peer-photo-remove'),
                    leading:
                        const Icon(Icons.delete_outline, color: Colors.red),
                    title: const Text('Remove photo',
                        style: TextStyle(color: Colors.red)),
                    onTap: () {
                      Navigator.pop(sheet);
                      _remove();
                    }),
            ])));
  }

  @override
  Widget build(BuildContext context) => Semantics(
      button: true,
      label: 'Change profile photo',
      child: GestureDetector(
          key: const ValueKey('peer-profile-photo'),
          onTap: _saving ? null : _open,
          child: Stack(clipBehavior: Clip.none, children: [
            PeerOwnPhoto(
                repo: widget.repo,
                size: widget.size,
                fallback: widget.fallback),
            if (_saving)
              Positioned.fill(
                  child: Container(
                      decoration: const BoxDecoration(
                          color: Colors.black38, shape: BoxShape.circle),
                      alignment: Alignment.center,
                      child: const SizedBox(
                          width: 20,
                          height: 20,
                          child: CircularProgressIndicator(
                              strokeWidth: 2, color: Colors.white)))),
            Positioned(
                bottom: -2,
                right: -2,
                child: Container(
                    padding: const EdgeInsets.all(4),
                    decoration: const BoxDecoration(
                        color: Colors.white, shape: BoxShape.circle),
                    child: Icon(Icons.camera_alt,
                        size: widget.size * 0.26, color: PeerTokens.green))),
          ])));
}
