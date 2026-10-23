// Explicit live verification runner; no mocked messages or authorization.
import 'dart:io';
import 'dart:ui' as ui;
import 'package:dio/dio.dart';
import 'package:dio/io.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/patient_chat_page.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class RealHttp extends HttpOverrides {
  @override HttpClient createHttpClient(SecurityContext? context)=>super.createHttpClient(context);
}
class MemoryTokens implements TokenStore {
  MemoryTokens(this.token); final String token;
  @override Future<String?> read(String key) async=>key=='access_token'?token:null;
  @override Future<void> write(String key,String value) async {}
  @override Future<void> delete(String key) async {}
  @override Future<void> clear() async {}
}
void main() {
  setUpAll(() async {
    await (FontLoader('Plus Jakarta Sans')..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf'))).load();
    await (FontLoader('MaterialIcons')..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'))).load();
  });
  testWidgets('Patient reopens the same real conversation and receives Web and Flutter replies', (tester) async {
    final env=Platform.environment;
    final dio=Dio(BaseOptions(baseUrl:'http://127.0.0.1:8080/api/v1'));
    dio.httpClientAdapter=IOHttpClientAdapter(createHttpClient:()=>HttpOverrides.runWithHttpOverrides(()=>HttpClient(),RealHttp()));
    final api=ApiClient(dio:dio,tokenStore:MemoryTokens(env['HD_PATIENT_TOKEN']!));
    final repo=ChatRepository(api);
    final conversation=await tester.runAsync(()=>repo.conversationForAppointment(env['HD_APPOINTMENT_ID']!));
    expect(conversation!.id,env['HD_CONVERSATION_ID']);
    tester.view.physicalSize=const Size(390,900); tester.view.devicePixelRatio=1;
    addTearDown(tester.view.resetPhysicalSize); addTearDown(tester.view.resetDevicePixelRatio);
    await tester.runAsync(() async {
    await tester.pumpWidget(MaterialApp(theme:PatientTokens.theme(),localizationsDelegates:AppLocalizations.localizationsDelegates,
      supportedLocales:AppLocalizations.supportedLocales,home:RepaintBoundary(key:const Key('capture'),child:PatientChatPage(
        conversation:conversation,doctorDisplayName:env['HD_DOCTOR_NAME']!,repo:repo))));
      await Future<void>.delayed(const Duration(milliseconds:300));
    });
    await tester.pump();
    for(var i=0;i<100 && find.text('Doctor Flutter reply in the same consultation.').evaluate().isEmpty;i++) {
      await tester.runAsync(()=>Future<void>.delayed(const Duration(milliseconds:100)));await tester.pump();
    }
    for(final message in ['Hello doctor. Consultation verification message.','Doctor Web reply in the same consultation.','Doctor Flutter reply in the same consultation.']) {
      expect(find.text(message),findsOneWidget);
    }
    expect(tester.takeException(),isNull);
    final boundary=tester.renderObject<RenderRepaintBoundary>(find.byKey(const Key('capture')));
    await tester.runAsync(() async {
      final image=await boundary.toImage(pixelRatio:2);final bytes=await image.toByteData(format:ui.ImageByteFormat.png);
      await File('../../docs/audit_evidence/patient_consultation/patient_flutter_shared_chat.png').writeAsBytes(bytes!.buffer.asUint8List());image.dispose();
    });
    await tester.pumpWidget(const SizedBox.shrink());api.dispose();
  });
}
