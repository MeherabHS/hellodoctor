// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/l10n/locale_controller.dart';
import '../../../l10n/app_localizations.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../modules/afya_module_widgets.dart';
import '../../auth/auth_cubit.dart';
import 'add_dependent_sheet.dart';
import 'family_profile_chip_row.dart';
import 'family_profiles_cubit.dart';
import 'language_modal.dart';
import '../../notifications/notification_cubit.dart';
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
                padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
                children: [
                  Text(l10n.navProfile,
                      style: const TextStyle(
                          fontSize: 22,
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text)),
                  const SizedBox(height: 14),
                  if (profile != null) _Hero(profile: profile),
                  const SizedBox(height: 20),
                  const _FamilyProfilesSection(),
                  const SizedBox(height: 18),
                  _SectionLabel(l10n.profileSettingsSection),
                  _SettingsTile(
                    icon: Icons.language_rounded,
                    tint: PatientTokens.tileBlue,
                    tintBg: PatientTokens.tileBlueBg,
                    title: l10n.languageTitle,
                    trailing: Builder(builder: (context) {
                      final locale = context.read<LocaleController>();
                      return AnimatedBuilder(
                        animation: locale,
                        builder: (context, _) => Text(
                          locale.locale.languageCode == 'sw'
                              ? l10n.languageSwahili
                              : l10n.languageEnglish,
                          style: const TextStyle(
                              fontSize: 13, color: PatientTokens.textSub),
                        ),
                      );
                    }),
                    onTap: () => showLanguageModal(
                        context, context.read<LocaleController>()),
                  ),
                  _SettingsTile(
                    icon: Icons.notifications_rounded,
                    tint: PatientTokens.tileAmber,
                    tintBg: PatientTokens.tileAmberBg,
                    title: l10n.notificationSettingsMenuTitle,
                    onTap: () {
                      final repo = context.read<NotificationCubit>().repo;
                      Navigator.of(context).push(MaterialPageRoute(
                          builder: (_) =>
                              NotificationSettingsPage(repo: repo)));
                    },
                  ),
                  _SettingsTile(
                    icon: Icons.lock_rounded,
                    tint: PatientTokens.tilePurple,
                    tintBg: PatientTokens.tilePurpleBg,
                    title: l10n.profilePrivacySecurity,
                    onTap: () => Navigator.of(context).push(MaterialPageRoute(
                        builder: (_) => const DataRightsPage())),
                  ),
                  const SizedBox(height: 14),
                  SizedBox(
                    width: double.infinity,
                    child: OutlinedButton.icon(
                      onPressed: () => context.read<AuthCubit>().logout(),
                      icon: const Icon(Icons.logout_rounded, size: 19),
                      style: OutlinedButton.styleFrom(
                        foregroundColor: PatientTokens.urgentIcon,
                        backgroundColor: PatientTokens.urgentCard,
                        side: BorderSide.none,
                        minimumSize: const Size.fromHeight(52),
                        shape: const StadiumBorder(),
                      ),
                      label: Text(l10n.profileLogout),
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

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);
  final String text;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Text(text,
            style: const TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w800,
                color: PatientTokens.text)),
      );
}

class _Hero extends StatelessWidget {
  const _Hero({required this.profile});
  final PatientProfileDto profile;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 14, 6, 14),
      decoration: BoxDecoration(
          color: PatientTokens.primaryLight,
          borderRadius: BorderRadius.circular(14)),
      child: Row(children: [
        const ProfilePhotoAvatar(radius: 30),
        const SizedBox(width: 14),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(profile.fullName,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontWeight: FontWeight.w800,
                      fontSize: 18,
                      color: PatientTokens.text)),
              const SizedBox(height: 3),
              Text(
                  '${_genderLabel(context, profile.gender)} • ${_age(context, profile.dateOfBirth)}',
                  style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                      color: PatientTokens.greetingSub)),
            ],
          ),
        ),
        IconButton(
          tooltip: AppLocalizations.of(context).profileEdit,
          icon: const Icon(Icons.edit_outlined, color: PatientTokens.navActive),
          onPressed: () => showProfileEditSheet(context, profile),
        ),
      ]),
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
                      fontSize: 15,
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
    required this.tint,
    required this.tintBg,
    required this.title,
    required this.onTap,
    this.trailing,
  });
  final IconData icon;
  final Color tint, tintBg;
  final String title;
  final VoidCallback onTap;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: AfyaCard(
          onTap: onTap,
          padding: const EdgeInsets.fromLTRB(12, 10, 8, 10),
          child: Row(children: [
            AfyaIconTile(icon: icon, color: tint, background: tintBg, size: 42),
            const SizedBox(width: 12),
            Expanded(
              child: Text(title,
                  style: const TextStyle(
                      fontSize: 14,
                      color: PatientTokens.text,
                      fontWeight: FontWeight.w700)),
            ),
            if (trailing != null) trailing!,
            const SizedBox(width: 2),
            const Icon(Icons.chevron_right_rounded,
                color: PatientTokens.navInactive),
          ]),
        ),
      );
}
