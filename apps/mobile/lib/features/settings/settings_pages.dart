import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../app.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/doctor_widgets.dart';
import '../auth/auth_cubit.dart';
import '../doctor/doctor_pages.dart';
import 'notification_settings_page.dart';

/// Settings home: the nested IA described in the HelloDoctor Doctor Flutter
/// notification/profile-parity spec — Profile, Professional Details,
/// Availability, Payout and Security as named sections, each either reusing
/// an existing screen (Profile/Professional Details both already live in
/// `ProfilePage`; Availability/Payout deep-link to the existing Schedule/
/// Wallet tabs rather than duplicating them) or getting a minimal new one
/// (Security).
class SettingsHomePage extends StatelessWidget {
  const SettingsHomePage({super.key});

  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.bg,
      body: SafeArea(
          child: Column(children: [
        const DoctorPageHeader('Settings', 'Your account and preferences'),
        Expanded(
            child: ListView(padding: const EdgeInsets.all(16), children: [
          _SettingsTile(
              icon: Icons.person_outline,
              title: 'Profile',
              subtitle: 'Name and professional details',
              onTap: () => Navigator.push(context,
                  MaterialPageRoute(builder: (_) => const ProfilePage()))),
          _SettingsTile(
              icon: Icons.calendar_month_outlined,
              title: 'Availability',
              subtitle: 'Manage your schedule slots',
              onTap: () => _switchToTab(context, tabIndex: 3)),
          _SettingsTile(
              icon: Icons.account_balance_wallet_outlined,
              title: 'Payout',
              subtitle: 'Earnings and payout destination',
              onTap: () => _switchToTab(context, tabIndex: 4)),
          _SettingsTile(
              icon: Icons.notifications_outlined,
              title: 'Notifications',
              subtitle: 'Quiet hours and notification categories',
              onTap: () => Navigator.push(
                  context,
                  MaterialPageRoute(
                      builder: (_) => const NotificationSettingsPage()))),
          _SettingsTile(
              icon: Icons.shield_outlined,
              title: 'Security',
              subtitle: 'Session and sign-out',
              onTap: () => Navigator.push(
                  context,
                  MaterialPageRoute(
                      builder: (_) => const SecuritySettingsPage()))),
        ]))
      ])));

  // Availability/Payout already have full, independently-maintained screens
  // on the main bottom nav (Schedule/Wallet) — Settings deep-links to the
  // same tab rather than re-implementing that UI, so there is exactly one
  // place each piece of data is rendered.
  void _switchToTab(BuildContext context, {required int tabIndex}) {
    Navigator.of(context).popUntil((r) => r.isFirst);
    DoctorShell.switchTab(context, tabIndex);
  }
}

class _SettingsTile extends StatelessWidget {
  const _SettingsTile({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onTap,
  });
  final IconData icon;
  final String title, subtitle;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: DoctorCard(
          child: ListTile(
              contentPadding: EdgeInsets.zero,
              leading: Icon(icon, color: DoctorTokens.primaryDark),
              title: Text(title,
                  style: const TextStyle(fontWeight: FontWeight.w700)),
              subtitle: Text(subtitle,
                  style: const TextStyle(
                      fontSize: 12, color: DoctorTokens.textSub)),
              trailing: const Icon(Icons.chevron_right, size: 18),
              onTap: onTap)));
}

/// Confirmed scope (see the plan's Security-tab decision): Doctor Web itself
/// has no self-service password/MFA management today — only session info and
/// logout — so Flutter matches that exactly rather than inventing a mobile-
/// only security surface the backend doesn't support.
class SecuritySettingsPage extends StatelessWidget {
  const SecuritySettingsPage({super.key});
  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.bg,
      appBar: AppBar(title: const Text('Security')),
      body: Padding(
          padding: const EdgeInsets.all(20),
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            const Text('Current session',
                style: TextStyle(fontSize: 16, fontWeight: FontWeight.w700)),
            const SizedBox(height: 6),
            const Text(
                'You are signed in on this device. Signing out ends this session; '
                'sign back in with your email and password.',
                style: TextStyle(color: DoctorTokens.textSub)),
            const SizedBox(height: 20),
            FilledButton(
                onPressed: () => context.read<AuthCubit>().logout(),
                child: const Text('Log out')),
          ])));
}
