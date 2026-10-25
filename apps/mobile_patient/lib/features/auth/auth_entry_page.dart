import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import 'auth_cubit.dart';
import 'email_otp_verify_page.dart';

class AuthEntryPage extends StatefulWidget {
  const AuthEntryPage({super.key, required this.auth});
  final AuthCubit auth;
  @override
  State<AuthEntryPage> createState() => _AuthEntryPageState();
}

class _AuthEntryPageState extends State<AuthEntryPage> {
  bool register = false, navigated = false;
  final email = TextEditingController();
  final phone = TextEditingController();
  final password = TextEditingController();
  final fullName = TextEditingController();
  final dateOfBirth = TextEditingController();
  String gender = 'OTHER';
  @override
  void dispose() {
    email.dispose();
    phone.dispose();
    password.dispose();
    fullName.dispose();
    dateOfBirth.dispose();
    super.dispose();
  }

  void submit() {
    if (register) {
      widget.auth.requestPatientRegistration(
          email.text, phone.text, password.text,
          fullName: fullName.text,
          dateOfBirth: dateOfBirth.text,
          gender: gender);
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
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
            child: BlocConsumer<AuthCubit, AuthState>(
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
                  return Center(
                      child: ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 520),
                          child: SingleChildScrollView(
                              padding:
                                  const EdgeInsets.fromLTRB(24, 34, 24, 28),
                              child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Wrap(
                                        spacing: 11,
                                        runSpacing: 8,
                                        crossAxisAlignment:
                                            WrapCrossAlignment.center,
                                        children: [
                                          Container(
                                              width: 38,
                                              height: 38,
                                              decoration: BoxDecoration(
                                                  color: PatientTokens.primary,
                                                  borderRadius:
                                                      BorderRadius.circular(
                                                          10)),
                                              child: const Icon(
                                                  Icons.add_rounded,
                                                  color: Colors.white,
                                                  size: 25)),
                                          const Text('AfyaMkononi',
                                              style: TextStyle(
                                                  fontSize: 22,
                                                  fontWeight: FontWeight.w700,
                                                  color: PatientTokens
                                                      .primaryDark))
                                        ]),
                                    const SizedBox(height: 42),
                                    Text(
                                        register
                                            ? 'Create your account'
                                            : 'Welcome back',
                                        style: Theme.of(context)
                                            .textTheme
                                            .headlineSmall),
                                    const SizedBox(height: 8),
                                    Text(
                                        register
                                            ? 'Set up your secure patient account.'
                                            : 'Access your healthcare account.',
                                        style: Theme.of(context)
                                            .textTheme
                                            .bodyMedium),
                                    const SizedBox(height: 27),
                                    PatientField(
                                        label: 'Email address',
                                        controller: email,
                                        hint: 'you@example.com',
                                        keyboardType:
                                            TextInputType.emailAddress),
                                    if (register) ...[
                                      const SizedBox(height: 17),
                                      PatientField(
                                          label: 'Full name',
                                          controller: fullName,
                                          hint: 'Your legal name'),
                                      const SizedBox(height: 17),
                                      PatientField(
                                          label: 'Date of birth',
                                          controller: dateOfBirth,
                                          hint: 'YYYY-MM-DD',
                                          readOnly: true,
                                          onTap: pickDate,
                                          suffixIcon: const Icon(
                                              Icons.calendar_today_outlined,
                                              size: 19)),
                                      const SizedBox(height: 17),
                                      const Text('Gender',
                                          style: TextStyle(
                                              fontSize: 13,
                                              fontWeight: FontWeight.w600,
                                              color: PatientTokens.text)),
                                      const SizedBox(height: 7),
                                      DropdownButtonFormField<String>(
                                          initialValue: gender,
                                          items: const [
                                            DropdownMenuItem(
                                                value: 'FEMALE',
                                                child: Text('Female')),
                                            DropdownMenuItem(
                                                value: 'MALE',
                                                child: Text('Male')),
                                            DropdownMenuItem(
                                                value: 'OTHER',
                                                child: Text('Other'))
                                          ],
                                          onChanged: busy
                                              ? null
                                              : (v) {
                                                  if (v != null) {
                                                    setState(() => gender = v);
                                                  }
                                                }),
                                      const SizedBox(height: 17),
                                      PatientField(
                                          label: 'Phone number',
                                          controller: phone,
                                          hint: '+254 7xx xxx xxx',
                                          keyboardType: TextInputType.phone),
                                    ],
                                    const SizedBox(height: 17),
                                    PatientField(
                                        label: 'Password',
                                        controller: password,
                                        hint: 'Enter your password',
                                        obscureText: true),
                                    if (!register)
                                      const Align(
                                          alignment: Alignment.centerRight,
                                          child: TextButton(
                                              onPressed: null,
                                              child: Text('Forgot password?'))),
                                    if (state is AuthFailure) ...[
                                      const SizedBox(height: 12),
                                      Text(state.message,
                                          style: const TextStyle(
                                              color: PatientTokens.danger))
                                    ],
                                    const SizedBox(height: 23),
                                    SizedBox(
                                        width: double.infinity,
                                        child: FilledButton(
                                            onPressed: busy ? null : submit,
                                            child: busy
                                                ? const SizedBox(
                                                    width: 19,
                                                    height: 19,
                                                    child:
                                                        CircularProgressIndicator(
                                                            strokeWidth: 2,
                                                            color:
                                                                Colors.white))
                                                : Text(register
                                                    ? 'Continue'
                                                    : 'Sign in'))),
                                    const SizedBox(height: 22),
                                    Center(
                                        child: Wrap(
                                            alignment: WrapAlignment.center,
                                            children: [
                                          Text(
                                              register
                                                  ? 'Already have an account? '
                                                  : 'New to AfyaMkononi? ',
                                              style: const TextStyle(
                                                  color:
                                                      PatientTokens.textSub)),
                                          GestureDetector(
                                              onTap: busy
                                                  ? null
                                                  : () => setState(() =>
                                                      register = !register),
                                              child: Text(
                                                  register
                                                      ? 'Sign in'
                                                      : 'Create an account',
                                                  style: const TextStyle(
                                                      color:
                                                          PatientTokens.primary,
                                                      fontWeight:
                                                          FontWeight.w700)))
                                        ])),
                                  ]))));
                })),
      );
}
