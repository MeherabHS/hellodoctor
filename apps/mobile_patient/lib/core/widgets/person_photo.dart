import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../network/api_client.dart';

/// Profile photos of people (the other side of a consultation or peer
/// session, or the signed-in peer), keyed by the API path that serves them.
///
/// Held in memory only, never in a disk cache (CLAUDE.md privacy rules), and
/// dropped on sign-out. Entries expire after a few minutes so a photo the
/// other person changes or removes shows up without restarting the app.
class PersonPhotoCache {
  PersonPhotoCache._();

  static const _ttl = Duration(minutes: 10);
  static final _entries = <String, (Uint8List?, DateTime)>{};
  static final _pending = <String, Future<Uint8List?>>{};

  /// Bumps whenever an entry is replaced from outside (own upload/remove),
  /// so every avatar showing that path rebuilds.
  static final changes = ValueNotifier<int>(0);

  static bool _fresh(String path) {
    final e = _entries[path];
    return e != null && DateTime.now().difference(e.$2) < _ttl;
  }

  /// The cached bytes, or null when nothing (or no photo) is cached.
  static Uint8List? peek(String path) =>
      _fresh(path) ? _entries[path]!.$1 : null;

  /// Fetches [path] once per expiry window. A 404 means "no photo" and is
  /// remembered; any other failure is not, so the next build retries.
  static Future<Uint8List?> load(ApiClient api, String path) {
    if (_fresh(path)) return Future.value(_entries[path]!.$1);
    return _pending[path] ??= () async {
      try {
        final bytes = Uint8List.fromList(await api.getBytes(path));
        _entries[path] = (bytes, DateTime.now());
        return bytes;
      } on ApiException catch (e) {
        if (e.status == 404) _entries[path] = (null, DateTime.now());
        return null;
      } catch (_) {
        return null;
      } finally {
        _pending.remove(path);
      }
    }();
  }

  /// Records a photo the user just saved (or removed, with null).
  static void put(String path, Uint8List? bytes) {
    _entries[path] = (bytes, DateTime.now());
    changes.value++;
  }

  static void clear() {
    _entries.clear();
    _pending.clear();
    changes.value++;
  }
}

/// A person's saved photo, cropped to [shape], or [fallback] (their
/// initials) while it loads or when they have none.
class PersonPhoto extends StatefulWidget {
  const PersonPhoto({
    super.key,
    required this.api,
    required this.path,
    required this.size,
    required this.fallback,
    this.borderRadius,
    this.border,
  });

  final ApiClient api;

  /// The API path serving the photo, e.g.
  /// `/appointments/<id>/counterpart-photo`. Null shows [fallback] only.
  final String? path;
  final double size;
  final Widget fallback;

  /// Rounded square when set; a circle otherwise.
  final BorderRadius? borderRadius;
  final BoxBorder? border;

  @override
  State<PersonPhoto> createState() => _PersonPhotoState();
}

class _PersonPhotoState extends State<PersonPhoto> {
  Uint8List? _bytes;

  @override
  void initState() {
    super.initState();
    PersonPhotoCache.changes.addListener(_load);
    _load();
  }

  @override
  void didUpdateWidget(covariant PersonPhoto old) {
    super.didUpdateWidget(old);
    if (old.path != widget.path) _load();
  }

  @override
  void dispose() {
    PersonPhotoCache.changes.removeListener(_load);
    super.dispose();
  }

  Future<void> _load() async {
    final path = widget.path;
    if (path == null) {
      if (_bytes != null) setState(() => _bytes = null);
      return;
    }
    final cached = PersonPhotoCache.peek(path);
    if (cached != null || _bytes != null) {
      setState(() => _bytes = cached);
    }
    final bytes = await PersonPhotoCache.load(widget.api, path);
    if (!mounted || widget.path != path) return;
    if (!identical(bytes, _bytes)) setState(() => _bytes = bytes);
  }

  @override
  Widget build(BuildContext context) {
    final bytes = _bytes;
    if (bytes == null) return widget.fallback;
    final radius = widget.borderRadius;
    return Container(
        key: const ValueKey('person-photo'),
        width: widget.size,
        height: widget.size,
        clipBehavior: Clip.antiAlias,
        decoration: BoxDecoration(
            shape: radius == null ? BoxShape.circle : BoxShape.rectangle,
            borderRadius: radius,
            border: widget.border),
        child: Image.memory(bytes,
            fit: BoxFit.cover,
            gaplessPlayback: true,
            width: widget.size,
            height: widget.size,
            errorBuilder: (_, __, ___) => widget.fallback));
  }
}

/// API paths for [PersonPhoto].
String appointmentCounterpartPhotoPath(String appointmentId) =>
    '/appointments/$appointmentId/counterpart-photo';
String peerRequestCounterpartPhotoPath(String requestId) =>
    '/peer-requests/$requestId/counterpart-photo';
