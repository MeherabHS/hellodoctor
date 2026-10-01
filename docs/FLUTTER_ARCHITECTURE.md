# Phase 9 — Flutter Mobile Architecture & Component Design

> **Document Version:** 1.0.0  
> **Target SDK:** Flutter 3.24+ / Dart 3.5+  
> **Target Platforms:** iOS 14+, Android 8.0+ (API Level 26+), Responsive Tablet  
> **Target Audience:** Claude Code Autonomous Implementation Agent

---

## 1. Architectural Style & Layering

HelloDoctor follows **Feature-First Clean Architecture** with **BLoC (Business Logic Component)** for predictable state management.

```
lib/
├── app.dart                            # MaterialApp setup, themes, global blocs
├── core/                               # Cross-cutting infrastructure
│   ├── network/                        # Dio client, interceptors, auth token refresh
│   ├── persistence/                    # Secure storage, local database (Drift)
│   ├── theme/                          # Colors, typography (tokens.css translation)
│   ├── localization/                   # EasyLocalization (en, sw)
│   ├── errors/                         # Failures, exceptions, user-facing mappers
│   └── di/                             # GetIt service locator setup
│
└── features/                           # Feature modules
    ├── auth/                           # Login, OTP verification, session
    ├── home/                           # Patient dashboard, emergency banner
    ├── doctor_discovery/               # Modality filtering, doctor list
    ├── appointment_booking/            # Slot carousel, multi-Rx upload (1-5 photos)
    ├── waiting_room/                   # Countdown, pre-flight checks, Rx viewer
    ├── consultation/                   # WebRTC video feed, session telemetry
    ├── clinical_chat/                  # 24h follow-up chat, canned pills
    ├── health_vault/                   # Downloadable prescriptions, CBC reports
    ├── grievance_redressal/            # Star-rating-free dispute filing
    ├── doctor_queue/                   # Physician duty toggle, incoming triage
    └── doctor_wallet/                  # 3-tier 20% debarred earnings breakdown
```

---

## 2. Core Dependencies & Justification

| Package | Version | Purpose & Architectural Justification |
|---|---|---|
| `flutter_bloc` | ^8.1.6 | Predictable, event-driven state management separating UI from business logic. |
| `get_it` | ^7.7.0 | Fast, decoupling service locator for dependency injection. |
| `go_router` | ^14.2.0 | Declarative URL-based routing with deep linking and route guards. |
| `dio` | ^5.6.0 | Robust HTTP client supporting interceptors (JWT refresh, trace IDs, network retry). |
| `freezed` & `freezed_annotation` | ^2.5.7 | Immutable domain models, union types for states, and pattern matching. |
| `flutter_secure_storage` | ^9.2.2 | Encrypted storage for JWTs (Keychain on iOS, Keystore on Android). |
| `drift` | ^2.20.0 | Type-safe local SQLite database for offline caching and health records. |
| `connectivity_plus` | ^6.0.5 | Observes cellular/WiFi network states to trigger offline action guards. |
| `flutter_webrtc` | ^0.11.7 | Native WebRTC bindings for 720p/1080p teleconsultation audio/video. |
| `cached_network_image` | ^3.4.0 | Efficient bitmap caching for doctor portraits and medical photos. |
| `shimmer` | ^3.0.0 | Hardware-accelerated skeleton shimmer animations matching `view-14`. |
| `easy_localization` | ^3.0.7 | Runtime localization toggling for English and Swahili without app reboots. |

---

## 3. Layer Responsibilities

### 3.1 Presentation Layer (`presentation/`)
- **Pages:** Top-level route widgets (e.g., `PatientHomePage`, `BookingPage`).
- **Widgets:** Composable, reusable UI elements (e.g., `EmergencyHelplineStrip`, `DoctorCard`, `PrescriptionGalleryGrid`).
- **BLoC / Cubit:** Listens to user interactions, emits state transitions (`Initial`, `Loading`, `Loaded`, `Error`), and orchestrates use cases.

### 3.2 Domain Layer (`domain/`)
- **Entities:** Pure Dart data classes without JSON serialization logic (e.g., `Doctor`, `Appointment`, `Grievance`).
- **Repository Interfaces:** Abstract contracts (e.g., `DoctorRepository`, `AppointmentRepository`).
- **Failures:** Strongly typed domain failure objects (`NetworkFailure`, `SlotUnavailableFailure`, `MaxUploadExceededFailure`).

### 3.3 Data Layer (`data/`)
- **DTOs / Models:** Data transfer objects with `@freezed` and `fromJson`/`toJson`.
- **Data Sources:** 
  - `RemoteDataSource`: Direct HTTP calls via `Dio`.
  - `LocalDataSource`: Cache reads and offline writes via `Drift` / `SharedPreferences`.
- **Repository Implementations:** Implements domain contracts, handles caching policies, and maps raw exceptions into domain `Failure` instances.

---

## 4. Key Feature Implementations

### 4.1 Multi-Prescription Upload (1 to 5 Images)
```dart
class PrescriptionIntakeBloc extends Bloc<PrescriptionIntakeEvent, PrescriptionIntakeState> {
  static const int maxAllowedImages = 5;

  PrescriptionIntakeBloc(this._repository) : super(const PrescriptionIntakeState.initial()) {
    on<_FilesSelected>((event, emit) async {
      final currentList = List<File>.from(state.selectedFiles);
      if (currentList.length + event.newFiles.length > maxAllowedImages) {
        final allowedCount = maxAllowedImages - currentList.length;
        currentList.addAll(event.newFiles.take(allowedCount));
        emit(state.copyWith(
          selectedFiles: currentList,
          warningMessage: "Maximum 5 prescription photos allowed. Extra photos were omitted.",
        ));
      } else {
        currentList.addAll(event.newFiles);
        emit(state.copyWith(selectedFiles: currentList, warningMessage: null));
      }
    });

    on<_FileRemoved>((event, emit) {
      final currentList = List<File>.from(state.selectedFiles)..removeAt(event.index);
      emit(state.copyWith(selectedFiles: currentList));
    });
  }
}
```

### 4.2 Doctor Earnings 20% Debarment Card Widget
```dart
class DoctorEarningsHeroCard extends StatelessWidget {
  final double totalGross;
  final double platformFeeWithheld; // Exactly totalGross * 0.20
  final double finalNet;            // Exactly totalGross * 0.80

  const DoctorEarningsHeroCard({
    super.key,
    required this.totalGross,
    required this.platformFeeWithheld,
    required this.finalNet,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      elevation: 2,
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _buildRow("Total Earnings (Gross)", "৳ ${totalGross.toStringAsFixed(2)}"),
            const SizedBox(height: 8),
            _buildRow("HeloDoc Platform Charge (20% Withheld)", "- ৳ ${platformFeeWithheld.toStringAsFixed(2)}", isDeduction: true),
            const Divider(height: 24, thickness: 1),
            _buildRow("Final Earning (Net Take-Home)", "৳ ${finalNet.toStringAsFixed(2)}", isHighlight: true),
            const SizedBox(height: 12),
            const Text(
              "Total calculation is based including the platform charge 20%.",
              style: TextStyle(fontSize: 11, fontStyle: FontStyle.italic, color: Colors.grey),
            ),
          ],
        ),
      ),
    );
  }
}
```
