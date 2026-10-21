import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/theme/tokens.dart';
import '../../l10n/app_localizations.dart';
import 'auth_cubit.dart';
import 'otp_verify_page.dart';

/// Entry point of the patient login/registration flow.
///
/// `prototype/index.html`'s `#authModal` models this as a single
/// password-or-OTP text field with no distinct OTP step — that doesn't match
/// the real backend contract (`POST /auth/register-otp` then
/// `POST /auth/verify-otp-and-login`, phone-number-only, no password), so
/// this screen and [OtpVerifyPage] are designed fresh against the real
/// 2-step contract rather than ported from the prototype.
class PhoneEntryPage extends StatefulWidget {
  const PhoneEntryPage({super.key, required this.auth});
  final AuthCubit auth;

  @override
  State<PhoneEntryPage> createState() => _PhoneEntryPageState();
}

class _PhoneEntryPageState extends State<PhoneEntryPage> {
  final _phoneController = TextEditingController();
  bool _navigated = false;

  @override
  void dispose() {
    _phoneController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: BlocConsumer<AuthCubit, AuthState>(
          bloc: widget.auth,
          listenWhen: (previous, current) => current is OtpSent,
          listener: (context, state) {
            if (state is OtpSent && !_navigated) {
              _navigated = true;
              Navigator.of(context)
                  .push(MaterialPageRoute(
                      builder: (_) => OtpVerifyPage(auth: widget.auth)))
                  .then((_) => _navigated = false);
            }
          },
          builder: (context, state) {
            final busy = state is AuthBusy;
            final l10n = AppLocalizations.of(context);
            return SingleChildScrollView(
              padding: const EdgeInsets.all(24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const SizedBox(height: 32),
                  Text(l10n.appTitle,
                      style: const TextStyle(
                          fontSize: 28,
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.primaryDark)),
                  const SizedBox(height: 8),
                  Text(l10n.authSignInSubtitle,
                      style: const TextStyle(color: PatientTokens.textSub)),
                  const SizedBox(height: 28),
                  TextField(
                    controller: _phoneController,
                    keyboardType: TextInputType.phone,
                    enabled: !busy,
                    decoration: InputDecoration(
                      labelText: l10n.authPhoneNumberLabel,
                      hintText: l10n.authPhoneNumberHint,
                    ),
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
                          : () => widget.auth.requestOtp(_phoneController.text),
                      style: FilledButton.styleFrom(
                          backgroundColor: PatientTokens.primary,
                          padding: const EdgeInsets.symmetric(vertical: 14)),
                      child: busy
                          ? const SizedBox(
                              height: 18,
                              width: 18,
                              child: CircularProgressIndicator(
                                  strokeWidth: 2, color: Colors.white))
                          : Text(l10n.authSendCodeButton),
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
