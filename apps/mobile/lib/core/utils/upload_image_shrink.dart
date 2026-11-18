import 'dart:math';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter_image_compress/flutter_image_compress.dart';

/// Longest edge, in pixels, of an image this app uploads. Matches the
/// backend's own maximum (`MAX_IMAGE_DIMENSION` in
/// `services/backend/src/services/upload_security_service.rs`), so nothing the
/// server keeps is lost; the server still compresses every image to its
/// per-image budget (600 KB for clinical images, 40 KB for profile pictures).
const int kUploadMaxEdge = 2000;

/// Target size for an image whose longest edge is [width]x[height] shrunk to
/// [maxEdge]: the short edge, used as both of flutter_image_compress's
/// `minWidth`/`minHeight`. With equal minimums its scale is
/// `min(width, height) / shortEdge`, so the result is the same whichever way
/// round the photo is (EXIF rotation included). Null when no shrink is needed.
int? shrinkTargetShortEdge(int width, int height,
    {int maxEdge = kUploadMaxEdge}) {
  final long = max(width, height);
  final short = min(width, height);
  if (long <= maxEdge || short <= 0) return null;
  return max(1, (short * maxEdge / long).floor());
}

/// Shrinks an image picked from Files (the photo picker already does this for
/// camera and gallery picks via `maxWidth`/`maxHeight`) so large photos do not
/// upload at full size over mobile data. Re-encoding also drops photo
/// metadata. Returns [bytes] unchanged when the image is already small enough
/// or cannot be processed here; the server validates and compresses either
/// way, so this is a bandwidth saving, never a gate.
Future<Uint8List> shrinkImageForUpload(Uint8List bytes,
    {required bool png}) async {
  try {
    final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
    final descriptor = await ui.ImageDescriptor.encoded(buffer);
    final target = shrinkTargetShortEdge(descriptor.width, descriptor.height);
    descriptor.dispose();
    buffer.dispose();
    if (target == null) return bytes;
    final out = await FlutterImageCompress.compressWithList(
      bytes,
      minWidth: target,
      minHeight: target,
      quality: 85,
      format: png ? CompressFormat.png : CompressFormat.jpeg,
    );
    return out.isEmpty ? bytes : out;
  } catch (_) {
    return bytes;
  }
}
