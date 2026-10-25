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
import 'core/notifications/local_notification_service.dart';
import 'core/theme/tokens.dart';
import 'core/validation/kenya_phone.dart';
import 'core/widgets/startup_error_page.dart';
import 'features/auth/auth_cubit.dart';
import 'features/auth/legal_consent_page.dart';
import 'features/doctor/doctor_cubit.dart';
import 'features/doctor/doctor_pages.dart';
import 'features/doctor/doctor_repository.dart';
import 'features/notifications/notification_cubit.dart';
import 'features/notifications/notification_repository.dart';
import 'features/settings/settings_pages.dart';

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
    return BlocProvider.value(
        value: auth!,
        child: BlocBuilder<AuthCubit, AuthState>(builder: (_, state) {
          Widget application(Widget home) => MaterialApp(
              navigatorObservers: [screenRouteObserver],
              debugShowCheckedModeBanner: false,
              theme: DoctorTokens.theme(),
              home: home);
          if (state is SignedIn) {
            // Scope both the Navigator and all routed subflows to this account.
            // Removing these providers on logout also disposes their route
            // stack and, for NotificationCubit, any in-flight requests.
            return MultiBlocProvider(providers: [
              BlocProvider<DoctorCubit>(
                  create: (_) => DoctorCubit(DoctorRepository(api!,
                      cache: cache, connectivity: connectivity!))),
              BlocProvider<NotificationCubit>(
                  create: (_) =>
                      NotificationCubit(NotificationRepository(api!))
                        ..startPolling(interval: const Duration(minutes: 1))),
            ], child: application(const DoctorShell()));
          }
          return application(state is SessionRestoring
              ? const _SplashPage()
              : LoginPage(auth: auth!));
        }));
  }
}

class _SplashPage extends StatelessWidget {
  const _SplashPage();
  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.bg,
      body: Center(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
        Container(
            width: 58,
            height: 58,
            decoration: BoxDecoration(
                color: DoctorTokens.primaryLight,
                borderRadius: BorderRadius.circular(18)),
            child: const Icon(Icons.medical_services_rounded,
                color: DoctorTokens.primary, size: 30)),
        const SizedBox(height: 16),
        const Text('AfyaMkononi',
            style: TextStyle(
                fontSize: 22,
                fontWeight: FontWeight.w800,
                color: DoctorTokens.text)),
        const SizedBox(height: 6),
        const Text('Restoring secure Doctor session…',
            style: TextStyle(fontSize: 12, color: DoctorTokens.textSub)),
        const SizedBox(height: 18),
        const SizedBox(
            width: 22,
            height: 22,
            child: CircularProgressIndicator(strokeWidth: 2))
      ])));
}

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
  @override
  void dispose() {
    email.dispose();
    password.dispose();
    otp.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext c) {
    return Scaffold(
        body: SafeArea(
            child: Center(
                child: SingleChildScrollView(
                    padding: const EdgeInsets.all(28),
                    child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 420),
                        child:
                            BlocBuilder<AuthCubit, AuthState>(builder: (_, s) {
                          final m = s is MfaRequired;
                          return Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Icon(Icons.medical_services_rounded,
                                    color: DoctorTokens.primary, size: 42),
                                const SizedBox(height: 14),
                                const Text('Physician Secure Login',
                                    style: TextStyle(
                                        fontSize: 24,
                                        fontWeight: FontWeight.w800,
                                        color: DoctorTokens.text)),
                                const SizedBox(height: 6),
                                Text(
                                    m
                                        ? 'Enter the verification code for this secure session.'
                                        : 'Licensed KMPDC practitioners gateway.',
                                    style: const TextStyle(
                                        color: DoctorTokens.textSub)),
                                const SizedBox(height: 26),
                                if (!m) ...[
                                  TextField(
                                      controller: email,
                                      keyboardType: TextInputType.emailAddress,
                                      autofillHints: const [
                                        AutofillHints.email
                                      ],
                                      decoration: const InputDecoration(
                                          labelText: 'Email address')),
                                  const SizedBox(height: 12),
                                  TextField(
                                      controller: password,
                                      obscureText: true,
                                      decoration: const InputDecoration(
                                          labelText: 'Password')),
                                  const SizedBox(height: 18),
                                  _button(
                                      'Continue to MFA',
                                      s is AuthBusy,
                                      () => widget.auth.login(email.text.trim(),
                                          password.text.trim())),
                                  Center(
                                      child: TextButton(
                                          onPressed: s is AuthBusy
                                              ? null
                                              : () => Navigator.push(
                                                  c,
                                                  MaterialPageRoute(
                                                      builder: (_) =>
                                                          RegistrationPage(
                                                              auth: widget
                                                                  .auth))),
                                          child:
                                              const Text('Register as Doctor')))
                                ] else ...[
                                  TextField(
                                      controller: otp,
                                      keyboardType: TextInputType.number,
                                      decoration: const InputDecoration(
                                          labelText:
                                              'One-time verification code')),
                                  const SizedBox(height: 18),
                                  _button('Verify and sign in', s is AuthBusy,
                                      () => widget.auth.verify(otp.text.trim()))
                                ],
                                if (s is SignedOut && s.notice != null)
                                  Padding(
                                      padding: const EdgeInsets.only(top: 14),
                                      child: Text(s.notice!,
                                          style: const TextStyle(
                                              color: DoctorTokens.danger))),
                                if (s is AuthFailure)
                                  Padding(
                                      padding: const EdgeInsets.only(top: 14),
                                      child: Text(s.message,
                                          style: const TextStyle(
                                              color: DoctorTokens.danger)))
                              ]);
                        }))))));
  }
}

class RegistrationPage extends StatefulWidget {
  const RegistrationPage({super.key, required this.auth});
  final AuthCubit auth;
  @override
  State<RegistrationPage> createState() => _RegistrationState();
}

class _RegistrationState extends State<RegistrationPage> {
  String? _clientError;

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

  Widget field(String key, String label,
          {TextInputType? type, bool secret = false, int? maxLength}) =>
      Padding(
          padding: const EdgeInsets.only(bottom: 10),
          child: TextField(
              controller: controllers[key],
              keyboardType: type,
              obscureText: secret,
              maxLength: maxLength,
              decoration: InputDecoration(labelText: label)));

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

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Register as Doctor')),
      body: BlocBuilder<AuthCubit, AuthState>(
          bloc: widget.auth,
          builder: (_, state) {
            if (state is RegistrationSubmitted) {
              return Center(
                  child: Padding(
                      padding: const EdgeInsets.all(28),
                      child: Column(mainAxisSize: MainAxisSize.min, children: [
                        const Icon(Icons.pending_actions,
                            color: DoctorTokens.primary, size: 52),
                        const SizedBox(height: 14),
                        const Text('Registration submitted',
                            style: TextStyle(
                                fontSize: 22, fontWeight: FontWeight.w800)),
                        const SizedBox(height: 8),
                        const Text(
                            'Your Doctor account is pending ClinicalAdmin verification.'),
                        const SizedBox(height: 18),
                        FilledButton(
                            onPressed: () => Navigator.pop(context),
                            child: const Text('Return to login'))
                      ])));
            }
            final busy = state is AuthBusy;
            // Length caps match the shared client/server contract: 150 for
            // short label-like fields, 255 for email (mirrors the backend's
            // own `validate_email` cap), 20 for a phone number, 300 for the
            // qualifications/address fields (longer than a label but not
            // free-form prose), and a 3-digit cap on experience years (a
            // numeric field — this just blocks obviously-absurd input, not a
            // length-of-text concern). Password is deliberately left
            // uncapped: enforcing a *maximum* password length is a
            // well-known anti-pattern, not a safety measure.
            return ListView(padding: const EdgeInsets.all(24), children: [
              const Text('Submit your credentials for verification.',
                  style: TextStyle(color: DoctorTokens.textSub)),
              const SizedBox(height: 18),
              field('name', 'Full name', maxLength: 150),
              const Text(
                  'Professional Registration — Kenya Medical Practitioners and Dentists Council (KMPDC) · Kenya'),
              field('license', 'KMPDC Registration Number', maxLength: 150),
              field('phone', 'Phone number',
                  type: TextInputType.phone, maxLength: 20),
              field('email', 'Email address (used to sign in)',
                  type: TextInputType.emailAddress, maxLength: 255),
              field('password',
                  'Password (8+ chars: upper, lower, number, symbol)',
                  secret: true),
              field('specialty', 'Primary specialty', maxLength: 150),
              field('experience', 'Experience years',
                  type: TextInputType.number, maxLength: 3),
              field('hospital', 'Current hospital', maxLength: 150),
              field('qualifications', 'Qualifications (comma separated)',
                  maxLength: 300),
              field('address', 'Residential address', maxLength: 300),
              const SizedBox(height: 8),
              FilledButton(
                  onPressed: busy
                      ? null
                      : () {
                          final passwordError = _passwordStrengthError(
                              controllers['password']!.text);
                          if (passwordError != null) {
                            setState(() => _clientError = passwordError);
                            return;
                          }
                          final rawPhone = controllers['phone']!.text;
                          final phoneError = _phoneError(rawPhone);
                          if (phoneError != null) {
                            setState(() => _clientError = phoneError);
                            return;
                          }
                          final normalizedPhone =
                              normalizeKenyaPhone(rawPhone)!;
                          setState(() => _clientError = null);
                          Navigator.push(
                              context,
                              MaterialPageRoute(
                                  builder: (_) => LegalConsentPage(
                                      auth: widget.auth,
                                      draft: DoctorRegistrationDraft(
                                          fullName: controllers['name']!.text,
                                          licenseNumber:
                                              controllers['license']!.text,
                                          phoneNumber: normalizedPhone,
                                          email: controllers['email']!.text,
                                          password:
                                              controllers['password']!.text,
                                          specialty:
                                              controllers['specialty']!.text,
                                          experienceYears:
                                              controllers['experience']!.text,
                                          hospital:
                                              controllers['hospital']!.text,
                                          qualifications:
                                              controllers['qualifications']!
                                                  .text,
                                          address:
                                              controllers['address']!.text))));
                        },
                  child: busy
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(strokeWidth: 2))
                      : const Text('Continue')),
              if (_clientError != null)
                Padding(
                    padding: const EdgeInsets.only(top: 12),
                    child: Text(_clientError!,
                        style: const TextStyle(color: DoctorTokens.danger)))
              else if (state is AuthFailure)
                Padding(
                    padding: const EdgeInsets.only(top: 12),
                    child: Text(state.message,
                        style: const TextStyle(color: DoctorTokens.danger)))
            ]);
          }));
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

class DoctorShell extends StatefulWidget {
  const DoctorShell({super.key});

  /// Lets a pushed page (e.g. Settings' Availability/Payout tiles) switch
  /// the shell back to one of its bottom-nav tabs instead of duplicating
  /// that tab's screen. Looks up the nearest `DoctorShell` ancestor's own
  /// state directly; a no-op if called outside the shell (shouldn't happen
  /// in practice, since every Doctor screen lives under it).
  static void switchTab(BuildContext context, int index) {
    context.findAncestorStateOfType<_ShellState>()?.selectTab(index);
  }

  @override
  State<DoctorShell> createState() => _ShellState();
}

class _ShellState extends State<DoctorShell> with WidgetsBindingObserver {
  int tab = 0;
  late final ScreenRefresh _queueRefresh;
  StreamSubscription<NotificationState>? _queueNoticeSubscription;
  Set<String> _knownNotificationIds = {};
  PushNotificationService? _pushService;

  void _refreshDoctorData() {
    if (mounted && tab == 0) unawaited(_queueRefresh.run());
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
    _queueRefresh.dispose();
    _queueNoticeSubscription?.cancel();
    unawaited(_pushService?.dispose());
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  void selectTab(int index) {
    setState(() {
      tab = index;
      ClientErrorReporter.current?.screen = const [
        'Queue',
        'Chat',
        'SmartRx',
        'Schedule',
        'Wallet',
        'Settings'
      ][index];
    });
    if (index == 0) _refreshDoctorData();
  }

  @override
  void initState() {
    super.initState();
    ClientErrorReporter.current?.screen = 'Queue';
    _queueRefresh = ScreenRefresh(() => context.read<DoctorCubit>().loadQueue(),
        interval: const Duration(seconds: 30),
        enabled: () => tab == 0,
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
      context.read<NotificationCubit>().load();
      unawaited(_registerDeviceBestEffort());
      unawaited(_initLocalNotificationDelivery());
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
    } catch (_) {
      // Best-effort: never block the shell or surface an error for this.
    }
  }

  @override
  Widget build(BuildContext c) {
    final professional = c.watch<DoctorCubit>().state.profile;
    if (professional != null && !professional.verified) {
      return Scaffold(
          appBar: AppBar(title: const Text('Professional Registration')),
          body: Padding(
              padding: const EdgeInsets.all(24),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(professional.name),
                    Text('KMPDC Registration: ${professional.license}'),
                    const Text(
                        'Kenya Medical Practitioners and Dentists Council · Kenya'),
                    Text('Status: ${professional.verificationLabel}'),
                    const Text(
                        'Manual AfyaMkononi registration review. Clinical access requires approval.'),
                    if (professional.reviewNote != null)
                      Text(professional.reviewNote!),
                    TextButton(
                        onPressed: c.read<DoctorCubit>().loadAll,
                        child: const Text('Refresh verification status')),
                    TextButton(
                        onPressed: c.read<AuthCubit>().logout,
                        child: const Text('Log out')),
                  ])));
    }
    const pages = [
      QueuePage(),
      ChatPage(),
      ScannerPage(),
      SchedulePage(),
      WalletPage(),
      SettingsHomePage()
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
    (Icons.people_outline, 'Queue'),
    (Icons.chat_bubble_outline, 'Chat'),
    (Icons.camera_alt_outlined, 'Scan Rx'),
    (Icons.calendar_month_outlined, 'Schedule'),
    (Icons.account_balance_wallet_outlined, 'Wallet'),
    (Icons.settings_outlined, 'Settings'),
  ];
  @override
  Widget build(BuildContext context) => Container(
      height: 76,
      decoration: const BoxDecoration(
          color: Colors.white,
          border: Border(top: BorderSide(color: DoctorTokens.border))),
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
                            size: 22,
                            color: selected == i
                                ? DoctorTokens.primary
                                : DoctorTokens.textSub),
                        const SizedBox(height: 3),
                        Text(items[i].$2,
                            style: TextStyle(
                                fontSize: 10,
                                fontWeight: selected == i
                                    ? FontWeight.w700
                                    : FontWeight.w600,
                                color: selected == i
                                    ? DoctorTokens.primary
                                    : DoctorTokens.textSub))
                      ])))
      ]));
}
