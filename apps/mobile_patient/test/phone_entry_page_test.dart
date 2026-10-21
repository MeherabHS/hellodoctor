import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/auth/phone_entry_page.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

void main() {
  testWidgets(
      'an invalid phone number shows a field error and never leaves this screen',
      (tester) async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    addTearDown(auth.close);
    await tester.pumpWidget(MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: PhoneEntryPage(auth: auth),
    ));

    await tester.enterText(find.byType(TextField), '12345');
    await tester.tap(find.text('Send verification code'));
    await tester.pump();

    expect(find.text('Enter a valid Kenyan phone number.'), findsOneWidget);
    expect(find.byType(PhoneEntryPage), findsOneWidget);
  });
}
