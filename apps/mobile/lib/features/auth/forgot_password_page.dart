import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/widgets/afya_auth_widgets.dart';
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

  @override
  Widget build(BuildContext context) => BlocConsumer<AuthCubit, AuthState>(
      bloc: widget.auth,
      listenWhen: (_, s) => s is PasswordResetOtpSent,
      listener: (context, state) {
        if (state is PasswordResetOtpSent && !navigated) {
          navigated = true;
          Navigator.of(context)
              .push(MaterialPageRoute(
                  builder: (_) =>
                      ResetOtpVerifyPage(auth: widget.auth, challenge: state)))
              .then((_) => navigated = false);
        }
      },
      builder: (c, s) {
        final busy = s is AuthBusy;
        return AfyaAuthFormPage(
            barTitle: 'Reset Password',
            heading: 'Forgot your password?',
            message:
                "Enter the email address on your account. If it matches an account, we'll send a verification code.",
            children: [
              AfyaAuthField(
                  controller: email,
                  hint: 'Email address',
                  icon: Icons.mail_outline,
                  keyboardType: TextInputType.emailAddress,
                  autofillHints: const [AutofillHints.email]),
              const SizedBox(height: 20),
              AfyaPrimaryButton(
                  label: 'Send verification code',
                  busy: busy,
                  onPressed: () =>
                      widget.auth.requestPasswordReset(email.text)),
              const SizedBox(height: 6),
              TextButton(
                  onPressed: busy ? null : () => Navigator.of(context).pop(),
                  child: const Text('Back to sign in')),
              if (s is AuthFailure) AfyaFormError(s.message),
            ]);
      });
}
