import 'package:flutter/material.dart';
import '../../core/theme/tokens.dart';
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
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
          child: Center(
            child: Padding(
              padding: const EdgeInsets.all(28),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    width: 72,
                    height: 72,
                    decoration: const BoxDecoration(
                        color: PatientTokens.primary, shape: BoxShape.circle),
                    child: const Icon(Icons.check_rounded,
                        color: Colors.white, size: 40),
                  ),
                  const SizedBox(height: 24),
                  Text('Password reset successful',
                      style: Theme.of(context).textTheme.headlineSmall,
                      textAlign: TextAlign.center),
                  const SizedBox(height: 8),
                  const Text(
                      'Your password has been changed. For your security, you have been signed out of every device and must sign in again with your new password.',
                      style: TextStyle(color: PatientTokens.textSub),
                      textAlign: TextAlign.center),
                  const SizedBox(height: 28),
                  SizedBox(
                      width: double.infinity,
                      child: FilledButton(
                          onPressed: () => _continue(context),
                          child: const Text('Continue to sign in'))),
                ],
              ),
            ),
          ),
        ),
      );
}
