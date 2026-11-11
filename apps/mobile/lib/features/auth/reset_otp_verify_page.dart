import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import 'auth_cubit.dart';
import 'create_new_password_page.dart';

/// Phase 2 — step 2 ("Check your email"). Enters the 6-digit code; on
/// success, moves on to [CreateNewPasswordPage].
class ResetOtpVerifyPage extends StatefulWidget {
  const ResetOtpVerifyPage(
      {super.key, required this.auth, required this.challenge});
  final AuthCubit auth;
  final PasswordResetOtpSent challenge;
  @override
  State<ResetOtpVerifyPage> createState() => _ResetOtpVerifyPageState();
}

class _ResetOtpVerifyPageState extends State<ResetOtpVerifyPage> {
  final code = TextEditingController();
  Timer? timer;
  int remaining = 0;
  bool navigated = false;

  @override
  void initState() {
    super.initState();
    remaining = widget.challenge.expiresInSeconds;
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
      appBar: AppBar(title: const Text('Check your email')),
      body: SafeArea(
          child: Center(
              child: SingleChildScrollView(
                  padding: const EdgeInsets.all(28),
                  child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 420),
                      child: BlocConsumer<AuthCubit, AuthState>(
                          bloc: widget.auth,
                          listenWhen: (_, s) => s is PasswordResetVerified,
                          listener: (context, state) {
                            if (state is PasswordResetVerified && !navigated) {
                              navigated = true;
                              Navigator.of(context)
                                  .push(MaterialPageRoute(
                                      builder: (_) => CreateNewPasswordPage(
                                          auth: widget.auth)))
                                  .then((_) => navigated = false);
                            }
                          },
                          builder: (c, s) {
                            final busy = s is AuthBusy;
                            return Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  const Text('Enter your verification code',
                                      style: TextStyle(
                                          fontSize: 22,
                                          fontWeight: FontWeight.w800,
                                          color: DoctorTokens.text)),
                                  const SizedBox(height: 6),
                                  Text(
                                      'If an account exists for ${widget.challenge.email}, we sent a 6-digit code to that address.',
                                      style: const TextStyle(
                                          color: DoctorTokens.textSub)),
                                  const SizedBox(height: 22),
                                  TextField(
                                      controller: code,
                                      keyboardType: TextInputType.number,
                                      decoration: const InputDecoration(
                                          labelText: 'Verification code')),
                                  const SizedBox(height: 6),
                                  Text(
                                      remaining > 0
                                          ? 'Code expires in ${remaining}s'
                                          : 'Code may have expired. You can still try, or request a new one.',
                                      style: const TextStyle(
                                          fontSize: 12,
                                          color: DoctorTokens.textSub)),
                                  const SizedBox(height: 18),
                                  _button(
                                      'Verify code',
                                      busy,
                                      () => widget.auth
                                          .verifyPasswordResetOtp(code.text)),
                                  Center(
                                      child: TextButton(
                                          onPressed: busy
                                              ? null
                                              : () =>
                                                  Navigator.of(context).pop(),
                                          child: const Text(
                                              "Didn't get a code? Start over"))),
                                  if (s is AuthFailure)
                                    Padding(
                                        padding: const EdgeInsets.only(top: 14),
                                        child: Text(s.message,
                                            style: const TextStyle(
                                                color: DoctorTokens.danger))),
                                ]);
                          }))))));
}
