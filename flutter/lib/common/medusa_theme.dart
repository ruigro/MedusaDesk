import 'package:flutter/material.dart';

/// Medusa Desk visual identity — Far Island Corporation.
///
/// Single source of truth for the brand palette: a dark-first "abyss" scheme
/// with a bioluminescent teal signature accent and a violet secondary.
/// Every other file should reference these tokens (usually through `MyTheme`)
/// instead of declaring hex values.
class MedusaColors {
  MedusaColors._();

  // Abyss neutrals (dark mode surfaces, darkest -> lightest).
  static const Color abyss = Color(0xFF060B14); // window scaffold
  static const Color deepSea = Color(0xFF0A1422); // panels / left pane
  static const Color surface = Color(0xFF101D30); // cards, input fill
  static const Color surfaceHi = Color(0xFF182B42); // hover / elevated
  static const Color hairline = Color(0xFF1F3450); // borders

  // Signature accents.
  static const Color biolume = Color(0xFF00E5C7); // primary teal
  static const Color biolumeDim = Color(0xFF00B39B); // light mode / pressed
  static const Color biolumeGlow = Color(0x5500E5C7); // shadows, focus rings
  static const Color biolumeFaint = Color(0x2200E5C7); // tints, selected bg
  static const Color violet = Color(0xFF8A7CFF); // secondary accent
  static const Color violetGlow = Color(0x338A7CFF);

  // Status.
  static const Color success = Color(0xFF2EE6A8);
  static const Color warn = Color(0xFFFFB454);
  static const Color danger = Color(0xFFFF5C7A);

  // Text.
  static const Color textPrimary = Color(0xFFE8F4F4);
  static const Color textMuted = Color(0xFF8DA3B9);
  // Dark ink for text rendered on top of the teal accent.
  static const Color inkOnBiolume = Color(0xFF04211C);

  // Light mode counterparts.
  static const Color lightBg = Color(0xFFF2F7F8);
  static const Color lightSurface = Color(0xFFFFFFFF);
  static const Color lightHairline = Color(0xFFDCE8EC);
  static const Color lightHover = Color(0xFFE4EEF1);

  // Reusable decorations.
  static const LinearGradient heroGradient = LinearGradient(
    begin: Alignment.topLeft,
    end: Alignment.bottomRight,
    colors: [surface, Color(0xFF0E2433)],
  );

  static const LinearGradient installCardGradient = LinearGradient(
    begin: Alignment.topLeft,
    end: Alignment.bottomRight,
    colors: [surface, Color(0xFF143A36)],
  );

  static List<BoxShadow> glow([Color color = biolumeGlow]) => [
        BoxShadow(color: color, blurRadius: 18, spreadRadius: -4),
      ];

  /// Standard Medusa card decoration; pass `hovered` for the lifted state.
  static BoxDecoration card(BuildContext context, {bool hovered = false}) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    return BoxDecoration(
      color: isDark
          ? (hovered ? surfaceHi : surface)
          : (hovered ? lightHover : lightSurface),
      borderRadius: BorderRadius.circular(14),
      border: Border.all(color: hovered ? biolume : (isDark ? hairline : lightHairline)),
      boxShadow: hovered ? glow() : null,
    );
  }
}
