import 'core/notifications/local_notification_service.dart';
import 'core/notifications/notification_taps.dart';
import 'features/notifications/notification_navigation.dart';
// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:provider/provider.dart';

import 'core/cache/offline_documents.dart';
import 'features/onboarding/onboarding_page.dart';
import 'core/cache/patient_cache_store.dart';
import 'core/l10n/locale_controller.dart';
import 'core/network/api_client.dart';
import 'core/network/screen_refresh.dart';
import 'core/network/client_error_reporter.dart';
import 'core/notifications/push_notification_service.dart';
import 'core/network/connectivity_service.dart';
import 'core/theme/tokens.dart';
import 'core/widgets/afya_bottom_nav.dart';
import 'core/widgets/afya_widgets.dart';
import 'core/widgets/startup_error_page.dart';
import 'features/auth/auth_cubit.dart';
import 'features/auth/auth_entry_page.dart';
import 'features/auth/profile_completion_page.dart';
import 'features/notifications/notification_cubit.dart';
import 'features/notifications/notification_repository.dart';
import 'features/patient/chat/patient_chat_list_page.dart';
import 'features/patient/home/home_cubit.dart';
import 'features/patient/home/home_repository.dart';
import 'features/patient/home/patient_home_page.dart';
import 'features/patient/modules/patient_modules.dart';
import 'features/patient/profile/family_profiles_cubit.dart';
import 'features/patient/profile/family_profiles_repository.dart';
import 'features/patient/profile/patient_profile_page.dart';
import 'features/patient/profile/profile_cubit.dart';
import 'features/patient/profile/profile_repository.dart';
import 'features/patient/reminders/reminders_cubit.dart';
import 'features/patient/reminders/reminders_page.dart';
import 'features/patient/services/patient_services_page.dart';
import 'l10n/app_localizations.dart';

export 'core/widgets/afya_bottom_nav.dart' show PatientBottomNav;

/// Root widget of the Patient app.
///
/// Wires the same service-construction / startup-error-recovery pattern as
/// `apps/mobile`'s `HelloDoctorApp` (session restore via `AuthCubit.restore`,
/// a `StartupErrorPage` fallback if service construction itself throws, a
/// foreground-resume session revalidation). `LocaleController`
/// (English/Swahili) is new plumbing the Doctor app has no equivalent of.
class HelloDoctorPatientApp extends StatefulWidget {
  const HelloDoctorPatientApp({super.key, this.apiClientFactory});

  /// Allows integration tests to exercise the real app/session navigator using
  /// a controlled transport, while keeping the same secure storage and cubits.
  final ApiClient Function(TokenStore store)? apiClientFactory;
  @override
  State<HelloDoctorPatientApp> createState() => _AppState();
}

class _AppState extends State<HelloDoctorPatientApp>
    with WidgetsBindingObserver {
  SecureTokenStore? secureStore;
  PatientCacheStore? cache;
  ConnectivityService? connectivity;
  ApiClient? api;
  AuthCubit? auth;
  LocaleController? locale;
  // Set only if wiring up the services above throws (e.g. a misconfigured
  // backend address). Building a bare widget tree here — rather than letting
  // the exception propagate out of initState — is what keeps a startup
  // failure from rendering as an unexplained blank screen on a real device.
  Object? _startupError;
  Timer? _incidentRetryTimer;
  StreamSubscription<AuthState>? _authSubscription;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _initializeServices();
  }

  /// Revalidates the session whenever the app returns to the foreground,
  /// instead of silently trusting whatever was last loaded while
  /// backgrounded — the device could have been offline for hours, or the
  /// refresh token could have been revoked (logout elsewhere, admin action)
  /// while this app sat suspended.
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
    auth?.close();
    api?.dispose();
    connectivity?.dispose();
    try {
      secureStore = SecureTokenStore();
      cache = PatientCacheStore(secureStore!);
      // Offline Health Records: encrypted document copies, wiped at sign-out.
      OfflineDocuments.configure(secureStore!);
      connectivity = ConnectivityService();
      api = widget.apiClientFactory?.call(secureStore!) ??
          ApiClient(tokenStore: secureStore);
      ClientErrorReporter.current = ClientErrorReporter(api!,
          authenticated: () => auth?.state is SignedIn);
      ClientErrorReporter.current!.loadBuildMetadata();
      auth = AuthCubit(api!, cache: cache);
      locale = LocaleController(secureStore!)..load();
      // Signed out (onboarding), the choice stays pending until sign-in.
      locale!.bindBackendSync((languageCode) => auth?.state is SignedIn
          ? NotificationRepository(api!).syncLanguage(languageCode)
          : Future<void>.error(StateError('signed out')));
      _authSubscription = auth!.stream.listen((state) {
        _incidentRetryTimer?.cancel();
        if (state is SignedIn) {
          // A language picked before sign-in (onboarding) reaches the account.
          locale?.flushPendingSync();
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
      unawaited(auth!.restore());
      _startupError = null;
    } catch (error, stack) {
      unawaited(ClientErrorReporter.captureStartup(error, stack));
      _startupError = error;
    }
  }

  @override
  Widget build(BuildContext context) {
    final startupError = _startupError;
    if (startupError != null) {
      return StartupErrorPage(
          error: startupError, onRetry: () => setState(_initializeServices));
    }
    return ListenableBuilder(
      listenable: locale!,
      builder: (context, _) => ChangeNotifierProvider<LocaleController>.value(
        value: locale!,
        child: BlocProvider.value(
          value: auth!,
          child: BlocBuilder<AuthCubit, AuthState>(builder: (context, state) {
            Widget application(Widget home) => MaterialApp(
                  navigatorObservers: [screenRouteObserver],
                  // A changed home does not dismiss routes pushed above it.
                  // Recreate the Navigator only at the authentication boundary:
                  // OTP disappears on sign-in, protected routes on sign-out.
                  key: ValueKey(state is SignedIn),
                  debugShowCheckedModeBanner: false,
                  theme: PatientTokens.theme(),
                  locale: locale!.locale,
                  localizationsDelegates:
                      AppLocalizations.localizationsDelegates,
                  supportedLocales: AppLocalizations.supportedLocales,
                  home: home,
                );
            if (state is SignedIn) {
              return application(_SignedInRoot(
                api: api!,
                cache: cache!,
                connectivity: connectivity!,
              ));
            }
            return application(state is SessionRestoring
                ? const _SplashPage()
                // First launch on this phone: onboarding, then sign-in.
                : OnboardingGate(
                    store: secureStore!, child: AuthEntryPage(auth: auth!)));
          }),
        ),
      ),
    );
  }
}

class _SplashPage extends StatelessWidget {
  const _SplashPage();
  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: PatientTokens.bg,
      body: Center(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
        const AfyaLogoMark(size: 64),
        const SizedBox(height: 16),
        const Text('AfyaMkononi',
            style: TextStyle(
                fontSize: 22,
                fontWeight: FontWeight.w800,
                color: PatientTokens.text)),
        const SizedBox(height: 6),
        Text(AppLocalizations.of(context).restoringSession,
            style: TextStyle(fontSize: 12, color: PatientTokens.textSub)),
        const SizedBox(height: 18),
        const SizedBox(
            width: 22,
            height: 22,
            child: CircularProgressIndicator(strokeWidth: 2))
      ])));
}

/// Owns the cross-screen cubits a signed-in patient session needs
/// (`ProfileCubit`, `HomeCubit`, `FamilyProfilesCubit` today; more land
/// alongside their own milestones) and the one-time profile-completion gate:
/// a freshly OTP-logged-in account still carries the placeholder
/// `patient_profiles` row (`PatientProfileDto.isPlaceholder`) until
/// corrected, so this loads the real profile once before deciding whether to
/// show [ProfileCompletionPage] or the real [PatientShell].
///
/// `FamilyProfilesCubit` lives here, not inside `PatientHomePage` or
/// `PatientProfilePage`, specifically so the "active profile" it owns
/// survives switching between the Home and Account tabs (§5b
/// multi-profile/family support) — it is torn down only when this whole
/// signed-in subtree is, i.e. on logout.
class _SignedInRoot extends StatefulWidget {
  const _SignedInRoot({
    required this.api,
    required this.cache,
    required this.connectivity,
  });
  final ApiClient api;
  final PatientCacheStore cache;
  final ConnectivityService connectivity;

  @override
  State<_SignedInRoot> createState() => _SignedInRootState();
}

class _SignedInRootState extends State<_SignedInRoot>
    with WidgetsBindingObserver {
  late final ProfileRepository _profileRepo;
  late final ProfileCubit _profileCubit;
  late final HomeCubit _homeCubit;
  late final FamilyProfilesCubit _familyProfilesCubit;
  late final NotificationCubit _notificationCubit;
  late final PushNotificationService _pushService;
  final _reminders = ReminderSession();
  StreamSubscription<String?>? _reminderStart;
  bool _completionHandled = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _profileRepo = ProfileRepository(widget.api,
        cache: widget.cache, connectivity: widget.connectivity);
    _profileCubit =
        ProfileCubit(_profileRepo, connectivity: widget.connectivity)..load();
    _homeCubit = HomeCubit(
        HomeRepository(widget.api,
            cache: widget.cache, connectivity: widget.connectivity),
        connectivity: widget.connectivity);
    _familyProfilesCubit = FamilyProfilesCubit(
        FamilyProfilesRepository(widget.api,
            cache: widget.cache, connectivity: widget.connectivity),
        connectivity: widget.connectivity)
      ..load();
    // M8: the in-app notification inbox (gap-free — `GET /notifications` was
    // already real; only the Flutter screen was missing). Lives here, not
    // inside `NotificationCenterPage`, so the unread badge on Home's bell and
    // the Layer 1 realtime-poll-to-local-notification bridge both keep
    // working while the patient is on any tab, not only while the inbox
    // screen itself is open.
    _notificationCubit = NotificationCubit(NotificationRepository(widget.api))
      ..load()
      ..startPolling(interval: const Duration(minutes: 1));
    // Re-arm this account's saved reminders once its id is known.
    _reminderStart = _profileCubit.stream
        .map((s) => s.profile?.userId)
        .where((id) => id != null && id.isNotEmpty)
        .take(1)
        .listen((id) {
      // The profile loads after the first frame, so localizations exist.
      if (!mounted) return;
      unawaited(_reminders.start(
          id!, reminderAlertText(AppLocalizations.of(context))));
    });
    _pushService = PushNotificationService(
      repository: _notificationCubit.repo,
      onNotificationReceived: _notificationCubit.load,
    );
    // M8-G-13: sequenced, not two independent `unawaited` calls — Android
    // notification channels must exist before FCM can be told to target one
    // (`android.notification.channel_id`), and a background/terminated push
    // referencing a not-yet-created channel is silently dropped by the OS.
    // Still a single `unawaited` overall, so this never blocks the first
    // frame; `_pushService` itself is still constructed synchronously above
    // since `dispose()` references it unconditionally.
    unawaited(
        _initializeLocalNotifications().then((_) => _pushService.initialize()));
  }

  Future<void> _initializeLocalNotifications() async {
    try {
      await LocalNotificationService.init();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Notifications', operation: 'local_init');
    }
  }

  /// Back in the foreground (often from the system "Alarms & reminders"
  /// page): re-arm reminders if their exact-alarm access changed meanwhile.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed || !mounted) return;
    final id = _profileCubit.state.profile?.userId;
    if (id == null || id.isEmpty) return;
    unawaited(
        _reminders.resume(id, reminderAlertText(AppLocalizations.of(context))));
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _profileCubit.close();
    _homeCubit.close();
    _familyProfilesCubit.close();
    // Personal reminder alerts belong to the signed-in account: cancel them
    // when the session ends so a signed-out phone shows nobody's reminders.
    _reminderStart?.cancel();
    unawaited(_reminders.end(_profileCubit.state.profile?.userId));
    _notificationCubit.close();
    unawaited(_pushService.dispose());
    NotificationTaps.clear();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MultiBlocProvider(
      providers: [
        BlocProvider.value(value: _profileCubit),
        BlocProvider.value(value: _homeCubit),
        BlocProvider.value(value: _familyProfilesCubit),
        BlocProvider.value(value: _notificationCubit),
      ],
      child: BlocBuilder<ProfileCubit, ProfileState>(
        builder: (context, state) {
          if (state.loading && state.profile == null) {
            return const _SplashPage();
          }
          final profile = state.profile;
          if (profile != null && profile.isPlaceholder && !_completionHandled) {
            return ProfileCompletionPage(
              onComplete: () => setState(() => _completionHandled = true),
            );
          }
          return NotificationNavigationHost(
              cubit: _notificationCubit, child: PatientShell(api: widget.api));
        },
      ),
    );
  }
}

class PatientShell extends StatefulWidget {
  const PatientShell({super.key, required this.api});
  final ApiClient api;

  @override
  State<PatientShell> createState() => _PatientShellState();
}

class _PatientShellState extends State<PatientShell> {
  int tab = 0;

  /// Messages is built on first visit, not at boot, so opening the app does
  /// not add a consultation-list request to the startup traffic.
  bool _messagesVisited = false;

  late final _nav = PatientShellNav(
    openServices: () => _selectTab(1),
    openMessages: () => _selectTab(2),
    openProfile: () => _selectTab(3),
    select: _selectTab,
  );

  static const _screens = ['Home', 'Services', 'Messages', 'Profile'];

  @override
  void initState() {
    super.initState();
    ClientErrorReporter.current?.screen = 'Home';
  }

  void _selectTab(int index) => setState(() {
        tab = index;
        if (index == 2) _messagesVisited = true;
        ClientErrorReporter.current?.screen = _screens[index];
      });

  /// Android Back on Services, Messages or Profile returns to Home; only Back
  /// on Home leaves the app (D-08c).
  @override
  Widget build(BuildContext context) => PopScope(
        canPop: tab == 0,
        onPopInvokedWithResult: (didPop, _) {
          if (!didPop) _selectTab(0);
        },
        child: _scaffold(context),
      );

  Widget _scaffold(BuildContext context) => Scaffold(
        backgroundColor: PatientTokens.bg,
        body: IndexedStack(index: tab, children: [
          PatientHomePage(nav: _nav),
          PatientServicesPage(nav: _nav),
          _messagesVisited
              ? buildPatientMessagesTab(context)
              : const SizedBox.shrink(),
          const PatientProfilePage(),
        ]),
        bottomNavigationBar: SafeArea(
            top: false,
            child: PatientBottomNav(selected: tab, onSelected: _selectTab)),
      );
}
