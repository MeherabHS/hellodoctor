import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_cubit.dart';

String? check({
  String fullName = 'Amina Otieno',
  String phone = '0712345678',
  String email = 'amina@example.com',
  String dateOfBirth = '1990-05-01',
  String address = 'Kilimani, Nairobi',
  String password = 'Strong!Pass1',
}) =>
    registrationFieldError(
        fullName: fullName,
        phone: phone,
        email: email,
        dateOfBirth: dateOfBirth,
        residentialAddress: address,
        password: password);

void main() {
  test('a complete form passes', () => expect(check(), isNull));

  test('each required field is named when missing, in form order', () {
    expect(check(fullName: ' '), 'Full name is required.');
    expect(check(phone: ''), 'Phone number is required.');
    expect(check(email: ''), 'Email address is required.');
    expect(check(dateOfBirth: ''), 'Date of birth is required.');
    expect(check(address: '   '), 'Address is required.');
    expect(check(password: ''), 'Password is required.');
    expect(check(fullName: '', address: ''), 'Full name is required.');
  });

  test('email and address are checked for shape', () {
    expect(check(email: 'not-an-email'), 'Enter a valid email address.');
    expect(check(address: 'ab'), 'Enter your address (3 to 300 characters).');
    expect(
        check(address: 'x' * 301), 'Enter your address (3 to 300 characters).');
  });
}
