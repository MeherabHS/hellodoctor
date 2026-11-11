import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import 'auth_cubit.dart';
import 'reset_otp_verify_page.dart';

/// Phase 2: Secure Email OTP Password Recovery — step 1. Collects only an
/// email address; the backend's response is identical whether or not an
/// eligible account exists, so this screen always proceeds to
/// [ResetOtpVerifyPage] on success rather than branching on account
/// existence (there is nothing here for it to branch on).
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
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: PatientTokens.bg,
        appBar: AppBar(title: const Text('Reset your password')),
        body: SafeArea(
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
            builder: (context, state) {
              final busy = state is AuthBusy;
              return SingleChildScrollView(
                padding: const EdgeInsets.fromLTRB(24, 22, 24, 28),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Forgot your password?',
                        style: Theme.of(context).textTheme.headlineSmall),
                    const SizedBox(height: 8),
                    const Text(
                        "Enter the email address on your account. If it matches an account, we'll send a verification code.",
                        style: TextStyle(color: PatientTokens.textSub)),
                    const SizedBox(height: 24),
                    PatientField(
                        label: 'Email address',
                        controller: email,
                        hint: 'you@example.com',
                        keyboardType: TextInputType.emailAddress),
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
                                : () => widget.auth
                                    .requestPasswordReset(email.text),
                            child: busy
                                ? const SizedBox(
                                    width: 19,
                                    height: 19,
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2, color: Colors.white))
                                : const Text('Send verification code'))),
                    const SizedBox(height: 16),
                    Center(
                      child: TextButton(
                        onPressed:
                            busy ? null : () => Navigator.of(context).pop(),
                        child: const Text('Back to sign in'),
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
