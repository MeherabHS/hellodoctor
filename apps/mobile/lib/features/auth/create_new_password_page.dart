import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import 'auth_cubit.dart';
import 'password_reset_success_page.dart';

/// Phase 2 — step 3. The only screen that actually changes the password.
/// Never signs the user in afterward — every existing session was revoked
/// by the backend as part of completing the reset.
class CreateNewPasswordPage extends StatefulWidget {
  const CreateNewPasswordPage({super.key, required this.auth});
  final AuthCubit auth;
  @override
  State<CreateNewPasswordPage> createState() => _CreateNewPasswordPageState();
}

class _CreateNewPasswordPageState extends State<CreateNewPasswordPage> {
  final password = TextEditingController();
  final confirm = TextEditingController();
  String? localError;
  bool navigated = false;

  @override
  void dispose() {
    password.dispose();
    confirm.dispose();
    super.dispose();
  }

  void submit() {
    if (password.text != confirm.text) {
      setState(() => localError = 'Passwords do not match.');
      return;
    }
    setState(() => localError = null);
    widget.auth.completePasswordReset(password.text);
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
      appBar: AppBar(title: const Text('Create a new password')),
      body: SafeArea(
          child: Center(
              child: SingleChildScrollView(
                  padding: const EdgeInsets.all(28),
                  child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 420),
                      child: BlocConsumer<AuthCubit, AuthState>(
                          bloc: widget.auth,
                          listenWhen: (_, s) => s is PasswordResetCompleted,
                          listener: (context, state) {
                            if (state is PasswordResetCompleted && !navigated) {
                              navigated = true;
                              Navigator.of(context).pushAndRemoveUntil(
                                  MaterialPageRoute(
                                      builder: (_) => PasswordResetSuccessPage(
                                          auth: widget.auth)),
                                  (route) => false);
                            }
                          },
                          builder: (c, s) {
                            final busy = s is AuthBusy;
                            return Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  const Text('Choose a new password',
                                      style: TextStyle(
                                          fontSize: 22,
                                          fontWeight: FontWeight.w800,
                                          color: DoctorTokens.text)),
                                  const SizedBox(height: 6),
                                  const Text(
                                      'Use at least 8 characters, with an uppercase letter, a lowercase letter, a digit and a special character.',
                                      style: TextStyle(
                                          color: DoctorTokens.textSub)),
                                  const SizedBox(height: 22),
                                  TextField(
                                      controller: password,
                                      obscureText: true,
                                      decoration: const InputDecoration(
                                          labelText: 'New password')),
                                  const SizedBox(height: 12),
                                  TextField(
                                      controller: confirm,
                                      obscureText: true,
                                      decoration: const InputDecoration(
                                          labelText: 'Confirm new password')),
                                  const SizedBox(height: 18),
                                  _button('Reset password', busy, submit),
                                  if (localError != null)
                                    Padding(
                                        padding: const EdgeInsets.only(top: 14),
                                        child: Text(localError!,
                                            style: const TextStyle(
                                                color: DoctorTokens.danger))),
                                  if (s is AuthFailure)
                                    Padding(
                                        padding: const EdgeInsets.only(top: 14),
                                        child: Text(s.message,
                                            style: const TextStyle(
                                                color: DoctorTokens.danger))),
                                ]);
                          }))))));
}
