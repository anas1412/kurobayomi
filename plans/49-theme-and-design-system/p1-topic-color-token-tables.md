# Area 49 (part 1) topic: color token tables for the 18 static schemes

Generated from the Kotlin sources by `49-theme-and-design-system/p1-tool-scheme_tables.py (run from plans/)` (repo-relative paths under `app/src/main/java/eu/kanade/presentation/theme/colorscheme/`). Values are exact: `Color(0xAARRGGBB)` as written in the source (alpha is always `FF`). Source comments are copied verbatim.

Conventions:
- "Omitted" tokens are not set in the Kotlin call. They take the androidx.compose.material3 default for that token (see `p1-behavior.md` section 1.2). Omitted counts are over the 36 base roles. Fixed roles (`primaryFixed` etc.) are never set by any scheme.
- Monet is not in this table (its colors are generated at runtime; see `p1-behavior.md` section 3).
- The AMOLED override (`BaseColorScheme.kt`) is applied on top of the dark table at runtime and is not repeated here.

## Summary

| Theme (object) | Source file | Mode | Explicit tokens | Omitted tokens (fall back to androidx M3 baseline) |
|---|---|---|---|---|
| CloudflareColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/CloudflareColorScheme.kt | dark | 22 | 14 |
| CloudflareColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/CloudflareColorScheme.kt | light | 22 | 14 |
| CottoncandyColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/CottoncandyColorScheme.kt | dark | 23 | 13 |
| CottoncandyColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/CottoncandyColorScheme.kt | light | 23 | 13 |
| DoomColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/DoomColorScheme.kt | dark | 22 | 14 |
| DoomColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/DoomColorScheme.kt | light | 22 | 14 |
| GreenAppleColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/GreenAppleColorScheme.kt | dark | 35 | 1 |
| GreenAppleColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/GreenAppleColorScheme.kt | light | 35 | 1 |
| LavenderColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/LavenderColorScheme.kt | dark | 35 | 1 |
| LavenderColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/LavenderColorScheme.kt | light | 35 | 1 |
| MatrixColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MatrixColorScheme.kt | dark | 22 | 14 |
| MatrixColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MatrixColorScheme.kt | light | 22 | 14 |
| MidnightDuskColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MidnightDuskColorScheme.kt | dark | 28 | 8 |
| MidnightDuskColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MidnightDuskColorScheme.kt | light | 28 | 8 |
| MochaColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MochaColorScheme.kt | dark | 23 | 13 |
| MochaColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MochaColorScheme.kt | light | 23 | 13 |
| MonochromeColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonochromeColorScheme.kt | dark | 35 | 1 |
| MonochromeColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonochromeColorScheme.kt | light | 35 | 1 |
| NordColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/NordColorScheme.kt | dark | 32 | 4 |
| NordColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/NordColorScheme.kt | light | 31 | 5 |
| SapphireColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/SapphireColorScheme.kt | dark | 22 | 14 |
| SapphireColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/SapphireColorScheme.kt | light | 22 | 14 |
| StrawberryColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/StrawberryColorScheme.kt | dark | 35 | 1 |
| StrawberryColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/StrawberryColorScheme.kt | light | 35 | 1 |
| TachiyomiColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TachiyomiColorScheme.kt | dark | 33 | 3 |
| TachiyomiColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TachiyomiColorScheme.kt | light | 33 | 3 |
| TakoColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TakoColorScheme.kt | dark | 28 | 8 |
| TakoColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TakoColorScheme.kt | light | 28 | 8 |
| TealTurqoiseColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TealTurqoiseColorScheme.kt | dark | 28 | 8 |
| TealTurqoiseColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TealTurqoiseColorScheme.kt | light | 28 | 8 |
| TidalWaveColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TidalWaveColorScheme.kt | dark | 28 | 8 |
| TidalWaveColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/TidalWaveColorScheme.kt | light | 28 | 8 |
| YinYangColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/YinYangColorScheme.kt | dark | 28 | 8 |
| YinYangColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/YinYangColorScheme.kt | light | 28 | 8 |
| YotsubaColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/YotsubaColorScheme.kt | dark | 28 | 8 |
| YotsubaColorScheme | app/src/main/java/eu/kanade/presentation/theme/colorscheme/YotsubaColorScheme.kt | light | 28 | 8 |

#### CloudflareColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFF38020 |  |
| onPrimary | 0xFF1B1B22 |  |
| primaryContainer | 0xFFF38020 |  |
| onPrimaryContainer | 0xFF1B1B22 |  |
| inversePrimary | 0xFFD6BAFF | Assuming 'inversePrimary' maps to 'cloudflare_primaryInverse' |
| secondary | 0xFFF38020 |  |
| onSecondary | 0xFF1B1B22 |  |
| secondaryContainer | 0xFFF38020 |  |
| onSecondaryContainer | 0xFF1B1B22 |  |
| tertiary | 0xFF1B1B22 |  |
| onTertiary | 0xFFF38020 |  |
| tertiaryContainer | 0xFF1B1B22 |  |
| onTertiaryContainer | 0xFFF38020 |  |
| background | 0xFF1B1B22 |  |
| onBackground | 0xFFEFF2F5 |  |
| surface | 0xFF1B1B22 |  |
| onSurface | 0xFFEFF2F5 |  |
| surfaceVariant | 0xFF3F3F46 |  |
| surfaceTint | 0xFFF38020 | Assuming 'surfaceTint' maps to 'cloudflare_primary' or similar |
| inverseSurface | 0xFFF3EFF4 |  |
| inverseOnSurface | 0xFF313033 |  |
| outline | 0xFFF38020 |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### CloudflareColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFF38020 |  |
| onPrimary | 0xFFEFF2F5 |  |
| primaryContainer | 0xFFF38020 |  |
| onPrimaryContainer | 0xFFEFF2F5 |  |
| inversePrimary | 0xFFD6BAFF | Assuming 'inversePrimary' maps to 'cloudflare_primaryInverse' |
| secondary | 0xFFF38020 |  |
| onSecondary | 0xFFEFF2F5 |  |
| secondaryContainer | 0xFFF38020 |  |
| onSecondaryContainer | 0xFFEFF2F5 |  |
| tertiary | 0xFFEFF2F5 |  |
| onTertiary | 0xFFF38020 |  |
| tertiaryContainer | 0xFFEFF2F5 |  |
| onTertiaryContainer | 0xFFF38020 |  |
| background | 0xFFEFF2F5 |  |
| onBackground | 0xFF1B1B22 |  |
| surface | 0xFFEFF2F5 |  |
| onSurface | 0xFF1B1B22 |  |
| surfaceVariant | 0xFFB9B0CC |  |
| surfaceTint | 0xFFF38020 | Assuming 'surfaceTint' maps to 'cloudflare_primary' or similar |
| inverseSurface | 0xFF313033 |  |
| inverseOnSurface | 0xFFF3EFF4 |  |
| outline | 0xFFF38020 |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### CottoncandyColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFFFB1C1 |  |
| onPrimary | 0xFF5F112B |  |
| primaryContainer | 0xFF7C2941 |  |
| onPrimaryContainer | 0xFFFFD9DF |  |
| secondary | 0xFF64D3FF |  |
| onSecondary | 0xFF003546 |  |
| secondaryContainer | 0xFF004D63 |  |
| onSecondaryContainer | 0xFFBCE9FF |  |
| tertiary | 0xFFFFB1C1 |  |
| onTertiary | 0xFF5F112B | Note: onTertiary color is assumed |
| tertiaryContainer | 0xFF7C2941 |  |
| onTertiaryContainer | 0xFFFFD9DF |  |
| background | 0xFF201A1B |  |
| onBackground | 0xFFECE0E0 |  |
| surface | 0xFF201A1B |  |
| onSurface | 0xFFECE0E0 |  |
| surfaceVariant | 0xFF524345 |  |
| onSurfaceVariant | 0xFFD6C2C4 |  |
| surfaceTint | 0xFFFFB1C1 |  |
| inverseSurface | 0xFFECE0E0 |  |
| inverseOnSurface | 0xFF201A1B |  |
| outline | 0xFF9F8C8F |  |
| inversePrimary | 0xFF9A4058 |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### CottoncandyColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF9A4058 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFFFD9DF |  |
| onPrimaryContainer | 0xFF3F0017 |  |
| secondary | 0xFF5BCEFA |  |
| onSecondary | 0xFFFFFFFF |  |
| secondaryContainer | 0xFFBCE9FF |  |
| onSecondaryContainer | 0xFF001F2A |  |
| tertiary | 0xFF9A4058 |  |
| onTertiary | 0xFFFFFFFF |  |
| tertiaryContainer | 0xFFFFD9DF |  |
| onTertiaryContainer | 0xFF3F0017 |  |
| background | 0xFFFFFBFF |  |
| onBackground | 0xFF201A1B |  |
| surface | 0xFFFFFBFF |  |
| onSurface | 0xFF201A1B |  |
| surfaceVariant | 0xFFF3DDE0 |  |
| onSurfaceVariant | 0xFF524345 |  |
| surfaceTint | 0xFF9A4058 |  |
| inverseSurface | 0xFF352F30 |  |
| inverseOnSurface | 0xFFFAEEEF |  |
| outline | 0xFF847375 |  |
| inversePrimary | 0xFFFFB1C1 |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### DoomColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFFF0000 |  |
| onPrimary | 0xFFFAFAFA |  |
| primaryContainer | 0xFFFF0000 |  |
| onPrimaryContainer | 0xFFFAFAFA |  |
| secondary | 0xFFFF0000 |  |
| onSecondary | 0xFFFAFAFA |  |
| secondaryContainer | 0xFFFF0000 |  |
| onSecondaryContainer | 0xFFFAFAFA |  |
| tertiary | 0xFFBFBFBF |  |
| onTertiary | 0xFFFF0000 |  |
| tertiaryContainer | 0xFFBFBFBF |  |
| onTertiaryContainer | 0xFFFF0000 |  |
| background | 0xFF1B1B1B |  |
| onBackground | 0xFFFFFFFF |  |
| surface | 0xFF1B1B1B |  |
| onSurface | 0xFFFFFFFF |  |
| surfaceVariant | 0xFF303030 |  |
| surfaceTint | 0xFFFF0000 |  |
| inverseSurface | 0xFFFAFAFA |  |
| inverseOnSurface | 0xFF313131 |  |
| outline | 0xFFFF0000 |  |
| inversePrimary | 0xFF6D0D0B |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### DoomColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFFF0000 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFFF0000 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| inversePrimary | 0xFF6D0D0B | Assuming 'inversePrimary' maps to 'doom_primaryInverse' |
| secondary | 0xFFFF0000 |  |
| onSecondary | 0xFFFFFFFF |  |
| secondaryContainer | 0xFFFF0000 |  |
| onSecondaryContainer | 0xFFFFFFFF |  |
| tertiary | 0xFFBFBFBF |  |
| onTertiary | 0xFFFF0000 |  |
| tertiaryContainer | 0xFFBFBFBF |  |
| onTertiaryContainer | 0xFFFF0000 |  |
| background | 0xFF212121 |  |
| onBackground | 0xFFFFFFFF |  |
| surface | 0xFF212121 |  |
| onSurface | 0xFFFFFFFF |  |
| surfaceVariant | 0xFF4D4D4D |  |
| surfaceTint | 0xFFFF0000 | Assuming 'surfaceTint' maps to 'doom_primary' or similar |
| inverseSurface | 0xFF424242 |  |
| inverseOnSurface | 0xFFFAFAFA |  |
| outline | 0xFFFF0000 |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### GreenAppleColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF7ADB8F |  |
| onPrimary | 0xFF003917 |  |
| primaryContainer | 0xFF017737 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| secondary | 0xFF7ADB8F | Unread badge |
| onSecondary | 0xFF003917 | Unread badge text |
| secondaryContainer | 0xFF017737 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFFFFFFF | Navigation bar selected icon |
| tertiary | 0xFFFFB3AC | Downloaded badge |
| onTertiary | 0xFF680008 | Downloaded badge text |
| tertiaryContainer | 0xFFC7282A |  |
| onTertiaryContainer | 0xFFFFFFFF |  |
| error | 0xFFFFB4AB |  |
| onError | 0xFF690005 |  |
| errorContainer | 0xFF93000A |  |
| onErrorContainer | 0xFFFFDAD6 |  |
| background | 0xFF0F1510 |  |
| onBackground | 0xFFDFE4DB |  |
| surface | 0xFF0F1510 |  |
| onSurface | 0xFFDFE4DB |  |
| surfaceVariant | 0xFF3F493F | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFBECABC |  |
| outline | 0xFF889487 |  |
| outlineVariant | 0xFF3F493F |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFFDFE4DB |  |
| inverseOnSurface | 0xFF2C322C |  |
| inversePrimary | 0xFF006D32 |  |
| surfaceDim | 0xFF0F1510 |  |
| surfaceBright | 0xFF353B35 |  |
| surfaceContainerLowest | 0xFF0A0F0B |  |
| surfaceContainerLow | 0xFF181D18 |  |
| surfaceContainer | 0xFF1C211C | Navigation bar background |
| surfaceContainerHigh | 0xFF262B26 |  |
| surfaceContainerHighest | 0xFF313630 |  |

Omitted: surfaceTint

#### GreenAppleColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF005927 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFF188140 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| secondary | 0xFF005927 | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFF97F7A9 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF000000 | Navigation bar selected icon |
| tertiary | 0xFF9D0012 | Downloaded badge |
| onTertiary | 0xFFFFFFFF | Downloaded badge text |
| tertiaryContainer | 0xFFD33131 |  |
| onTertiaryContainer | 0xFFFFFFFF |  |
| error | 0xFFBA1A1A |  |
| onError | 0xFFFFFFFF |  |
| errorContainer | 0xFFFFDAD6 |  |
| onErrorContainer | 0xFF410002 |  |
| background | 0xFFF6FBF2 |  |
| onBackground | 0xFF181D18 |  |
| surface | 0xFFF6FBF2 |  |
| onSurface | 0xFF181D18 |  |
| surfaceVariant | 0xFFDAE6D7 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF3F493F |  |
| outline | 0xFF6F7A6E |  |
| outlineVariant | 0xFFBECABC |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFF2C322C |  |
| inverseOnSurface | 0xFFEDF2E9 |  |
| inversePrimary | 0xFF7ADB8F |  |
| surfaceDim | 0xFFD6DCD3 |  |
| surfaceBright | 0xFFF6FBF2 |  |
| surfaceContainerLowest | 0xFFFFFFFF |  |
| surfaceContainerLow | 0xFFF0F5EC |  |
| surfaceContainer | 0xFFEAEFE6 | Navigation bar background |
| surfaceContainerHigh | 0xFFE4EAE1 |  |
| surfaceContainerHighest | 0xFFDFE4DB |  |

Omitted: surfaceTint

#### LavenderColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFA177FF |  |
| onPrimary | 0xFF3D0090 |  |
| primaryContainer | 0xFFA177FF |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| secondary | 0xFFA177FF | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFF423271 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFA177FF | Navigation bar selected icon |
| tertiary | 0xFFCDBDFF | Downloaded badge |
| onTertiary | 0xFF360096 | Downloaded badge text |
| tertiaryContainer | 0xFF5512D8 |  |
| onTertiaryContainer | 0xFFEFE6FF |  |
| error | 0xFFFFB4AB |  |
| onError | 0xFF690005 |  |
| errorContainer | 0xFF93000A |  |
| onErrorContainer | 0xFFFFDAD6 |  |
| background | 0xFF111129 |  |
| onBackground | 0xFFE7E0EC |  |
| surface | 0xFF111129 |  |
| onSurface | 0xFFE7E0EC |  |
| surfaceVariant | 0xFF3D2F6B | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFCBC3D6 |  |
| outline | 0xFF958E9F |  |
| outlineVariant | 0xFF4A4453 |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFFE7E0EC |  |
| inverseOnSurface | 0xFF322F38 |  |
| inversePrimary | 0xFF6D41C8 |  |
| surfaceDim | 0xFF111129 |  |
| surfaceBright | 0xFF3B3841 |  |
| surfaceContainerLowest | 0xFF15132D |  |
| surfaceContainerLow | 0xFF171531 |  |
| surfaceContainer | 0xFF1D193B | Navigation bar background |
| surfaceContainerHigh | 0xFF241F41 |  |
| surfaceContainerHighest | 0xFF282446 |  |

Omitted: surfaceTint

#### LavenderColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF6D41C8 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFF7B46AF |  |
| onPrimaryContainer | 0xFF130038 |  |
| secondary | 0xFF7B46AF | Unread badge |
| onSecondary | 0xFFEDE2FF | Unread badge text |
| secondaryContainer | 0xFFC9B0E6 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF7B46AF | Navigation bar selector icon |
| tertiary | 0xFFEDE2FF | Downloaded badge |
| onTertiary | 0xFF7B46AF | Downloaded badge text |
| tertiaryContainer | 0xFF6D3BF0 |  |
| onTertiaryContainer | 0xFFFFFFFF |  |
| error | 0xFFBA1A1A |  |
| onError | 0xFFFFFFFF |  |
| errorContainer | 0xFFFFDAD6 |  |
| onErrorContainer | 0xFF410002 |  |
| background | 0xFFEDE2FF |  |
| onBackground | 0xFF1D1A22 |  |
| surface | 0xFFEDE2FF |  |
| onSurface | 0xFF1D1A22 |  |
| surfaceVariant | 0xFFE4D5F8 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF4A4453 |  |
| outline | 0xFF7B7485 |  |
| outlineVariant | 0xFFCBC3D6 |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFF322F38 |  |
| inverseOnSurface | 0xFFF5EEFA |  |
| inversePrimary | 0xFFA177FF |  |
| surfaceDim | 0xFFDED7E3 |  |
| surfaceBright | 0xFFEDE2FF |  |
| surfaceContainerLowest | 0xFFDACCEC |  |
| surfaceContainerLow | 0xFFDED0F1 |  |
| surfaceContainer | 0xFFE4D5F8 | Navigation bar background |
| surfaceContainerHigh | 0xFFEADCFD |  |
| surfaceContainerHighest | 0xFFEEE2FF |  |

Omitted: surfaceTint

#### MatrixColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF00FF00 |  |
| onPrimary | 0xFFFAFAFA |  |
| primaryContainer | 0xFF00FF00 |  |
| onPrimaryContainer | 0xFFFAFAFA |  |
| secondary | 0xFF00FF00 |  |
| onSecondary | 0xFFFAFAFA |  |
| secondaryContainer | 0xFF00FF00 |  |
| onSecondaryContainer | 0xFFFAFAFA |  |
| tertiary | 0xFFFFFFFF |  |
| onTertiary | 0xFF00FF00 |  |
| tertiaryContainer | 0xFFFFFFFF |  |
| onTertiaryContainer | 0xFF00FF00 |  |
| background | 0xFF111111 |  |
| onBackground | 0xFFFFFFFF |  |
| surface | 0xFF111111 |  |
| onSurface | 0xFFFFFFFF |  |
| surfaceVariant | 0xFF212121 |  |
| surfaceTint | 0xFF00FF00 |  |
| inverseSurface | 0xFFFAFAFA |  |
| inverseOnSurface | 0xFF313131 |  |
| outline | 0xFF00FF00 |  |
| inversePrimary | 0xFF007700 |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### MatrixColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF00FF00 |  |
| onPrimary | 0xFF000000 |  |
| primaryContainer | 0xFF00FF00 |  |
| onPrimaryContainer | 0xFF000000 |  |
| secondary | 0xFF00FF00 |  |
| onSecondary | 0xFF000000 |  |
| secondaryContainer | 0xFF00FF00 |  |
| onSecondaryContainer | 0xFF000000 |  |
| tertiary | 0xFF000000 |  |
| onTertiary | 0xFF00FF00 |  |
| tertiaryContainer | 0xFF000000 |  |
| onTertiaryContainer | 0xFF00FF00 |  |
| background | 0xFF000000 |  |
| onBackground | 0xFFFFFFFF |  |
| surface | 0xFF000000 |  |
| onSurface | 0xFFFFFFFF |  |
| surfaceVariant | 0xFF111111 |  |
| surfaceTint | 0xFF00FF00 |  |
| inverseSurface | 0xFF424242 |  |
| inverseOnSurface | 0xFFFAFAFA |  |
| outline | 0xFF00FF00 |  |
| inversePrimary | 0xFF007700 |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### MidnightDuskColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFF02475 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFBD1C5C |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| inversePrimary | 0xFFF02475 |  |
| secondary | 0xFFF02475 | Unread badge |
| onSecondary | 0xFF16151D | Unread badge text |
| secondaryContainer | 0xFF66183C | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFF02475 | Navigation bar selector icon |
| tertiary | 0xFF55971C | Downloaded badge |
| onTertiary | 0xFF16151D | Downloaded badge text |
| tertiaryContainer | 0xFF386412 |  |
| onTertiaryContainer | 0xFFE5E1E5 |  |
| background | 0xFF16151D |  |
| onBackground | 0xFFE5E1E5 |  |
| surface | 0xFF16151D |  |
| onSurface | 0xFFE5E1E5 |  |
| surfaceVariant | 0xFF281624 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFD6C1C4 |  |
| surfaceTint | 0xFFF02475 |  |
| inverseSurface | 0xFF333043 |  |
| inverseOnSurface | 0xFFFFFFFF |  |
| outline | 0xFF9F8C8F |  |
| surfaceContainerLowest | 0xFF221320 |  |
| surfaceContainerLow | 0xFF251522 |  |
| surfaceContainer | 0xFF281624 | Navigation bar background |
| surfaceContainerHigh | 0xFF2D1C2A |  |
| surfaceContainerHighest | 0xFF2F1F2C |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### MidnightDuskColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFBB0054 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFFFD9E1 |  |
| onPrimaryContainer | 0xFF3F0017 |  |
| inversePrimary | 0xFFFFB1C4 |  |
| secondary | 0xFFBB0054 | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFFEFBAD4 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFD1377C | Navigation bar selector icon |
| tertiary | 0xFF006638 | Downloaded badge |
| onTertiary | 0xFFFFFFFF | Downloaded badge text |
| tertiaryContainer | 0xFF00894B |  |
| onTertiaryContainer | 0xFF2D1600 |  |
| background | 0xFFFFFBFF |  |
| onBackground | 0xFF1C1B1F |  |
| surface | 0xFFFFFBFF |  |
| onSurface | 0xFF1C1B1F |  |
| surfaceVariant | 0xFFF9E6F1 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF524346 |  |
| surfaceTint | 0xFFBB0054 |  |
| inverseSurface | 0xFF313033 |  |
| inverseOnSurface | 0xFFF4F0F4 |  |
| outline | 0xFF847376 |  |
| surfaceContainerLowest | 0xFFDAC0CD |  |
| surfaceContainerLow | 0xFFE8D1DD |  |
| surfaceContainer | 0xFFF9E6F1 | Navigation bar background |
| surfaceContainerHigh | 0xFFFCF3F8 |  |
| surfaceContainerHighest | 0xFFFEF9FC |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### MochaColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFEBC248 |  |
| onPrimary | 0xFF3D2F00 |  |
| primaryContainer | 0xFF584400 |  |
| onPrimaryContainer | 0xFFFFE08D |  |
| secondary | 0xFFEBC248 |  |
| onSecondary | 0xFF3D2F00 |  |
| secondaryContainer | 0xFF584400 |  |
| onSecondaryContainer | 0xFFFFE08D |  |
| tertiary | 0xFFADCFAD |  |
| onTertiary | 0xFF19361F |  |
| tertiaryContainer | 0xFF304D34 |  |
| onTertiaryContainer | 0xFFC9ECC8 |  |
| background | 0xFF1E1B16 |  |
| onBackground | 0xFFE8E1D9 |  |
| surface | 0xFF1E1B16 |  |
| onSurface | 0xFFE8E1D9 |  |
| surfaceVariant | 0xFF4C4639 |  |
| onSurfaceVariant | 0xFFCFC5B4 |  |
| surfaceTint | 0xFFEBC248 |  |
| inverseSurface | 0xFFEDE0DD |  |
| inverseOnSurface | 0xFF211A18 |  |
| outline | 0xFF989080 |  |
| inversePrimary | 0xFFAE3200 |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### MochaColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF745B00 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFFFE08D |  |
| onPrimaryContainer | 0xFF241A00 |  |
| secondary | 0xFF745B00 |  |
| onSecondary | 0xFFFFFFFF |  |
| secondaryContainer | 0xFFFFE08D |  |
| onSecondaryContainer | 0xFF241A00 |  |
| tertiary | 0xFF47664A |  |
| onTertiary | 0xFFFFFFFF |  |
| tertiaryContainer | 0xFFC9ECC8 |  |
| onTertiaryContainer | 0xFF04210B |  |
| background | 0xFFFFFBFF |  |
| onBackground | 0xFF1E1B16 |  |
| surface | 0xFFFFFBFF |  |
| onSurface | 0xFF1E1B16 |  |
| surfaceVariant | 0xFFEBE1CF |  |
| onSurfaceVariant | 0xFF4C4639 |  |
| surfaceTint | 0xFF745B00 |  |
| inverseSurface | 0xFF362F2D |  |
| inverseOnSurface | 0xFFFBEAEB |  |
| outline | 0xFF7E7667 |  |
| inversePrimary | 0xFFFFB59D |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### MonochromeColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFFFFFFF |  |
| onPrimary | 0xFF000000 |  |
| primaryContainer | 0xFFFFFFFF |  |
| onPrimaryContainer | 0xFF000000 |  |
| secondary | 0xFFFFFFFF |  |
| onSecondary | 0xFF000000 |  |
| secondaryContainer | 0xFF777777 |  |
| onSecondaryContainer | 0xFF000000 |  |
| tertiary | 0xFF777777 |  |
| onTertiary | 0xFFFFFFFF |  |
| tertiaryContainer | 0xFFFFFFFF |  |
| onTertiaryContainer | 0xFF000000 |  |
| error | 0xFFFFFFFF |  |
| onError | 0xFF000000 |  |
| errorContainer | 0xFFFFFFFF |  |
| onErrorContainer | 0xFF000000 |  |
| background | 0xFF000000 |  |
| onBackground | 0xFFFFFFFF |  |
| surface | 0xFF000000 |  |
| onSurface | 0xFFFFFFFF |  |
| surfaceVariant | 0xFF000000 |  |
| onSurfaceVariant | 0xFFFFFFFF |  |
| outline | 0xFFFFFFFF |  |
| outlineVariant | 0xFFFFFFFF |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFFFFFFFF |  |
| inverseOnSurface | 0xFF000000 |  |
| inversePrimary | 0xFF000000 |  |
| surfaceDim | 0xFF000000 |  |
| surfaceBright | 0xFFFFFFFF |  |
| surfaceContainerLowest | 0xFF000000 |  |
| surfaceContainerLow | 0xFF000000 |  |
| surfaceContainer | 0xFF000000 |  |
| surfaceContainerHigh | 0xFF000000 |  |
| surfaceContainerHighest | 0xFF000000 |  |

Omitted: surfaceTint

#### MonochromeColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF000000 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFF000000 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| secondary | 0xFF000000 |  |
| onSecondary | 0xFFFFFFFF |  |
| secondaryContainer | 0xFF888888 |  |
| onSecondaryContainer | 0xFFFFFFFF |  |
| tertiary | 0xFF888888 |  |
| onTertiary | 0xFFFFFFFF |  |
| tertiaryContainer | 0xFF000000 |  |
| onTertiaryContainer | 0xFFFFFFFF |  |
| error | 0xFF000000 |  |
| onError | 0xFFFFFFFF |  |
| errorContainer | 0xFF000000 |  |
| onErrorContainer | 0xFFFFFFFF |  |
| background | 0xFFFFFFFF |  |
| onBackground | 0xFF000000 |  |
| surface | 0xFFFFFFFF |  |
| onSurface | 0xFF000000 |  |
| surfaceVariant | 0xFFFFFFFF |  |
| onSurfaceVariant | 0xFF000000 |  |
| outline | 0xFF000000 |  |
| outlineVariant | 0xFF000000 |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFF000000 |  |
| inverseOnSurface | 0xFFFFFFFF |  |
| inversePrimary | 0xFFFFFFFF |  |
| surfaceDim | 0xFFFFFFFF |  |
| surfaceBright | 0xFFFFFFFF |  |
| surfaceContainerLowest | 0xFFFFFFFF |  |
| surfaceContainerLow | 0xFFFFFFFF |  |
| surfaceContainer | 0xFFFFFFFF |  |
| surfaceContainerHigh | 0xFFFFFFFF |  |
| surfaceContainerHighest | 0xFFFFFFFF |  |

Omitted: surfaceTint

#### NordColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF88C0D0 |  |
| onPrimary | 0xFF2E3440 |  |
| primaryContainer | 0xFF88C0D0 |  |
| onPrimaryContainer | 0xFF2E3440 |  |
| inversePrimary | 0xFF397E91 |  |
| secondary | 0xFF81A1C1 | Unread badge |
| onSecondary | 0xFF2E3440 | Unread badge text |
| secondaryContainer | 0xFF506275 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF88C0D0 | Navigation bar selector icon |
| tertiary | 0xFF5E81AC | Downloaded badge |
| onTertiary | 0xFF000000 | Downloaded badge text |
| tertiaryContainer | 0xFF5E81AC |  |
| onTertiaryContainer | 0xFF000000 |  |
| background | 0xFF2E3440 |  |
| onBackground | 0xFFECEFF4 |  |
| surface | 0xFF2E3440 |  |
| onSurface | 0xFFECEFF4 |  |
| surfaceVariant | 0xFF414C5C | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFECEFF4 |  |
| surfaceTint | 0xFF88C0D0 |  |
| inverseSurface | 0xFFD8DEE9 |  |
| inverseOnSurface | 0xFF2E3440 |  |
| outline | 0xFF6D717B |  |
| outlineVariant | 0xFF90939A |  |
| onError | 0xFF2E3440 |  |
| errorContainer | 0xFFBF616A |  |
| onErrorContainer | 0xFF000000 |  |
| surfaceContainerLowest | 0xFF373F4D |  |
| surfaceContainerLow | 0xFF3E4756 |  |
| surfaceContainer | 0xFF414C5C |  |
| surfaceContainerHigh | 0xFF4E5766 |  |
| surfaceContainerHighest | 0xFF505968 | Navigation bar background |

Omitted: error, scrim, surfaceBright, surfaceDim

#### NordColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF5E81AC |  |
| onPrimary | 0xFF000000 |  |
| primaryContainer | 0xFF5E81AC |  |
| onPrimaryContainer | 0xFF000000 |  |
| inversePrimary | 0xFF8CA8CD |  |
| secondary | 0xFF81A1C1 | Unread badge |
| onSecondary | 0xFF2E3440 | Unread badge text |
| secondaryContainer | 0xFF91B4D7 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF2E3440 | Navigation bar selector icon |
| tertiary | 0xFF88C0D0 | Downloaded badge |
| onTertiary | 0xFF2E3440 | Downloaded badge text |
| tertiaryContainer | 0xFF88C0D0 |  |
| onTertiaryContainer | 0xFF2E3440 |  |
| background | 0xFFECEFF4 |  |
| onBackground | 0xFF2E3440 |  |
| surface | 0xFFE5E9F0 |  |
| onSurface | 0xFF2E3440 |  |
| surfaceVariant | 0xFFDAE0EA | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF2E3440 |  |
| surfaceTint | 0xFF5E81AC |  |
| inverseSurface | 0xFF3B4252 |  |
| inverseOnSurface | 0xFFECEFF4 |  |
| outline | 0xFF2E3440 |  |
| onError | 0xFFECEFF4 |  |
| errorContainer | 0xFFBF616A |  |
| onErrorContainer | 0xFF000000 |  |
| surfaceContainerLowest | 0xFFD1D7E0 |  |
| surfaceContainerLow | 0xFFD6DCE6 |  |
| surfaceContainer | 0xFFDAE0EA | Navigation bar background |
| surfaceContainerHigh | 0xFFE9EDF3 |  |
| surfaceContainerHighest | 0xFFF2F4F8 |  |

Omitted: error, outlineVariant, scrim, surfaceBright, surfaceDim

#### SapphireColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF1E88E5 |  |
| onPrimary | 0xFFFAFAFA |  |
| primaryContainer | 0xFF1E88E5 |  |
| onPrimaryContainer | 0xFFFAFAFA |  |
| inversePrimary | 0xFF2979FF | Assuming 'inversePrimary' maps to 'sapphire_primaryInverse' |
| secondary | 0xFF1E88E5 |  |
| onSecondary | 0xFFFAFAFA |  |
| secondaryContainer | 0xFF1E88E5 |  |
| onSecondaryContainer | 0xFFFAFAFA |  |
| tertiary | 0xFF212121 |  |
| onTertiary | 0xFF1E88E5 |  |
| tertiaryContainer | 0xFF212121 |  |
| onTertiaryContainer | 0xFF1E88E5 |  |
| background | 0xFF212121 |  |
| onBackground | 0xFFFFFFFF |  |
| surface | 0xFF212121 |  |
| onSurface | 0xFFFFFFFF |  |
| surfaceVariant | 0xFF424242 |  |
| surfaceTint | 0xFF1E88E5 | Assuming 'surfaceTint' maps to 'sapphire_primary' or similar |
| inverseSurface | 0xFFFAFAFA |  |
| inverseOnSurface | 0xFF313131 |  |
| outline | 0xFF1E88E5 |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### SapphireColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF1E88E5 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFF1E88E5 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| inversePrimary | 0xFF2979FF | Assuming 'inversePrimary' maps to 'sapphire_primaryInverse' |
| secondary | 0xFF1E88E5 |  |
| onSecondary | 0xFFFFFFFF |  |
| secondaryContainer | 0xFF1E88E5 |  |
| onSecondaryContainer | 0xFFFFFFFF |  |
| tertiary | 0xFFE1F5FE |  |
| onTertiary | 0xFF1E88E5 |  |
| tertiaryContainer | 0xFFE1F5FE |  |
| onTertiaryContainer | 0xFF1E88E5 |  |
| background | 0xFFFFFFFF |  |
| onBackground | 0xFF212121 |  |
| surface | 0xFFFFFFFF |  |
| onSurface | 0xFF212121 |  |
| surfaceVariant | 0xFFB3E5FC |  |
| surfaceTint | 0xFF1E88E5 | Assuming 'surfaceTint' maps to 'sapphire_primary' or similar |
| inverseSurface | 0xFF424242 |  |
| inverseOnSurface | 0xFFFAFAFA |  |
| outline | 0xFF1E88E5 |  |

Omitted: onSurfaceVariant, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest, surfaceDim

#### StrawberryColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFFFB2B8 |  |
| onPrimary | 0xFF67001D |  |
| primaryContainer | 0xFFD53855 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| secondary | 0xFFED4A65 | Unread badge |
| onSecondary | 0xFF201A1A | Unread badge text |
| secondaryContainer | 0xFF91002A | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFFFFFFF | Navigation bar selector icon |
| tertiary | 0xFFE8C08E | Downloaded badge |
| onTertiary | 0xFF201A1A | Downloaded badge text |
| tertiaryContainer | 0xFF775930 |  |
| onTertiaryContainer | 0xFFFFF7F1 |  |
| error | 0xFFFFB4AB |  |
| onError | 0xFF690005 |  |
| errorContainer | 0xFF93000A |  |
| onErrorContainer | 0xFFFFDAD6 |  |
| background | 0xFF201A1A |  |
| onBackground | 0xFFF7DCDD |  |
| surface | 0xFF201A1A |  |
| onSurface | 0xFFF7DCDD |  |
| surfaceVariant | 0xFF322727 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFE1BEC0 |  |
| outline | 0xFFA9898B |  |
| outlineVariant | 0xFF594042 |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFFF7DCDD |  |
| inverseOnSurface | 0xFF3D2C2D |  |
| inversePrimary | 0xFFB61F40 |  |
| surfaceDim | 0xFF1D1011 |  |
| surfaceBright | 0xFF463536 |  |
| surfaceContainerLowest | 0xFF2C2222 |  |
| surfaceContainerLow | 0xFF302525 |  |
| surfaceContainer | 0xFF322727 | Navigation bar background |
| surfaceContainerHigh | 0xFF3C2F2F |  |
| surfaceContainerHighest | 0xFF463737 |  |

Omitted: surfaceTint

#### StrawberryColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFA10833 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFD53855 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| secondary | 0xFFA10833 | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFFD53855 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFF6EAED | Navigation bar selector icon |
| tertiary | 0xFF5F441D | Downloaded badge |
| onTertiary | 0xFFFFFFFF | Downloaded badge text |
| tertiaryContainer | 0xFF87683D |  |
| onTertiaryContainer | 0xFFFFFFFF |  |
| error | 0xFFBA1A1A |  |
| onError | 0xFFFFFFFF |  |
| errorContainer | 0xFFFFDAD6 |  |
| onErrorContainer | 0xFF410002 |  |
| background | 0xFFFAFAFA |  |
| onBackground | 0xFF261819 |  |
| surface | 0xFFFAFAFA |  |
| onSurface | 0xFF261819 |  |
| surfaceVariant | 0xFFF6EAED | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF594042 |  |
| outline | 0xFF8D7071 |  |
| outlineVariant | 0xFFE1BEC0 |  |
| scrim | 0xFF000000 |  |
| inverseSurface | 0xFF3D2C2D |  |
| inverseOnSurface | 0xFFFFECED |  |
| inversePrimary | 0xFFFFB2B8 |  |
| surfaceDim | 0xFFEED4D5 |  |
| surfaceBright | 0xFFFFF8F7 |  |
| surfaceContainerLowest | 0xFFF7DCDD |  |
| surfaceContainerLow | 0xFFFDE2E3 |  |
| surfaceContainer | 0xFFF6EAED | Navigation bar background |
| surfaceContainerHigh | 0xFFFFF0F0 |  |
| surfaceContainerHighest | 0xFFFFFFFF |  |

Omitted: surfaceTint

#### TachiyomiColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFB0C6FF |  |
| onPrimary | 0xFF002D6E |  |
| primaryContainer | 0xFF00429B |  |
| onPrimaryContainer | 0xFFD9E2FF |  |
| inversePrimary | 0xFF0058CA |  |
| secondary | 0xFFB0C6FF | Unread badge |
| onSecondary | 0xFF002D6E | Unread badge text |
| secondaryContainer | 0xFF00429B | Navigation bar selector pill & pro |
| onSecondaryContainer | 0xFFD9E2FF | Navigation bar selector icon |
| tertiary | 0xFF7ADC77 | Downloaded badge |
| onTertiary | 0xFF003909 | Downloaded badge text |
| tertiaryContainer | 0xFF005312 |  |
| onTertiaryContainer | 0xFF95F990 |  |
| background | 0xFF1B1B1F |  |
| onBackground | 0xFFE3E2E6 |  |
| surface | 0xFF1B1B1F |  |
| onSurface | 0xFFE3E2E6 |  |
| surfaceVariant | 0xFF211F26 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFC5C6D0 |  |
| surfaceTint | 0xFFB0C6FF |  |
| inverseSurface | 0xFFE3E2E6 |  |
| inverseOnSurface | 0xFF1B1B1F |  |
| error | 0xFFFFB4AB |  |
| onError | 0xFF690005 |  |
| errorContainer | 0xFF93000A |  |
| onErrorContainer | 0xFFFFDAD6 |  |
| outline | 0xFF8F9099 |  |
| outlineVariant | 0xFF44464F |  |
| surfaceContainerLowest | 0xFF1A181D |  |
| surfaceContainerLow | 0xFF1E1C22 |  |
| surfaceContainer | 0xFF211F26 | Navigation bar background |
| surfaceContainerHigh | 0xFF292730 |  |
| surfaceContainerHighest | 0xFF302E38 |  |

Omitted: scrim, surfaceBright, surfaceDim

#### TachiyomiColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF0058CA |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFD9E2FF |  |
| onPrimaryContainer | 0xFF001945 |  |
| inversePrimary | 0xFFB0C6FF |  |
| secondary | 0xFF0058CA | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFFD9E2FF | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF001945 | Navigation bar selector icon |
| tertiary | 0xFF006E1B | Downloaded badge |
| onTertiary | 0xFFFFFFFF | Downloaded badge text |
| tertiaryContainer | 0xFF95F990 |  |
| onTertiaryContainer | 0xFF002203 |  |
| background | 0xFFFEFBFF |  |
| onBackground | 0xFF1B1B1F |  |
| surface | 0xFFFEFBFF |  |
| onSurface | 0xFF1B1B1F |  |
| surfaceVariant | 0xFFF3EDF7 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF44464F |  |
| surfaceTint | 0xFF0058CA |  |
| inverseSurface | 0xFF303034 |  |
| inverseOnSurface | 0xFFF2F0F4 |  |
| error | 0xFFBA1A1A |  |
| onError | 0xFFFFFFFF |  |
| errorContainer | 0xFFFFDAD6 |  |
| onErrorContainer | 0xFF410002 |  |
| outline | 0xFF757780 |  |
| outlineVariant | 0xFFC5C6D0 |  |
| surfaceContainerLowest | 0xFFF5F1F8 |  |
| surfaceContainerLow | 0xFFF7F2FA |  |
| surfaceContainer | 0xFFF3EDF7 | Navigation bar background |
| surfaceContainerHigh | 0xFFFCF7FF |  |
| surfaceContainerHighest | 0xFFFCF7FF |  |

Omitted: scrim, surfaceBright, surfaceDim

#### TakoColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFF3B375 |  |
| onPrimary | 0xFF38294E |  |
| primaryContainer | 0xFFF3B375 |  |
| onPrimaryContainer | 0xFF38294E |  |
| inversePrimary | 0xFF84531E |  |
| secondary | 0xFFF3B375 | Unread badge |
| onSecondary | 0xFF38294E | Unread badge text |
| secondaryContainer | 0xFF5C4D4B | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFF3B375 | Navigation bar selector icon |
| tertiary | 0xFF66577E | Downloaded badge |
| onTertiary | 0xFFF3B375 | Downloaded badge text |
| tertiaryContainer | 0xFF4E4065 |  |
| onTertiaryContainer | 0xFFEDDCFF |  |
| background | 0xFF21212E |  |
| onBackground | 0xFFE3E0F2 |  |
| surface | 0xFF21212E |  |
| onSurface | 0xFFE3E0F2 |  |
| surfaceVariant | 0xFF2A2A3C | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFCBC4CE |  |
| surfaceTint | 0xFF66577E |  |
| inverseSurface | 0xFFE5E1E6 |  |
| inverseOnSurface | 0xFF1B1B1E |  |
| outline | 0xFF958F99 |  |
| surfaceContainerLowest | 0xFF20202E |  |
| surfaceContainerLow | 0xFF262636 |  |
| surfaceContainer | 0xFF2A2A3C | Navigation bar background |
| surfaceContainerHigh | 0xFF303044 |  |
| surfaceContainerHighest | 0xFF36364D |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### TakoColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF66577E |  |
| onPrimary | 0xFFF3B375 |  |
| primaryContainer | 0xFF66577E |  |
| onPrimaryContainer | 0xFFF3B375 |  |
| inversePrimary | 0xFFD6BAFF |  |
| secondary | 0xFF66577E | Unread badge |
| onSecondary | 0xFFF3B375 | Unread badge text |
| secondaryContainer | 0xFFC8BED0 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF66577E | Navigation bar selector icon |
| tertiary | 0xFFF3B375 | Downloaded badge |
| onTertiary | 0xFF574360 | Downloaded badge text |
| tertiaryContainer | 0xFFFDD6B0 |  |
| onTertiaryContainer | 0xFF221437 |  |
| background | 0xFFF7F5FF |  |
| onBackground | 0xFF1B1B22 |  |
| surface | 0xFFF7F5FF |  |
| onSurface | 0xFF1B1B22 |  |
| surfaceVariant | 0xFFE8E0EB | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF49454E |  |
| surfaceTint | 0xFF66577E |  |
| inverseSurface | 0xFF313033 |  |
| inverseOnSurface | 0xFFF3EFF4 |  |
| outline | 0xFF7A757E |  |
| surfaceContainerLowest | 0xFFD7D0DA |  |
| surfaceContainerLow | 0xFFDFD8E2 |  |
| surfaceContainer | 0xFFE8E0EB | Navigation bar background |
| surfaceContainerHigh | 0xFFEEE6F1 |  |
| surfaceContainerHighest | 0xFFF7EEFA |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### TealTurqoiseColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF40E0D0 |  |
| onPrimary | 0xFF000000 |  |
| primaryContainer | 0xFF40E0D0 |  |
| onPrimaryContainer | 0xFF000000 |  |
| inversePrimary | 0xFF008080 |  |
| secondary | 0xFF40E0D0 | Unread badge |
| onSecondary | 0xFF000000 | Unread badge text |
| secondaryContainer | 0xFF18544E | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF40E0D0 | Navigation bar selector icon |
| tertiary | 0xFFBF1F2F | Downloaded badge |
| onTertiary | 0xFFFFFFFF | Downloaded badge text |
| tertiaryContainer | 0xFF200508 |  |
| onTertiaryContainer | 0xFFBF1F2F |  |
| background | 0xFF202125 |  |
| onBackground | 0xFFDFDEDA |  |
| surface | 0xFF202125 |  |
| onSurface | 0xFFDFDEDA |  |
| surfaceVariant | 0xFF233133 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFDFDEDA |  |
| surfaceTint | 0xFF40E0D0 |  |
| inverseSurface | 0xFFDFDEDA |  |
| inverseOnSurface | 0xFF202125 |  |
| outline | 0xFF899391 |  |
| surfaceContainerLowest | 0xFF202C2E |  |
| surfaceContainerLow | 0xFF222F31 |  |
| surfaceContainer | 0xFF233133 | Navigation bar background |
| surfaceContainerHigh | 0xFF28383A |  |
| surfaceContainerHighest | 0xFF2F4244 |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### TealTurqoiseColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF008080 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFF008080 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| inversePrimary | 0xFF40E0D0 |  |
| secondary | 0xFF008080 | Unread badge text |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFFCFE5E4 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF008080 | Navigation bar selector icon |
| tertiary | 0xFFFF7F7F | Downloaded badge |
| onTertiary | 0xFF000000 | Downloaded badge text |
| tertiaryContainer | 0xFF2A1616 |  |
| onTertiaryContainer | 0xFFFF7F7F |  |
| background | 0xFFFAFAFA |  |
| onBackground | 0xFF050505 |  |
| surface | 0xFFFAFAFA |  |
| onSurface | 0xFF050505 |  |
| surfaceVariant | 0xFFEBF3F1 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF050505 |  |
| surfaceTint | 0xFFBFDFDF |  |
| inverseSurface | 0xFF050505 |  |
| inverseOnSurface | 0xFFFAFAFA |  |
| outline | 0xFF6F7977 |  |
| surfaceContainerLowest | 0xFFE1E9E7 |  |
| surfaceContainerLow | 0xFFE6EEEC |  |
| surfaceContainer | 0xFFEBF3F1 | Navigation bar background |
| surfaceContainerHigh | 0xFFF0F8F6 |  |
| surfaceContainerHighest | 0xFFF7FFFD |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### TidalWaveColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF5ED4FC |  |
| onPrimary | 0xFF003544 |  |
| primaryContainer | 0xFF004D61 |  |
| onPrimaryContainer | 0xFFB8EAFF |  |
| inversePrimary | 0xFFA12B03 |  |
| secondary | 0xFF5ED4FC | Unread badge |
| onSecondary | 0xFF003544 | Unread badge text |
| secondaryContainer | 0xFF004D61 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFB8EAFF | Navigation bar selector icon |
| tertiary | 0xFF92F7BC | Downloaded badge |
| onTertiary | 0xFF001C3B | Downloaded badge text |
| tertiaryContainer | 0xFFC3FADA |  |
| onTertiaryContainer | 0xFF78FFD6 |  |
| background | 0xFF001C3B |  |
| onBackground | 0xFFD5E3FF |  |
| surface | 0xFF001C3B |  |
| onSurface | 0xFFD5E3FF |  |
| surfaceVariant | 0xFF082B4B | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFBFC8CC |  |
| surfaceTint | 0xFF5ED4FC |  |
| inverseSurface | 0xFFFFE3C4 |  |
| inverseOnSurface | 0xFF001C3B |  |
| outline | 0xFF8A9296 |  |
| surfaceContainerLowest | 0xFF072642 |  |
| surfaceContainerLow | 0xFF072947 |  |
| surfaceContainer | 0xFF082B4B | Navigation bar background |
| surfaceContainerHigh | 0xFF093257 |  |
| surfaceContainerHighest | 0xFF0A3861 |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### TidalWaveColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF006780 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFB4D4DF |  |
| onPrimaryContainer | 0xFF001F28 |  |
| inversePrimary | 0xFFFF987F |  |
| secondary | 0xFF006780 | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFF9AE1FF | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF001F28 | Navigation bar selector icon |
| tertiary | 0xFF92F7BC | Downloaded badge |
| onTertiary | 0xFF001C3B | Downloaded badge text |
| tertiaryContainer | 0xFFC3FADA |  |
| onTertiaryContainer | 0xFF78FFD6 |  |
| background | 0xFFFDFBFF |  |
| onBackground | 0xFF001C3B |  |
| surface | 0xFFFDFBFF |  |
| onSurface | 0xFF001C3B |  |
| surfaceVariant | 0xFFE8EFF5 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF40484C |  |
| surfaceTint | 0xFF006780 |  |
| inverseSurface | 0xFF020400 |  |
| inverseOnSurface | 0xFFFFE3C4 |  |
| outline | 0xFF70787C |  |
| surfaceContainerLowest | 0xFFE2E8EC |  |
| surfaceContainerLow | 0xFFE5ECF1 |  |
| surfaceContainer | 0xFFE8EFF5 | Navigation bar background |
| surfaceContainerHigh | 0xFFEDF4FA |  |
| surfaceContainerHighest | 0xFFF5FAFF |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### YinYangColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFFFFFFF |  |
| onPrimary | 0xFF5A5A5A |  |
| primaryContainer | 0xFFFFFFFF |  |
| onPrimaryContainer | 0xFF000000 |  |
| inversePrimary | 0xFFCECECE |  |
| secondary | 0xFFFFFFFF | Unread badge |
| onSecondary | 0xFF5A5A5A | Unread badge text |
| secondaryContainer | 0xFF717171 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFE4E4E4 | Navigation bar selector icon |
| tertiary | 0xFF000000 | Downloaded badge |
| onTertiary | 0xFFFFFFFF | Downloaded badge text |
| tertiaryContainer | 0xFF00419E |  |
| onTertiaryContainer | 0xFFD8E2FF |  |
| background | 0xFF1E1E1E |  |
| onBackground | 0xFFE6E6E6 |  |
| surface | 0xFF1E1E1E |  |
| onSurface | 0xFFE6E6E6 |  |
| surfaceVariant | 0xFF313131 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFD1D1D1 |  |
| surfaceTint | 0xFFFFFFFF |  |
| inverseSurface | 0xFFE6E6E6 |  |
| inverseOnSurface | 0xFF1E1E1E |  |
| outline | 0xFF999999 |  |
| surfaceContainerLowest | 0xFF2A2A2A |  |
| surfaceContainerLow | 0xFF2D2D2D |  |
| surfaceContainer | 0xFF313131 | Navigation bar background |
| surfaceContainerHigh | 0xFF383838 |  |
| surfaceContainerHighest | 0xFF3F3F3F |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### YinYangColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFF000000 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFF000000 |  |
| onPrimaryContainer | 0xFFFFFFFF |  |
| inversePrimary | 0xFFA6A6A6 |  |
| secondary | 0xFF000000 | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFFDDDDDD | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF0C0C0C | Navigation bar selector icon |
| tertiary | 0xFFFFFFFF | Downloaded badge |
| onTertiary | 0xFF000000 | Downloaded badge text |
| tertiaryContainer | 0xFFD8E2FF |  |
| onTertiaryContainer | 0xFF001947 |  |
| background | 0xFFFDFDFD |  |
| onBackground | 0xFF222222 |  |
| surface | 0xFFFDFDFD |  |
| onSurface | 0xFF222222 |  |
| surfaceVariant | 0xFFE8E8E8 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF515151 |  |
| surfaceTint | 0xFF000000 |  |
| inverseSurface | 0xFF333333 |  |
| inverseOnSurface | 0xFFF4F4F4 |  |
| outline | 0xFF838383 |  |
| surfaceContainerLowest | 0xFFCFCFCF |  |
| surfaceContainerLow | 0xFFDADADA |  |
| surfaceContainer | 0xFFE8E8E8 | Navigation bar background |
| surfaceContainerHigh | 0xFFECECEC |  |
| surfaceContainerHighest | 0xFFEFEFEF |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### YotsubaColorScheme / dark

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFFFB59D |  |
| onPrimary | 0xFF5F1600 |  |
| primaryContainer | 0xFF862200 |  |
| onPrimaryContainer | 0xFFFFDBCF |  |
| inversePrimary | 0xFFAE3200 |  |
| secondary | 0xFFFFB59D | Unread badge |
| onSecondary | 0xFF5F1600 | Unread badge text |
| secondaryContainer | 0xFF862200 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFFFFDBCF | Navigation bar selector icon |
| tertiary | 0xFFD7C68D | Downloaded badge |
| onTertiary | 0xFF3A2F05 | Downloaded badge text |
| tertiaryContainer | 0xFF524619 |  |
| onTertiaryContainer | 0xFFF5E2A7 |  |
| background | 0xFF211A18 |  |
| onBackground | 0xFFEDE0DD |  |
| surface | 0xFF211A18 |  |
| onSurface | 0xFFEDE0DD |  |
| surfaceVariant | 0xFF332723 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFFD8C2BC |  |
| surfaceTint | 0xFFFFB59D |  |
| inverseSurface | 0xFFEDE0DD |  |
| inverseOnSurface | 0xFF211A18 |  |
| outline | 0xFFA08C87 |  |
| surfaceContainerLowest | 0xFF2E221F |  |
| surfaceContainerLow | 0xFF312521 |  |
| surfaceContainer | 0xFF332723 | Navigation bar background |
| surfaceContainerHigh | 0xFF413531 |  |
| surfaceContainerHighest | 0xFF4C403D |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim

#### YotsubaColorScheme / light

| Token | Value | Source comment |
|---|---|---|
| primary | 0xFFAE3200 |  |
| onPrimary | 0xFFFFFFFF |  |
| primaryContainer | 0xFFFFDBCF |  |
| onPrimaryContainer | 0xFF3B0A00 |  |
| inversePrimary | 0xFFFFB59D |  |
| secondary | 0xFFAE3200 | Unread badge |
| onSecondary | 0xFFFFFFFF | Unread badge text |
| secondaryContainer | 0xFFEBCDC2 | Navigation bar selector pill & progress indicator (remaining) |
| onSecondaryContainer | 0xFF3B0A00 | Navigation bar selector icon |
| tertiary | 0xFF6B5E2F | Downloaded badge |
| onTertiary | 0xFFFFFFFF | Downloaded badge text |
| tertiaryContainer | 0xFFF5E2A7 |  |
| onTertiaryContainer | 0xFF231B00 |  |
| background | 0xFFFCFCFC |  |
| onBackground | 0xFF211A18 |  |
| surface | 0xFFFCFCFC |  |
| onSurface | 0xFF211A18 |  |
| surfaceVariant | 0xFFF6EBE7 | Navigation bar background (ThemePrefWidget) |
| onSurfaceVariant | 0xFF53433F |  |
| surfaceTint | 0xFFAE3200 |  |
| inverseSurface | 0xFF362F2D |  |
| inverseOnSurface | 0xFFFBEEEB |  |
| outline | 0xFF85736E |  |
| surfaceContainerLowest | 0xFFECE3E0 |  |
| surfaceContainerLow | 0xFFF1E7E4 |  |
| surfaceContainer | 0xFFF6EBE7 | Navigation bar background |
| surfaceContainerHigh | 0xFFFAF4F2 |  |
| surfaceContainerHighest | 0xFFFBF6F4 |  |

Omitted: error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim
