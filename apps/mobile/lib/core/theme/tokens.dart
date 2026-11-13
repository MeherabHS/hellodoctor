import 'package:flutter/material.dart';

abstract final class DoctorTokens {
  static const bg = Color(0xFFF5F9F7);
  static const surface = Colors.white;
  static const surfaceSubtle = Color(0xFFF0F6F3);
  // Doctor reference board, panel 1 "Sign In" button: #058152 (top) to
  // #01764B (bottom), #128359 through the label. One flat value is used.
  static const primary = Color(0xFF05804F);
  static const primaryDark = Color(0xFF01764B);
  // Board headings ("Welcome, Doctor") and body copy.
  static const navy = Color(0xFF0B1026);
  static const slate = Color(0xFF4C5161);
  static const fieldText = Color(0xFF37394A);
  static const fieldBorder = Color(0xFFE1E6EB);
  static const noticeBg = Color(0xFFE1F1F5);
  static const screen = Color(0xFFFDFDFD);
  static const primaryLight = Color(0xFFD1FAE5);
  static const text = Color(0xFF0F291E);
  static const textSub = Color(0xFF789A90);
  static const border = Color(0xFFDFECE6);
  static const muted = Color(0xFF52796F);
  static const warning = Color(0xFFF59E0B);
  static const warningLight = Color(0xFFFEF3C7);
  static const info = Color(0xFF2563EB);
  static const infoLight = Color(0xFFEFF6FF);
  static const rose = Color(0xFFFB7185);
  static const roseLight = Color(0xFFFFE4E6);
  static const danger = Color(0xFFDC2626);
  static const radius = 16.0;
  static const cardRadius = 16.0;
  static const chipRadius = 30.0;
  static const navHeight = 80.0;
  static const inputHeight = 44.0;
  static const pad = 18.0;
  static ThemeData theme() => ThemeData(
        useMaterial3: true,
        scaffoldBackgroundColor: screen,
        colorScheme: ColorScheme.fromSeed(seedColor: primary, surface: surface),
        fontFamily: 'Plus Jakarta Sans',
        appBarTheme: const AppBarTheme(
            backgroundColor: screen,
            surfaceTintColor: Colors.transparent,
            elevation: 0,
            scrolledUnderElevation: 0,
            foregroundColor: navy,
            titleTextStyle: TextStyle(
                fontFamily: 'Plus Jakarta Sans',
                fontSize: 19,
                fontWeight: FontWeight.w800,
                color: navy)),
        filledButtonTheme: FilledButtonThemeData(
            style: FilledButton.styleFrom(
                backgroundColor: primary,
                foregroundColor: Colors.white,
                minimumSize: const Size(0, 50),
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(14)),
                textStyle: const TextStyle(
                    fontFamily: 'Plus Jakarta Sans',
                    fontSize: 16,
                    fontWeight: FontWeight.w800))),
        textButtonTheme: TextButtonThemeData(
            style: TextButton.styleFrom(
                foregroundColor: primaryDark,
                textStyle: const TextStyle(
                    fontFamily: 'Plus Jakarta Sans',
                    fontWeight: FontWeight.w700))),
        cardTheme: const CardThemeData(
            color: surface, elevation: 0, margin: EdgeInsets.zero),
        inputDecorationTheme: InputDecorationTheme(
          filled: true,
          fillColor: surface,
          border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: border)),
          enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: border)),
          focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: primary, width: 1.5)),
        ),
      );
}
