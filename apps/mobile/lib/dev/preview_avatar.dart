// DESIGN PREVIEW ONLY — never used by the shipped app.
//
// Draws a plain silhouette "profile photo" as PNG bytes, so the design
// preview can show where saved profile photos appear without using a
// picture of any real person. Each seed gets its own colours.
import 'dart:io' show ZLibCodec;
import 'dart:math' as math;
import 'dart:typed_data';

const _palettes = <List<int>>[
  [0x2E, 0x7D, 0x5B, 0xF4, 0xD9, 0xB8], // green / warm skin
  [0x5B, 0x4B, 0xC4, 0xE8, 0xC1, 0x9A], // purple / tan
  [0xC2, 0x5B, 0x2E, 0x8D, 0x5A, 0x3B], // orange / deep brown
  [0x1F, 0x6F, 0xB2, 0xC6, 0x8E, 0x62], // blue / brown
];

/// A [size]×[size] PNG: coloured background, head and shoulders.
Uint8List previewAvatarPng(int seed, {int size = 160}) {
  final p = _palettes[seed.abs() % _palettes.length];
  final bg = [p[0], p[1], p[2]];
  final skin = [p[3], p[4], p[5]];
  final shirt = [
    (p[0] * 0.6).round(),
    (p[1] * 0.6).round(),
    (p[2] * 0.6).round()
  ];
  final raw = BytesBuilder();
  final c = size / 2;
  for (var y = 0; y < size; y++) {
    raw.addByte(0); // filter: none
    for (var x = 0; x < size; x++) {
      // Soft vertical gradient behind the figure.
      final t = y / size;
      var px = [for (final v in bg) (v + (255 - v) * 0.35 * (1 - t)).round()];
      final headDy = y - size * 0.40;
      final headDx = x - c;
      final inHead = headDx * headDx / math.pow(size * 0.19, 2) +
              headDy * headDy / math.pow(size * 0.22, 2) <=
          1;
      final bodyDy = y - size * 1.02;
      final inBody = headDx * headDx / math.pow(size * 0.38, 2) +
              bodyDy * bodyDy / math.pow(size * 0.36, 2) <=
          1;
      final hairDy = y - size * 0.31;
      final inHair = headDx * headDx / math.pow(size * 0.21, 2) +
                  hairDy * hairDy / math.pow(size * 0.17, 2) <=
              1 &&
          y < size * 0.33;
      if (inBody) px = shirt;
      if (inHead) px = skin;
      if (inHair) px = const [0x2A, 0x1E, 0x18];
      raw.add(px);
    }
  }
  return _png(size, size, raw.toBytes());
}

Uint8List _png(int w, int h, Uint8List rgbRows) {
  final out = BytesBuilder()
    ..add(const [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
  final ihdr = ByteData(13)
    ..setUint32(0, w)
    ..setUint32(4, h)
    ..setUint8(8, 8) // bit depth
    ..setUint8(9, 2) // RGB
    ..setUint8(10, 0)
    ..setUint8(11, 0)
    ..setUint8(12, 0);
  _chunk(out, 'IHDR', ihdr.buffer.asUint8List());
  _chunk(out, 'IDAT', Uint8List.fromList(ZLibCodec().encode(rgbRows)));
  _chunk(out, 'IEND', Uint8List(0));
  return out.toBytes();
}

void _chunk(BytesBuilder out, String type, Uint8List data) {
  final typeBytes = Uint8List.fromList(type.codeUnits);
  out.add((ByteData(4)..setUint32(0, data.length)).buffer.asUint8List());
  out.add(typeBytes);
  out.add(data);
  final crc = _crc32([...typeBytes, ...data]);
  out.add((ByteData(4)..setUint32(0, crc)).buffer.asUint8List());
}

int _crc32(List<int> bytes) {
  var crc = 0xFFFFFFFF;
  for (final b in bytes) {
    crc ^= b;
    for (var k = 0; k < 8; k++) {
      crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xEDB88320 : crc >> 1;
    }
  }
  return (crc ^ 0xFFFFFFFF) & 0xFFFFFFFF;
}
