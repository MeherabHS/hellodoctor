import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/theme/tokens.dart';
import '../../l10n/app_localizations.dart';
import '../patient/profile/profile_cubit.dart';
import '../patient/profile/profile_photo_avatar.dart';

/// Shown once, right after first sign-in, while the patient's
/// `patient_profiles` row still matches the exact OTP-login placeholder
/// (`PatientProfileDto.isPlaceholder` — see `auth_service::verify_otp_and_login`,
/// which seeds `full_name: "Patient User"`, `date_of_birth: "1995-01-01"`,
/// `gender: "OTHER"` with no prior way to correct it). Collects the minimum
/// real identity fields via the now-real `PATCH /patients/me` (backend gap
/// #1), then proceeds into the app.
class ProfileCompletionPage extends StatefulWidget {
  const ProfileCompletionPage({super.key, required this.onComplete});
  final VoidCallback onComplete;

  @override
  State<ProfileCompletionPage> createState() => _ProfileCompletionPageState();
}

class _ProfileCompletionPageState extends State<ProfileCompletionPage> {
  final _nameController = TextEditingController();
  final _dobController = TextEditingController();
  String _gender = 'OTHER';
  String? _error;

  @override
  void dispose() {
    _nameController.dispose();
    _dobController.dispose();
    super.dispose();
  }

  Future<void> _pickDate() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: DateTime.now().subtract(const Duration(days: 365 * 25)),
      firstDate: DateTime(1900),
      lastDate: DateTime.now(),
    );
    if (picked != null) {
      _dobController.text =
          '${picked.year.toString().padLeft(4, '0')}-${picked.month.toString().padLeft(2, '0')}-${picked.day.toString().padLeft(2, '0')}';
      setState(() {});
    }
  }

  Future<void> _submit() async {
    final l10n = AppLocalizations.of(context);
    setState(() => _error = null);
    if (_nameController.text.trim().isEmpty) {
      setState(() => _error = l10n.authEnterFullName);
      return;
    }
    if (_dobController.text.trim().isEmpty) {
      setState(() => _error = l10n.authSelectDateOfBirth);
      return;
    }
    final ok = await context.read<ProfileCubit>().save(
          fullName: _nameController.text.trim(),
          dateOfBirth: _dobController.text.trim(),
          gender: _gender,
        );
    if (!mounted) return;
    if (ok) {
      widget.onComplete();
    } else {
      setState(() => _error = context.read<ProfileCubit>().state.error ??
          l10n.authUnableToSaveProfile);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const SizedBox(height: 16),
              Text(l10n.authProfileCompletionTitle,
                  style: const TextStyle(
                      fontSize: 22,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.text)),
              const SizedBox(height: 6),
              Text(l10n.authProfileCompletionSubtitle,
                  style: const TextStyle(color: PatientTokens.textSub)),
              const SizedBox(height: 20),
              // Entirely optional (item 5/6 of the profile-picture
              // requirements): a picker/permission/upload failure here is
              // handled inside `ProfilePhotoAvatar` itself and never
              // prevents `_submit` below from completing registration.
              Center(
                child: Column(
                  children: [
                    const ProfilePhotoAvatar(radius: 36),
                    const SizedBox(height: 8),
                    Text(l10n.profilePhotoAdd,
                        style: const TextStyle(
                            fontSize: 12, color: PatientTokens.textSub)),
                  ],
                ),
              ),
              const SizedBox(height: 20),
              TextField(
                controller: _nameController,
                decoration: InputDecoration(labelText: l10n.authFullNameLabel),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _dobController,
                readOnly: true,
                onTap: _pickDate,
                decoration: InputDecoration(
                    labelText: l10n.authDateOfBirthLabel,
                    suffixIcon: const Icon(Icons.calendar_today)),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<String>(
                initialValue: _gender,
                decoration: InputDecoration(labelText: l10n.authGenderLabel),
                items: [
                  DropdownMenuItem(value: 'MALE', child: Text(l10n.genderMale)),
                  DropdownMenuItem(
                      value: 'FEMALE', child: Text(l10n.genderFemale)),
                  DropdownMenuItem(
                      value: 'OTHER', child: Text(l10n.genderOther)),
                ],
                onChanged: (value) =>
                    setState(() => _gender = value ?? _gender),
              ),
              if (_error != null) ...[
                const SizedBox(height: 12),
                Text(_error!,
                    style: const TextStyle(color: PatientTokens.danger)),
              ],
              const SizedBox(height: 24),
              BlocBuilder<ProfileCubit, ProfileState>(
                builder: (context, state) => SizedBox(
                  width: double.infinity,
                  child: FilledButton(
                    onPressed: state.saving ? null : _submit,
                    style: FilledButton.styleFrom(
                        backgroundColor: PatientTokens.primary,
                        padding: const EdgeInsets.symmetric(vertical: 14)),
                    child: state.saving
                        ? const SizedBox(
                            height: 18,
                            width: 18,
                            child: CircularProgressIndicator(
                                strokeWidth: 2, color: Colors.white))
                        : Text(l10n.continueButton),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
