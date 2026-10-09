import 'package:flutter/material.dart';

abstract final class DoctorTokens {
  static const bg = Color(0xFFF5F9F7);
  static const surface = Colors.white;
  static const surfaceSubtle = Color(0xFFF0F6F3);
  static const primary = Color(0xFF059669);
  static const primaryDark = Color(0xFF047857);
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
        scaffoldBackgroundColor: bg,
        colorScheme: ColorScheme.fromSeed(seedColor: primary, surface: surface),
        fontFamily: 'Plus Jakarta Sans',
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
        ),
      );
}
