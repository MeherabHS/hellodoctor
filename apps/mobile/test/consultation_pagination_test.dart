import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'doctor_cubit_test.dart' show StubDoctorRepository;

class PagedRepository extends StubDoctorRepository {
  final queueOffsets = <int>[], chatOffsets = <int>[];
  bool failSecondPage = false;
  @override
  Future<List<AppointmentDto>> queue({int limit = 25, int offset = 0}) async {
    queueOffsets.add(offset);
    if (failSecondPage && offset == 100) {
      throw const ApiException(503, 'Second page unavailable');
    }
    return List.generate(
        offset == 0 ? 100 : 1,
        (i) => AppointmentDto(
            id: 'a${offset + i}',
            status: 'CONFIRMED',
            lifecycle: 'UPCOMING',
            modality: offset + i == 100 ? 'CHAT' : 'VIDEO',
            patientName: 'Meherab Shafin',
            conversationId: 'c${offset + i}'));
  }

  @override
  Future<List<ChatDto>> chats({int limit = 25, int offset = 0}) async {
    chatOffsets.add(offset);
    return List.generate(
        offset == 0 ? 100 : 1,
        (i) => ChatDto(
            id: 'c${offset + i}',
            appointmentId: 'a${offset + i}',
            patientName: 'Meherab Shafin',
            modality: offset + i == 100 ? 'CHAT' : 'VIDEO'));
  }
}

void main() {
  test(
      'Doctor loads a Patient-specific paid CHAT booking beyond both first pages without changing order',
      () async {
    final repo = PagedRepository();
    final cubit = DoctorCubit(repo);
    await cubit.loadAll();
    expect(cubit.state.chats, hasLength(100));
    expect(cubit.state.hasMoreChats, isTrue);
    expect(repo.chatOffsets, [0]);
    await cubit.refreshChats(older: true);
    expect(cubit.state.hasMoreChats, isFalse);
    expect(repo.queueOffsets, [0, 100]);
    expect(repo.chatOffsets, [0, 100]);
    expect(cubit.state.queue, hasLength(101));
    expect(cubit.state.chats, hasLength(101));
    final appointments = cubit.state.queue.cast<AppointmentDto>();
    expect(appointments.map((a) => a.id), List.generate(101, (i) => 'a$i'));
    expect(appointments.last.patientName, 'Meherab Shafin');
    expect(appointments.last.modality, 'CHAT');
    final chat = cubit.state.chats.last as ChatDto;
    expect(chat.id, appointments.last.conversationId);
    expect(chat.appointmentId, appointments.last.id);
    expect(chat.patientName, 'Meherab Shafin');
    expect(cubit.state.error, isNull);
    await cubit.close();
  });
  test(
      'a failed later queue page is an error, never a silently truncated successful queue',
      () async {
    final repo = PagedRepository()..failSecondPage = true;
    final cubit = DoctorCubit(repo);
    await cubit.loadQueue();
    expect(repo.queueOffsets, [0, 100]);
    expect(cubit.state.queue, isEmpty);
    // Keep the established 5xx sanitization contract; internal error text is
    // not exposed in the Doctor UI. The queue must still fail explicitly.
    expect(
        cubit.state.error, 'A temporary service error occurred. Please retry.');
    expect(cubit.state.loading, isFalse);
    await cubit.close();
  });
}
