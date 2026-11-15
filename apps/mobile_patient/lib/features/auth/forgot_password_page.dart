import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/widgets/patient_widgets.dart';
import '../../l10n/app_localizations.dart';
import 'auth_cubit.dart';
import 'auth_scene.dart';
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

  /// "Forgot your password?": dark green card over the clinic scene, with the
  /// lock tile overlapping its top edge.
  ///
  /// The supplied design offers "email address or phone number" and "a link
  /// or code". Password recovery here is by email code only, so the label and
  /// the sentence say that; the layout is otherwise the design's.
  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: AuthColors.cream,
        body: BlocConsumer<AuthCubit, AuthState>(
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
            return AuthScene(
              name: 'forgot',
              headerRows: 600,
              topColor: const Color(0xFFFEF2DF),
              sideMargin: 16,
              overlap: 8,
              child: Stack(clipBehavior: Clip.none, children: [
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.fromLTRB(22, 50, 22, 22),
                  decoration: BoxDecoration(
                      color: AuthColors.darkCard,
                      borderRadius: BorderRadius.circular(36)),
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const Text('Forgot your password?',
                          textAlign: TextAlign.center,
                          style: TextStyle(
                              fontSize: 26,
                              height: 1.2,
                              fontWeight: FontWeight.w800,
                              letterSpacing: -.3,
                              color: Colors.white)),
                      const SizedBox(height: 10),
                      const Text(
                          "No worries. Enter your email address and we'll "
                          'send you a code to reset your password.',
                          textAlign: TextAlign.center,
                          style: TextStyle(
                              fontSize: 14.5,
                              height: 1.45,
                              color: Colors.white)),
                      const SizedBox(height: 22),
                      PatientField(
                          label: 'Email address',
                          controller: email,
                          hint: 'you@example.com',
                          keyboardType: TextInputType.emailAddress,
                          labelIcon: Icons.mail_outline_rounded,
                          labelColor: Colors.white,
                          prefixIcon: Icons.mail_outline_rounded,
                          fillColor: Colors.white,
                          borderColor: Colors.white,
                          borderRadius: 16),
                      if (state is AuthFailure ||
                          state is PasswordResetUnavailable) ...[
                        const SizedBox(height: 10),
                        Text(
                            state is AuthFailure
                                ? state.message
                                : AppLocalizations.of(context)
                                    .passwordResetUnavailable,
                            textAlign: TextAlign.center,
                            style: const TextStyle(
                                fontSize: 13,
                                fontWeight: FontWeight.w600,
                                color: Color(0xFFFFC9C4))),
                      ],
                      const SizedBox(height: 18),
                      AuthPrimaryButton(
                          label: 'Reset password',
                          trailingIcon: Icons.arrow_forward_rounded,
                          busy: busy,
                          color: AuthColors.resetButton,
                          onPressed: busy
                              ? null
                              : () =>
                                  widget.auth.requestPasswordReset(email.text)),
                      const SizedBox(height: 10),
                      TextButton.icon(
                        onPressed:
                            busy ? null : () => Navigator.of(context).pop(),
                        icon: const Icon(Icons.arrow_back_rounded,
                            size: 20, color: Colors.white),
                        label: const Text('Back to log in',
                            style: TextStyle(
                                fontSize: 15,
                                fontWeight: FontWeight.w600,
                                color: Colors.white)),
                      ),
                    ],
                  ),
                ),
                Positioned(
                  top: -24,
                  left: 0,
                  right: 0,
                  child: Center(
                    child: Container(
                      width: 68,
                      height: 62,
                      decoration: BoxDecoration(
                          color: AuthColors.lockTile,
                          borderRadius: BorderRadius.circular(18)),
                      child: const Icon(Icons.lock_reset_rounded,
                          size: 36, color: AuthColors.darkButton),
                    ),
                  ),
                ),
              ]),
            );
          },
        ),
      );
}
