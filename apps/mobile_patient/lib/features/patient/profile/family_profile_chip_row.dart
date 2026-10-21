// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';
import 'profile_models.dart';

/// `prototype/index.html` view-7's "Family Profiles" chip row
/// (`Rafiq (Self)` / `Amina (Spouse)` / `Zayd (Child)`), made real — the
/// prototype's own chips are confirmed purely cosmetic (no click handlers),
/// so this widget is the first real implementation, not a port.
class FamilyProfileChipRow extends StatelessWidget {
  const FamilyProfileChipRow({
    super.key,
    required this.profiles,
    required this.activeProfileId,
    required this.onSelect,
    this.onAddDependent,
  });

  final List<PatientProfileDto> profiles;
  final String? activeProfileId;
  final ValueChanged<String?> onSelect;
  final VoidCallback? onAddDependent;

  @override
  Widget build(BuildContext context) {
    if (profiles.isEmpty) return const SizedBox.shrink();
    return SizedBox(
      height: 92,
      child: ListView(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 18),
        children: [
          for (final profile in profiles) ...[
            _ProfileChip(
              profile: profile,
              active: _isActive(profile),
              onTap: () => onSelect(profile.isSelf ? null : profile.id),
            ),
            const SizedBox(width: 10),
          ],
          if (onAddDependent != null) _AddDependentChip(onTap: onAddDependent!),
        ],
      ),
    );
  }

  bool _isActive(PatientProfileDto profile) =>
      activeProfileId == null ? profile.isSelf : profile.id == activeProfileId;
}

class _ProfileChip extends StatelessWidget {
  const _ProfileChip(
      {required this.profile, required this.active, required this.onTap});
  final PatientProfileDto profile;
  final bool active;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final label =
        '${_firstName(profile.fullName)} (${profile.isSelf ? l10n.profileSelf : (profile.relationship ?? l10n.profileDependent)})';
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
      child: Container(
        width: 84,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 12),
        decoration: BoxDecoration(
          color: active ? PatientTokens.primaryLight : PatientTokens.surface,
          borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
          border: Border.all(
              color: active ? PatientTokens.primary : PatientTokens.border),
        ),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(profile.isSelf ? Icons.person : Icons.group_outlined,
                color: active ? PatientTokens.primary : PatientTokens.textSub,
                size: 24),
            const SizedBox(height: 6),
            Text(
              label,
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                color: active ? PatientTokens.primaryDark : PatientTokens.text,
              ),
            ),
          ],
        ),
      ),
    );
  }

  String _firstName(String fullName) {
    final trimmed = fullName.trim();
    if (trimmed.isEmpty) return 'Profile';
    return trimmed.split(RegExp(r'\s+')).first;
  }
}

class _AddDependentChip extends StatelessWidget {
  const _AddDependentChip({required this.onTap});
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
        child: Container(
          width: 84,
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 12),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
            border: Border.all(
                color: PatientTokens.border,
                style: BorderStyle.solid,
                width: 1.4),
          ),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(Icons.add_circle_outline,
                  color: PatientTokens.primaryDark, size: 24),
              SizedBox(height: 6),
              Text(
                '${AppLocalizations.of(context).dependentAdd.split(' ').first}\n${AppLocalizations.of(context).profileDependent}',
                textAlign: TextAlign.center,
                style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w700,
                    color: PatientTokens.primaryDark),
              ),
            ],
          ),
        ),
      );
}
