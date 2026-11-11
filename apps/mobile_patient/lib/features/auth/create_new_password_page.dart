import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import 'auth_cubit.dart';
import 'password_reset_success_page.dart';

/// Phase 2 — step 3. The only screen that actually changes the password.
/// On success the backend has already revoked every existing session for
/// the account, so this page never signs the user in — it only moves on to
/// [PasswordResetSuccessPage], which sends them back to an ordinary sign-in
/// form.
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

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: PatientTokens.bg,
        appBar: AppBar(title: const Text('Create a new password')),
        body: SafeArea(
          child: BlocConsumer<AuthCubit, AuthState>(
            bloc: widget.auth,
            listenWhen: (_, s) => s is PasswordResetCompleted,
            listener: (context, state) {
              if (state is PasswordResetCompleted && !navigated) {
                navigated = true;
                Navigator.of(context).pushAndRemoveUntil(
                    MaterialPageRoute(
                        builder: (_) => PasswordResetSuccessPage(auth: widget.auth)),
                    (route) => false);
              }
            },
            builder: (context, state) {
              final busy = state is AuthBusy;
              return SingleChildScrollView(
                padding: const EdgeInsets.fromLTRB(24, 22, 24, 28),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Choose a new password',
                        style: Theme.of(context).textTheme.headlineSmall),
                    const SizedBox(height: 8),
                    const Text(
                        'Use at least 8 characters, with an uppercase letter, a lowercase letter, a digit and a special character.',
                        style: TextStyle(color: PatientTokens.textSub)),
                    const SizedBox(height: 24),
                    PatientField(
                        label: 'New password',
                        controller: password,
                        hint: 'Enter a new password',
                        obscureText: true),
                    const SizedBox(height: 17),
                    PatientField(
                        label: 'Confirm new password',
                        controller: confirm,
                        hint: 'Re-enter the new password',
                        obscureText: true),
                    if (localError != null) ...[
                      const SizedBox(height: 12),
                      Text(localError!,
                          style: const TextStyle(color: PatientTokens.danger)),
                    ],
                    if (state is AuthFailure) ...[
                      const SizedBox(height: 12),
                      Text(state.message,
                          style: const TextStyle(color: PatientTokens.danger)),
                    ],
                    const SizedBox(height: 26),
                    SizedBox(
                        width: double.infinity,
                        child: FilledButton(
                            onPressed: busy ? null : submit,
                            child: busy
                                ? const SizedBox(
                                    width: 19,
                                    height: 19,
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2, color: Colors.white))
                                : const Text('Reset password'))),
                  ],
                ),
              );
            },
          ),
        ),
      );
}
