import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/widgets/afya_auth_widgets.dart';
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
  bool _hide = true;

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
  Widget build(BuildContext context) => BlocConsumer<AuthCubit, AuthState>(
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
      builder: (c, s) {
        final busy = s is AuthBusy;
        return AfyaAuthFormPage(
            barTitle: 'New Password',
            heading: 'Choose a new password',
            message:
                'Use at least 8 characters, with an uppercase letter, a lowercase letter, a digit and a special character.',
            children: [
              AfyaAuthField(
                  controller: password,
                  hint: 'New password',
                  icon: Icons.lock_outline,
                  obscure: _hide,
                  onToggleObscure: () => setState(() => _hide = !_hide)),
              const SizedBox(height: 12),
              AfyaAuthField(
                  controller: confirm,
                  hint: 'Confirm new password',
                  icon: Icons.lock_outline,
                  obscure: _hide),
              const SizedBox(height: 20),
              AfyaPrimaryButton(
                  label: 'Reset password', busy: busy, onPressed: submit),
              if (localError != null) AfyaFormError(localError!),
              if (s is AuthFailure) AfyaFormError(s.message),
            ]);
      });
}
