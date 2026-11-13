import 'package:flutter/material.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/afya_auth_widgets.dart';
import 'auth_cubit.dart';

/// Phase 2 — final screen. Deliberately does not sign the user in: every
/// existing session for the account was revoked when the reset completed,
/// so "Continue to sign in" always lands on the ordinary sign-in form.
class PasswordResetSuccessPage extends StatelessWidget {
  const PasswordResetSuccessPage({super.key, required this.auth});
  final AuthCubit auth;

  void _continue(BuildContext context) {
    auth.restartPasswordReset();
    Navigator.of(context).popUntil((route) => route.isFirst);
  }

  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.screen,
      body: AfyaReviewStatusView(
          icon: Icons.check_circle,
          title: 'Password Reset Successful',
          message:
              'Your password has been changed. For your security, you have been signed out of every device and must sign in again with your new password.',
          steps: const [],
          primaryLabel: 'Continue to sign in',
          onPrimary: () => _continue(context)));
}
