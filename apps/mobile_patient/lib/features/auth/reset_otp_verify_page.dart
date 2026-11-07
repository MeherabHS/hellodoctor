import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import 'auth_cubit.dart';
import 'create_new_password_page.dart';

/// Phase 2 — step 2 ("Check your email"). Enters the 6-digit code; on
/// success, the backend returns a one-time `reset_authorization` the cubit
/// holds privately, and this screen moves on to
/// [CreateNewPasswordPage] — it never sees or displays that value itself.
class ResetOtpVerifyPage extends StatefulWidget {
  const ResetOtpVerifyPage({super.key, required this.auth, required this.challenge});
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

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: PatientTokens.bg,
        appBar: AppBar(title: const Text('Check your email')),
        body: SafeArea(
          child: BlocConsumer<AuthCubit, AuthState>(
            bloc: widget.auth,
            listenWhen: (_, s) => s is PasswordResetVerified,
            listener: (context, state) {
              if (state is PasswordResetVerified && !navigated) {
                navigated = true;
                Navigator.of(context)
                    .push(MaterialPageRoute(
                        builder: (_) => CreateNewPasswordPage(auth: widget.auth)))
                    .then((_) => navigated = false);
              }
            },
            builder: (context, state) {
              final busy = state is AuthBusy;
              return SingleChildScrollView(
                padding: const EdgeInsets.fromLTRB(24, 22, 24, 28),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Enter your verification code',
                        style: Theme.of(context).textTheme.headlineSmall),
                    const SizedBox(height: 8),
                    Text(
                        'If an account exists for ${widget.challenge.email}, we sent a 6-digit code to that address.',
                        style: const TextStyle(color: PatientTokens.textSub)),
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
                            : 'Code may have expired. You can still try, or request a new one.',
                        style: const TextStyle(
                            fontSize: 12, color: PatientTokens.textSub)),
                    if (state is AuthFailure) ...[
                      const SizedBox(height: 12),
                      Text(state.message,
                          style: const TextStyle(color: PatientTokens.danger)),
                    ],
                    const SizedBox(height: 26),
                    SizedBox(
                        width: double.infinity,
                        child: FilledButton(
                            onPressed: busy
                                ? null
                                : () => widget.auth.verifyPasswordResetOtp(code.text),
                            child: busy
                                ? const SizedBox(
                                    width: 19,
                                    height: 19,
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2, color: Colors.white))
                                : const Text('Verify code'))),
                    const SizedBox(height: 16),
                    Center(
                      child: TextButton(
                        onPressed:
                            busy ? null : () => Navigator.of(context).pop(),
                        child: const Text("Didn't get a code? Start over"),
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
