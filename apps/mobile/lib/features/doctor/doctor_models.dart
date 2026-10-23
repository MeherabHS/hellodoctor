typedef Json = Map<String, dynamic>;

String textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();
String? optionalText(dynamic value) => value?.toString();

class AppointmentDto {
  const AppointmentDto(
      {required this.id,
      required this.status,
      required this.lifecycle,
      this.number,
      this.patientId,
      this.patientName,
      this.modality,
      this.start,
      this.end,
      this.historyFrom,
      this.historyUntil,
      this.historyActive,
      this.complaint,
      this.completedAt,
      this.conversationId,
      this.chatExpiresAt,
      this.chatLocked,
      this.consultationSessionStatus,
      this.consultationRecoveryState,
      this.emptyRoomDeadline,
      this.patientPresent,
      this.doctorPresent,
      this.queueActionable = true});
  final bool queueActionable;
  bool get isQueueActionable =>
      queueActionable &&
      const ['CONFIRMED', 'WAITING', 'IN_CONSULTATION'].contains(status);
  final String? conversationId;
  final DateTime? chatExpiresAt;
  final bool? chatLocked;
  final String? consultationSessionStatus, consultationRecoveryState;
  final DateTime? emptyRoomDeadline;
  final bool? patientPresent, doctorPresent;
  bool get hasActiveConsultation =>
      modality == 'VIDEO' &&
      status == 'IN_CONSULTATION' &&
      consultationRecoveryState != 'NEEDS_REVIEW' &&
      const ['INITIALIZED', 'ACTIVE'].contains(consultationSessionStatus);
  bool get isWaitingForReconnect =>
      hasActiveConsultation && consultationRecoveryState == 'GRACE_PERIOD';
  bool get chatClosed =>
      chatLocked == true || (chatExpiresAt?.isBefore(DateTime.now()) ?? false);
  bool get isChat => modality == 'CHAT';
  bool get canStart =>
      const ['CONFIRMED', 'WAITING', 'IN_CONSULTATION'].contains(status) &&
      !(modality == 'VIDEO' && status == 'WAITING') &&
      const ['CHAT', 'VIDEO'].contains(modality) &&
      !(isChat && chatClosed);
  String get startLabel =>
      isChat ? 'Start chat consultation' : 'Start video consultation';
  final String id, status, lifecycle;
  final String? number,
      patientId,
      patientName,
      modality,
      complaint,
      completedAt;
  final DateTime? start, end, historyFrom, historyUntil;
  final bool? historyActive;
  factory AppointmentDto.fromJson(Json j) => AppointmentDto(
      id: textOf(j['id'] ?? j['appointment_id']),
      number: optionalText(j['appointment_number']),
      patientId: optionalText(j['patient_id']),
      patientName: optionalText(j['patient_display_name'] ?? j['patient_name']),
      modality: optionalText(j['modality']),
      status: textOf(j['status']),
      lifecycle: textOf(j['doctor_lifecycle'] ?? j['status']),
      start: DateTime.tryParse(textOf(j['start_time'])),
      end: DateTime.tryParse(textOf(j['end_time'])),
      historyFrom: DateTime.tryParse(textOf(j['history_access_from'])),
      historyUntil: DateTime.tryParse(textOf(j['history_access_until'])),
      historyActive: j['history_access_active'] as bool?,
      complaint: optionalText(j['chief_complaint'] ?? j['clinical_outcome']),
      completedAt: optionalText(j['completed_at']),
      conversationId: optionalText(j['conversation_id']),
      chatExpiresAt: DateTime.tryParse(textOf(j['chat_expires_at'])),
      chatLocked: j['chat_locked'] as bool?,
      consultationSessionStatus: optionalText(j['consultation_session_status']),
      consultationRecoveryState: optionalText(j['consultation_recovery_state']),
      emptyRoomDeadline: DateTime.tryParse(textOf(j['empty_room_deadline'])),
      patientPresent: j['patient_present'] as bool?,
      doctorPresent: j['doctor_present'] as bool?,
      queueActionable: j['queue_actionable'] == true);
}

class SlotDto {
  const SlotDto(
      {required this.id,
      required this.status,
      this.start,
      this.end,
      this.appointmentId,
      this.patientId,
      this.patientName,
      this.bookable = true});
  final String id, status;
  final bool bookable;
  final DateTime? start, end;
  final String? appointmentId, patientId, patientName;
  factory SlotDto.fromJson(Json j) => SlotDto(
      id: textOf(j['id']),
      status: textOf(j['status']),
      bookable: j['bookable'] == true,
      start: DateTime.tryParse(textOf(j['start_time'])),
      end: DateTime.tryParse(textOf(j['end_time'])),
      appointmentId: optionalText(j['appointment_id']),
      patientId: optionalText(j['patient_id']),
      patientName: optionalText(j['patient_display_name']));
}

class ChatDto {
  const ChatDto(
      {required this.id,
      this.appointmentId,
      this.patientId,
      this.patientName,
      this.lastMessageAt,
      this.expiresAt,
      this.locked,
      this.modality,
      this.appointmentStatus});
  final String id;
  final String? appointmentId, patientId, patientName, lastMessageAt, expiresAt;
  final String? modality, appointmentStatus;
  bool get isClosed =>
      locked == true ||
      (DateTime.tryParse(expiresAt ?? '')?.isBefore(DateTime.now()) ?? false) ||
      const ['CANCELLED', 'REFUNDED', 'NO_SHOW', 'PENDING_PAYMENT']
          .contains(appointmentStatus);
  String get actionLabel => isClosed
      ? 'View consultation history'
      : modality == 'CHAT' && lastMessageAt == null
          ? 'Start chat consultation'
          : 'Open chat';
  final bool? locked;
  factory ChatDto.fromJson(Json j) => ChatDto(
      id: textOf(j['id']),
      appointmentId: optionalText(j['appointment_id']),
      patientId: optionalText(j['patient_id']),
      patientName: optionalText(j['patient_display_name']),
      lastMessageAt: optionalText(j['last_message_at']),
      expiresAt: optionalText(j['expires_at']),
      locked: j['is_locked'] as bool?,
      modality: optionalText(j['modality']),
      appointmentStatus: optionalText(j['appointment_status']));
}

class MessageDto {
  const MessageDto(
      {required this.id,
      required this.content,
      required this.sentAt,
      this.senderId,
      this.attachmentId,
      this.attachmentSize});
  final String id, content, sentAt;
  final String? senderId, attachmentId;
  final int? attachmentSize;
  factory MessageDto.fromJson(Json j) => MessageDto(
      id: textOf(j['id']),
      content: textOf(j['content']),
      sentAt: textOf(j['sent_at']),
      senderId: optionalText(j['sender_id']),
      attachmentId: optionalText(j['attachment_id']),
      attachmentSize: (j['attachment'] as Map?)?['size_bytes'] as int?);
}

class QnaDto {
  const QnaDto(
      {required this.id,
      required this.question,
      required this.status,
      this.category,
      this.createdAt,
      this.answer,
      this.answeringDoctor,
      this.answeredAt});
  final String id, question, status;
  final String? category, createdAt, answer, answeringDoctor, answeredAt;
  factory QnaDto.fromJson(Json j) => QnaDto(
      id: textOf(j['id']),
      question: textOf(j['question_text']),
      status: textOf(j['status'], 'OPEN'),
      category: optionalText(j['category']),
      createdAt: optionalText(j['created_at']),
      answer: optionalText(j['answer_text']),
      answeringDoctor: optionalText(j['answering_doctor']),
      answeredAt: optionalText(j['answered_at']));
}

class WalletDto {
  const WalletDto(
      {this.doctorName,
      this.currency = 'KES',
      this.gross,
      this.withheld,
      this.net,
      this.pending,
      this.finalizedUnpaid,
      this.scheduledUnpaid,
      this.totalPaid,
      this.basis,
      this.platformChargePercent,
      this.lastDisbursementAt,
      this.lastDisbursementAmount});
  final String? doctorName, currency, basis;
  final String? gross, withheld, net, pending;
  final String? finalizedUnpaid, scheduledUnpaid, totalPaid;
  factory WalletDto.fromJson(Json j) => WalletDto(
      doctorName: optionalText(j['doctor_name']),
      currency: optionalText(j['currency']),
      gross: optionalText(j['total_gross']),
      withheld: optionalText(j['withheld_fee']),
      net: optionalText(j['final_net']),
      pending: optionalText(j['pending_disbursement']),
      finalizedUnpaid: optionalText(j['finalized_unpaid']),
      scheduledUnpaid: optionalText(j['scheduled_unpaid']),
      totalPaid: optionalText(j['total_paid']),
      basis: optionalText(j['basis_note']),
      platformChargePercent: j['platform_charge_percent']?.toString(),
      lastDisbursementAt: optionalText(j['last_disbursement_at']),
      lastDisbursementAmount: optionalText(j['last_disbursement_amount']));
  final String? platformChargePercent,
      lastDisbursementAt,
      lastDisbursementAmount;
}

class ProfileDto {
  const ProfileDto(
      {required this.name,
      required this.license,
      required this.hospital,
      required this.specialty,
      required this.verificationStatus,
      required this.qualifications,
      this.licenseAuthority = 'KMPDC',
      this.licenseCountry = 'KE',
      this.reviewNote,
      this.userId});
  final String name, license, hospital, specialty;
  final String verificationStatus, licenseAuthority, licenseCountry;
  final String? reviewNote;
  bool get verified => verificationStatus == 'VERIFIED';
  String get verificationLabel => switch (verificationStatus) {
        'VERIFIED' => 'Verified',
        'REJECTED' => 'Rejected',
        'PENDING' => 'Pending review',
        _ => 'Verification unavailable',
      };
  final List<String> qualifications;
  // The doctor's own `users.id` — distinct from the doctor_profile id used
  // elsewhere. Lets the UI tell apart a doctor-authored vs. patient-authored
  // chat message, since chat_messages.sender_id is FK'd to users(id).
  final String? userId;
  factory ProfileDto.fromJson(Json j) => ProfileDto(
      name: textOf(j['full_name']),
      license: textOf(j['license_number']),
      hospital: textOf(j['current_hospital']),
      specialty: textOf(j['primary_specialty']),
      verificationStatus: textOf(j['professional_verification_status']),
      licenseAuthority: textOf(j['license_authority']),
      licenseCountry: textOf(j['license_country']),
      reviewNote: optionalText(j['professional_verification_note']),
      qualifications: (j['qualifications'] as List? ?? const [])
          .map((e) => e.toString())
          .toList(),
      userId: optionalText(j['user_id']));
}

int compareMessageInstants(String left, String right) {
  final a = DateTime.tryParse(left), b = DateTime.tryParse(right);
  return a != null && b != null ? a.compareTo(b) : left.compareTo(right);
}
