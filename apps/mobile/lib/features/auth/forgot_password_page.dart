import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import 'auth_cubit.dart';
import 'reset_otp_verify_page.dart';

/// Phase 2: Secure Email OTP Password Recovery — step 1. Collects only an
/// email address; the backend's response is identical whether or not an
/// eligible account exists, so this screen always proceeds to
/// [ResetOtpVerifyPage] on success.
class ForgotPasswordPage extends StatefulWidget {
  const ForgotPasswordPage({super.key, required this.auth});
  final AuthCubit auth;
  @override
  State<ForgotPasswordPage> createState() => _ForgotPasswordPageState();
}

class _ForgotPasswordPageState extends State<ForgotPasswordPage> {
  final email = TextEditingController();
  bool navigated = false;

  @override
  void dispose() {
    email.dispose();
    super.dispose();
  }

  Widget _button(String label, bool busy, VoidCallback f) => SizedBox(
      width: double.infinity,
      child: FilledButton(
          onPressed: busy ? null : f,
          style: FilledButton.styleFrom(
              backgroundColor: DoctorTokens.primary,
              padding: const EdgeInsets.symmetric(vertical: 14)),
          child: busy
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(
                      color: Colors.white, strokeWidth: 2))
              : Text(label)));

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Reset your password')),
      body: SafeArea(
          child: Center(
              child: SingleChildScrollView(
                  padding: const EdgeInsets.all(28),
                  child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 420),
                      child: BlocConsumer<AuthCubit, AuthState>(
                          bloc: widget.auth,
                          listenWhen: (_, s) => s is PasswordResetOtpSent,
                          listener: (context, state) {
                            if (state is PasswordResetOtpSent && !navigated) {
                              navigated = true;
                              Navigator.of(context)
                                  .push(MaterialPageRoute(
                                      builder: (_) => ResetOtpVerifyPage(
                                          auth: widget.auth, challenge: state)))
                                  .then((_) => navigated = false);
                            }
                          },
                          builder: (c, s) {
                            final busy = s is AuthBusy;
                            return Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  const Text('Forgot your password?',
                                      style: TextStyle(
                                          fontSize: 22,
                                          fontWeight: FontWeight.w800,
                                          color: DoctorTokens.text)),
                                  const SizedBox(height: 6),
                                  const Text(
                                      "Enter the email address on your account. If it matches an account, we'll send a verification code.",
                                      style: TextStyle(
                                          color: DoctorTokens.textSub)),
                                  const SizedBox(height: 22),
                                  TextField(
                                      controller: email,
                                      keyboardType: TextInputType.emailAddress,
                                      autofillHints: const [
                                        AutofillHints.email
                                      ],
                                      decoration: const InputDecoration(
                                          labelText: 'Email address')),
                                  const SizedBox(height: 18),
                                  _button(
                                      'Send verification code',
                                      busy,
                                      () => widget.auth
                                          .requestPasswordReset(email.text)),
                                  Center(
                                      child: TextButton(
                                          onPressed: busy
                                              ? null
                                              : () =>
                                                  Navigator.of(context).pop(),
                                          child:
                                              const Text('Back to sign in'))),
                                  if (s is AuthFailure)
                                    Padding(
                                        padding: const EdgeInsets.only(top: 14),
                                        child: Text(s.message,
                                            style: const TextStyle(
                                                color: DoctorTokens.danger))),
                                ]);
                          }))))));
}
