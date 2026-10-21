import '../../../core/l10n/patient_display.dart';

typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();
String? _optionalText(dynamic value) => value?.toString();
DateTime _dateOf(dynamic value) =>
    DateTime.tryParse(_textOf(value))?.toLocal() ??
    DateTime.fromMillisecondsSinceEpoch(0);

/// Mirrors one entry of `GET /patients/me/qna/questions`
/// (`patient_handlers::list_answered_questions`) — a Doctor-answered Free
/// Health Q&A entry (plan's gap #6, M8). Never carries the asking patient's
/// identity: `health_questions`' SELECT grant for `hellodoctor_api` is
/// column-scoped to exclude `author_patient_id`
/// (`20261026000001_canonical_user_isolation`), so there is no field here to
/// parse even if the server tried to send one.
class AnsweredQuestionDto {
  const AnsweredQuestionDto({
    required this.id,
    required this.questionText,
    required this.createdAt,
    required this.answerText,
    required this.answeredAt,
    required this.answeringDoctor,
    required this.answeringDoctorSpecialty,
    this.category,
    this.status = 'ANSWERED',
    this.answeringDoctorVerified = false,
  });
  final String id, questionText, answerText, answeringDoctor;
  final String answeringDoctorSpecialty;
  final String? category;
  final String status;
  final bool answeringDoctorVerified;
  final DateTime createdAt;
  final DateTime? answeredAt;
  bool get isAnswered => status == 'ANSWERED' && answerText.isNotEmpty;
  bool get isInternalFixture => category == 'WEB_FIXTURE';
  String get doctorDisplayName => patientDoctorName(answeringDoctor);
  String get displayCategory =>
      qnaCategories.contains(category) ? category! : qnaCategories.first;

  factory AnsweredQuestionDto.fromJson(Json j) => AnsweredQuestionDto(
        id: _textOf(j['id']),
        questionText: _textOf(j['question_text']),
        category: _optionalText(j['category']),
        createdAt: _dateOf(j['created_at']),
        answerText: _textOf(j['answer_text']),
        answeredAt: j['answered_at'] == null ? null : _dateOf(j['answered_at']),
        status: _textOf(
            j['status'], j['answer_text'] == null ? 'OPEN' : 'ANSWERED'),
        answeringDoctorVerified: j['answering_doctor_verified'] == true,
        answeringDoctor: _textOf(j['answering_doctor']),
        answeringDoctorSpecialty: _textOf(j['answering_doctor_specialty']),
      );
}

/// Fixed topic chips offered when asking a question — `health_questions
/// .category` is a free-text `VARCHAR(80)`, not a backend-enforced enum, so
/// this list (mirroring `prototype/index.html` view-12's topic strip) is a
/// client-side convenience only, the same shape as the Grievance modal's
/// fixed category list.
const qnaCategories = <String>[
  'General Health',
  'Fevers & Infections',
  'Child Health',
  'Diet & Diabetes',
  'Skin & Allergy',
];
