import 'package:flutter/material.dart';

/// Shared Patient visual system. Keep screen styling in these tokens.
abstract final class PatientTokens {
  static const bg = Color(0xFFF7FAF9);
  static const surface = Colors.white;
  static const surfaceSubtle = Color(0xFFF1F5F3);
  static const surfaceAlt = Color(0xFFEEF4F1);
  static const primary = Color(0xFF087A5B);
  static const primaryDark = Color(0xFF075E49);
  static const primaryLight = Color(0xFFDDF3E9);
  static const text = Color(0xFF172321);
  static const textSub = Color(0xFF687773);
  static const border = Color(0xFFDDE6E2);
  static const muted = Color(0xFF527169);
  static const warning = Color(0xFFB7791F);
  static const warningLight = Color(0xFFFFF6DD);
  static const info = Color(0xFF2563EB);
  static const infoLight = Color(0xFFEFF6FF);
  static const danger = Color(0xFFB42318);
  static const dangerLight = Color(0xFFFFF1F0);
  static const rose = Color(0xFFF9738F);
  static const roseLight = Color(0xFFFFE7EB);
  static const amberText = Color(0xFF996515);
  static const roseText = Color(0xFF9F1239);
  static const radius = 12.0;
  static const cardRadius = 16.0;
  static const chipRadius = 9.0;
  static const navHeight = 68.0;
  static const inputHeight = 52.0;
  static const pad = 20.0;

  static ThemeData theme() {
    final scheme = ColorScheme.fromSeed(
        seedColor: primary, brightness: Brightness.light, surface: surface);
    return ThemeData(
      useMaterial3: true,
      scaffoldBackgroundColor: bg,
      colorScheme: scheme.copyWith(
          primary: primary,
          onPrimary: Colors.white,
          surface: surface,
          onSurface: text,
          outline: border),
      fontFamily: 'Plus Jakarta Sans',
      textTheme: const TextTheme(
        headlineSmall: TextStyle(
            fontSize: 27,
            height: 1.15,
            fontWeight: FontWeight.w700,
            color: text),
        titleLarge: TextStyle(
            fontSize: 20,
            height: 1.2,
            fontWeight: FontWeight.w700,
            color: text),
        titleMedium: TextStyle(
            fontSize: 16,
            height: 1.3,
            fontWeight: FontWeight.w600,
            color: text),
        bodyLarge: TextStyle(fontSize: 16, height: 1.45, color: text),
        bodyMedium: TextStyle(fontSize: 14, height: 1.45, color: textSub),
        labelLarge: TextStyle(
            fontFamily: 'Plus Jakarta Sans',
            fontSize: 15,
            fontWeight: FontWeight.w600),
      ),
      appBarTheme: const AppBarTheme(
          backgroundColor: bg,
          foregroundColor: text,
          elevation: 0,
          centerTitle: false,
          titleTextStyle: TextStyle(
              fontFamily: 'Plus Jakarta Sans',
              fontSize: 20,
              fontWeight: FontWeight.w700,
              color: text)),
      bottomSheetTheme: const BottomSheetThemeData(
          backgroundColor: surface, surfaceTintColor: surface),
      cardTheme: const CardThemeData(
          color: surface, elevation: 0, margin: EdgeInsets.zero),
      dividerTheme:
          const DividerThemeData(color: border, thickness: 1, space: 1),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: surface,
        contentPadding:
            const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
        border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(radius),
            borderSide: const BorderSide(color: border)),
        enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(radius),
            borderSide: const BorderSide(color: border)),
        focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(radius),
            borderSide: const BorderSide(color: primary, width: 1.5)),
        errorBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(radius),
            borderSide: const BorderSide(color: danger)),
        focusedErrorBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(radius),
            borderSide: const BorderSide(color: danger, width: 1.5)),
        hintStyle: const TextStyle(color: textSub, fontSize: 15),
      ),
      filledButtonTheme: FilledButtonThemeData(
          style: FilledButton.styleFrom(
              backgroundColor: primary,
              foregroundColor: Colors.white,
              minimumSize: const Size.fromHeight(50),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(radius)),
              textStyle: const TextStyle(
                  fontFamily: 'Plus Jakarta Sans',
                  fontSize: 15,
                  fontWeight: FontWeight.w600))),
      outlinedButtonTheme: OutlinedButtonThemeData(
          style: OutlinedButton.styleFrom(
              foregroundColor: primaryDark,
              minimumSize: const Size.fromHeight(50),
              side: const BorderSide(color: border),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(radius)),
              textStyle: const TextStyle(
                  fontFamily: 'Plus Jakarta Sans',
                  fontSize: 15,
                  fontWeight: FontWeight.w600))),
    );
  }
}
