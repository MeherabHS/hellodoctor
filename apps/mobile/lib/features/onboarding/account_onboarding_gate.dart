import 'dart:async';

import 'package:flutter/material.dart';

import '../../core/network/api_client.dart';

/// Key of the "this account has finished or skipped onboarding on this
/// phone" flag. [kind] is `doctor` or `peer`; [account] identifies the
/// signed-in account. [SecureTokenStore.clear] keeps every key starting
/// with [keptOnSignOutPrefix], so signing out does not show onboarding again.
String onboardingSeenKey(String kind, String account) =>
    '$keptOnSignOutPrefix$kind:$account';

/// Shows [onboarding] instead of [child] the first time an approved account
/// opens the app on this phone (first sign-in, or first launch after the
/// account is approved). Until the account is known and its flag has been
/// read, [child] shows, so a returning user never sees a blank screen. A
/// storage error counts as "already seen".
class AccountOnboardingGate extends StatefulWidget {
  /// Whether an onboarding is on screen: null while the gate has not
  /// decided yet (profile or flag still loading). Prompts that would
  /// otherwise cover it (the battery-optimisation dialog) wait for
  /// [whenHidden].
  static final showing = ValueNotifier<bool?>(null);

  /// Completes once a gate has decided and no onboarding is on screen.
  static Future<void> whenHidden() {
    if (showing.value == false) return Future<void>.value();
    final done = Completer<void>();
    void listener() {
      if (showing.value == false && !done.isCompleted) {
        showing.removeListener(listener);
        done.complete();
      }
    }

    showing.addListener(listener);
    return done.future;
  }

  const AccountOnboardingGate(
      {super.key,
      required this.store,
      required this.kind,
      required this.account,
      required this.onboarding,
      required this.child});
  final TokenStore store;
  final String kind;

  /// Null until the account's profile has loaded.
  final String? account;
  final Widget Function(VoidCallback onDone) onboarding;
  final Widget child;
  @override
  State<AccountOnboardingGate> createState() => _AccountOnboardingGateState();
}

class _AccountOnboardingGateState extends State<AccountOnboardingGate> {
  bool? _seen;

  @override
  void initState() {
    super.initState();
    AccountOnboardingGate.showing.value = null;
    _check();
  }

  @override
  void didUpdateWidget(AccountOnboardingGate old) {
    super.didUpdateWidget(old);
    if (old.account != widget.account || old.kind != widget.kind) {
      _seen = null;
      _check();
    }
  }

  void _check() {
    final account = widget.account;
    if (account == null) return;
    widget.store.read(onboardingSeenKey(widget.kind, account)).then(
        (value) => _set(account, value != null),
        onError: (_) => _set(account, true));
  }

  void _set(String account, bool seen) {
    if (mounted && account == widget.account) {
      setState(() => _seen = seen);
      AccountOnboardingGate.showing.value = !seen;
    }
  }

  @override
  void dispose() {
    if (AccountOnboardingGate.showing.value != false) {
      AccountOnboardingGate.showing.value = false;
    }
    super.dispose();
  }

  Future<void> _finish() async {
    final account = widget.account;
    if (account == null) return;
    _set(account, true);
    try {
      await widget.store.write(onboardingSeenKey(widget.kind, account), '1');
    } catch (_) {/* shown again next launch at worst */}
  }

  @override
  Widget build(BuildContext context) =>
      _seen == false ? widget.onboarding(_finish) : widget.child;
}
