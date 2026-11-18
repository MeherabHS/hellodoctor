import 'features/peer/peer_pages.dart';
import 'features/peer/peer_repository.dart';
import 'core/notifications/notification_taps.dart';
import 'features/notifications/notification_navigation.dart';
import 'core/l10n/doctor_locale.dart';
import 'l10n/app_localizations.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'core/network/api_client.dart';
import 'core/network/screen_refresh.dart';
import 'core/network/client_error_reporter.dart';
import 'core/notifications/push_notification_service.dart';
import 'core/network/connectivity_service.dart';
import 'core/cache/doctor_cache_store.dart';
import 'core/notifications/background_sync_worker.dart';
import 'core/notifications/battery_optimization.dart';
import 'core/notifications/local_notification_service.dart';
import 'core/theme/tokens.dart';
import 'core/validation/kenya_phone.dart';
import 'core/widgets/afya_auth_widgets.dart';
import 'core/widgets/startup_error_page.dart';
import 'features/auth/auth_cubit.dart';
import 'features/auth/forgot_password_page.dart';
import 'core/legal/legal_links.dart';
import 'features/auth/legal_consent_page.dart';
import 'features/auth/welcome_pages.dart';
import 'features/doctor/doctor_cubit.dart';
import 'features/doctor/doctor_pages.dart';
import 'features/doctor/doctor_repository.dart';
import 'features/notifications/notification_cubit.dart';
import 'features/notifications/notification_repository.dart';
import 'features/home/consultation_history_tab.dart';
import 'features/home/home_pages.dart';

class HelloDoctorApp extends StatefulWidget {
  const HelloDoctorApp({super.key});
  @override
  State<HelloDoctorApp> createState() => _AppState();
}

class _AppState extends State<HelloDoctorApp> with WidgetsBindingObserver {
  SecureTokenStore? secureStore;
  DoctorCacheStore? cache;
  ConnectivityService? connectivity;
  ApiClient? api;
  AuthCubit? auth;
  // Set only if wiring up the services above throws (e.g. a misconfigured
  // backend address). Building a bare widget tree here — rather than letting
  // the exception propagate out of initState — is what keeps a startup
  // failure from rendering as an unexplained blank screen on a real device.
  Object? _startupError;
  DoctorLocaleController? locale;
  Timer? _incidentRetryTimer;
  StreamSubscription<AuthState>? _authSubscription;
  StreamSubscription<NetworkStatus>? _networkSubscription;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _initializeServices();
  }

  /// Revalidates the session whenever the app returns to the foreground,
  /// instead of silently trusting whatever was last loaded while
  /// backgrounded — the device could have been offline for hours, or the
  /// refresh token could have been revoked (logout elsewhere, MFA reset,
  /// admin action) while this app sat suspended. Deliberately lightweight:
  /// reuses the same `refreshSession()` the API client already calls on an
  /// ordinary 401, which on genuine failure invokes the already-wired
  /// `onSessionExpired` callback (see `_initializeServices` -> `AuthCubit`)
  /// to sign the doctor out — no new state-transition path is introduced.
  /// Errors are swallowed here because this is a background correctness
  /// check, not a user-initiated action; the resulting `SignedOut` state (if
  /// any) is what actually surfaces to the UI.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed && auth?.state is SignedIn) {
      api
          ?.refreshSession()
          .then((_) => ClientErrorReporter.current?.flushStartup())
          .catchError((_) {});
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    ClientErrorReporter.current = null;
    _incidentRetryTimer?.cancel();
    _authSubscription?.cancel();
    _networkSubscription?.cancel();
    auth?.close();
    api?.dispose();
    connectivity?.dispose();
    locale?.dispose();
    super.dispose();
  }

  void _initializeServices() {
    ClientErrorReporter.current = null;
    // Dispose anything a previous failed attempt partially constructed
    // before retrying, so a tapped "Try again" can't leak sockets/listeners.
    _incidentRetryTimer?.cancel();
    _authSubscription?.cancel();
    _networkSubscription?.cancel();
    auth?.close();
    api?.dispose();
    connectivity?.dispose();
    try {
      secureStore = SecureTokenStore();
      cache = DoctorCacheStore(secureStore!);
      connectivity = ConnectivityService();
      api = ApiClient(tokenStore: secureStore);
      ClientErrorReporter.current = ClientErrorReporter(api!,
          authenticated: () => auth?.state is SignedIn);
      ClientErrorReporter.current!.loadBuildMetadata();
      locale ??= DoctorLocaleController(secureStore!)..load();
      auth = AuthCubit(api!, cache: cache);
      _authSubscription = auth!.stream.listen((state) {
        _incidentRetryTimer?.cancel();
        if (state is SignedIn) {
          unawaited(ClientErrorReporter.current?.flushStartup());
          _incidentRetryTimer =
              Timer.periodic(const Duration(seconds: 30), (_) {
            if (WidgetsBinding.instance.lifecycleState ==
                AppLifecycleState.resumed) {
              unawaited(ClientErrorReporter.current?.flushStartup());
            }
          });
        }
      });
      _networkSubscription = connectivity!.changes.listen((status) {
        if (status == NetworkStatus.online) {
          unawaited(ClientErrorReporter.current?.flushStartup());
        }
      });
      unawaited(auth!.restore());
      _startupError = null;
    } catch (error, stack) {
      unawaited(ClientErrorReporter.captureStartup(error, stack));
      _startupError = error;
    }
  }

  @override
  Widget build(BuildContext c) {
    final startupError = _startupError;
    if (startupError != null) {
      return StartupErrorPage(
          error: startupError, onRetry: () => setState(_initializeServices));
    }
    return DoctorLocaleScope(
        controller: locale!,
        child: ListenableBuilder(
            listenable: locale!,
            builder: (context, _) => BlocProvider.value(
                value: auth!,
                child: BlocBuilder<AuthCubit, AuthState>(builder: (_, state) {
                  Widget application(Widget home) => MaterialApp(
                      key: ValueKey(state is SignedIn),
                      locale: locale!.locale,
                      localizationsDelegates:
                          AppLocalizations.localizationsDelegates,
                      supportedLocales: AppLocalizations.supportedLocales,
                      navigatorObservers: [screenRouteObserver],
                      debugShowCheckedModeBanner: false,
                      theme: DoctorTokens.theme(),
                      home: home);
                  if (state is SignedIn && state.isPeer) {
                    // Peer Navigators get their own screens; no doctor data.
                    return application(PeerApp(repo: PeerRepository(api!)));
                  }
                  if (state is SignedIn) {
                    // Scope both the Navigator and all routed subflows to this account.
                    // Removing these providers on logout also disposes their route
                    // stack and, for NotificationCubit, any in-flight requests.
                    return MultiBlocProvider(
                        providers: [
                          BlocProvider<DoctorCubit>(
                              create: (_) => DoctorCubit(DoctorRepository(api!,
                                  cache: cache, connectivity: connectivity!))),
                          BlocProvider<NotificationCubit>(
                              create: (_) => NotificationCubit(
                                  NotificationRepository(api!))
                                ..startPolling(
                                    interval: const Duration(minutes: 1))),
                        ],
                        child: application(Builder(
                            builder: (context) => NotificationNavigationHost(
                                cubit: context.read<NotificationCubit>(),
                                child: const DoctorShell()))));
                  }
                  return application(state is SessionRestoring
                      ? const _SplashPage()
                      : WelcomePage(auth: auth!));
                }))));
  }
}

class _SplashPage extends StatelessWidget {
  const _SplashPage();
  @override
  Widget build(BuildContext context) => const Scaffold(
      backgroundColor: DoctorTokens.screen,
      body: Center(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
        AfyaBrandHeader(),
        SizedBox(height: 22),
        SizedBox(
            width: 22,
            height: 22,
            child: CircularProgressIndicator(
                strokeWidth: 2, color: DoctorTokens.primary)),
        SizedBox(height: 12),
        Text('Restoring your secure session…',
            style: TextStyle(fontSize: 13, color: DoctorTokens.slate)),
      ])));
}

/// Panel 1 of the Doctor reference board. Layout only: every action still
/// goes through the same [AuthCubit] calls as before.
class LoginPage extends StatefulWidget {
  const LoginPage({super.key, required this.auth});
  final AuthCubit auth;
  @override
  State<LoginPage> createState() => _LoginState();
}

class _LoginState extends State<LoginPage> {
  final email = TextEditingController(),
      password = TextEditingController(),
      otp = TextEditingController();
  bool _hidePassword = true;
  @override
  void dispose() {
    email.dispose();
    password.dispose();
    otp.dispose();
    super.dispose();
  }

  void _signIn() => widget.auth.login(email.text.trim(), password.text.trim());

  @override
  Widget build(BuildContext c) {
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            child: SingleChildScrollView(
                child: Center(
                    child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 460),
                        child:
                            BlocBuilder<AuthCubit, AuthState>(builder: (_, s) {
                          final m = s is MfaRequired;
                          final busy = s is AuthBusy;
                          return Column(children: [
                            // Built to
                            // docs/design/afyamkononi-doctor-signin-reference.png;
                            // the scene is cut from that design.
                            const SizedBox(height: 16),
                            const Padding(
                                padding: EdgeInsets.symmetric(horizontal: 72),
                                child: AfyaBrandLockup()),
                            const SizedBox(height: 6),
                            AspectRatio(
                                aspectRatio: 941 / 562,
                                child: Image.asset(
                                    'assets/auth/signin_scene.webp',
                                    fit: BoxFit.cover,
                                    excludeFromSemantics: true)),
                            const SizedBox(height: 14),
                            Text(m ? 'Check your email' : 'Welcome, Team',
                                style: const TextStyle(
                                    fontSize: 27,
                                    fontWeight: FontWeight.w800,
                                    letterSpacing: -0.3,
                                    color: DoctorTokens.navy)),
                            const SizedBox(height: 6),
                            Padding(
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 28),
                                child: Text(
                                    m
                                        ? 'Enter the verification code for this secure session.'
                                        : 'Sign in to your professional account',
                                    textAlign: TextAlign.center,
                                    style: const TextStyle(
                                        fontSize: 16,
                                        color: DoctorTokens.slate))),
                            const SizedBox(height: 22),
                            Padding(
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 28),
                                child: Column(children: [
                                  if (!m) ...[
                                    AfyaAuthField(
                                        controller: email,
                                        hint: 'Email address',
                                        icon: Icons.mail_outline,
                                        keyboardType:
                                            TextInputType.emailAddress,
                                        autofillHints: const [
                                          AutofillHints.email
                                        ],
                                        textInputAction: TextInputAction.next),
                                    const SizedBox(height: 14),
                                    AfyaAuthField(
                                        controller: password,
                                        hint: 'Password',
                                        icon: Icons.lock_outline,
                                        obscure: _hidePassword,
                                        onToggleObscure: () => setState(() =>
                                            _hidePassword = !_hidePassword),
                                        onSubmitted: (_) {
                                          if (!busy) _signIn();
                                        }),
                                    const SizedBox(height: 24),
                                    AfyaPrimaryButton(
                                        label: 'Sign In',
                                        busy: busy,
                                        onPressed: _signIn),
                                  ] else ...[
                                    AfyaAuthField(
                                        controller: otp,
                                        hint: 'One-time verification code',
                                        icon: Icons.pin_outlined,
                                        keyboardType: TextInputType.number),
                                    const SizedBox(height: 34),
                                    AfyaPrimaryButton(
                                        label: 'Verify and sign in',
                                        busy: busy,
                                        onPressed: () => widget.auth
                                            .verify(otp.text.trim())),
                                  ],
                                  if (s is SignedOut && s.notice != null)
                                    AfyaFormError(s.notice!),
                                  if (s is AuthFailure)
                                    AfyaFormError(s.message),
                                ])),
                            const SizedBox(height: 20),
                            const Padding(
                                padding: EdgeInsets.symmetric(horizontal: 38),
                                child: AfyaSecurityNote(
                                    'Two-factor authentication (2FA) keeps your account secure.')),
                            const SizedBox(height: 10),
                            if (m)
                              TextButton(
                                  onPressed:
                                      busy ? null : widget.auth.restartLogin,
                                  child: const Text('Back to sign in'))
                            else
                              // Registration is reached from the Welcome
                              // screen, so only this link sits here.
                              TextButton(
                                  onPressed: busy
                                      ? null
                                      : () => Navigator.push(
                                          c,
                                          MaterialPageRoute(
                                              builder: (_) =>
                                                  ForgotPasswordPage(
                                                      auth: widget.auth))),
                                  child: const Text('Forgot password?',
                                      style: TextStyle(fontSize: 16))),
                            const SizedBox(height: 16),
                          ]);
                        }))))));
  }
}

/// Panel 12 of the Doctor reference board: "Sign Up" then "Professional
/// Details", then the legal agreements and "Account Under Review". The
/// request sent to the server is unchanged — the same ten required values,
/// collected over two screens instead of one.
class RegistrationPage extends StatefulWidget {
  const RegistrationPage({super.key, required this.auth});
  final AuthCubit auth;
  @override
  State<RegistrationPage> createState() => _RegistrationState();
}

class _RegistrationState extends State<RegistrationPage> {
  String? _clientError;
  int _step = 0;
  bool _hidePassword = true;

  final controllers = <String, TextEditingController>{
    'name': TextEditingController(),
    'license': TextEditingController(),
    'phone': TextEditingController(),
    'email': TextEditingController(),
    'password': TextEditingController(),
    'specialty': TextEditingController(),
    'experience': TextEditingController(),
    'hospital': TextEditingController(),
    'qualifications': TextEditingController(),
    'address': TextEditingController(),
  };

  @override
  void dispose() {
    for (final controller in controllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  /// Mirrors the backend's `validate_password_strength` rule (see
  /// `services/backend/src/domain/validation.rs`): at least 8 characters,
  /// containing an uppercase letter, a lowercase letter, a digit, and a
  /// special character. Checked client-side purely to save a round trip on
  /// an obviously-weak password — the backend remains the authoritative
  /// enforcement point regardless of what this client does.
  String? _passwordStrengthError(String password) {
    if (password.length < 8) {
      return 'Password must be at least 8 characters.';
    }
    final missing = <String>[
      if (!password.contains(RegExp(r'[A-Z]'))) 'an uppercase letter',
      if (!password.contains(RegExp(r'[a-z]'))) 'a lowercase letter',
      if (!password.contains(RegExp(r'[0-9]'))) 'a number',
      if (!password
          .contains(RegExp(r'''[!@#$%^&*()_+\-=\[\]{};:,.<>/?\\|~`"' ]''')))
        'a special character (e.g. @ ! # \$ %)',
    ];
    if (missing.isNotEmpty) {
      return 'Password must include ${missing.join(', ')}.';
    }
    return null;
  }

  /// Mirrors the backend's `normalize_kenya_phone_number` rule (see
  /// `services/backend/src/domain/validation.rs` and
  /// `lib/core/validation/kenya_phone.dart`). Checked client-side purely to
  /// save a round trip on an obviously-invalid phone number — the backend
  /// remains the authoritative enforcement point regardless of what this
  /// client does.
  String? _phoneError(String raw) => normalizeKenyaPhone(raw) == null
      ? 'Enter a Kenyan phone number with 9 digits after +254, e.g. 0712 345 678 or +254 712 345 678.'
      : null;

  void _continueFromAccount() {
    final error = _requiredError(const {
          'name': 'Full name',
          'email': 'Email address',
          'password': 'Password',
          'phone': 'Phone number',
        }) ??
        _emailError(controllers['email']!.text) ??
        _passwordStrengthError(controllers['password']!.text) ??
        _phoneError(controllers['phone']!.text);
    setState(() {
      _clientError = error;
      if (error == null) _step = 1;
    });
  }

  /// First empty required field, in form order, as a readable message.
  String? _requiredError(Map<String, String> fields) {
    for (final entry in fields.entries) {
      if (controllers[entry.key]!.text.trim().isEmpty) {
        return '${entry.value} is required.';
      }
    }
    return null;
  }

  String? _emailError(String raw) =>
      RegExp(r'^[^@\s]+@[^@\s]+\.[^@\s]+$').hasMatch(raw.trim())
          ? null
          : 'Enter a valid email address.';

  /// Mirrors the backend's `doctor_register` checks so an incomplete form is
  /// caught here, before the agreements step, instead of after it.
  String? _professionalError() {
    final missing = _requiredError(const {
      'license': 'KMPDC registration number',
      'experience': 'Years of experience',
      'hospital': 'Hospital name',
      'specialty': 'Specialty',
      'qualifications': 'Qualifications',
      'address': 'Residential address',
    });
    if (missing != null) return missing;
    if (controllers['license']!.text.trim().length < 3) {
      return 'KMPDC registration number must be at least 3 characters.';
    }
    final years = int.tryParse(controllers['experience']!.text.trim());
    if (years == null || years < 0 || years > 100) {
      return 'Years of experience must be a whole number from 0 to 100.';
    }
    final qualifications = controllers['qualifications']!
        .text
        .split(',')
        .map((q) => q.trim())
        .where((q) => q.isNotEmpty)
        .toList();
    if (qualifications.isEmpty ||
        qualifications.length > 10 ||
        qualifications.any((q) => q.length > 150)) {
      return 'List 1 to 10 qualifications, separated by commas.';
    }
    return null;
  }

  void _continueToAgreements() {
    final normalizedPhone = normalizeKenyaPhone(controllers['phone']!.text);
    if (normalizedPhone == null) {
      setState(() => _step = 0);
      return;
    }
    final error = _professionalError();
    if (error != null) {
      setState(() => _clientError = error);
      return;
    }
    setState(() => _clientError = null);
    Navigator.push(
        context,
        MaterialPageRoute(
            builder: (_) => LegalConsentPage(
                auth: widget.auth,
                draft: DoctorRegistrationDraft(
                    fullName: controllers['name']!.text,
                    licenseNumber: controllers['license']!.text,
                    phoneNumber: normalizedPhone,
                    email: controllers['email']!.text,
                    password: controllers['password']!.text,
                    specialty: controllers['specialty']!.text,
                    experienceYears: controllers['experience']!.text,
                    hospital: controllers['hospital']!.text,
                    qualifications: controllers['qualifications']!.text,
                    address: controllers['address']!.text))));
  }

  // Length caps match the shared client/server contract: 150 for short
  // label-like fields, 255 for email, 20 for a phone number, 300 for the
  // qualifications/address fields and a 3-digit cap on experience years.
  // Password is deliberately left uncapped.
  List<Widget> _accountStep(bool busy) => [
        const Center(child: AfyaLogoMark(size: 62)),
        const SizedBox(height: 8),
        const Text('Create Doctor Account',
            textAlign: TextAlign.center,
            style: TextStyle(
                fontSize: 23,
                fontWeight: FontWeight.w800,
                color: DoctorTokens.navy)),
        const SizedBox(height: 4),
        const Text('Join AfyaMkononi as a verified doctor.',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 15, color: DoctorTokens.slate)),
        const SizedBox(height: 20),
        AfyaAuthField(
            controller: controllers['name']!,
            hint: 'Full Name',
            icon: Icons.person_outline,
            requiredMark: true,
            maxLength: 150,
            textInputAction: TextInputAction.next),
        const SizedBox(height: 12),
        AfyaAuthField(
            controller: controllers['email']!,
            hint: 'Email Address',
            icon: Icons.mail_outline,
            requiredMark: true,
            keyboardType: TextInputType.emailAddress,
            maxLength: 255,
            textInputAction: TextInputAction.next),
        const SizedBox(height: 12),
        AfyaAuthField(
            controller: controllers['password']!,
            hint: 'Password',
            icon: Icons.lock_outline,
            requiredMark: true,
            obscure: _hidePassword,
            onToggleObscure: () =>
                setState(() => _hidePassword = !_hidePassword),
            textInputAction: TextInputAction.next),
        const SizedBox(height: 12),
        AfyaAuthField(
            controller: controllers['phone']!,
            hint: 'Phone Number',
            icon: Icons.phone_outlined,
            requiredMark: true,
            keyboardType: TextInputType.phone,
            maxLength: 20),
        const SizedBox(height: 6),
        const Text('Password: 8+ characters with upper, lower, number, symbol.',
            style: TextStyle(fontSize: 12, color: DoctorTokens.slate)),
        const SizedBox(height: 18),
        AfyaPrimaryButton(
            label: 'Create Account',
            busy: busy,
            onPressed: _continueFromAccount),
        const SizedBox(height: 10),
        const RegistrationLegalNotice(
            text: 'By registering you agree to the AfyaMkononi Doctor Terms '
                'and Privacy Policy.',
            pages: doctorRegistrationPages,
            textColor: DoctorTokens.slate,
            linkColor: DoctorTokens.primary),
        const SizedBox(height: 10),
        Row(mainAxisAlignment: MainAxisAlignment.center, children: [
          const Text('Already have an account?',
              style: TextStyle(fontSize: 14, color: DoctorTokens.slate)),
          TextButton(
              onPressed: busy ? null : () => Navigator.pop(context),
              child: const Text('Sign In'))
        ]),
      ];

  /// Order and wording per product-owner instruction (2026-11-14): KMPDC
  /// registration number, years of experience, hospital name, specialty,
  /// then qualifications and residential address. All are required.
  List<Widget> _professionalStep(bool busy) => [
        AfyaLabeledField(
            label: 'KMPDC registration number',
            hint: 'e.g. A12345',
            controller: controllers['license']!,
            maxLength: 150),
        AfyaLabeledField(
            label: 'Years of experience',
            hint: '0 to 100',
            controller: controllers['experience']!,
            keyboardType: TextInputType.number,
            maxLength: 3),
        AfyaLabeledField(
            label: 'Hospital name',
            hint: 'Where you currently practise',
            controller: controllers['hospital']!,
            maxLength: 200),
        AfyaLabeledField(
            label: 'Specialty',
            hint: 'e.g. General Practitioner',
            controller: controllers['specialty']!,
            maxLength: 80),
        AfyaLabeledField(
            label: 'Qualifications',
            hint: 'Comma separated, e.g. MBChB, MMed',
            controller: controllers['qualifications']!,
            maxLength: 300),
        AfyaLabeledField(
            label: 'Residential address',
            controller: controllers['address']!,
            maxLength: 300),
        const SizedBox(height: 6),
        AfyaPrimaryButton(
            label: 'Next', busy: busy, onPressed: _continueToAgreements),
      ];

  @override
  Widget build(BuildContext context) => PopScope(
      canPop: _step == 0,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) setState(() => _step = 0);
      },
      child: Scaffold(
          backgroundColor: DoctorTokens.screen,
          appBar: AppBar(
              title: Text(_step == 0 ? 'Sign Up' : 'Professional Details')),
          body: BlocBuilder<AuthCubit, AuthState>(
              bloc: widget.auth,
              builder: (_, state) {
                if (state is RegistrationSubmitted) {
                  return RegistrationSubmittedView(
                      onDone: () => Navigator.pop(context));
                }
                final busy = state is AuthBusy;
                return ListView(
                    padding: const EdgeInsets.fromLTRB(28, 8, 28, 24),
                    children: [
                      ...(_step == 0
                          ? _accountStep(busy)
                          : _professionalStep(busy)),
                      if (_clientError != null)
                        AfyaFormError(_clientError!)
                      else if (state is AuthFailure)
                        AfyaFormError(state.message)
                    ]);
              })));
}

/// Panel 12, third phone: shown once the server has accepted a registration.
class RegistrationSubmittedView extends StatelessWidget {
  const RegistrationSubmittedView({super.key, required this.onDone});
  final VoidCallback onDone;
  @override
  Widget build(BuildContext context) => AfyaReviewStatusView(
      title: 'Account Under Review',
      message: 'Your details have been submitted for verification. '
          'We will notify you once your account is approved.',
      steps: const [
        AfyaReviewStep('Profile submitted', done: true),
        AfyaReviewStep('Documents under review', done: true),
        AfyaReviewStep('Approval & activation', done: false),
      ],
      primaryLabel: 'OK',
      onPrimary: onDone);
}

/// Signed-in gate for a doctor whose registration is not verified. Wording
/// follows the server's status; nothing here grants or assumes access.
class VerificationGateView extends StatelessWidget {
  const VerificationGateView(
      {super.key,
      required this.name,
      required this.license,
      required this.status,
      required this.onRefresh,
      required this.onLogout,
      this.reviewNote});
  final String name, license, status;
  final String? reviewNote;
  final VoidCallback onRefresh, onLogout;
  @override
  Widget build(BuildContext context) {
    final details = [name, 'KMPDC Registration: $license'];
    if (status == 'PENDING') {
      return AfyaReviewStatusView(
          title: 'Account Under Review',
          message: 'Your details have been submitted for verification. '
              'We will notify you once your account is approved.',
          details: details,
          steps: const [
            AfyaReviewStep('Profile submitted', done: true),
            AfyaReviewStep('Documents under review', done: true),
            AfyaReviewStep('Approval & activation', done: false),
          ],
          note: reviewNote,
          primaryLabel: 'Refresh status',
          onPrimary: onRefresh,
          secondaryLabel: 'Log out',
          onSecondary: onLogout);
    }
    final (IconData icon, Color color, String title, String message) =
        switch (status) {
      'REJECTED' => (
          Icons.gpp_bad_outlined,
          DoctorTokens.danger,
          'Registration Not Approved',
          'Your registration was reviewed and was not approved. '
              'Clinical access is not available.'
        ),
      'REVOKED' => (
          Icons.gpp_bad_outlined,
          DoctorTokens.danger,
          'Verification Withdrawn',
          'Your verification has been withdrawn. '
              'Clinical access is not available.'
        ),
      _ => (
          Icons.gpp_maybe_outlined,
          DoctorTokens.warning,
          'Verification Unavailable',
          'Your verification status could not be confirmed. '
              'Clinical access requires approval.'
        ),
    };
    return AfyaReviewStatusView(
        icon: icon,
        iconColor: color,
        title: title,
        message: message,
        details: details,
        steps: const [],
        note: reviewNote,
        primaryLabel: 'Refresh status',
        onPrimary: onRefresh,
        secondaryLabel: 'Log out',
        onSecondary: onLogout);
  }
}

class DoctorShell extends StatefulWidget {
  const DoctorShell({super.key});

  /// Lets a pushed page (e.g. Settings' Availability/Payout tiles) switch
  /// the shell back to one of its bottom-nav tabs instead of duplicating
  /// that tab's screen. Looks up the nearest `DoctorShell` ancestor's own
  /// state directly; a no-op if called outside the shell (shouldn't happen
  /// in practice, since every Doctor screen lives under it).
  /// Shows the schedule inside the Appointments tab, keeping the bottom
  /// navigation visible as the schedule design draws it. Callable from a
  /// pushed page as well as from a tab.
  static void openSchedule(BuildContext context) {
    final shell = _ShellState.current;
    if (shell == null) return;
    Navigator.of(shell.context).popUntil((route) => route.isFirst);
    shell.openSchedule();
  }

  static void switchTab(BuildContext context, int index) {
    context.findAncestorStateOfType<_ShellState>()?.selectTab(index);
  }

  @override
  State<DoctorShell> createState() => _ShellState();
}

class _ShellState extends State<DoctorShell> with WidgetsBindingObserver {
  static _ShellState? current;
  int tab = 0;
  bool _schedule = false;

  void openSchedule() => setState(() {
        tab = 1;
        _schedule = true;
        ClientErrorReporter.current?.screen = 'Schedule';
      });
  late final ScreenRefresh _queueRefresh;
  StreamSubscription<NotificationState>? _queueNoticeSubscription;
  Set<String> _knownNotificationIds = {};
  PushNotificationService? _pushService;

  void _refreshDoctorData() {
    if (mounted && tab <= 1) unawaited(_queueRefresh.run());
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) _refreshDoctorData();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _queueRefresh.attach(context);
  }

  @override
  void dispose() {
    if (current == this) current = null;
    _queueRefresh.dispose();
    _queueNoticeSubscription?.cancel();
    unawaited(_pushService?.dispose());
    NotificationTaps.clear();
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  void selectTab(int index) {
    setState(() {
      tab = index;
      _schedule = false;
      ClientErrorReporter.current?.screen =
          const ['Home', 'Appointments', 'History', 'More'][index];
    });
    if (index <= 1) _refreshDoctorData();
  }

  @override
  void initState() {
    super.initState();
    current = this;
    ClientErrorReporter.current?.screen = 'Home';
    _queueRefresh = ScreenRefresh(() => context.read<DoctorCubit>().loadQueue(),
        interval: const Duration(seconds: 30),
        enabled: () => tab <= 1,
        networkChanges: context.read<DoctorCubit>().connectivity?.changes);
    final notifications = context.read<NotificationCubit>();
    _knownNotificationIds =
        notifications.state.items.map((item) => item.id).toSet();
    _queueNoticeSubscription = notifications.stream.listen((state) {
      final ids = state.items.map((item) => item.id).toSet();
      final hasNew = ids.difference(_knownNotificationIds).isNotEmpty;
      _knownNotificationIds = ids;
      if (hasNew) _refreshDoctorData();
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      context.read<DoctorCubit>().loadAll();
      unawaited(context.read<DoctorCubit>().loadBookingPolicy());
      context.read<NotificationCubit>().load();
      // M8-G-13: sequenced, not two independent `unawaited` calls — Android
      // notification channels must exist before FCM can be told to target
      // one (`android.notification.channel_id`), and a background/terminated
      // push referencing a not-yet-created channel is silently dropped by
      // the OS. Still a single `unawaited` overall, so this never blocks
      // the first frame.
      unawaited(_initLocalNotificationDelivery()
          .then((_) => _registerDeviceBestEffort()));
    });
  }

  /// Bootstraps Layers 2/3 of the no-external-push notification delivery
  /// architecture (see docs/DOCTOR_NOTIFICATION_DELIVERY_ARCHITECTURE.md).
  /// Layer 1 (realtime poll) is already running — `NotificationCubit.
  /// startPolling()` is called right at construction in `app.dart`.
  ///
  /// Each step is independently best-effort: a denied permission never
  /// blocks the shell (spec §18 — "no crash... explain in Settings"), and
  /// `registerBackgroundSync`'s own `ExistingWorkPolicy.keep` already makes
  /// calling it again on every shell mount harmless, so no "have we done
  /// this already" flag is needed here.
  Future<void> _initLocalNotificationDelivery() async {
    try {
      await LocalNotificationService.init();
      await LocalNotificationService.requestPermission();
      await registerBackgroundSync();
      // Once per install: how to stop this phone delaying reminders.
      if (mounted) await BatteryOptimization.maybePrompt(context);
    } catch (_) {
      // Best-effort — never block the shell or surface an error for this.
    }
  }

  /// Best-effort device-token registration — fire-and-forget telemetry, not
  /// critical path, so any failure (offline, backend hiccup) is swallowed
  /// rather than surfaced to the doctor. See
  /// `NotificationRepository.registerDevice`'s doc comment: the backend push
  /// transport is a confirmed no-op stub today, so this only records a
  /// registration row for whenever a real push provider is wired up.
  Future<void> _registerDeviceBestEffort() async {
    try {
      final cubit = context.read<NotificationCubit>();
      _pushService ??= PushNotificationService(
        repository: cubit.repo,
        onNotificationReceived: cubit.load,
      );
      await _pushService!.initialize();
      if (mounted) {
        final controller = DoctorLocaleScope.maybeOf(context);
        final revision = controller?.revision;
        final preferences = await cubit.repo.getPreferences();
        if (mounted) {
          await DoctorLocaleScope.maybeOf(context)?.setLanguage(
              preferences.preferredLanguage,
              expectedRevision: revision);
        }
      }
    } catch (_) {
      // Best-effort: never block the shell or surface an error for this.
    }
  }

  @override
  Widget build(BuildContext c) {
    final professional = c.watch<DoctorCubit>().state.profile;
    if (professional != null && !professional.verified) {
      return Scaffold(
          backgroundColor: DoctorTokens.screen,
          body: VerificationGateView(
              name: professional.name,
              license: professional.license,
              status: professional.verificationStatus,
              reviewNote: professional.reviewNote,
              onRefresh: c.read<DoctorCubit>().loadAll,
              onLogout: c.read<AuthCubit>().logout));
    }
    // Home and Appointments show live appointment data; both are refreshed
    // by the shell. Chat, prescriptions, wallet, Q&A and articles are reached
    // from More (product-owner decision, 2026-11-13).
    final pages = [
      HomePage(onOpenTab: selectTab),
      _schedule
          ? SchedulePage(header: DoctorTopBar(onMenu: () => selectTab(3)))
          : AppointmentsPage(
              onBack: () => selectTab(0), onOpenSchedule: openSchedule),
      ConsultationHistoryTab(
          onBack: () => selectTab(0), onOpenAppointments: () => selectTab(1)),
      MorePage(onBack: () => selectTab(0)),
    ];
    return Scaffold(
        body: pages[tab],
        bottomNavigationBar: SafeArea(
            top: false,
            child: DoctorBottomNav(selected: tab, onSelected: selectTab)));
  }
}

class DoctorBottomNav extends StatelessWidget {
  const DoctorBottomNav(
      {super.key, required this.selected, required this.onSelected});
  final int selected;
  final ValueChanged<int> onSelected;
  static const items = <(IconData, String)>[
    (Icons.home_outlined, 'Home'),
    (Icons.calendar_month_outlined, 'Appointments'),
    (Icons.history, 'History'),
    (Icons.more_horiz, 'More'),
  ];
  @override
  Widget build(BuildContext context) => Container(
      height: 76,
      decoration: const BoxDecoration(
          color: Colors.white,
          border: Border(top: BorderSide(color: DoctorTokens.fieldBorder))),
      padding: const EdgeInsets.only(top: 7, bottom: 5),
      child: Row(mainAxisAlignment: MainAxisAlignment.spaceAround, children: [
        for (var i = 0; i < items.length; i++)
          Expanded(
              child: InkWell(
                  onTap: () => onSelected(i),
                  child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(items[i].$1,
                            size: 25,
                            color: selected == i
                                ? DoctorTokens.primary
                                : DoctorTokens.fieldText),
                        const SizedBox(height: 3),
                        Text(items[i].$2,
                            style: TextStyle(
                                fontSize: 11,
                                fontWeight: selected == i
                                    ? FontWeight.w800
                                    : FontWeight.w500,
                                color: selected == i
                                    ? DoctorTokens.primary
                                    : DoctorTokens.fieldText))
                      ])))
      ]));
}
