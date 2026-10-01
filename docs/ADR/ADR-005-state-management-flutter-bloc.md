# ADR-005: State Management Strategy in Flutter (BLoC Pattern)

> **Status:** APPROVED  
> **Date:** 2026-10-01  
> **Deciders:** Principal Systems Architect, Mobile Lead

---

## 1. Context
HelloDoctor mobile applications feature complex asynchronous state transitions: multi-prescription image uploads with live preview capping, real-time waiting room countdown timers, WebRTC call state changes, and live 20% platform charge wallet arithmetic. An unprincipled state management choice risks race conditions and UI desynchronization.

## 2. Decision
We choose **`flutter_bloc`** (Bloc & Cubit) as the standard state management pattern across the Flutter mobile application.

## 3. Alternatives Considered
1. **Provider / ChangeNotifier:** Simple, but mutable state can lead to unpredictable rebuilds and untraceable state mutations in complex asynchronous flows.
2. **Riverpod:** Highly capable, but less prescriptive structure can lead to inconsistent state patterns across different developer contributions.
3. **GetX:** Combines routing, state, and dependency injection in an unconventional manner that bypasses Flutter's widget tree conventions and complicates unit testing.

## 4. Rationale
- **Predictable Event-to-State Mapping:** BLoC enforces unidirectional data flow: UI dispatches events -> BLoC processes business logic -> BLoC emits immutable states -> UI rebuilds via `BlocBuilder`.
- **Testability:** `bloc_test` provides structured tools to verify state sequences deterministically.
- **Traceability:** State transitions can be globally observed and logged using `BlocObserver`.

## 5. Consequences
- **Positive:** Robust separation of concerns, high testability, predictable state transitions.
- **Trade-off:** Slightly more boilerplate code per feature (mitigated via code generation and templates).
