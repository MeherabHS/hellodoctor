"""Check the actual APK's DEX for Room's reflectively invoked constructor.

Usage: python check_release_constructor.py path/to/app-release.apk
This checks compiled release output, which Flutter widget tests cannot exercise.
"""

import struct
import sys
import zipfile


def uint(data, offset):
    return struct.unpack_from('<I', data, offset)[0]


def uleb(data, offset):
    result = shift = 0
    while True:
        byte = data[offset]
        offset += 1
        result |= (byte & 127) << shift
        if byte < 128:
            return result, offset
        shift += 7
        if shift > 35:
            raise ValueError('Malformed DEX integer')


def has_public_constructor(data, descriptor):
    if not data.startswith(b'dex\n'):
        raise ValueError('Expected standard APK DEX')
    string_count, string_offset = struct.unpack_from('<II', data, 56)
    strings = []
    for index in range(string_count):
        offset = uint(data, string_offset + 4 * index)
        _, offset = uleb(data, offset)
        strings.append(data[offset:data.index(b'\0', offset)].decode('utf-8', errors='replace'))
    type_count, type_offset = struct.unpack_from('<II', data, 64)
    types = [strings[uint(data, type_offset + 4 * index)] for index in range(type_count)]
    proto_offset = uint(data, 76)
    method_offset = uint(data, 92)
    class_count, class_offset = struct.unpack_from('<II', data, 96)
    for index in range(class_count):
        offset = class_offset + 32 * index
        if types[uint(data, offset)] != descriptor:
            continue
        offset = uint(data, offset + 24)
        if not offset:
            return False
        counts = []
        for _ in range(4):
            count, offset = uleb(data, offset)
            counts.append(count)
        for _ in range(counts[0] + counts[1]):
            _, offset = uleb(data, offset)
            _, offset = uleb(data, offset)
        method_index = 0
        for _ in range(counts[2]):
            delta, offset = uleb(data, offset)
            flags, offset = uleb(data, offset)
            code, offset = uleb(data, offset)
            method_index += delta
            _, proto_index, name_index = struct.unpack_from('<HHI', data, method_offset + 8 * method_index)
            proto = proto_offset + 12 * proto_index
            if (strings[name_index] == '<init>' and flags & 1 and code
                    and types[uint(data, proto + 4)] == 'V'
                    and uint(data, proto + 8) == 0):
                return True
        return False
    return False


if __name__ == '__main__':
    target = 'Landroidx/work/impl/WorkDatabase_Impl;'
    with zipfile.ZipFile(sys.argv[1]) as apk:
        found = any(has_public_constructor(apk.read(name), target)
                    for name in apk.namelist()
                    if name.startswith('classes') and name.endswith('.dex'))
    if not found:
        raise SystemExit('FAIL: WorkDatabase_Impl public no-argument constructor missing from release DEX')
    print('PASS: WorkDatabase_Impl public no-argument constructor exists in release DEX')
