// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';
import '../../../l10n/app_localizations.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/validation/kenya_phone.dart';
import 'family_profiles_cubit.dart';

/// `prototype/index.html` view-7's "+ Add Dependent" link, made real. Fields
/// mirror `ProfileCompletionPage` (full name / DOB / gender) plus the
/// dependent-only `relationship` field required by
/// `POST /patients/me/profiles` (`DependentProfileCreate`,
/// `patient_handlers.rs`) — a free-text label ("Spouse", "Child", "Parent",
/// ...), not a backend-interpreted enum, so this is a plain text field
/// rather than a constrained dropdown.
Future<void> showAddDependentSheet(BuildContext context) {
  final cubit = context.read<FamilyProfilesCubit>();
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    builder: (sheetContext) => BlocProvider.value(
      value: cubit,
      child: const _AddDependentSheet(),
    ),
  );
}

class _AddDependentSheet extends StatefulWidget {
  const _AddDependentSheet();

  @override
  State<_AddDependentSheet> createState() => _AddDependentSheetState();
}

class _AddDependentSheetState extends State<_AddDependentSheet> {
  final _nameController = TextEditingController();
  final _dobController = TextEditingController();
  final _relationshipController = TextEditingController();
  final _bloodGroupController = TextEditingController();
  final _weightController = TextEditingController();
  final _emergencyPhoneController = TextEditingController();
  final _emergencyRelationController = TextEditingController();
  String _gender = 'OTHER';
  String? _error;

  @override
  void dispose() {
    _nameController.dispose();
    _dobController.dispose();
    _relationshipController.dispose();
    _bloodGroupController.dispose();
    _weightController.dispose();
    _emergencyPhoneController.dispose();
    _emergencyRelationController.dispose();
    super.dispose();
  }

  Future<void> _pickDate() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: DateTime.now().subtract(const Duration(days: 365 * 10)),
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
    if (_nameController.text.trim().isEmpty) {
      setState(
          () => _error = AppLocalizations.of(context).dependentNameRequired);
      return;
    }
    if (_dobController.text.trim().isEmpty) {
      setState(
          () => _error = AppLocalizations.of(context).dependentDobRequired);
      return;
    }
    if (_relationshipController.text.trim().isEmpty) {
      setState(() =>
          _error = AppLocalizations.of(context).dependentRelationshipRequired);
      return;
    }
    String? normalizedPhone;
    if (_emergencyPhoneController.text.trim().isNotEmpty) {
      normalizedPhone = normalizeKenyaPhone(_emergencyPhoneController.text);
      if (normalizedPhone == null) {
        setState(() =>
            _error = AppLocalizations.of(context).profileInvalidEmergencyPhone);
        return;
      }
    }
    final created = await context.read<FamilyProfilesCubit>().createDependent(
          fullName: _nameController.text.trim(),
          dateOfBirth: _dobController.text.trim(),
          gender: _gender,
          relationship: _relationshipController.text.trim(),
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
    if (created != null) {
      Navigator.of(context).pop();
    } else {
      setState(() => _error =
          context.read<FamilyProfilesCubit>().state.createError ??
              AppLocalizations.of(context).dependentAddError);
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
            Text(AppLocalizations.of(context).dependentAdd,
                style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
            const SizedBox(height: 4),
            Text(AppLocalizations.of(context).dependentDescription,
                style: TextStyle(fontSize: 12, color: PatientTokens.textSub)),
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
              controller: _relationshipController,
              decoration: InputDecoration(
                  labelText:
                      AppLocalizations.of(context).dependentRelationship),
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
            BlocBuilder<FamilyProfilesCubit, FamilyProfilesState>(
              builder: (context, state) => SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: state.creating ? null : _submit,
                  style: FilledButton.styleFrom(
                      backgroundColor: PatientTokens.primary,
                      padding: const EdgeInsets.symmetric(vertical: 14)),
                  child: state.creating
                      ? const SizedBox(
                          height: 18,
                          width: 18,
                          child: CircularProgressIndicator(
                              strokeWidth: 2, color: Colors.white))
                      : Text(AppLocalizations.of(context).dependentAdd),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
