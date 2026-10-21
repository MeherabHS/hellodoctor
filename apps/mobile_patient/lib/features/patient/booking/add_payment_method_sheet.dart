// ignore_for_file: prefer_const_constructors, prefer_const_literals_to_create_immutables, use_build_context_synchronously
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/validation/kenya_phone.dart';
import '../../../l10n/app_localizations.dart';
import 'booking_cubit.dart';

/// Checkout's "+ Add M-Pesa Number" sheet — mirrors
/// `profile/add_dependent_sheet.dart`'s shape exactly (single bottom sheet,
/// `BlocProvider.value` of the caller's existing cubit, inline validation
/// before the repository call). [profileId] is threaded straight through to
/// `BookingCubit.addMpesaPaymentMethod` — whichever profile (self or
/// dependent) is active when this sheet opens.
Future<void> showAddPaymentMethodSheet(BuildContext context,
    {required String? profileId}) {
  final cubit = context.read<BookingCubit>();
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    builder: (sheetContext) => BlocProvider.value(
      value: cubit,
      child: _AddPaymentMethodSheet(profileId: profileId),
    ),
  );
}

class _AddPaymentMethodSheet extends StatefulWidget {
  const _AddPaymentMethodSheet({required this.profileId});
  final String? profileId;

  @override
  State<_AddPaymentMethodSheet> createState() => _AddPaymentMethodSheetState();
}

class _AddPaymentMethodSheetState extends State<_AddPaymentMethodSheet> {
  final _phoneController = TextEditingController();
  String? _localError;

  @override
  void dispose() {
    _phoneController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final l10n = AppLocalizations.of(context);
    setState(() => _localError = null);
    final normalized = normalizeKenyaPhone(_phoneController.text);
    if (normalized == null) {
      setState(() => _localError = l10n.bookingAddPaymentMethodInvalidPhone);
      return;
    }
    final cubit = context.read<BookingCubit>();
    final ok = await cubit.addMpesaPaymentMethod(normalized,
        profileId: widget.profileId);
    if (!mounted) return;
    if (ok) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
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
            Text(l10n.bookingAddPaymentMethodTitle,
                style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
            const SizedBox(height: 4),
            Text(l10n.bookingAddPaymentMethodDescription,
                style: const TextStyle(
                    fontSize: 12, color: PatientTokens.textSub)),
            const SizedBox(height: 14),
            TextField(
              controller: _phoneController,
              keyboardType: TextInputType.phone,
              decoration: InputDecoration(
                hintText: l10n.bookingAddPaymentMethodHint,
              ),
            ),
            if (_localError != null) ...[
              const SizedBox(height: 10),
              Text(_localError!,
                  style: const TextStyle(color: PatientTokens.danger)),
            ],
            BlocBuilder<BookingCubit, BookingState>(
              builder: (context, state) {
                if (state.addPaymentMethodError == null) {
                  return const SizedBox.shrink();
                }
                return Padding(
                  padding: const EdgeInsets.only(top: 10),
                  child: Text(state.addPaymentMethodError!,
                      style: const TextStyle(color: PatientTokens.danger)),
                );
              },
            ),
            const SizedBox(height: 18),
            BlocBuilder<BookingCubit, BookingState>(
              builder: (context, state) => SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: state.addingPaymentMethod ? null : _submit,
                  style: FilledButton.styleFrom(
                      backgroundColor: PatientTokens.primary,
                      padding: const EdgeInsets.symmetric(vertical: 14)),
                  child: state.addingPaymentMethod
                      ? const SizedBox(
                          height: 18,
                          width: 18,
                          child: CircularProgressIndicator(
                              strokeWidth: 2, color: Colors.white))
                      : Text(l10n.bookingAddPaymentMethodSave),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
