import 'package:flutter/material.dart';

class MedusaSkin {
  const MedusaSkin({
    required this.key,
    required this.label,
    required this.description,
    required this.palette,
  });

  final String key;
  final String label;
  final String description;
  final MedusaPalette palette;
}

class MedusaPalette {
  const MedusaPalette({
    required this.abyss,
    required this.deepSea,
    required this.surface,
    required this.surfaceHi,
    required this.hairline,
    required this.biolume,
    required this.biolumeDim,
    required this.biolumeGlow,
    required this.biolumeFaint,
    required this.violet,
    required this.violetGlow,
    required this.success,
    required this.warn,
    required this.danger,
    required this.textPrimary,
    required this.textMuted,
    required this.inkOnBiolume,
    required this.lightBg,
    required this.lightSurface,
    required this.lightHairline,
    required this.lightHover,
    required this.heroEnd,
    required this.installEnd,
    required this.lightBorder2,
    required this.darkBorder2,
    required this.darkErrorBannerBg,
  });

  final Color abyss;
  final Color deepSea;
  final Color surface;
  final Color surfaceHi;
  final Color hairline;
  final Color biolume;
  final Color biolumeDim;
  final Color biolumeGlow;
  final Color biolumeFaint;
  final Color violet;
  final Color violetGlow;
  final Color success;
  final Color warn;
  final Color danger;
  final Color textPrimary;
  final Color textMuted;
  final Color inkOnBiolume;
  final Color lightBg;
  final Color lightSurface;
  final Color lightHairline;
  final Color lightHover;
  final Color heroEnd;
  final Color installEnd;
  final Color lightBorder2;
  final Color darkBorder2;
  final Color darkErrorBannerBg;
}

class MedusaSkins {
  MedusaSkins._();

  static const defaultKey = 'medusa';

  static const List<MedusaSkin> all = [
    MedusaSkin(
      key: defaultKey,
      label: 'Medusa',
      description: 'Current abyss teal skin.',
      palette: MedusaPalette(
        abyss: Color(0xFF060B14),
        deepSea: Color(0xFF0A1422),
        surface: Color(0xFF101D30),
        surfaceHi: Color(0xFF182B42),
        hairline: Color(0xFF1F3450),
        biolume: Color(0xFF00E5C7),
        biolumeDim: Color(0xFF00B39B),
        biolumeGlow: Color(0x5500E5C7),
        biolumeFaint: Color(0x2200E5C7),
        violet: Color(0xFF8A7CFF),
        violetGlow: Color(0x338A7CFF),
        success: Color(0xFF2EE6A8),
        warn: Color(0xFFFFB454),
        danger: Color(0xFFFF5C7A),
        textPrimary: Color(0xFFE8F4F4),
        textMuted: Color(0xFF8DA3B9),
        inkOnBiolume: Color(0xFF04211C),
        lightBg: Color(0xFFF2F7F8),
        lightSurface: Color(0xFFFFFFFF),
        lightHairline: Color(0xFFDCE8EC),
        lightHover: Color(0xFFE4EEF1),
        heroEnd: Color(0xFF0E2433),
        installEnd: Color(0xFF143A36),
        lightBorder2: Color(0xFFBBCDD4),
        darkBorder2: Color(0xFF2C4763),
        darkErrorBannerBg: Color(0xFF3A1430),
      ),
    ),
    MedusaSkin(
      key: 'far_island',
      label: 'Far Island',
      description: 'Original black and gold Far Island mood.',
      palette: MedusaPalette(
        abyss: Color(0xFF191A22),
        deepSea: Color(0xFF20212A),
        surface: Color(0xFF292832),
        surfaceHi: Color(0xFF34323D),
        hairline: Color(0xFF61502B),
        biolume: Color(0xFFFFC21A),
        biolumeDim: Color(0xFFD99A08),
        biolumeGlow: Color(0x55FFC21A),
        biolumeFaint: Color(0x24FFC21A),
        violet: Color(0xFF4DD4C5),
        violetGlow: Color(0x334DD4C5),
        success: Color(0xFF57D68D),
        warn: Color(0xFFFFB23F),
        danger: Color(0xFFFF6A5F),
        textPrimary: Color(0xFFF5F1E8),
        textMuted: Color(0xFFC3BBA6),
        inkOnBiolume: Color(0xFF251901),
        lightBg: Color(0xFFFFF8EA),
        lightSurface: Color(0xFFFFFFFF),
        lightHairline: Color(0xFFE6D7B5),
        lightHover: Color(0xFFFFF0CE),
        heroEnd: Color(0xFF4B3406),
        installEnd: Color(0xFF6B4905),
        lightBorder2: Color(0xFFD8C28B),
        darkBorder2: Color(0xFF7B642E),
        darkErrorBannerBg: Color(0xFF3A1610),
      ),
    ),
    MedusaSkin(
      key: 'polar',
      label: 'Polar Glass',
      description: 'Bright cool skin for daytime work.',
      palette: MedusaPalette(
        abyss: Color(0xFF0E1726),
        deepSea: Color(0xFF152238),
        surface: Color(0xFF1D2D46),
        surfaceHi: Color(0xFF28405E),
        hairline: Color(0xFF375676),
        biolume: Color(0xFF58D7FF),
        biolumeDim: Color(0xFF278BC4),
        biolumeGlow: Color(0x5558D7FF),
        biolumeFaint: Color(0x2258D7FF),
        violet: Color(0xFFB58CFF),
        violetGlow: Color(0x33B58CFF),
        success: Color(0xFF49D8A7),
        warn: Color(0xFFFFC766),
        danger: Color(0xFFFF647D),
        textPrimary: Color(0xFFF3FAFF),
        textMuted: Color(0xFFA8BDD2),
        inkOnBiolume: Color(0xFF041923),
        lightBg: Color(0xFFF4F9FD),
        lightSurface: Color(0xFFFFFFFF),
        lightHairline: Color(0xFFD7E6F1),
        lightHover: Color(0xFFE7F3FA),
        heroEnd: Color(0xFF123B56),
        installEnd: Color(0xFF164A62),
        lightBorder2: Color(0xFFBBD0DE),
        darkBorder2: Color(0xFF476A8A),
        darkErrorBannerBg: Color(0xFF3B1730),
      ),
    ),
    MedusaSkin(
      key: 'night_ops',
      label: 'Night Ops',
      description: 'Quiet graphite with sharp signal green.',
      palette: MedusaPalette(
        abyss: Color(0xFF0B0D10),
        deepSea: Color(0xFF12161B),
        surface: Color(0xFF1A2026),
        surfaceHi: Color(0xFF252D34),
        hairline: Color(0xFF34414A),
        biolume: Color(0xFFA6FF6A),
        biolumeDim: Color(0xFF6FC43D),
        biolumeGlow: Color(0x55A6FF6A),
        biolumeFaint: Color(0x22A6FF6A),
        violet: Color(0xFFFF8C5A),
        violetGlow: Color(0x33FF8C5A),
        success: Color(0xFF7EE66A),
        warn: Color(0xFFFFD36A),
        danger: Color(0xFFFF5E5E),
        textPrimary: Color(0xFFF0F5EA),
        textMuted: Color(0xFFA5B09A),
        inkOnBiolume: Color(0xFF132007),
        lightBg: Color(0xFFF5F8F2),
        lightSurface: Color(0xFFFFFFFF),
        lightHairline: Color(0xFFDDE8D3),
        lightHover: Color(0xFFEAF3E2),
        heroEnd: Color(0xFF203117),
        installEnd: Color(0xFF2C3F1D),
        lightBorder2: Color(0xFFC4D3B8),
        darkBorder2: Color(0xFF4F6542),
        darkErrorBannerBg: Color(0xFF321616),
      ),
    ),
  ];

  static MedusaSkin _currentSkin = all.first;

  static MedusaSkin get currentSkin => _currentSkin;

  static MedusaPalette get current => _currentSkin.palette;

  static String normalize(String? key) {
    if (key == null || key.isEmpty) {
      return defaultKey;
    }
    return all.any((skin) => skin.key == key) ? key : defaultKey;
  }

  static void setCurrent(String? key) {
    final normalized = normalize(key);
    _currentSkin = all.firstWhere((skin) => skin.key == normalized);
  }
}

/// Skin-aware visual tokens for Medusa Desk.
class MedusaColors {
  MedusaColors._();

  static MedusaPalette get _p => MedusaSkins.current;

  static Color get abyss => _p.abyss;
  static Color get deepSea => _p.deepSea;
  static Color get surface => _p.surface;
  static Color get surfaceHi => _p.surfaceHi;
  static Color get hairline => _p.hairline;
  static Color get biolume => _p.biolume;
  static Color get biolumeDim => _p.biolumeDim;
  static Color get biolumeGlow => _p.biolumeGlow;
  static Color get biolumeFaint => _p.biolumeFaint;
  static Color get violet => _p.violet;
  static Color get violetGlow => _p.violetGlow;
  static Color get success => _p.success;
  static Color get warn => _p.warn;
  static Color get danger => _p.danger;
  static Color get textPrimary => _p.textPrimary;
  static Color get textMuted => _p.textMuted;
  static Color get inkOnBiolume => _p.inkOnBiolume;
  static Color get lightBg => _p.lightBg;
  static Color get lightSurface => _p.lightSurface;
  static Color get lightHairline => _p.lightHairline;
  static Color get lightHover => _p.lightHover;
  static Color get lightBorder2 => _p.lightBorder2;
  static Color get darkBorder2 => _p.darkBorder2;
  static Color get darkErrorBannerBg => _p.darkErrorBannerBg;

  static LinearGradient get heroGradient => LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [surface, _p.heroEnd],
      );

  static LinearGradient get installCardGradient => LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [surface, _p.installEnd],
      );

  static List<BoxShadow> glow([Color? color]) => [
        BoxShadow(
            color: color ?? biolumeGlow, blurRadius: 18, spreadRadius: -4),
      ];

  /// Standard Medusa card decoration; pass `hovered` for the lifted state.
  static BoxDecoration card(BuildContext context, {bool hovered = false}) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return BoxDecoration(
      color: isDark
          ? (hovered ? surfaceHi : surface)
          : (hovered ? lightHover : lightSurface),
      borderRadius: BorderRadius.circular(14),
      border: Border.all(
          color: hovered ? biolume : (isDark ? hairline : lightHairline)),
      boxShadow: hovered ? glow() : null,
    );
  }
}
