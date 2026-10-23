// Explicit live verification runner; not part of the offline unit-test suite.
import 'dart:io';
import 'dart:ui' as ui;
import 'package:dio/dio.dart';
import 'package:dio/io.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

class RealHttp extends HttpOverrides {
  @override
  HttpClient createHttpClient(SecurityContext? context) => super.createHttpClient(context);
}
class MemoryTokens implements TokenStore {
  MemoryTokens(this.token);
  final String token;
  @override Future<String?> read(String key) async => key=='access_token'?token:null;
  @override Future<void> write(String key,String value) async {}
  @override Future<void> delete(String key) async {}
  @override Future<void> clear() async {}
}
void main() {
  setUpAll(() async {
    await (FontLoader('Plus Jakarta Sans')..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf'))).load();
    await (FontLoader('MaterialIcons')..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'))).load();
  });
  testWidgets('Doctor Flutter opens authoritative paid chat and replies to Patient and Web history', (tester) async {
    final env=Platform.environment;
    final dio=Dio(BaseOptions(baseUrl:'http://127.0.0.1:8080/api/v1'));
    dio.httpClientAdapter=IOHttpClientAdapter(createHttpClient:()=>HttpOverrides.runWithHttpOverrides(()=>HttpClient(),RealHttp()));
    final api=ApiClient(dio:dio,tokenStore:MemoryTokens(env['HD_DOCTOR_TOKEN']!));
    final repo=DoctorRepository(api);
    final cubit=DoctorCubit(repo);
    final queue=await tester.runAsync(()=>repo.queue(limit:100));
    final a=queue!.firstWhere((a)=>a.id==env['HD_APPOINTMENT_ID']);
    expect(a.patientName,env['HD_PATIENT_NAME']); expect(a.modality,'CHAT');
    final chat=await tester.runAsync(()=>repo.conversationForAppointment(a));
    expect(chat!.id,env['HD_CONVERSATION_ID']);
    final profile=await tester.runAsync(repo.profile);
    cubit.emit(DoctorState(profile:profile,queue:queue,chats:[chat]));
    tester.view.physicalSize=const Size(390,900); tester.view.devicePixelRatio=1;
    addTearDown(tester.view.resetPhysicalSize); addTearDown(tester.view.resetDevicePixelRatio);
    await tester.runAsync(() async {
      await tester.pumpWidget(BlocProvider.value(value:cubit,child:MaterialApp(theme:DoctorTokens.theme(),home:RepaintBoundary(key:const Key('capture'),child:ChatThreadPage(chat:chat)))));
      await Future<void>.delayed(const Duration(milliseconds:300));
    });
    await tester.pump();
    Future<void> waitFor(String value) async {
      for(var i=0;i<100 && find.text(value).evaluate().isEmpty;i++) {
        await tester.runAsync(()=>Future<void>.delayed(const Duration(milliseconds:100))); await tester.pump();
      }
      expect(find.text(value),findsOneWidget);
    }
    await waitFor('Hello doctor. Consultation verification message.');
    await waitFor('Doctor Web reply in the same consultation.');
    expect(find.byTooltip('Start Video Call').hitTestable(),findsNothing);
    await tester.enterText(find.byType(TextField),'Doctor Flutter reply in the same consultation.');
    await tester.runAsync(() async {
      await tester.testTextInput.receiveAction(TextInputAction.send);
      await Future<void>.delayed(const Duration(milliseconds:300));
    });
    await waitFor('Doctor Flutter reply in the same consultation.');
    expect(tester.takeException(),isNull);
    final boundary=tester.renderObject<RenderRepaintBoundary>(find.byKey(const Key('capture')));
    await tester.runAsync(() async {
      final image=await boundary.toImage(pixelRatio:2);
      final bytes=await image.toByteData(format:ui.ImageByteFormat.png);
      await File('../../docs/audit_evidence/patient_consultation/doctor_flutter_shared_chat.png').writeAsBytes(bytes!.buffer.asUint8List()); image.dispose();
    });
    await tester.pumpWidget(const SizedBox.shrink()); await cubit.close(); api.dispose();
  });
}
