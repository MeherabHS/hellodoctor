// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';
import '../../../l10n/app_localizations.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/validation/kenya_phone.dart';
import 'profile_cubit.dart';
import 'profile_models.dart';

Future<void> showProfileEditSheet(
    BuildContext context, PatientProfileDto profile) {
  final cubit = context.read<ProfileCubit>();
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    builder: (sheetContext) => BlocProvider.value(
      value: cubit,
      child: _ProfileEditSheet(profile: profile),
    ),
  );
}

class _ProfileEditSheet extends StatefulWidget {
  const _ProfileEditSheet({required this.profile});
  final PatientProfileDto profile;

  @override
  State<_ProfileEditSheet> createState() => _ProfileEditSheetState();
}

class _ProfileEditSheetState extends State<_ProfileEditSheet> {
  late final TextEditingController _nameController;
  late final TextEditingController _dobController;
  late final TextEditingController _bloodGroupController;
  late final TextEditingController _weightController;
  late final TextEditingController _emergencyPhoneController;
  late final TextEditingController _emergencyRelationController;
  late String _gender;
  String? _error;

  @override
  void initState() {
    super.initState();
    final p = widget.profile;
    _nameController = TextEditingController(text: p.fullName);
    _dobController = TextEditingController(text: p.dateOfBirth);
    _bloodGroupController = TextEditingController(text: p.bloodGroup ?? '');
    _weightController = TextEditingController(text: p.weightKg ?? '');
    _emergencyPhoneController =
        TextEditingController(text: p.emergencyContactPhone ?? '');
    _emergencyRelationController =
        TextEditingController(text: p.emergencyContactRelation ?? '');
    _gender = p.gender;
  }

  @override
  void dispose() {
    _nameController.dispose();
    _dobController.dispose();
    _bloodGroupController.dispose();
    _weightController.dispose();
    _emergencyPhoneController.dispose();
    _emergencyRelationController.dispose();
    super.dispose();
  }

  Future<void> _pickDate() async {
    final initial = DateTime.tryParse(_dobController.text) ??
        DateTime.now().subtract(const Duration(days: 365 * 25));
    final picked = await showDatePicker(
      context: context,
      initialDate: initial,
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
    setState(() => _error = null);
    String? normalizedPhone;
    if (_emergencyPhoneController.text.trim().isNotEmpty) {
      normalizedPhone = normalizeKenyaPhone(_emergencyPhoneController.text);
      if (normalizedPhone == null) {
        setState(() =>
            _error = AppLocalizations.of(context).profileInvalidEmergencyPhone);
        return;
      }
    }
    final ok = await context.read<ProfileCubit>().save(
          fullName: _nameController.text.trim().isEmpty
              ? null
              : _nameController.text.trim(),
          dateOfBirth: _dobController.text.trim().isEmpty
              ? null
              : _dobController.text.trim(),
          gender: _gender,
          bloodGroup: _bloodGroupController.text.trim().isEmpty
              ? null
              : _bloodGroupController.text.trim(),
          weightKg: _weightController.text.trim().isEmpty
              ? null
              : _weightController.text.trim(),
          emergencyContactPhone: normalizedPhone,
          emergencyContactRelation:
              _emergencyRelationController.text.trim().isEmpty
                  ? null
                  : _emergencyRelationController.text.trim(),
        );
    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop();
    } else {
      setState(() => _error = context.read<ProfileCubit>().state.error ??
          AppLocalizations.of(context).authUnableToSaveProfile);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.only(
        left: 20,
        right: 20,
        top: 20,
        bottom: MediaQuery.of(context).viewInsets.bottom + 20,
      ),
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(AppLocalizations.of(context).profileEdit,
                style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
            const SizedBox(height: 14),
            TextField(
              controller: _nameController,
              decoration: InputDecoration(
                  labelText: AppLocalizations.of(context).authFullNameLabel),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _dobController,
              readOnly: true,
              onTap: _pickDate,
              decoration: InputDecoration(
                  labelText: AppLocalizations.of(context).authDateOfBirthLabel,
                  suffixIcon: Icon(Icons.calendar_today)),
            ),
            const SizedBox(height: 10),
            DropdownButtonFormField<String>(
              initialValue: _gender,
              decoration: InputDecoration(
                  labelText: AppLocalizations.of(context).authGenderLabel),
              items: [
                DropdownMenuItem(
                    value: 'MALE',
                    child: Text(AppLocalizations.of(context).genderMale)),
                DropdownMenuItem(
                    value: 'FEMALE',
                    child: Text(AppLocalizations.of(context).genderFemale)),
                DropdownMenuItem(
                    value: 'OTHER',
                    child: Text(AppLocalizations.of(context).genderOther)),
              ],
              onChanged: (value) => setState(() => _gender = value ?? _gender),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _bloodGroupController,
              decoration: InputDecoration(
                  labelText: AppLocalizations.of(context).profileBloodGroup),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _weightController,
              keyboardType:
                  const TextInputType.numberWithOptions(decimal: true),
              decoration: InputDecoration(
                  labelText: AppLocalizations.of(context).profileWeight),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _emergencyPhoneController,
              keyboardType: TextInputType.phone,
              decoration: InputDecoration(
                  labelText:
                      AppLocalizations.of(context).profileEmergencyPhone),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _emergencyRelationController,
              decoration: InputDecoration(
                  labelText:
                      AppLocalizations.of(context).profileEmergencyRelation),
            ),
            if (_error != null) ...[
              const SizedBox(height: 10),
              Text(_error!,
                  style: const TextStyle(color: PatientTokens.danger)),
            ],
            const SizedBox(height: 18),
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
                      : Text(AppLocalizations.of(context).profileSave),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
