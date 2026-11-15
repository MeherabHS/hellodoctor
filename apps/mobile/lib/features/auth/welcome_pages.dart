import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../app.dart';
import '../../core/theme/tokens.dart';
import '../../core/validation/kenya_phone.dart';
import '../../core/widgets/afya_auth_widgets.dart';
import '../../core/network/api_client.dart';
import '../peer/peer_repository.dart';
import 'auth_cubit.dart';

/// Signed-out entry screen and the Peer Navigator application form, built to
/// `docs/design/afyamkononi-doctor-welcome-reference.png` and
/// `docs/design/afyamkononi-peer-registration-reference.png`.

const _wordmark = Color(0xFF0E4A4A);
const _iconTile = Color(0xFFE9F7E9); // sampled from the welcome design

/// Mark, wordmark and the "Prevent · Test · Treat · Stay Healthy" line.
class AfyaBrandLockup extends StatelessWidget {
  const AfyaBrandLockup({super.key, this.scale = 1});
  final double scale;
  @override
  Widget build(BuildContext context) => FittedBox(
      fit: BoxFit.scaleDown,
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        AfyaLogoMark(size: 74 * scale),
        SizedBox(width: 8 * scale),
        Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('AfyaMkononi',
              style: TextStyle(
                  fontSize: 34 * scale,
                  height: 1.05,
                  fontWeight: FontWeight.w800,
                  letterSpacing: -0.5,
                  color: _wordmark)),
          Text('Prevent  ·  Test  ·  Treat  ·  Stay Healthy',
              style: TextStyle(
                  fontSize: 11.5 * scale,
                  fontWeight: FontWeight.w600,
                  color: _wordmark)),
        ])
      ]));
}

class _Wave extends StatelessWidget {
  const _Wave();
  @override
  Widget build(BuildContext context) =>
      Image.asset('assets/auth/wave_footer.jpg',
          width: double.infinity,
          height: 40,
          fit: BoxFit.fill,
          excludeFromSemantics: true);
}

class WelcomePage extends StatelessWidget {
  const WelcomePage({super.key, required this.auth});
  final AuthCubit auth;

  void _open(BuildContext context, Widget page) =>
      Navigator.push(context, MaterialPageRoute(builder: (_) => page));

  @override
  Widget build(BuildContext context) => Scaffold(
      backgroundColor: DoctorTokens.screen,
      body: SafeArea(
          bottom: false,
          child: LayoutBuilder(
              builder: (context, box) => SingleChildScrollView(
                  child: ConstrainedBox(
                      constraints: BoxConstraints(minHeight: box.maxHeight),
                      child: Center(
                          child: ConstrainedBox(
                              constraints: const BoxConstraints(maxWidth: 460),
                              child: Column(children: [
                                const SizedBox(height: 26),
                                const Padding(
                                    padding:
                                        EdgeInsets.symmetric(horizontal: 48),
                                    child: AfyaBrandLockup()),
                                const SizedBox(height: 30),
                                AspectRatio(
                                    aspectRatio: 827 / 520,
                                    child: Image.asset(
                                        'assets/auth/welcome_scene.jpg',
                                        fit: BoxFit.cover,
                                        excludeFromSemantics: true)),
                                const SizedBox(height: 8),
                                const Text('Welcome, Team',
                                    style: TextStyle(
                                        fontSize: 34,
                                        fontWeight: FontWeight.w800,
                                        letterSpacing: -0.5,
                                        color: DoctorTokens.navy)),
                                const SizedBox(height: 6),
                                const Padding(
                                    padding:
                                        EdgeInsets.symmetric(horizontal: 44),
                                    child: Text(
                                        'Join AfyaMkononi’s team to provide care, support and services to our patients.',
                                        textAlign: TextAlign.center,
                                        style: TextStyle(
                                            fontSize: 17,
                                            height: 1.3,
                                            color: DoctorTokens.slate))),
                                const SizedBox(height: 18),
                                Padding(
                                    padding: const EdgeInsets.symmetric(
                                        horizontal: 18),
                                    child: Column(children: [
                                      _RoleCard(
                                          icon: Icons.medical_services_outlined,
                                          title: 'Register as Doctor',
                                          subtitle:
                                              'Provide clinical consultations, treatment and follow-up care.',
                                          onTap: () => _open(context,
                                              RegistrationPage(auth: auth))),
                                      const SizedBox(height: 12),
                                      _RoleCard(
                                          icon: Icons.groups_outlined,
                                          title: 'Register as Peer Navigator',
                                          subtitle:
                                              'Offer guidance, support and linkage to care services.',
                                          onTap: () => _open(
                                              context,
                                              PeerRegistrationPage(
                                                  repo: PeerRepository(
                                                      auth.api)))),
                                    ])),
                                const SizedBox(height: 14),
                                const Padding(
                                    padding:
                                        EdgeInsets.symmetric(horizontal: 30),
                                    child: Divider(
                                        height: 1,
                                        color: DoctorTokens.fieldBorder)),
                                const SizedBox(height: 12),
                                const Text('Already have an account?',
                                    style: TextStyle(
                                        fontSize: 14,
                                        color: DoctorTokens.fieldText)),
                                const SizedBox(height: 10),
                                Padding(
                                    padding: const EdgeInsets.symmetric(
                                        horizontal: 23),
                                    child: AfyaPrimaryButton(
                                        label: 'Sign In',
                                        onPressed: () => _open(
                                            context, LoginPage(auth: auth)))),
                                BlocBuilder<AuthCubit, AuthState>(
                                    bloc: auth,
                                    builder: (_, s) => s is SignedOut &&
                                            s.notice != null
                                        ? Padding(
                                            padding: const EdgeInsets.symmetric(
                                                horizontal: 24),
                                            child: AfyaFormError(s.notice!))
                                        : const SizedBox.shrink()),
                                const SizedBox(height: 14),
                                const _Wave(),
                              ]))))))));
}

class _RoleCard extends StatelessWidget {
  const _RoleCard(
      {required this.icon,
      required this.title,
      required this.subtitle,
      required this.onTap});
  final IconData icon;
  final String title, subtitle;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Material(
      color: Colors.white,
      shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: const BorderSide(color: DoctorTokens.fieldBorder)),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
          onTap: onTap,
          child: Padding(
              padding: const EdgeInsets.fromLTRB(11, 10, 10, 10),
              child: Row(children: [
                Container(
                    width: 58,
                    height: 58,
                    decoration: BoxDecoration(
                        color: _iconTile,
                        borderRadius: BorderRadius.circular(13)),
                    child: Icon(icon, size: 32, color: DoctorTokens.primary)),
                const SizedBox(width: 20),
                Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      Text(title,
                          style: const TextStyle(
                              fontSize: 17.5,
                              fontWeight: FontWeight.w800,
                              color: DoctorTokens.navy)),
                      const SizedBox(height: 3),
                      Text(subtitle,
                          style: const TextStyle(
                              fontSize: 14,
                              height: 1.3,
                              color: DoctorTokens.fieldText)),
                    ])),
                const Icon(Icons.chevron_right,
                    size: 30, color: DoctorTokens.primary)
              ]))));
}

/// Kenya's 47 counties, used as the required location list. Whether location
/// should be finer than county is an open product decision.
const kenyaCounties = [
  'Baringo',
  'Bomet',
  'Bungoma',
  'Busia',
  'Elgeyo-Marakwet',
  'Embu',
  'Garissa',
  'Homa Bay',
  'Isiolo',
  'Kajiado',
  'Kakamega',
  'Kericho',
  'Kiambu',
  'Kilifi',
  'Kirinyaga',
  'Kisii',
  'Kisumu',
  'Kitui',
  'Kwale',
  'Laikipia',
  'Lamu',
  'Machakos',
  'Makueni',
  'Mandera',
  'Marsabit',
  'Meru',
  'Migori',
  'Mombasa',
  'Murang’a',
  'Nairobi',
  'Nakuru',
  'Nandi',
  'Narok',
  'Nyamira',
  'Nyandarua',
  'Nyeri',
  'Samburu',
  'Siaya',
  'Taita-Taveta',
  'Tana River',
  'Tharaka-Nithi',
  'Trans Nzoia',
  'Turkana',
  'Uasin Gishu',
  'Vihiga',
  'Wajir',
  'West Pokot',
];

/// The Peer Navigator application form.
///
/// There is no Peer Navigator role, registration endpoint or admin review on
/// the server yet (see docs/doctor-ui/PEER_NAVIGATOR_ROLE_AND_API_GAP_ANALYSIS.md).
/// This screen therefore checks the form and then says plainly that
/// applications are not open. It sends nothing and creates no account. It
/// asks for no KMPDC number, licence, specialty or any other clinical
/// credential, by design.
class PeerRegistrationPage extends StatefulWidget {
  const PeerRegistrationPage({super.key, required this.repo});

  /// Peer sign-up and the three Peer documents (Terms, Privacy Notice,
  /// Confidentiality, Conduct & Safeguarding Agreement).
  final PeerRepository repo;
  @override
  State<PeerRegistrationPage> createState() => _PeerRegistrationState();
}

class _PeerRegistrationState extends State<PeerRegistrationPage> {
  final name = TextEditingController(),
      email = TextEditingController(),
      phone = TextEditingController(),
      password = TextEditingController(),
      confirm = TextEditingController(),
      facility = TextEditingController();
  String? location, error;
  bool agreed = false, hide = true, hideConfirm = true, checked = false;
  bool busy = false;
  List<Map<String, dynamic>>? documents;

  @override
  void initState() {
    super.initState();
    widget.repo.peerDocuments().then((docs) {
      if (mounted) setState(() => documents = docs);
    }).catchError((Object _) {
      if (mounted) setState(() => documents = const []);
    });
  }

  void _readDocument(Map<String, dynamic> doc) => showDialog<void>(
      context: context,
      builder: (c) => AlertDialog(
              title: Text(doc['title']?.toString() ?? 'Document'),
              content: SingleChildScrollView(
                  child: Text(doc['body']?.toString() ?? '')),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(c),
                    child: const Text('Close'))
              ]));

  @override
  void dispose() {
    for (final c in [name, email, phone, password, confirm, facility]) {
      c.dispose();
    }
    super.dispose();
  }

  String? _validate() {
    if (name.text.trim().isEmpty) return 'Enter your full name.';
    final mail = email.text.trim();
    if (!mail.contains('@') || !mail.contains('.')) {
      return 'Enter a valid email address.';
    }
    if (normalizeKenyaPhone(phone.text) == null) {
      return 'Enter a Kenyan phone number, e.g. 0712 345 678.';
    }
    final p = password.text;
    if (p.length < 8 ||
        !p.contains(RegExp(r'[A-Z]')) ||
        !p.contains(RegExp(r'[a-z]')) ||
        !p.contains(RegExp(r'[0-9]')) ||
        !p.contains(RegExp(r'[^A-Za-z0-9]'))) {
      return 'Password needs 8+ characters with upper, lower, number and symbol.';
    }
    if (p != confirm.text) return 'Passwords do not match.';
    if (location == null) return 'Select your location.';
    if (!agreed) {
      return 'Read and accept the Peer Navigator documents to continue.';
    }
    return null;
  }

  Future<void> _submit() async {
    final problem = _validate() ??
        ((documents ?? const []).length < 3
            ? 'The Peer Navigator documents could not be loaded. Check your connection and try again.'
            : null);
    setState(() => error = problem);
    if (problem != null || busy) return;
    setState(() => busy = true);
    try {
      await widget.repo.register(
          fullName: name.text.trim(),
          email: email.text.trim().toLowerCase(),
          phone: normalizeKenyaPhone(phone.text)!,
          password: password.text,
          county: location!,
          organization: facility.text,
          acceptedDocuments: documents!);
      if (mounted) setState(() => checked = true);
    } on ApiException catch (e) {
      if (mounted) setState(() => error = e.userMessage);
    } catch (_) {
      if (mounted) {
        setState(() => error = 'Unable to register right now. Please retry.');
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Widget _label(String text, {String? suffix, bool required = false}) =>
      Padding(
          padding: const EdgeInsets.fromLTRB(8, 12, 0, 6),
          child: Text.rich(TextSpan(
              text: text,
              style: const TextStyle(
                  fontSize: 14.5,
                  fontWeight: FontWeight.w800,
                  color: DoctorTokens.navy),
              children: [
                if (required)
                  const TextSpan(
                      text: ' *',
                      style: TextStyle(color: DoctorTokens.primary)),
                if (suffix != null)
                  TextSpan(
                      text: ' $suffix',
                      style: const TextStyle(
                          fontWeight: FontWeight.w500,
                          color: DoctorTokens.fieldText))
              ])));

  @override
  Widget build(BuildContext context) {
    if (checked) {
      return Scaffold(
          backgroundColor: DoctorTokens.screen,
          body: AfyaReviewStatusView(
              icon: Icons.groups_outlined,
              title: 'Application submitted',
              message:
                  'Thank you. An AfyaMkononi Admin will review your Peer Navigator application. Sign in with your email and password to check its status.',
              steps: const [],
              primaryLabel: 'Back',
              onPrimary: () => Navigator.pop(context)));
    }
    return Scaffold(
        backgroundColor: DoctorTokens.screen,
        body: SafeArea(
            bottom: false,
            child: SingleChildScrollView(
                child: Center(
                    child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 460),
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              SizedBox(
                                  height: 56,
                                  child: Stack(
                                      alignment: Alignment.center,
                                      children: [
                                        Align(
                                            alignment: Alignment.centerLeft,
                                            child: IconButton(
                                                onPressed: () =>
                                                    Navigator.pop(context),
                                                tooltip: 'Back',
                                                icon: const Icon(
                                                    Icons.arrow_back,
                                                    color: DoctorTokens.navy))),
                                        const Padding(
                                            padding: EdgeInsets.symmetric(
                                                horizontal: 60),
                                            child: AfyaBrandLockup(scale: .58)),
                                      ])),
                              AspectRatio(
                                  aspectRatio: 827 / 377,
                                  child: Image.asset(
                                      'assets/auth/peer_scene.jpg',
                                      fit: BoxFit.cover,
                                      excludeFromSemantics: true)),
                              const SizedBox(height: 6),
                              const Text('Register as Peer Navigator',
                                  textAlign: TextAlign.center,
                                  style: TextStyle(
                                      fontSize: 24,
                                      fontWeight: FontWeight.w800,
                                      letterSpacing: -0.3,
                                      color: DoctorTokens.navy)),
                              const SizedBox(height: 6),
                              const Padding(
                                  padding: EdgeInsets.symmetric(horizontal: 34),
                                  child: Text(
                                      'Join our support team to offer guidance, encourage care, and link patients to health services.',
                                      textAlign: TextAlign.center,
                                      style: TextStyle(
                                          fontSize: 15,
                                          height: 1.35,
                                          color: DoctorTokens.slate))),
                              Padding(
                                  padding:
                                      const EdgeInsets.fromLTRB(18, 14, 18, 0),
                                  child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.stretch,
                                      children: [
                                        AfyaAuthField(
                                            controller: name,
                                            hint: 'Full name',
                                            icon: Icons.person_outline,
                                            maxLength: 150),
                                        const SizedBox(height: 10),
                                        AfyaAuthField(
                                            controller: email,
                                            hint: 'Email address',
                                            icon: Icons.mail_outline,
                                            keyboardType:
                                                TextInputType.emailAddress,
                                            maxLength: 255),
                                        const SizedBox(height: 10),
                                        AfyaAuthField(
                                            controller: phone,
                                            hint: 'Phone number',
                                            icon: Icons.phone_outlined,
                                            keyboardType: TextInputType.phone,
                                            maxLength: 20),
                                        const SizedBox(height: 10),
                                        AfyaAuthField(
                                            controller: password,
                                            hint: 'Password',
                                            icon: Icons.lock_outline,
                                            obscure: hide,
                                            onToggleObscure: () =>
                                                setState(() => hide = !hide)),
                                        const SizedBox(height: 10),
                                        AfyaAuthField(
                                            controller: confirm,
                                            hint: 'Confirm password',
                                            icon: Icons.lock_outline,
                                            obscure: hideConfirm,
                                            onToggleObscure: () => setState(
                                                () => hideConfirm =
                                                    !hideConfirm)),
                                        _label('Location', required: true),
                                        DropdownButtonFormField<String>(
                                            initialValue: location,
                                            isExpanded: true,
                                            icon: const Icon(Icons.keyboard_arrow_down,
                                                color: DoctorTokens.navy),
                                            hint: const Text(
                                                'Select your location (required)',
                                                style: TextStyle(
                                                    fontSize: 15,
                                                    color: DoctorTokens.slate)),
                                            decoration: InputDecoration(
                                                isDense: true,
                                                filled: true,
                                                fillColor: Colors.white,
                                                contentPadding: const EdgeInsets.fromLTRB(
                                                    0, 15, 12, 15),
                                                prefixIcon: const Padding(
                                                    padding: EdgeInsets.only(
                                                        left: 16, right: 12),
                                                    child: Icon(Icons.location_on_outlined,
                                                        size: 22,
                                                        color: DoctorTokens
                                                            .fieldText)),
                                                prefixIconConstraints:
                                                    const BoxConstraints(minWidth: 0),
                                                enabledBorder: OutlineInputBorder(borderRadius: BorderRadius.circular(12), borderSide: const BorderSide(color: DoctorTokens.fieldBorder))),
                                            items: [
                                              for (final county
                                                  in kenyaCounties)
                                                DropdownMenuItem(
                                                    value: county,
                                                    child: Text(county))
                                            ],
                                            onChanged: (v) => setState(() => location = v)),
                                        _label('Facility / Organization',
                                            suffix: '(Optional)'),
                                        AfyaAuthField(
                                            controller: facility,
                                            hint:
                                                'Facility or organization name',
                                            icon: Icons.apartment_outlined,
                                            maxLength: 150),
                                        const SizedBox(height: 12),
                                        const Text(
                                            'Peer Navigator documents (required)',
                                            style: TextStyle(
                                                fontWeight: FontWeight.w800,
                                                color: DoctorTokens.navy)),
                                        if (documents == null)
                                          const Padding(
                                              padding: EdgeInsets.all(8),
                                              child: LinearProgressIndicator())
                                        else
                                          for (final doc in documents!)
                                            TextButton.icon(
                                                style: TextButton.styleFrom(
                                                    alignment:
                                                        Alignment.centerLeft),
                                                onPressed: () =>
                                                    _readDocument(doc),
                                                icon: const Icon(
                                                    Icons.description_outlined,
                                                    size: 18),
                                                label: Text(
                                                    doc['title']?.toString() ??
                                                        '')),
                                        Row(children: [
                                          Checkbox(
                                              value: agreed,
                                              activeColor: DoctorTokens.primary,
                                              onChanged: (v) => setState(
                                                  () => agreed = v ?? false)),
                                          const Expanded(
                                              child: Text.rich(
                                                  TextSpan(
                                                      text:
                                                          'I have read and agree to the Peer Navigator Terms, Privacy Notice and Confidentiality, Conduct & Safeguarding Agreement.'),
                                                  style: TextStyle(
                                                      fontSize: 14.5,
                                                      color: DoctorTokens
                                                          .fieldText)))
                                        ]),
                                        const SizedBox(height: 8),
                                        AfyaPrimaryButton(
                                            label: 'Register as Peer Navigator',
                                            busy: busy,
                                            onPressed: busy ? null : _submit),
                                        if (error != null)
                                          AfyaFormError(error!),
                                      ])),
                              const SizedBox(height: 16),
                              const _Wave(),
                            ]))))));
  }
}
