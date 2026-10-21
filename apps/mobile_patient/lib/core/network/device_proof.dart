import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Native implementation signs with a nonexportable Android Keystore P-256 key.
/// This is path-bound HelloDoctor proof, not a generic RFC 9449 implementation.
class DeviceProofSigner {
  DeviceProofSigner({bool enabled = true})
      : enabled = enabled &&
            !kIsWeb &&
            defaultTargetPlatform == TargetPlatform.android;
  final bool enabled;
  static const _channel = MethodChannel('hellodoctor/device_proof');

  Future<String?> sign(String method, String path, String binding) async {
    if (!enabled) return null;
    final proof = await _channel.invokeMethod<String>('sign', {
      'method': method.toUpperCase(),
      'path': path,
      'binding': binding,
    });
    if (proof == null || proof.isEmpty) {
      throw StateError('Session proof unavailable. Please sign in again.');
    }
    return proof;
  }
}
