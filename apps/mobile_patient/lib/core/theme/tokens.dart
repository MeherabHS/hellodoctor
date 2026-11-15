import 'package:flutter/material.dart';

/// Shared Patient visual system. Keep screen styling in these tokens.
abstract final class PatientTokens {
  // Values below marked "ref" were sampled from
  // docs/design/afyamkononi-patient-ui-reference.jpeg (a 1214 x 1295 JPEG,
  // about 216 px per phone), so they are close estimates, not exact specs.
  static const bg = Colors.white; // ref: phone body #FEFEFC
  static const surface = Colors.white;
  static const surfaceSubtle = Color(0xFFF1F5F3);
  static const surfaceAlt = Color(0xFFEEF4F1);
  static const primary =
      Color(0xFF009A73); // ref: primary buttons, active pills
  static const primaryDark = Color(0xFF00765A); // ref: selected chip
  static const primaryLight = Color(0xFFDEF5ED); // ref: greeting card
  static const text = Color(0xFF10212B);
  static const textSub = Color(0xFF687773);
  static const border = Color(0xFFE3EAEE);
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

  // --- AfyaMkononi reference palette (ref) ---
  static const brandTeal = Color(0xFF17607A); // wordmark
  static const brandTealSoft = Color(0xFF3F7F86); // tagline
  static const navy = Color(0xFF0B2A3F); // hero headline
  static const ink = Color(0xFF33485A); // tile labels
  static const greetingSub = Color(0xFF406B67);
  static const avatarGreen = Color(0xFF0B5A45);
  static const navActive = Color(0xFF0B7253);
  static const navInactive = Color(0xFF3C505F);
  static const hint = Color(0xFF7E8A95);
  static const heroSky = Color(0xFF85DCF0);
  static const heroSkyLight = Color(0xFFD9F2F7);
  static const heroCta = Color(0xFF09738B);
  static const tealCta = Color(0xFF007F8C);
  static const mintCard = Color(0xFFE5F8F2);
  static const infoCard = Color(0xFFE9F6FC);
  static const urgentCard = Color(0xFFFCF0F2);
  static const urgentIcon = Color(0xFFDB3B43);
  static const amberCard = Color(0xFFFEF0D6);
  static const infoAccent = Color(0xFF1F5FA8); // info-card titles
  static const introBlue = Color(0xFF1D4FB8); // support-chat intro
  static const cardTint = Color(0xFFF5F7FC); // faint lilac-grey cards
  static const doctorCard = Color(0xFFEAF2FB); // video-consultation hero
  static const doctorPanel = Color(0xFFD3E4F8);
  // Service-tile icon / background pairs.
  static const tileGreen = Color(0xFF1B8A5E);
  static const tileGreenBg = Color(0xFFDFF5EA);
  static const tileBlue = Color(0xFF3075F4);
  static const tileBlueBg = Color(0xFFE4EEFD);
  static const tileRed = Color(0xFFEA324E);
  static const tileRedBg = Color(0xFFFDE8EB);
  static const tileAmber = Color(0xFFF8A626);
  static const tileAmberBg = Color(0xFFFDF0D2);
  static const tilePurple = Color(0xFF8450CF);
  static const tilePurpleBg = Color(0xFFF0E6FB);
  static const tileSlateBlue = Color(0xFF3866AB);
  static const tileSlateBlueBg = Color(0xFFE6F1F9);
  static const softShadow = [
    BoxShadow(color: Color(0x14102A43), blurRadius: 14, offset: Offset(0, 3)),
  ];
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
