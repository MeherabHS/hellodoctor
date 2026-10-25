// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/l10n/locale_controller.dart';
import '../../../l10n/app_localizations.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../auth/auth_cubit.dart';
import 'add_dependent_sheet.dart';
import 'family_profile_chip_row.dart';
import 'family_profiles_cubit.dart';
import 'language_modal.dart';
import '../../notifications/notification_settings_page.dart';
import 'profile_cubit.dart';
import 'profile_edit_sheet.dart';
import 'profile_models.dart';
import 'profile_photo_avatar.dart';
import 'data_rights_page.dart';

/// `prototype/index.html` view-7 — "PROFILE & FAMILY HEALTH HUB".
///
/// "Family Profiles" is now real (§5b multi-profile/family support,
/// `FamilyProfilesCubit`) — the prototype's own chips were confirmed purely
/// cosmetic before this. "Payment Methods & bKash" and "Privacy & Security"
/// remain honest "coming soon" taps: they are inert in the prototype too and
/// have no backend support, not silently dropped and not faked as working.
class PatientProfilePage extends StatefulWidget {
  const PatientProfilePage({super.key});

  @override
  State<PatientProfilePage> createState() => _PatientProfilePageState();
}

class _PatientProfilePageState extends State<PatientProfilePage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<ProfileCubit>().load();
      context.read<FamilyProfilesCubit>().load();
    });
  }

  void _comingSoon(String feature) {
    final l10n = AppLocalizations.of(context);
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(l10n.comingSoonSnackbar(feature))),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: BlocBuilder<ProfileCubit, ProfileState>(
          builder: (context, state) {
            final l10n = AppLocalizations.of(context);
            if (state.loading && state.profile == null) {
              return const Center(child: CircularProgressIndicator());
            }
            if (state.error != null && state.profile == null) {
              return ErrorPanel(state.error!,
                  onRetry: () => context.read<ProfileCubit>().load());
            }
            final profile = state.profile;
            return RefreshIndicator(
              onRefresh: () => Future.wait([
                context.read<ProfileCubit>().load(),
                context.read<FamilyProfilesCubit>().load(),
              ]),
              child: ListView(
                padding: const EdgeInsets.all(18),
                children: [
                  if (profile != null) _Hero(profile: profile),
                  const SizedBox(height: 18),
                  const _FamilyProfilesSection(),
                  const SizedBox(height: 4),
                  _SettingsTile(
                    icon: Icons.language,
                    title: l10n.languageTitle,
                    trailing: Builder(builder: (context) {
                      final locale = context.read<LocaleController>();
                      return AnimatedBuilder(
                        animation: locale,
                        builder: (context, _) => Text(
                          locale.locale.languageCode == 'sw'
                              ? l10n.languageSwahili
                              : l10n.languageEnglish,
                          style: const TextStyle(color: PatientTokens.textSub),
                        ),
                      );
                    }),
                    onTap: () => showLanguageModal(
                        context, context.read<LocaleController>()),
                  ),
                  _SettingsTile(
                    icon: Icons.notifications_outlined,
                    title: l10n.notificationSettingsMenuTitle,
                    onTap: () => Navigator.of(context).push(MaterialPageRoute(
                        builder: (_) => const NotificationSettingsPage())),
                  ),
                  _SettingsTile(
                    icon: Icons.payments_outlined,
                    title: l10n.profilePaymentMethods,
                    onTap: () => _comingSoon(l10n.profilePaymentMethods),
                  ),
                  _SettingsTile(
                    icon: Icons.privacy_tip_outlined,
                    title: l10n.profilePrivacySecurity,
                    onTap: () => Navigator.of(context).push(MaterialPageRoute(
                        builder: (_) => const DataRightsPage())),
                  ),
                  _SettingsTile(
                    icon: Icons.report_outlined,
                    title: l10n.profileReportIssue,
                    color: PatientTokens.danger,
                    onTap: () => _comingSoon(l10n.profileReportIssue),
                  ),
                  const SizedBox(height: 18),
                  SizedBox(
                    width: double.infinity,
                    child: OutlinedButton(
                      onPressed: () => context.read<AuthCubit>().logout(),
                      style: OutlinedButton.styleFrom(
                        foregroundColor: PatientTokens.danger,
                        side: const BorderSide(color: PatientTokens.danger),
                        padding: const EdgeInsets.symmetric(vertical: 14),
                      ),
                      child: Text(l10n.profileLogout),
                    ),
                  ),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}

class _Hero extends StatelessWidget {
  const _Hero({required this.profile});
  final PatientProfileDto profile;

  @override
  Widget build(BuildContext context) {
    return PatientCard(
      child: Column(
        children: [
          Row(children: [
            const ProfilePhotoAvatar(radius: 28),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(profile.fullName,
                      style: const TextStyle(
                          fontWeight: FontWeight.w800,
                          fontSize: 16,
                          color: PatientTokens.text)),
                  Text(
                      '${_genderLabel(context, profile.gender)} • ${_age(context, profile.dateOfBirth)}',
                      style: const TextStyle(
                          fontSize: 12, color: PatientTokens.textSub)),
                ],
              ),
            ),
            IconButton(
              icon: const Icon(Icons.edit_outlined,
                  color: PatientTokens.primaryDark),
              onPressed: () => showProfileEditSheet(context, profile),
            ),
          ]),
        ],
      ),
    );
  }

  String _genderLabel(BuildContext context, String gender) => switch (gender) {
        'MALE' => AppLocalizations.of(context).genderMale,
        'FEMALE' => AppLocalizations.of(context).genderFemale,
        _ => AppLocalizations.of(context).genderOther,
      };

  String _age(BuildContext context, String isoDate) {
    final dob = DateTime.tryParse(isoDate);
    if (dob == null) return '';
    final now = DateTime.now();
    var age = now.year - dob.year;
    if (now.month < dob.month ||
        (now.month == dob.month && now.day < dob.day)) {
      age--;
    }
    return AppLocalizations.of(context).profileAge(age.toString());
  }
}

class _FamilyProfilesSection extends StatelessWidget {
  const _FamilyProfilesSection();

  @override
  Widget build(BuildContext context) {
    return BlocBuilder<FamilyProfilesCubit, FamilyProfilesState>(
      builder: (context, state) {
        if (state.loading && state.profiles.isEmpty) {
          return const Padding(
            padding: EdgeInsets.symmetric(horizontal: 2),
            child: SkeletonBlock(height: 92),
          );
        }
        if (state.error != null && state.profiles.isEmpty) {
          return InlineError(state.error!,
              onRetry: () => context.read<FamilyProfilesCubit>().load());
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: EdgeInsets.symmetric(horizontal: 2, vertical: 6),
              child: Text(AppLocalizations.of(context).profileFamily,
                  style: TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.text)),
            ),
            FamilyProfileChipRow(
              profiles: state.profiles,
              activeProfileId: state.activeProfileId,
              onSelect: (id) =>
                  context.read<FamilyProfilesCubit>().setActiveProfile(id),
              onAddDependent: () => showAddDependentSheet(context),
            ),
            if (state.activeProfile != null && !state.activeProfile!.isSelf)
              Padding(
                padding: const EdgeInsets.fromLTRB(2, 8, 2, 0),
                child: Text(
                    AppLocalizations.of(context)
                        .profileManaging(state.activeProfile!.fullName),
                    style: const TextStyle(
                        fontSize: 11.5, color: PatientTokens.textSub)),
              ),
          ],
        );
      },
    );
  }
}

class _SettingsTile extends StatelessWidget {
  const _SettingsTile({
    required this.icon,
    required this.title,
    required this.onTap,
    this.trailing,
    this.color,
  });
  final IconData icon;
  final String title;
  final VoidCallback onTap;
  final Widget? trailing;
  final Color? color;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(bottom: 8),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
          child: PatientCard(
            child: Row(children: [
              Icon(icon, color: color ?? PatientTokens.textSub, size: 20),
              const SizedBox(width: 12),
              Expanded(
                child: Text(title,
                    style: TextStyle(
                        color: color ?? PatientTokens.text,
                        fontWeight: FontWeight.w600)),
              ),
              if (trailing != null) trailing!,
              const SizedBox(width: 4),
              const Icon(Icons.chevron_right,
                  color: PatientTokens.textSub, size: 18),
            ]),
          ),
        ),
      );
}
