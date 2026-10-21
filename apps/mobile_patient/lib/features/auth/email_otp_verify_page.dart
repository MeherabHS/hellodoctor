import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/network/api_client.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import 'auth_cubit.dart';

class EmailOtpVerifyPage extends StatefulWidget {
  const EmailOtpVerifyPage(
      {super.key, required this.auth, required this.challenge});
  final AuthCubit auth;
  final EmailChallengeSent challenge;
  @override
  State<EmailOtpVerifyPage> createState() => _EmailOtpVerifyPageState();
}

class _EmailOtpVerifyPageState extends State<EmailOtpVerifyPage> {
  final code = TextEditingController();
  Timer? timer;
  int remaining = 0;
  List<Map<String, dynamic>> legalDocuments = [];
  final Set<String> accepted = {};
  String? error;

  @override
  void initState() {
    super.initState();
    remaining = widget.challenge.expiresInSeconds;
    if (widget.challenge.registration) _loadConsent();
    timer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      if (remaining <= 1) {
        timer?.cancel();
        setState(() => remaining = 0);
      } else {
        setState(() => remaining--);
      }
    });
  }

  @override
  void dispose() {
    timer?.cancel();
    code.dispose();
    super.dispose();
  }

  Future<void> _loadConsent() async {
    try {
      final response = await widget.auth.api.get('/legal/documents');
      final docs = (response.data['data'] as List)
          .whereType<Map>()
          .map((e) => Map<String, dynamic>.from(e))
          .where((d) => d['document_type'].toString().startsWith('PATIENT_'))
          .toList();
      if (mounted) setState(() => legalDocuments = docs);
    } on ApiException catch (e) {
      if (mounted) setState(() => error = e.userMessage);
    } catch (_) {
      if (mounted) {
        setState(() => error = 'Unable to load the required notices.');
      }
    }
  }

  void _read(Map<String, dynamic> doc) {
    showDialog<void>(
        context: context,
        builder: (_) => AlertDialog(
                title: Text(doc['title'].toString()),
                content: SingleChildScrollView(
                    child: Text(doc['body'].toString(),
                        style: const TextStyle(height: 1.45))),
                actions: [
                  TextButton(
                      onPressed: () => Navigator.pop(context),
                      child: const Text('Close'))
                ]));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      appBar: AppBar(title: const Text('Verify your email')),
      body: SafeArea(
        child: BlocBuilder<AuthCubit, AuthState>(
          bloc: widget.auth,
          builder: (context, state) {
            final busy = state is AuthBusy;
            return SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(24, 22, 24, 28),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Check your inbox',
                        style: Theme.of(context).textTheme.headlineSmall),
                    const SizedBox(height: 8),
                    const Text(
                        'Enter the six-digit code sent to your email address.',
                        style: TextStyle(color: PatientTokens.textSub)),
                    const SizedBox(height: 24),
                    PatientField(
                        label: 'Verification code',
                        controller: code,
                        hint: '000000',
                        keyboardType: TextInputType.number),
                    const SizedBox(height: 8),
                    Text(
                        remaining > 0
                            ? 'Code expires in ${remaining}s'
                            : 'Code expired. Start again.',
                        style: const TextStyle(
                            fontSize: 12, color: PatientTokens.textSub)),
                    if (widget.challenge.debugOtpCode != null) ...[
                      const SizedBox(height: 16),
                      Container(
                          padding: const EdgeInsets.all(12),
                          decoration: BoxDecoration(
                              color: PatientTokens.warningLight,
                              borderRadius:
                                  BorderRadius.circular(PatientTokens.radius)),
                          child: Text(
                              'Development code: ${widget.challenge.debugOtpCode}',
                              style: const TextStyle(
                                  color: PatientTokens.amberText,
                                  fontWeight: FontWeight.w600))),
                    ],
                    if (widget.challenge.registration) ...[
                      const SizedBox(height: 28),
                      PatientSectionTitle('Review and acknowledge',
                          action: legalDocuments.isEmpty
                              ? null
                              : '${accepted.length}/${legalDocuments.length}'),
                      const SizedBox(height: 4),
                      const Text(
                          'Read each notice before creating your account. Your choices are recorded with the document version.',
                          style: TextStyle(
                              color: PatientTokens.textSub, fontSize: 13)),
                      const SizedBox(height: 12),
                      if (legalDocuments.isEmpty && error == null)
                        const LinearProgressIndicator(minHeight: 2),
                      for (final doc in legalDocuments)
                        _LegalRow(
                            doc: doc,
                            checked: accepted
                                .contains(doc['document_type'].toString()),
                            enabled: !busy,
                            onRead: () => _read(doc),
                            onChanged: (value) => setState(() {
                                  if (value == true) {
                                    accepted
                                        .add(doc['document_type'].toString());
                                  } else {
                                    accepted.remove(
                                        doc['document_type'].toString());
                                  }
                                })),
                    ],
                    if (error != null)
                      Padding(
                          padding: const EdgeInsets.only(top: 10),
                          child: Text(error!,
                              style: const TextStyle(
                                  color: PatientTokens.danger))),
                    const SizedBox(height: 26),
                    SizedBox(
                        width: double.infinity,
                        child: FilledButton(
                            onPressed: busy
                                ? null
                                : () {
                                    if (widget.challenge.registration &&
                                        accepted.length <
                                            legalDocuments.length) {
                                      setState(() => error =
                                          'Please read and acknowledge each notice before continuing.');
                                      return;
                                    }
                                    widget.auth.verifyPatientEmailOtp(code.text,
                                        challengeToken:
                                            widget.challenge.challengeToken,
                                        acceptedDocuments: legalDocuments
                                            .map((d) => {
                                                  'type': d['document_type']
                                                      .toString(),
                                                  'version':
                                                      d['version'].toString()
                                                })
                                            .toList());
                                  },
                            child: busy
                                ? const SizedBox(
                                    width: 19,
                                    height: 19,
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2, color: Colors.white))
                                : const Text('Verify and continue'))),
                    if (state is AuthFailure)
                      Padding(
                          padding: const EdgeInsets.only(top: 12),
                          child: Text((state).message,
                              style: const TextStyle(
                                  color: PatientTokens.danger))),
                  ]),
            );
          },
        ),
      ),
    );
  }
}

class _LegalRow extends StatelessWidget {
  const _LegalRow(
      {required this.doc,
      required this.checked,
      required this.enabled,
      required this.onRead,
      required this.onChanged});
  final Map<String, dynamic> doc;
  final bool checked, enabled;
  final VoidCallback onRead;
  final ValueChanged<bool?> onChanged;
  @override
  Widget build(BuildContext context) => Container(
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
          color: PatientTokens.surface,
          border: Border.all(
              color: checked ? PatientTokens.primary : PatientTokens.border),
          borderRadius: BorderRadius.circular(PatientTokens.radius)),
      child: Row(children: [
        Checkbox(
            value: checked,
            onChanged: enabled ? onChanged : null,
            activeColor: PatientTokens.primary),
        Expanded(
            child: Text(doc['title'].toString(),
                style: const TextStyle(
                    fontSize: 14, fontWeight: FontWeight.w600))),
        TextButton(onPressed: onRead, child: const Text('Read')),
        const SizedBox(width: 6)
      ]));
}
