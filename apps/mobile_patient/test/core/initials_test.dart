import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/utils/initials.dart';
import 'package:hellodoctor_mobile_patient/core/widgets/afya_widgets.dart';

void main() {
  test('brackets, titles and punctuation never become initials', () {
    expect(nameInitials('Dr. Jane Mwangi (sample)'), 'JM');
    expect(nameInitials('Jane (sample) Mwangi'), 'JM');
    expect(nameInitials('Dr Jane [Peer] Mwangi'), 'JM');
    expect(nameInitials('(sample) Jane'), 'J');
    expect(nameInitials('Doctor Amina Otieno-Wanjiru'), 'AO');
    expect(nameInitials('  "Brian"  O\'Neil  '), 'BO');
    expect(nameInitials('Prof. Émile Zola'), 'ÉZ');
    expect(nameInitials('Dr. 123 (sample)'), '?');
    expect(nameInitials(''), '?');
    expect(nameInitials(null, fallback: ''), '');
    expect(nameInitials('neema'), 'N');
  });

  test('the round avatar keeps two letters for a single name', () {
    expect(AfyaInitialsAvatar.initialsOf('Neema (Peer)'), 'NE');
    expect(AfyaInitialsAvatar.initialsOf('Dr. Jane Mwangi (sample)'), 'JM');
    expect(AfyaInitialsAvatar.initialsOf('(x)'), '');
  });
}
