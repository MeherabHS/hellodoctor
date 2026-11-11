import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/network/api_client.dart';
import '../../core/cache/patient_cache_store.dart';
import '../../core/validation/kenya_phone.dart';
import '../../core/validation/otp_validation.dart';
import '../notifications/notification_repository.dart';
import 'auth_repository.dart';

sealed class AuthState {
  const AuthState();
}

/// Shown only while a saved session is being silently restored on app start,
/// so the UI doesn't flash the phone-entry screen before falling back to it.
class SessionRestoring extends AuthState {
  const SessionRestoring();
}

class SignedOut extends AuthState {
  const SignedOut({this.notice});
  final String? notice;
}

class AuthBusy extends AuthState {
  const AuthBusy();
}

class EmailChallengeSent extends AuthState {
  const EmailChallengeSent(
      {required this.challengeToken,
      required this.expiresInSeconds,
      required this.registration,
      this.debugOtpCode});
  final String challengeToken;
  final int expiresInSeconds;
  final bool registration;
  final String? debugOtpCode;
}

/// An OTP was requested for [phoneNumber] (already normalized to
/// `+254XXXXXXXXX`) and the app is now waiting for the 6-digit code. Carries
/// [expiresInSeconds] for a resend countdown, and [debugOtpCode] — present
/// only against a `development` backend — so the OTP entry screen can show
/// it behind an explicit "DEV" marker.
class OtpSent extends AuthState {
  const OtpSent({
    required this.phoneNumber,
    required this.expiresInSeconds,
    this.debugOtpCode,
  });
  final String phoneNumber;
  final int expiresInSeconds;
  final String? debugOtpCode;
}

class SignedIn extends AuthState {
  const SignedIn();
}

class AuthFailure extends AuthState {
  const AuthFailure(this.message);
  final String message;
}

/// Phase 2: Secure Email OTP Password Recovery — step 1 complete. The
/// backend never reveals whether [email] actually has an eligible account;
/// the UI must show the same "check your email" screen either way.
class PasswordResetOtpSent extends AuthState {
  const PasswordResetOtpSent({required this.email, required this.expiresInSeconds});
  final String email;
  final int expiresInSeconds;
}

/// Step 2 complete: the OTP was correct. [resetAuthorization] is held only
/// in the cubit's private field, never exposed on this state, so no widget
/// can accidentally log or display it.
class PasswordResetVerified extends AuthState {
  const PasswordResetVerified();
}

/// Step 3 complete. Deliberately carries no session — every existing
/// session for the account was revoked by the backend as part of
/// completing the reset, so the only correct next screen is an ordinary
/// sign-in form, never an auto-login.
class PasswordResetCompleted extends AuthState {
  const PasswordResetCompleted();
}

class AuthCubit extends Cubit<AuthState> {
  AuthCubit(this.api, {this.cache, AuthRepository? repository})
      : repository = repository ?? AuthRepository(api),
        super(const SignedOut()) {
    api.onSessionExpired = () {
      _phoneNumber = null;
      cache?.clear();
      if (!isClosed) emit(const SignedOut());
    };
  }
  final ApiClient api;
  final AuthRepository repository;
  final PatientCacheStore? cache;
  String? _phoneNumber;
  bool _busy = false;

  Future<void> requestPatientLogin(String email, String password) async {
    if (_busy) return;
    _busy = true;
    emit(const AuthBusy());
    try {
      final result = await repository.requestPatientLogin(
          email: email.trim().toLowerCase(), password: password);
      emit(EmailChallengeSent(
          challengeToken: result.challengeToken,
          expiresInSeconds: result.expiresInSeconds,
          registration: false,
          debugOtpCode: developmentOtpEnabled(
                  environment: const String.fromEnvironment('ENVIRONMENT'),
                  flag: const bool.fromEnvironment('DEV_AUTH_OTP_ENABLED'))
              ? result.debugOtpCode
              : null));
    } on ApiException catch (e) {
      emit(AuthFailure(_authError(e, registration: false)));
    } catch (_) {
      emit(const AuthFailure('Unable to start sign in. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  Future<void> requestPatientRegistration(
      String email, String phone, String password,
      {required String fullName,
      required String dateOfBirth,
      required String gender}) async {
    if (_busy) return;
    _busy = true;
    emit(const AuthBusy());
    try {
      final normalized = normalizeKenyaPhone(phone);
      if (normalized == null) {
        emit(const AuthFailure('Enter a valid Kenyan phone number.'));
        return;
      }
      final result = await repository.requestPatientRegistration(
          email: email.trim().toLowerCase(),
          phoneNumber: normalized,
          password: password,
          fullName: fullName,
          dateOfBirth: dateOfBirth,
          gender: gender);
      emit(EmailChallengeSent(
          challengeToken: result.challengeToken,
          expiresInSeconds: result.expiresInSeconds,
          registration: true,
          debugOtpCode: developmentOtpEnabled(
                  environment: const String.fromEnvironment('ENVIRONMENT'),
                  flag: const bool.fromEnvironment('DEV_AUTH_OTP_ENABLED'))
              ? result.debugOtpCode
              : null));
    } on ApiException catch (e) {
      emit(AuthFailure(_authError(e, registration: true)));
    } catch (_) {
      emit(const AuthFailure('Unable to start registration. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  Future<void> verifyPatientEmailOtp(String otp,
      {required String challengeToken,
      List<Map<String, String>> acceptedDocuments = const []}) async {
    if (_busy || otp.length != 6 || int.tryParse(otp) == null) {
      if (!_busy) {
        emit(const AuthFailure('Enter the 6-digit verification code.'));
      }
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      await repository.verifyPatientEmailOtp(
          challengeToken: challengeToken,
          otpCode: otp.trim(),
          acceptedDocuments: acceptedDocuments);
      await cache?.clear();
      final access = await api.read('access_token');
      if (access != null) await cache?.initializeScope(access);
      emit(const SignedIn());
    } on ApiException catch (e) {
      emit(AuthFailure(_authError(e, otp: true)));
    } catch (_) {
      emit(const AuthFailure('Unable to verify this code. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  String _authError(ApiException e,
      {bool registration = false, bool otp = false}) {
    if (e.status == 401) {
      return otp
          ? 'The verification code is invalid or expired.'
          : 'Email or password is incorrect.';
    }
    if (e.status == 422) {
      return e.message.isNotEmpty
          ? e.message
          : 'Please check the entered information.';
    }
    return e.userMessage;
  }

  Future<void> restore() async {
    if (_busy) return;
    _busy = true;
    // Emitted synchronously, before the first `await` below, so the very
    // first frame after app start already reflects "restoring" rather than
    // the default SignedOut() the cubit was seeded with.
    emit(const SessionRestoring());
    try {
      final refreshBeforeRestore =
          await api.read('refresh_token').timeout(const Duration(seconds: 4));
      if (refreshBeforeRestore == null) {
        await cache?.clear();
        emit(const SignedOut());
        return;
      }
      final accessBeforeRestore = await api.read('access_token');
      await cache?.initializeScope(accessBeforeRestore ?? refreshBeforeRestore);
      await api.refreshSession().timeout(const Duration(seconds: 8));
      final access =
          await api.read('access_token').timeout(const Duration(seconds: 4));
      final refresh =
          await api.read('refresh_token').timeout(const Duration(seconds: 4));
      if (access != null || refresh != null) {
        await cache?.initializeScope(access ?? refresh!);
      }
      emit(const SignedIn());
    } on ApiException catch (e) {
      if (e.status == null) {
        emit(const SignedIn());
      } else {
        await cache?.clear();
        emit(const SignedOut());
      }
    } catch (_) {
      emit(const SignedOut());
    } finally {
      _busy = false;
    }
  }

  void restartLogin() {
    if (_busy) return;
    _phoneNumber = null;
    emit(const SignedOut());
  }

  // ---------------- Phase 2: Secure Email OTP Password Recovery ----------------

  String? _resetEmail;
  String? _resetAuthorization;

  Future<void> requestPasswordReset(String email) async {
    if (_busy) return;
    final trimmed = email.trim().toLowerCase();
    if (trimmed.isEmpty || !trimmed.contains('@')) {
      emit(const AuthFailure('Enter a valid email address.'));
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      await repository.requestPasswordReset(email: trimmed);
      _resetEmail = trimmed;
      // The backend's request-reset response carries no expiry field (it
      // never varies by account existence, so there is nothing
      // account-specific to report back) — 300s mirrors the backend's own
      // `PASSWORD_RESET_OTP_TTL_SECONDS` default, shown only as a countdown
      // convenience, never relied on for correctness.
      emit(PasswordResetOtpSent(email: trimmed, expiresInSeconds: 300));
    } on ApiException catch (e) {
      emit(AuthFailure(e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to request a password reset. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  Future<void> verifyPasswordResetOtp(String otpCode) async {
    if (_busy || _resetEmail == null) return;
    if (otpCode.trim().length != 6 || int.tryParse(otpCode.trim()) == null) {
      emit(const AuthFailure('Enter the 6-digit verification code.'));
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      final result = await repository.verifyPasswordResetOtp(
          email: _resetEmail!, otpCode: otpCode.trim());
      _resetAuthorization = result.resetAuthorization;
      emit(const PasswordResetVerified());
    } on ApiException catch (e) {
      emit(AuthFailure(e.status == 401
          ? 'The verification code is invalid or has expired.'
          : e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to verify this code. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  Future<void> completePasswordReset(String newPassword) async {
    if (_busy || _resetAuthorization == null) return;
    _busy = true;
    emit(const AuthBusy());
    try {
      await repository.completePasswordReset(
          resetAuthorization: _resetAuthorization!, newPassword: newPassword);
      _resetAuthorization = null;
      _resetEmail = null;
      emit(const PasswordResetCompleted());
    } on ApiException catch (e) {
      emit(AuthFailure(e.status == 422
          ? e.message.isNotEmpty ? e.message : 'Choose a stronger password.'
          : e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to reset the password. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  /// Abandons an in-progress reset (e.g. the user backs out) and returns to
  /// the ordinary sign-in screen.
  void restartPasswordReset() {
    if (_busy) return;
    _resetEmail = null;
    _resetAuthorization = null;
    emit(const SignedOut());
  }

  /// Step 1: `POST /auth/register-otp`. [rawPhone] is normalized via
  /// [normalizeKenyaPhone] client-side first — a structurally invalid number
  /// never reaches the network.
  Future<void> requestOtp(String rawPhone) async {
    if (_busy) return;
    final phone = normalizeKenyaPhone(rawPhone);
    if (phone == null) {
      emit(const AuthFailure('Enter a valid Kenyan phone number.'));
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      final result = await repository.requestOtp(phone);
      _phoneNumber = phone;
      emit(OtpSent(
        phoneNumber: phone,
        expiresInSeconds: result.expiresInSeconds,
        debugOtpCode: developmentOtpEnabled(
          environment: const String.fromEnvironment('ENVIRONMENT'),
          flag: const bool.fromEnvironment('DEV_AUTH_OTP_ENABLED'),
        )
            ? result.debugOtpCode
            : null,
      ));
    } on ApiException catch (e) {
      emit(AuthFailure(e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to request a verification code. '
          'Please retry.'));
    } finally {
      _busy = false;
    }
  }

  /// Step 2: `POST /auth/verify-otp-and-login`. Only callable after
  /// [requestOtp] has populated the phone number this code is being verified
  /// against — the backend contract never reuses a `session_token` here.
  Future<void> verifyOtp(String otp,
      {List<Map<String, String>> acceptedDocuments = const []}) async {
    if (_busy || _phoneNumber == null) return;
    if (!isValidOtpFormat(otp)) {
      emit(const AuthFailure('Enter the 6-digit verification code.'));
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      // The returned `PatientUser` is intentionally not kept beyond this
      // call — the cubit's `SignedIn` state carries no payload (matching the
      // Doctor app's own convention), since the only durable source of
      // patient profile data is the backend profile endpoint, not this
      // one-shot login response.
      await repository.verifyOtp(
          phoneNumber: _phoneNumber!,
          otpCode: otp.trim(),
          acceptedDocuments: acceptedDocuments);
      // A new interactive login is an explicit session/account boundary.
      await cache?.clear();
      final accessToken = await api.read('access_token');
      if (accessToken != null) await cache?.initializeScope(accessToken);
      _phoneNumber = null;
      emit(const SignedIn());
    } on ApiException catch (e) {
      emit(AuthFailure(e.status == 401
          ? 'The verification code is invalid or has expired. Request a new '
              'code or try again.'
          : e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to verify this code. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  Future<void> logout() async {
    if (_busy) return;
    _busy = true;
    String? notice;
    try {
      // Best-effort: a signed-out device must stop receiving push for this
      // account. Already reported through ClientErrorReporter inside
      // NotificationRepository.revokeDevice on failure, so swallowed here
      // rather than surfaced as the (unrelated) server-logout notice below.
      try {
        final notificationRepo = NotificationRepository(api);
        await notificationRepo.revokeDevice(
            deviceId: await notificationRepo.deviceId());
      } catch (_) {}
      await repository.logout();
    } catch (_) {
      notice = 'Signed out on this device. Server revocation could not be '
          'confirmed. Connect to the service and sign in again.';
    } finally {
      await api.clearSession();
      await cache?.clear();
      _phoneNumber = null;
      emit(SignedOut(notice: notice));
      _busy = false;
    }
  }

  @override
  Future<void> close() {
    api.onSessionExpired = null;
    return super.close();
  }
}
