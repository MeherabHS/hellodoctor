import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../auth/auth_cubit.dart';

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
