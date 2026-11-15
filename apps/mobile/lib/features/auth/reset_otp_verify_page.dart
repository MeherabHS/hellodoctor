import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/afya_auth_widgets.dart';
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

  @override
  Widget build(BuildContext context) => BlocConsumer<AuthCubit, AuthState>(
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
      builder: (c, s) {
        final busy = s is AuthBusy;
        return AfyaAuthFormPage(
            barTitle: 'Check Your Email',
            heading: 'Enter your verification code',
            message:
                'If an account exists for ${widget.challenge.email}, we sent a 6-digit code to that address.',
            children: [
              AfyaAuthField(
                  controller: code,
                  hint: 'Verification code',
                  icon: Icons.pin_outlined,
                  keyboardType: TextInputType.number),
              const SizedBox(height: 8),
              Text(
                  remaining > 0
                      ? 'Code expires in ${remaining}s'
                      : 'Code may have expired. You can still try, or request a new one.',
                  textAlign: TextAlign.center,
                  style:
                      const TextStyle(fontSize: 12, color: DoctorTokens.slate)),
              const SizedBox(height: 20),
              AfyaPrimaryButton(
                  label: 'Verify code',
                  busy: busy,
                  onPressed: () =>
                      widget.auth.verifyPasswordResetOtp(code.text)),
              const SizedBox(height: 6),
              TextButton(
                  onPressed: busy ? null : () => Navigator.of(context).pop(),
                  child: const Text("Didn't get a code? Start over")),
              if (s is AuthFailure) AfyaFormError(s.message),
            ]);
      });
}
