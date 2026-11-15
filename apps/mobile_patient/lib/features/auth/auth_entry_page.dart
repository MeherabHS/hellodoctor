import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import 'auth_cubit.dart';
import 'auth_scene.dart';
import 'email_otp_verify_page.dart';
import 'forgot_password_page.dart';

class AuthEntryPage extends StatefulWidget {
  const AuthEntryPage({super.key, required this.auth});
  final AuthCubit auth;
  @override
  State<AuthEntryPage> createState() => _AuthEntryPageState();
}

class _AuthEntryPageState extends State<AuthEntryPage> {
  bool register = false, navigated = false, _obscure = true;
  final email = TextEditingController();
  final phone = TextEditingController();
  final password = TextEditingController();
  final fullName = TextEditingController();
  final dateOfBirth = TextEditingController();
  final address = TextEditingController();
  String gender = 'OTHER';
  @override
  void dispose() {
    email.dispose();
    phone.dispose();
    password.dispose();
    fullName.dispose();
    dateOfBirth.dispose();
    address.dispose();
    super.dispose();
  }

  void submit() {
    if (register) {
      widget.auth.requestPatientRegistration(
          email.text, phone.text, password.text,
          fullName: fullName.text,
          dateOfBirth: dateOfBirth.text,
          gender: gender,
          residentialAddress: address.text);
    } else {
      widget.auth.requestPatientLogin(email.text, password.text);
    }
  }

  Future<void> pickDate() async {
    final picked = await showDatePicker(
        context: context,
        firstDate: DateTime(1900),
        lastDate: DateTime.now(),
        initialDate: DateTime(DateTime.now().year - 30));
    if (picked != null) {
      setState(
          () => dateOfBirth.text = picked.toIso8601String().substring(0, 10));
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: register ? AuthColors.cream : const Color(0xFFCFE9DD),
        body: BlocConsumer<AuthCubit, AuthState>(
            bloc: widget.auth,
            listenWhen: (_, s) => s is EmailChallengeSent,
            listener: (context, state) {
              if (state is EmailChallengeSent && !navigated) {
                navigated = true;
                Navigator.of(context)
                    .push(MaterialPageRoute(
                        builder: (_) => EmailOtpVerifyPage(
                            auth: widget.auth, challenge: state)))
                    .then((_) => navigated = false);
              }
            },
            builder: (context, state) {
              final busy = state is AuthBusy;
              final error = state is AuthFailure ? state.message : null;
              return register
                  ? _registerScene(context, busy, error)
                  : _loginScene(context, busy, error);
            }),
      );

  Widget _eye() => IconButton(
      onPressed: () => setState(() => _obscure = !_obscure),
      tooltip: _obscure ? 'Show password' : 'Hide password',
      icon: Icon(
          _obscure ? Icons.visibility_outlined : Icons.visibility_off_outlined,
          size: 21,
          color: AuthColors.body));

  Widget _error(String? message) => message == null
      ? const SizedBox.shrink()
      : Padding(
          padding: const EdgeInsets.only(top: 10),
          child: Text(message,
              style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: PatientTokens.danger)));

  /// "Welcome back": frosted glass card over the clinic scene.
  Widget _loginScene(BuildContext context, bool busy, String? error) =>
      AuthScene(
        name: 'login',
        headerRows: 548,
        topColor: const Color(0xFFB9E6ED),
        child: Container(
          padding: const EdgeInsets.fromLTRB(22, 26, 22, 22),
          decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: .58),
              borderRadius: BorderRadius.circular(36),
              border: Border.all(
                  color: Colors.white.withValues(alpha: .85), width: 1.5)),
          child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Padding(
                  padding: EdgeInsets.only(left: 8),
                  child: Text('Welcome back',
                      style: TextStyle(
                          fontSize: 30,
                          height: 1.15,
                          fontWeight: FontWeight.w800,
                          letterSpacing: -.4,
                          color: AuthColors.title)),
                ),
                const SizedBox(height: 4),
                const Padding(
                  padding: EdgeInsets.only(left: 8),
                  child: Text('Access your healthcare account.',
                      style: TextStyle(fontSize: 14, color: AuthColors.body)),
                ),
                const SizedBox(height: 22),
                PatientField(
                    label: 'Email address',
                    controller: email,
                    hint: 'you@example.com',
                    keyboardType: TextInputType.emailAddress,
                    labelIcon: Icons.mail_outline_rounded,
                    labelColor: AuthColors.label,
                    fillColor: Colors.white.withValues(alpha: .62),
                    borderColor: Colors.white,
                    borderRadius: 18),
                const SizedBox(height: 16),
                PatientField(
                    label: 'Password',
                    controller: password,
                    hint: 'Enter your password',
                    obscureText: _obscure,
                    suffixIcon: _eye(),
                    labelIcon: Icons.lock_outline_rounded,
                    labelColor: AuthColors.label,
                    fillColor: Colors.white.withValues(alpha: .62),
                    borderColor: Colors.white,
                    borderRadius: 18),
                Align(
                    alignment: Alignment.centerRight,
                    child: TextButton(
                        onPressed: busy
                            ? null
                            : () => Navigator.of(context).push(
                                MaterialPageRoute(
                                    builder: (_) =>
                                        ForgotPasswordPage(auth: widget.auth))),
                        child: const Text('Forgot password?',
                            style: TextStyle(
                                fontSize: 13.5,
                                fontWeight: FontWeight.w700,
                                color: AuthColors.link,
                                decoration: TextDecoration.underline,
                                decorationColor: AuthColors.link)))),
                _error(error),
                const SizedBox(height: 12),
                AuthPrimaryButton(
                    label: 'Log in',
                    icon: Icons.login_rounded,
                    busy: busy,
                    gradient: const [
                      AuthColors.buttonTop,
                      AuthColors.buttonBottom
                    ],
                    onPressed: busy ? null : submit),
                const SizedBox(height: 18),
                Row(children: [
                  const SizedBox(
                      width: 16, child: Divider(color: Color(0x66406058))),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Wrap(
                        alignment: WrapAlignment.center,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          const Text('New to AfyaMkononi? ',
                              style: TextStyle(
                                  fontSize: 12.5, color: AuthColors.body)),
                          GestureDetector(
                              onTap: busy
                                  ? null
                                  : () => setState(() => register = true),
                              child: const Text('Create an account',
                                  style: TextStyle(
                                      fontSize: 12.5,
                                      color: AuthColors.link,
                                      fontWeight: FontWeight.w800,
                                      decoration: TextDecoration.underline,
                                      decorationColor: AuthColors.link))),
                        ]),
                  ),
                  const SizedBox(width: 8),
                  const SizedBox(
                      width: 16, child: Divider(color: Color(0x66406058))),
                ]),
              ]),
        ),
      );

  /// "Create your AfyaMkononi account": cream card over the countryside scene.
  ///
  /// The supplied design shows four fields (name, phone, optional email,
  /// password). Registration here also needs a required email, date of birth,
  /// address (product-owner instruction 2026-11-14) and gender; those fields
  /// are kept in the design's style. Every field is required and marked *.
  Widget _registerScene(BuildContext context, bool busy, String? error) {
    PatientField f(
            String label, TextEditingController c, String hint, IconData icon,
            {TextInputType? type,
            bool obscure = false,
            bool readOnly = false,
            bool required = true,
            VoidCallback? onTap,
            Widget? suffix}) =>
        PatientField(
            label: label,
            requiredMark: required,
            controller: c,
            hint: hint,
            keyboardType: type,
            obscureText: obscure,
            readOnly: readOnly,
            onTap: onTap,
            suffixIcon: suffix,
            labelIcon: icon,
            labelColor: AuthColors.title,
            fillColor: Colors.white.withValues(alpha: .55),
            borderColor: AuthColors.fieldBorder,
            borderRadius: 14);
    return AuthScene(
      name: 'register',
      headerRows: 474,
      topColor: const Color(0xFFFEF6E6),
      sideMargin: 24,
      child: Container(
        padding: const EdgeInsets.fromLTRB(20, 20, 20, 18),
        decoration: BoxDecoration(
            color: AuthColors.cream, borderRadius: BorderRadius.circular(32)),
        child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text('Create your AfyaMkononi account',
                  style: TextStyle(
                      fontSize: 22,
                      height: 1.2,
                      fontWeight: FontWeight.w800,
                      letterSpacing: -.3,
                      color: AuthColors.title)),
              const SizedBox(height: 6),
              const Text(
                  'Join AfyaMkononi to access healthcare services, testing, '
                  'treatment, and support — all in one place.',
                  style: TextStyle(
                      fontSize: 13, height: 1.4, color: AuthColors.body)),
              const SizedBox(height: 16),
              f('Full name', fullName, 'Enter your full name',
                  Icons.person_outline_rounded),
              const SizedBox(height: 12),
              f('Phone number', phone, '07XX XXX XXX', Icons.phone_outlined,
                  type: TextInputType.phone),
              const SizedBox(height: 12),
              f('Email address', email, 'you@example.com',
                  Icons.mail_outline_rounded,
                  type: TextInputType.emailAddress),
              const SizedBox(height: 12),
              f('Date of birth', dateOfBirth, 'YYYY-MM-DD', Icons.cake_outlined,
                  readOnly: true,
                  onTap: pickDate,
                  suffix: const Icon(Icons.calendar_today_outlined, size: 18)),
              const SizedBox(height: 12),
              f('Address', address, 'Estate / street, town',
                  Icons.home_outlined,
                  type: TextInputType.streetAddress),
              const SizedBox(height: 12),
              const Row(children: [
                Icon(Icons.wc_rounded, size: 19, color: AuthColors.title),
                SizedBox(width: 8),
                Text.rich(
                    TextSpan(text: 'Gender', children: [
                      TextSpan(
                          text: ' *',
                          style: TextStyle(
                              color: PatientTokens.danger,
                              fontWeight: FontWeight.w800)),
                    ]),
                    style: TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                        color: AuthColors.title)),
              ]),
              const SizedBox(height: 7),
              DropdownButtonFormField<String>(
                  initialValue: gender,
                  isExpanded: true,
                  decoration: InputDecoration(
                      fillColor: Colors.white.withValues(alpha: .55),
                      enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(14),
                          borderSide:
                              const BorderSide(color: AuthColors.fieldBorder))),
                  items: const [
                    DropdownMenuItem(value: 'FEMALE', child: Text('Female')),
                    DropdownMenuItem(value: 'MALE', child: Text('Male')),
                    DropdownMenuItem(value: 'OTHER', child: Text('Other'))
                  ],
                  onChanged: busy
                      ? null
                      : (v) {
                          if (v != null) setState(() => gender = v);
                        }),
              const SizedBox(height: 12),
              f('Create password', password, 'Enter a strong password',
                  Icons.lock_outline_rounded,
                  obscure: _obscure, suffix: _eye()),
              const SizedBox(height: 6),
              const Text(
                  'Use at least 8 characters, with upper and lower case '
                  'letters, a number and a symbol.',
                  style: TextStyle(
                      fontSize: 11.5, height: 1.35, color: AuthColors.body)),
              _error(error),
              const SizedBox(height: 16),
              AuthPrimaryButton(
                  label: 'Create account',
                  icon: Icons.person_add_alt_1_outlined,
                  busy: busy,
                  onPressed: busy ? null : submit),
              const SizedBox(height: 14),
              Center(
                  child: Wrap(alignment: WrapAlignment.center, children: [
                const Text('Already have an account? ',
                    style: TextStyle(fontSize: 13, color: AuthColors.body)),
                GestureDetector(
                    onTap: busy ? null : () => setState(() => register = false),
                    child: const Text('Log in',
                        style: TextStyle(
                            fontSize: 13,
                            color: AuthColors.link,
                            fontWeight: FontWeight.w800)))
              ])),
            ]),
      ),
    );
  }
}
