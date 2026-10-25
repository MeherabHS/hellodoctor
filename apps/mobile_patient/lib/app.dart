// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:provider/provider.dart';

import 'core/cache/patient_cache_store.dart';
import 'core/l10n/locale_controller.dart';
import 'core/network/api_client.dart';
import 'core/network/screen_refresh.dart';
import 'core/network/client_error_reporter.dart';
import 'core/notifications/push_notification_service.dart';
import 'core/network/connectivity_service.dart';
import 'core/theme/tokens.dart';
import 'core/widgets/startup_error_page.dart';
import 'features/auth/auth_cubit.dart';
import 'features/auth/auth_entry_page.dart';
import 'features/auth/profile_completion_page.dart';
import 'features/notifications/notification_cubit.dart';
import 'features/notifications/notification_repository.dart';
import 'features/patient/blogs/blogs_page.dart';
import 'features/patient/blogs/blogs_repository.dart';
import 'features/patient/home/home_cubit.dart';
import 'features/patient/home/home_repository.dart';
import 'features/patient/home/patient_home_page.dart';
import 'features/patient/profile/family_profiles_cubit.dart';
import 'features/patient/profile/family_profiles_repository.dart';
import 'features/patient/profile/patient_profile_page.dart';
import 'features/patient/profile/profile_cubit.dart';
import 'features/patient/profile/profile_repository.dart';
import 'features/patient/services/patient_services_page.dart';
import 'l10n/app_localizations.dart';

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
      connectivity = ConnectivityService();
      api = widget.apiClientFactory?.call(secureStore!) ??
          ApiClient(tokenStore: secureStore);
      ClientErrorReporter.current = ClientErrorReporter(api!,
          authenticated: () => auth?.state is SignedIn);
      ClientErrorReporter.current!.loadBuildMetadata();
      auth = AuthCubit(api!, cache: cache);
      locale = LocaleController(secureStore!)..load();
      locale!.bindBackendSync(
          (languageCode) => NotificationRepository(api!).syncLanguage(languageCode));
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
                : AuthEntryPage(auth: auth!));
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
        Container(
            width: 58,
            height: 58,
            decoration: BoxDecoration(
                color: PatientTokens.primaryLight,
                borderRadius: BorderRadius.circular(18)),
            child: const Icon(Icons.medical_services_rounded,
                color: PatientTokens.primary, size: 30)),
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

class _SignedInRootState extends State<_SignedInRoot> {
  late final ProfileRepository _profileRepo;
  late final ProfileCubit _profileCubit;
  late final HomeCubit _homeCubit;
  late final FamilyProfilesCubit _familyProfilesCubit;
  late final NotificationCubit _notificationCubit;
  late final PushNotificationService _pushService;
  bool _completionHandled = false;

  @override
  void initState() {
    super.initState();
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
    _pushService = PushNotificationService(
      repository: _notificationCubit.repo,
      onNotificationReceived: _notificationCubit.load,
    );
    unawaited(_pushService.initialize());
  }

  @override
  void dispose() {
    _profileCubit.close();
    _homeCubit.close();
    _familyProfilesCubit.close();
    _notificationCubit.close();
    unawaited(_pushService.dispose());
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
          return PatientShell(api: widget.api);
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

  // Not `static const`: `PatientHomePage` needs an instance callback
  // (`onOpenAccountTab`) back into this state, mirroring the prototype's
  // `onclick="switchScreen(7)"` avatar tap now that the Profile screen is a
  // real tab rather than a "coming soon" stub.
  late final _pages = [
    PatientHomePage(onOpenAccountTab: () => _selectTab(3)),
    const PatientServicesPage(),
    BlogsPage(repo: BlogsRepository(widget.api)),
    const PatientProfilePage(),
  ];

  @override
  void initState() {
    super.initState();
    ClientErrorReporter.current?.screen = 'Home';
  }

  void _selectTab(int index) => setState(() {
        tab = index;
        ClientErrorReporter.current?.screen =
            const ['Home', 'Services', 'Blogs', 'Profile'][index];
      });

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: PatientTokens.bg,
        body: IndexedStack(index: tab, children: _pages),
        bottomNavigationBar: SafeArea(
            top: false,
            child: PatientBottomNav(selected: tab, onSelected: _selectTab)),
      );
}

class PatientBottomNav extends StatelessWidget {
  const PatientBottomNav(
      {super.key, required this.selected, required this.onSelected});
  final int selected;
  final ValueChanged<int> onSelected;
  static const items = <(IconData, String)>[
    (Icons.home_outlined, 'Home'),
    (Icons.grid_view_outlined, 'Services'),
    (Icons.article_outlined, 'Blogs'),
    (Icons.person_outline, 'Account'),
  ];
  @override
  Widget build(BuildContext context) => Container(
      height: 76,
      decoration: const BoxDecoration(
          color: Colors.white,
          border: Border(top: BorderSide(color: PatientTokens.border))),
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
                                ? PatientTokens.primary
                                : PatientTokens.textSub),
                        const SizedBox(height: 3),
                        Text(
                            [
                              AppLocalizations.of(context).navHome,
                              AppLocalizations.of(context).navServices,
                              AppLocalizations.of(context).navBlogs,
                              AppLocalizations.of(context).accountLabel,
                            ][i],
                            style: TextStyle(
                                fontSize: 10,
                                fontWeight: selected == i
                                    ? FontWeight.w700
                                    : FontWeight.w600,
                                color: selected == i
                                    ? PatientTokens.primary
                                    : PatientTokens.textSub))
                      ])))
      ]));
}
