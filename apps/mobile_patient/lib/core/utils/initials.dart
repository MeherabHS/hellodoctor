/// Avatar initials from a person's display name.
///
/// Names reach avatars with titles ("Dr. Jane Mwangi"), notes in brackets
/// ("Jane Mwangi (sample)", "Brian [Peer]"), punctuation and digits. Only
/// words that start with a letter count, bracketed parts and honorifics are
/// dropped, and the first and last remaining words give the initials, so
/// "Dr. Jane Mwangi (sample)" gives "JM", never "J(".
String nameInitials(String? name, {String fallback = '?'}) {
  final words = initialWords(name);
  if (words.isEmpty) return fallback;
  if (words.length == 1) return words.first[0].toUpperCase();
  return (words.first[0] + words.last[0]).toUpperCase();
}

final _bracketed = RegExp(r'\([^)]*\)|\[[^\]]*\]|\{[^}]*\}|<[^>]*>');
final _letter = RegExp(r'\p{L}', unicode: true);
final _wordChars = RegExp(r"[\p{L}\p{M}'’-]+", unicode: true);
const _titles = {
  'dr', 'doctor', 'prof', 'professor', 'mr', 'mrs', 'ms', 'miss', 'mx',
  'sr', 'sister', 'nurse', 'daktari', 'bw', 'bi',
};

/// The name's words that can give an initial, each trimmed to start at its
/// first letter, with bracketed notes and titles removed.
List<String> initialWords(String? name) {
  final cleaned = (name ?? '').replaceAll(_bracketed, ' ');
  final words = <String>[];
  for (final match in _wordChars.allMatches(cleaned)) {
    var word = match.group(0)!;
    final start = word.indexOf(_letter);
    if (start < 0) continue;
    word = word.substring(start);
    if (_titles.contains(word.toLowerCase())) continue;
    words.add(word);
  }
  return words;
}
