import 'package:flutter/material.dart';
import '../theme/tokens.dart';

/// Presentation pieces for the signed-out and approval screens, measured
/// from the Doctor reference board (panel 1 "Login & Access" and panel 12
/// "Sign Up / Registration"). No behaviour lives here.

/// The AfyaMkononi heart mark.
class AfyaLogoMark extends StatelessWidget {
  const AfyaLogoMark({super.key, this.size = 68});
  final double size;
  @override
  Widget build(BuildContext context) =>
      Image.asset('assets/brand/afya_logo.png',
          width: size,
          height: size,
          filterQuality: FilterQuality.medium,
          semanticLabel: 'AfyaMkononi');
}

/// Wordmark colour sampled from the board: #001520 at its darkest, drawn as
/// a deep teal in the artwork.
const _wordmark = Color(0xFF0E4A4A);

/// Mark, "AfyaMkononi" and the "Doctor App" line (panel 1 header).
class AfyaBrandHeader extends StatelessWidget {
  const AfyaBrandHeader({super.key, this.product = 'Doctor App'});
  final String product;
  @override
  Widget build(BuildContext context) =>
      Column(mainAxisSize: MainAxisSize.min, children: [
        const AfyaLogoMark(),
        const SizedBox(height: 4),
        const Text('AfyaMkononi',
            style: TextStyle(
                fontSize: 29,
                height: 1.1,
                fontWeight: FontWeight.w800,
                letterSpacing: -0.4,
                color: _wordmark)),
        Text(product,
            style: const TextStyle(
                fontSize: 21,
                height: 1.25,
                fontWeight: FontWeight.w700,
                color: DoctorTokens.navy)),
      ]);
}

/// Rounded field with a leading icon (panel 1) and an optional red required
/// marker after the hint (panel 12).
class AfyaAuthField extends StatelessWidget {
  const AfyaAuthField(
      {super.key,
      required this.controller,
      required this.hint,
      this.icon,
      this.requiredMark = false,
      this.obscure = false,
      this.onToggleObscure,
      this.keyboardType,
      this.autofillHints,
      this.maxLength,
      this.textInputAction,
      this.onSubmitted});
  final TextEditingController controller;
  final String hint;
  final IconData? icon;
  final bool requiredMark, obscure;
  final VoidCallback? onToggleObscure;
  final TextInputType? keyboardType;
  final Iterable<String>? autofillHints;
  final int? maxLength;
  final TextInputAction? textInputAction;
  final ValueChanged<String>? onSubmitted;

  @override
  Widget build(BuildContext context) {
    OutlineInputBorder border(Color color, [double width = 1]) =>
        OutlineInputBorder(
            borderRadius: BorderRadius.circular(12),
            borderSide: BorderSide(color: color, width: width));
    return TextField(
        controller: controller,
        obscureText: obscure,
        keyboardType: keyboardType,
        autofillHints: autofillHints,
        maxLength: maxLength,
        textInputAction: textInputAction,
        onSubmitted: onSubmitted,
        style: const TextStyle(
            fontSize: 15,
            fontWeight: FontWeight.w500,
            color: DoctorTokens.fieldText),
        decoration: InputDecoration(
            counterText: '',
            filled: true,
            fillColor: Colors.white,
            isDense: true,
            contentPadding:
                const EdgeInsets.symmetric(horizontal: 14, vertical: 17),
            prefixIcon: icon == null
                ? null
                : Padding(
                    padding: const EdgeInsets.only(left: 16, right: 12),
                    child: Icon(icon, size: 22, color: DoctorTokens.fieldText)),
            prefixIconConstraints: const BoxConstraints(minWidth: 0),
            suffixIcon: onToggleObscure == null
                ? null
                : IconButton(
                    onPressed: onToggleObscure,
                    tooltip: obscure ? 'Show password' : 'Hide password',
                    icon: Icon(
                        obscure
                            ? Icons.visibility_outlined
                            : Icons.visibility_off_outlined,
                        size: 22,
                        color: DoctorTokens.fieldText)),
            label: requiredMark
                ? Text.rich(TextSpan(text: hint, children: const [
                    TextSpan(
                        text: ' *',
                        style: TextStyle(color: DoctorTokens.danger))
                  ]))
                : Text(hint),
            floatingLabelBehavior: FloatingLabelBehavior.never,
            labelStyle: const TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w500,
                color: DoctorTokens.slate),
            border: border(DoctorTokens.fieldBorder),
            enabledBorder: border(DoctorTokens.fieldBorder),
            focusedBorder: border(DoctorTokens.primary, 1.5)));
  }
}

/// Field with its label above it (panel 12, "Professional Details").
class AfyaLabeledField extends StatelessWidget {
  const AfyaLabeledField(
      {super.key,
      required this.label,
      required this.controller,
      this.requiredMark = true,
      this.keyboardType,
      this.maxLength,
      this.hint = ''});
  final String label, hint;
  final TextEditingController controller;
  final bool requiredMark;
  final TextInputType? keyboardType;
  final int? maxLength;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text.rich(TextSpan(
            text: label,
            style: const TextStyle(
                fontSize: 14,
                fontWeight: FontWeight.w700,
                color: DoctorTokens.navy),
            children: [
              if (requiredMark)
                const TextSpan(
                    text: ' *', style: TextStyle(color: DoctorTokens.danger))
            ])),
        const SizedBox(height: 6),
        AfyaAuthField(
            controller: controller,
            hint: hint,
            keyboardType: keyboardType,
            maxLength: maxLength)
      ]));
}

/// Full-width green button (panel 1 "Sign In": 54 dp tall, 14 dp radius).
class AfyaPrimaryButton extends StatelessWidget {
  const AfyaPrimaryButton(
      {super.key,
      required this.label,
      required this.onPressed,
      this.busy = false});
  final String label;
  final VoidCallback? onPressed;
  final bool busy;
  @override
  Widget build(BuildContext context) => SizedBox(
      width: double.infinity,
      height: 54,
      child: DecoratedBox(
          decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              boxShadow: const [
                BoxShadow(
                    color: Color(0x3305804F),
                    blurRadius: 10,
                    offset: Offset(0, 4))
              ]),
          child: FilledButton(
              onPressed: busy ? null : onPressed,
              style: FilledButton.styleFrom(
                  backgroundColor: DoctorTokens.primary,
                  disabledBackgroundColor:
                      DoctorTokens.primary.withValues(alpha: .55),
                  disabledForegroundColor: Colors.white,
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(14))),
              child: busy
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(
                          color: Colors.white, strokeWidth: 2))
                  : Text(label,
                      style: const TextStyle(
                          fontSize: 17, fontWeight: FontWeight.w800)))));
}

/// Pale information band with a shield (panel 1, the 2FA note).
class AfyaSecurityNote extends StatelessWidget {
  const AfyaSecurityNote(this.text, {super.key});
  final String text;
  @override
  Widget build(BuildContext context) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 13),
      decoration: BoxDecoration(
          color: DoctorTokens.noticeBg,
          borderRadius: BorderRadius.circular(12)),
      child: Row(children: [
        const Icon(Icons.verified_user_outlined,
            size: 28, color: DoctorTokens.primary),
        const SizedBox(width: 12),
        Expanded(
            child: Text(text,
                style: const TextStyle(
                    fontSize: 13.5,
                    height: 1.35,
                    fontWeight: FontWeight.w500,
                    color: DoctorTokens.slate)))
      ]));
}

/// Inline error under a form.
class AfyaFormError extends StatelessWidget {
  const AfyaFormError(this.message, {super.key});
  final String message;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(top: 14),
      child: Text(message,
          textAlign: TextAlign.center,
          style: const TextStyle(
              color: DoctorTokens.danger, fontWeight: FontWeight.w600)));
}

/// One line of the review checklist (panel 12, third phone).
class AfyaReviewStep {
  const AfyaReviewStep(this.label, {required this.done});
  final String label;
  final bool done;
}

/// "Account Under Review" and its rejected / unavailable variants. Used both
/// right after registration and as the signed-in gate for a doctor whose
/// registration is not verified. Text comes from the caller; this widget
/// decides nothing about status.
class AfyaReviewStatusView extends StatelessWidget {
  const AfyaReviewStatusView(
      {super.key,
      required this.title,
      required this.message,
      required this.steps,
      required this.primaryLabel,
      required this.onPrimary,
      this.icon = Icons.verified_user_outlined,
      this.iconColor = DoctorTokens.primary,
      this.details = const [],
      this.note,
      this.secondaryLabel,
      this.onSecondary});
  final String title, message, primaryLabel;
  final List<AfyaReviewStep> steps;
  final VoidCallback? onPrimary;
  final IconData icon;
  final Color iconColor;

  /// Small identifying lines (name, registration number).
  final List<String> details;

  /// The reviewer's note, when the server supplied one.
  final String? note;
  final String? secondaryLabel;
  final VoidCallback? onSecondary;

  @override
  Widget build(BuildContext context) => SafeArea(
      child: Center(
          child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(28, 24, 28, 24),
              child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 420),
                  child: Column(mainAxisSize: MainAxisSize.min, children: [
                    Icon(icon, size: 104, color: iconColor),
                    const SizedBox(height: 14),
                    Text(title,
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                            fontSize: 25,
                            fontWeight: FontWeight.w800,
                            color: DoctorTokens.navy)),
                    const SizedBox(height: 10),
                    Text(message,
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                            fontSize: 15,
                            height: 1.45,
                            color: DoctorTokens.slate)),
                    if (details.isNotEmpty) ...[
                      const SizedBox(height: 12),
                      for (final line in details)
                        Text(line,
                            textAlign: TextAlign.center,
                            style: const TextStyle(
                                fontSize: 13,
                                fontWeight: FontWeight.w600,
                                color: DoctorTokens.fieldText))
                    ],
                    if (steps.isNotEmpty) ...[
                      const SizedBox(height: 20),
                      Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 14, vertical: 6),
                          decoration: BoxDecoration(
                              color: Colors.white,
                              borderRadius: BorderRadius.circular(14),
                              border:
                                  Border.all(color: DoctorTokens.fieldBorder)),
                          child: Column(children: [
                            for (final step in steps)
                              Padding(
                                  padding:
                                      const EdgeInsets.symmetric(vertical: 9),
                                  child: Row(children: [
                                    const Icon(Icons.check_circle_outline,
                                        size: 22, color: DoctorTokens.primary),
                                    const SizedBox(width: 12),
                                    Expanded(
                                        child: Text(step.label,
                                            style: const TextStyle(
                                                fontSize: 15,
                                                fontWeight: FontWeight.w600,
                                                color: DoctorTokens.navy))),
                                    Icon(
                                        step.done
                                            ? Icons.check_circle
                                            : Icons.radio_button_unchecked,
                                        size: 22,
                                        color: step.done
                                            ? DoctorTokens.primary
                                            : DoctorTokens.textSub)
                                  ]))
                          ]))
                    ],
                    if (note != null) ...[
                      const SizedBox(height: 14),
                      Container(
                          width: double.infinity,
                          padding: const EdgeInsets.all(14),
                          decoration: BoxDecoration(
                              color: DoctorTokens.noticeBg,
                              borderRadius: BorderRadius.circular(12)),
                          child: Text(note!,
                              style: const TextStyle(
                                  fontSize: 14,
                                  height: 1.4,
                                  color: DoctorTokens.fieldText)))
                    ],
                    const SizedBox(height: 24),
                    AfyaPrimaryButton(
                        label: primaryLabel, onPressed: onPrimary),
                    if (secondaryLabel != null)
                      Padding(
                          padding: const EdgeInsets.only(top: 6),
                          child: TextButton(
                              onPressed: onSecondary,
                              child: Text(secondaryLabel!)))
                  ])))));
}

/// Shell for the short signed-out forms (password reset): back bar, logo,
/// centred heading and message, then the form.
class AfyaAuthFormPage extends StatelessWidget {
  const AfyaAuthFormPage(
      {super.key,
      required this.barTitle,
      required this.heading,
      required this.message,
      required this.children,
      this.showBar = true});
  final String barTitle, heading, message;
  final List<Widget> children;
  final bool showBar;
  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.screen,
      appBar: showBar ? AppBar(title: Text(barTitle)) : null,
      body: SafeArea(
          child: Center(
              child: SingleChildScrollView(
                  padding: const EdgeInsets.fromLTRB(28, 8, 28, 28),
                  child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 420),
                      child: Column(children: [
                        const AfyaLogoMark(size: 62),
                        const SizedBox(height: 10),
                        Text(heading,
                            textAlign: TextAlign.center,
                            style: const TextStyle(
                                fontSize: 23,
                                fontWeight: FontWeight.w800,
                                color: DoctorTokens.navy)),
                        const SizedBox(height: 6),
                        Text(message,
                            textAlign: TextAlign.center,
                            style: const TextStyle(
                                fontSize: 15,
                                height: 1.4,
                                color: DoctorTokens.slate)),
                        const SizedBox(height: 22),
                        ...children
                      ]))))));
}
