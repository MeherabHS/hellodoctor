import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/network/api_client.dart';
import '../../core/theme/tokens.dart';
import '../../l10n/app_localizations.dart';
import 'auth_cubit.dart';

/// Second step of the real backend OTP contract: enter the 6-digit code sent
/// for the phone number [PhoneEntryPage] already submitted via
/// `POST /auth/register-otp`. See `auth_cubit.dart`'s `OtpSent` state doc for
/// why a `session_token` is never sent from here.
class OtpVerifyPage extends StatefulWidget {
  const OtpVerifyPage({super.key, required this.auth});
  final AuthCubit auth;

  @override
  State<OtpVerifyPage> createState() => _OtpVerifyPageState();
}

class _PatientConsentDocument {
  const _PatientConsentDocument({
    required this.version,
    required this.title,
    required this.body,
  });
  final String version;
  final String title;
  final String body;
}

class _OtpVerifyPageState extends State<OtpVerifyPage> {
  final _codeController = TextEditingController();
  Timer? _ticker;
  int _secondsRemaining = 0;
  _PatientConsentDocument? _consent;
  bool _consentAccepted = false;
  String? _consentError;

  @override
  void initState() {
    super.initState();
    final state = widget.auth.state;
    if (state is OtpSent) _startCountdown(state.expiresInSeconds);
    _loadConsent();
  }

  Future<void> _loadConsent() async {
    try {
      final response = await widget.auth.api.get('/legal/documents');
      final documents = (response.data['data'] as List).whereType<Map>();
      final consent = documents.cast<Map<String, dynamic>?>().firstWhere(
            (doc) => doc?['document_type'] == 'PATIENT_DATA_SHARING_CONSENT',
            orElse: () => null,
          );
      if (!mounted) return;
      if (consent == null) {
        setState(() => _consentError =
            AppLocalizations.of(context).authDataConsentLoadError);
        return;
      }
      setState(() {
        _consent = _PatientConsentDocument(
          version: consent['version'].toString(),
          title: consent['title'].toString(),
          body: consent['body'].toString(),
        );
        _consentError = null;
      });
    } on ApiException catch (error) {
      if (mounted) setState(() => _consentError = error.userMessage);
    } catch (_) {
      if (mounted) {
        setState(() => _consentError =
            AppLocalizations.of(context).authDataConsentLoadError);
      }
    }
  }

  Future<void> _showConsent() async {
    final consent = _consent;
    if (consent == null) return;
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(consent.title),
        content: SingleChildScrollView(child: Text(consent.body)),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: Text(MaterialLocalizations.of(context).closeButtonLabel)),
        ],
      ),
    );
  }

  void _startCountdown(int seconds) {
    _ticker?.cancel();
    setState(() => _secondsRemaining = seconds);
    _ticker = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (!mounted) {
        timer.cancel();
        return;
      }
      if (_secondsRemaining <= 1) {
        timer.cancel();
        setState(() => _secondsRemaining = 0);
      } else {
        setState(() => _secondsRemaining -= 1);
      }
    });
  }

  @override
  void dispose() {
    _ticker?.cancel();
    _codeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: BlocConsumer<AuthCubit, AuthState>(
          bloc: widget.auth,
          listenWhen: (previous, current) =>
              current is OtpSent && previous is! OtpSent,
          listener: (context, state) {
            if (state is OtpSent) _startCountdown(state.expiresInSeconds);
          },
          builder: (context, state) {
            final busy = state is AuthBusy;
            final sent = state is OtpSent ? state : null;
            final l10n = AppLocalizations.of(context);
            return SingleChildScrollView(
              padding: const EdgeInsets.all(24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  IconButton(
                    onPressed: () => Navigator.of(context).maybePop(),
                    icon:
                        const Icon(Icons.arrow_back, color: PatientTokens.text),
                  ),
                  const SizedBox(height: 8),
                  Text(l10n.authEnterCodeTitle,
                      style: const TextStyle(
                          fontSize: 22,
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text)),
                  const SizedBox(height: 8),
                  Text(
                    sent != null
                        ? l10n.authCodeSentTo(sent.phoneNumber)
                        : l10n.authEnterSixDigitCode,
                    style: const TextStyle(color: PatientTokens.textSub),
                  ),
                  if (sent?.debugOtpCode != null) ...[
                    const SizedBox(height: 14),
                    Container(
                      padding: const EdgeInsets.all(10),
                      decoration: BoxDecoration(
                        color: PatientTokens.warningLight,
                        borderRadius:
                            BorderRadius.circular(PatientTokens.radius),
                      ),
                      child: Text(
                        l10n.authDevOnlyCode(sent!.debugOtpCode!),
                        style: const TextStyle(
                            color: PatientTokens.warning,
                            fontWeight: FontWeight.w700),
                      ),
                    ),
                  ],
                  const SizedBox(height: 18),
                  if (_consentError != null) ...[
                    Text(_consentError!,
                        style: const TextStyle(color: PatientTokens.danger)),
                    if (_consent == null)
                      TextButton(
                        onPressed: busy ? null : _loadConsent,
                        child: Text(l10n.retryButton),
                      ),
                  ],
                  if (_consent != null)
                    Card(
                      margin: const EdgeInsets.only(top: 8),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          ListTile(
                            title: Text(l10n.authDataConsentTitle,
                                style: const TextStyle(
                                    fontWeight: FontWeight.w700)),
                            subtitle: Text(l10n.authDataConsentSummary),
                          ),
                          TextButton(
                            onPressed: _showConsent,
                            child: Text(l10n.authDataConsentView),
                          ),
                          CheckboxListTile(
                            value: _consentAccepted,
                            onChanged: busy
                                ? null
                                : (value) => setState(
                                    () => _consentAccepted = value ?? false),
                            controlAffinity: ListTileControlAffinity.leading,
                            title: Text(l10n.authDataConsentAccept),
                          ),
                        ],
                      ),
                    ),
                  const SizedBox(height: 24),
                  TextField(
                    controller: _codeController,
                    keyboardType: TextInputType.number,
                    maxLength: 6,
                    enabled: !busy,
                    style: const TextStyle(
                        fontSize: 22,
                        letterSpacing: 6,
                        fontWeight: FontWeight.w700),
                    decoration: InputDecoration(
                        labelText: l10n.authVerificationCodeLabel,
                        counterText: ''),
                  ),
                  if (state is AuthFailure) ...[
                    const SizedBox(height: 12),
                    Text(state.message,
                        style: const TextStyle(color: PatientTokens.danger)),
                  ],
                  const SizedBox(height: 20),
                  SizedBox(
                    width: double.infinity,
                    child: FilledButton(
                      onPressed: busy
                          ? null
                          : () {
                              if (_consent == null || !_consentAccepted) {
                                setState(() => _consentError =
                                    l10n.authDataConsentRequired);
                                return;
                              }
                              widget.auth.verifyOtp(_codeController.text,
                                  acceptedDocuments: [
                                    {
                                      'type': 'PATIENT_DATA_SHARING_CONSENT',
                                      'version': _consent!.version,
                                    }
                                  ]);
                            },
                      style: FilledButton.styleFrom(
                          backgroundColor: PatientTokens.primary,
                          padding: const EdgeInsets.symmetric(vertical: 14)),
                      child: busy
                          ? const SizedBox(
                              height: 18,
                              width: 18,
                              child: CircularProgressIndicator(
                                  strokeWidth: 2, color: Colors.white))
                          : Text(l10n.authVerifyButton),
                    ),
                  ),
                  const SizedBox(height: 12),
                  Center(
                    child: TextButton(
                      onPressed: (busy || _secondsRemaining > 0 || sent == null)
                          ? null
                          : () => widget.auth.requestOtp(sent.phoneNumber),
                      child: Text(_secondsRemaining > 0
                          ? l10n.authResendCodeIn(_secondsRemaining)
                          : l10n.authResendCode),
                    ),
                  ),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}
