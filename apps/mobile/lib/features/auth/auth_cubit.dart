import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/network/api_client.dart';
import '../../core/cache/doctor_cache_store.dart';
import '../notifications/notification_repository.dart';

sealed class AuthState {
  const AuthState();
}

/// Shown only while a saved session is being silently restored on app start,
/// so the UI doesn't flash the login form before falling back to it.
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

class MfaRequired extends AuthState {
  const MfaRequired(this.challenge);
  final String challenge;
}

class SignedIn extends AuthState {
  const SignedIn();
}

class AuthFailure extends AuthState {
  const AuthFailure(this.message);
  final String message;
}

class RegistrationSubmitted extends AuthState {
  const RegistrationSubmitted(this.doctorId);
  final String doctorId;
}

/// Phase 2: Secure Email OTP Password Recovery — step 1 complete. The
/// backend never reveals whether [email] actually has an eligible account;
/// the UI must show the same "check your email" screen either way.
class PasswordResetOtpSent extends AuthState {
  const PasswordResetOtpSent({required this.email, required this.expiresInSeconds});
  final String email;
  final int expiresInSeconds;
}

/// Step 2 complete: the OTP was correct. The resulting bearer secret is
/// held only in the cubit's private field, never exposed on this state.
class PasswordResetVerified extends AuthState {
  const PasswordResetVerified();
}

/// Step 3 complete. Every existing session for the account was revoked by
/// the backend as part of completing the reset, so the only correct next
/// screen is an ordinary sign-in form, never an auto-login.
class PasswordResetCompleted extends AuthState {
  const PasswordResetCompleted();
}

class AuthCubit extends Cubit<AuthState> {
  AuthCubit(this.api, {this.cache}) : super(const SignedOut()) {
    api.onSessionExpired = () {
      _challenge = null;
      cache?.clear();
      if (!isClosed) emit(const SignedOut());
    };
  }
  final ApiClient api;
  final DoctorCacheStore? cache;
  String? _challenge;
  bool _busy = false;

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
    _challenge = null;
    emit(const SignedOut());
  }

  Future<void> login(String email, String password) async {
    if (_busy) return;
    if (email.trim().isEmpty || password.isEmpty) {
      emit(const AuthFailure('Enter your email address and password.'));
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      final response = await api.post('/auth/doctor/login',
          data: {'email': email.trim().toLowerCase(), 'password': password});
      final data = response.data['data'];
      _challenge = data['session_token'] as String;
      final debugOtp = data['debug_otp_code'] as String?;
      if (debugOtp != null &&
          developmentOtpEnabled(
              environment: const String.fromEnvironment('ENVIRONMENT'),
              flag: const bool.fromEnvironment('DEV_AUTH_OTP_ENABLED'))) {
        await _completeVerification(debugOtp);
      } else {
        emit(MfaRequired(_challenge!));
      }
    } on ApiException catch (e) {
      emit(AuthFailure(e.status == 401
          ? 'The email address or password is incorrect.'
          : e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to sign in. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  Future<void> registerDoctor({
    required String fullName,
    required String licenseNumber,
    required String phoneNumber,
    required String email,
    required String password,
    required String specialty,
    required String experienceYears,
    required String hospital,
    required String qualifications,
    required String address,
    required List<Map<String, String>> acceptedDocuments,
  }) async {
    if (_busy) return;
    if ([
      fullName,
      licenseNumber,
      phoneNumber,
      email,
      password,
      specialty,
      experienceYears,
      hospital,
      qualifications,
      address
    ].any((value) => value.trim().isEmpty)) {
      emit(const AuthFailure('Complete all required registration fields.'));
      return;
    }
    if (!email.contains('@') || !email.contains('.')) {
      emit(const AuthFailure('Enter a valid email address.'));
      return;
    }
    final years = int.tryParse(experienceYears.trim());
    if (years == null || years < 0 || years > 100) {
      emit(const AuthFailure(
          'Experience years must be a number from 0 to 100.'));
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      final response = await api.post('/auth/doctor/register',
          data: {
            'full_name': fullName.trim(),
            'license_number': licenseNumber.trim(),
            'phone_number': phoneNumber.trim(),
            'email': email.trim().toLowerCase(),
            'password': password,
            'primary_specialty': specialty.trim(),
            'experience_years': years,
            'current_hospital': hospital.trim(),
            'qualifications': qualifications
                .split(',')
                .map((value) => value.trim())
                .where((value) => value.isNotEmpty)
                .toList(),
            'residential_address': address.trim(),
            'accepted_documents': acceptedDocuments,
          },
          idempotent: true);
      final data = Map<String, dynamic>.from(response.data['data'] as Map);
      emit(RegistrationSubmitted(data['doctor_id'].toString()));
    } on ApiException catch (e) {
      emit(AuthFailure(e.status == 409
          ? 'Those registration details are already in use.'
          : e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to submit registration. Please retry.'));
    } finally {
      _busy = false;
    }
  }

  Future<void> verify(String otp) async {
    if (_busy || _challenge == null) return;
    if (otp.trim().isEmpty) {
      emit(const AuthFailure('Enter the verification code.'));
      return;
    }
    _busy = true;
    emit(const AuthBusy());
    try {
      await _completeVerification(otp.trim());
    } finally {
      _busy = false;
    }
  }

  // Shared by the manual OTP screen (verify) and the automatic dev-backend
  // path (login). Callers own _busy/AuthBusy around this.
  Future<void> _completeVerification(String otp) async {
    try {
      final response = await api.post('/auth/doctor/verify-otp',
          data: {'session_token': _challenge, 'otp_code': otp});
      final data = response.data['data'];
      await api.saveTokens(
          data['access_token'] as String, data['refresh_token'] as String);
      // A new interactive login is an explicit session/account boundary.
      await cache?.clear();
      await cache?.initializeScope(data['access_token'] as String);
      _challenge = null;
      emit(const SignedIn());
    } on ApiException catch (e) {
      emit(AuthFailure(e.status == 401
          ? 'The verification code is invalid or the challenge expired. Retry or sign in again.'
          : e.userMessage));
    } catch (_) {
      emit(const AuthFailure('Unable to verify this session. Please retry.'));
    }
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
      await api.post('/auth/password-reset/request', data: {'email': trimmed});
      _resetEmail = trimmed;
      // The request-reset response carries no expiry field (it never varies
      // by account existence) — 300s mirrors the backend's own
      // `PASSWORD_RESET_OTP_TTL_SECONDS` default, a countdown convenience
      // only, never relied on for correctness.
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
      final response = await api.post('/auth/password-reset/verify-otp',
          data: {'email': _resetEmail, 'otp_code': otpCode.trim()});
      final data = response.data['data'];
      _resetAuthorization = data['reset_authorization'] as String;
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
      await api.post('/auth/password-reset/complete', data: {
        'reset_authorization': _resetAuthorization,
        'new_password': newPassword,
      });
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
      final refresh = await api.read('refresh_token');
      if (refresh != null) {
        await api.post('/auth/logout', data: {'refresh_token': refresh});
      }
    } catch (_) {
      notice = 'Signed out on this device. Server revocation could not be '
          'confirmed. Connect to the service and sign in again.';
    } finally {
      await api.clearSession();
      await cache?.clear();
      _challenge = null;
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
