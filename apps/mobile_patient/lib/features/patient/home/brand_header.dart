import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../../notifications/notification_center_page.dart';
import '../../notifications/notification_cubit.dart';
import '../../notifications/notification_navigation.dart';
import '../modules/patient_modules.dart';
import '../profile/family_profiles_cubit.dart';
import '../profile/profile_cubit.dart';

/// What the app's main header needs. Captured where the session cubits are in
/// scope (a tab under `_SignedInRoot`) and handed to pushed screens, which sit
/// outside that provider scope (see D-04 in the gap analysis).
class AppHeaderDeps {
  const AppHeaderDeps({
    required this.notifications,
    required this.profile,
    required this.onBell,
    required this.onAvatar,
  });

  final NotificationCubit notifications;
  final ProfileCubit profile;
  final VoidCallback onBell, onAvatar;

  /// [context] must be under the signed-in providers. The bell opens the
  /// notification inbox; the avatar returns to the shell and opens Profile.
  factory AppHeaderDeps.of(BuildContext context, PatientShellNav nav) {
    final notifications = context.read<NotificationCubit>();
    final profile = context.read<ProfileCubit>();
    final family = context.read<FamilyProfilesCubit>();
    final navigator = Navigator.of(context);
    return AppHeaderDeps(
      notifications: notifications,
      profile: profile,
      onBell: () => navigator.push(MaterialPageRoute(
          builder: (_) => NotificationCenterPage(
              cubit: notifications,
              deps: AppointmentRouteDeps(
                  familyProfilesCubit: family,
                  myUserId: profile.state.profile?.userId)))),
      onAvatar: () {
        navigator.popUntil((r) => r.isFirst);
        nav.openProfile();
      },
    );
  }
}

/// The app's main header (reference panel 1): logo, wordmark, tagline,
/// notification bell with unread count, and the initials avatar. Shared by
/// Home and the HIV Prevention & Sexual Health screens.
class AfyaBrandHeader extends StatelessWidget {
  const AfyaBrandHeader({super.key, required this.deps});
  final AppHeaderDeps deps;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 6, 12, 0),
      child: Row(children: [
        const AfyaLogoMark(size: 48),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                const FittedBox(
                  fit: BoxFit.scaleDown,
                  alignment: Alignment.centerLeft,
                  child: Text('AfyaMkononi',
                      maxLines: 1,
                      style: TextStyle(
                          fontSize: 22,
                          height: 1.15,
                          fontWeight: FontWeight.w800,
                          letterSpacing: -.2,
                          color: PatientTokens.brandTeal)),
                ),
                const SizedBox(height: 3),
                FittedBox(
                  fit: BoxFit.scaleDown,
                  alignment: Alignment.centerLeft,
                  child: Text(l10n.brandTagline,
                      maxLines: 1,
                      style: const TextStyle(
                          fontSize: 10.5,
                          fontWeight: FontWeight.w600,
                          color: PatientTokens.brandTealSoft)),
                ),
              ]),
        ),
        BlocBuilder<NotificationCubit, NotificationState>(
          bloc: deps.notifications,
          builder: (context, state) => IconButton(
              onPressed: deps.onBell,
              // The bell opens the inbox (D-08d: it used to be labelled
              // "Notification settings").
              tooltip: l10n.notificationsTitle,
              icon: Badge(
                  isLabelVisible: state.unreadCount > 0,
                  label: Text(
                      state.unreadCount > 99 ? '99+' : '${state.unreadCount}'),
                  backgroundColor: PatientTokens.danger,
                  child: const Icon(Icons.notifications_none_rounded,
                      size: 26, color: PatientTokens.navy))),
        ),
        BlocBuilder<ProfileCubit, ProfileState>(
          bloc: deps.profile,
          builder: (context, state) => InkWell(
            onTap: deps.onAvatar,
            customBorder: const CircleBorder(),
            child: Padding(
              padding: const EdgeInsets.all(6),
              child: _HeaderAvatar(profile: deps.profile, state: state),
            ),
          ),
        ),
      ]),
    );
  }
}

/// The signed-in patient's saved photo, or their initials when there is
/// none. Shares [ProfileCubit]'s in-memory photo with the Profile screen, so
/// a new photo appears here as soon as it is saved.
class _HeaderAvatar extends StatefulWidget {
  const _HeaderAvatar({required this.profile, required this.state});
  final ProfileCubit profile;
  final ProfileState state;
  @override
  State<_HeaderAvatar> createState() => _HeaderAvatarState();
}

class _HeaderAvatarState extends State<_HeaderAvatar> {
  @override
  void initState() {
    super.initState();
    // One attempt per header; a patient without a photo keeps the initials.
    final s = widget.profile.state;
    if (s.photoBytes == null && !s.photoLoading && !widget.profile.isClosed) {
      widget.profile.loadPhoto();
    }
  }

  @override
  Widget build(BuildContext context) {
    final bytes = widget.state.photoBytes;
    if (bytes == null) {
      return AfyaInitialsAvatar(name: widget.state.profile?.fullName, size: 36);
    }
    return ClipOval(
        child: Image.memory(bytes,
            key: const ValueKey('home-header-photo'),
            width: 36,
            height: 36,
            fit: BoxFit.cover,
            gaplessPlayback: true));
  }
}
