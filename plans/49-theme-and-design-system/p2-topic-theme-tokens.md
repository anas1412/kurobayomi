# Topic: theme tokens, scheme coverage and XML-versus-Kotlin drift

Deep dive for area 49 part 2. Used by `p2-rust-design.md` (tables, fallback), `p2-edge-cases-and-tests.md` (tests and vectors) and `p2-tasks.md` (T-49b-002, T-49b-004, T-49b-039, T-49b-041).

Root: `/home/blackbox/aniyomi-src/`.

---

## 1. The token model

Compose M3 `ColorScheme` has 36 colour roles (listed in `p2-data-and-config.md` section 1.8). The schemes in this repo use two more keys that exist only in the XML:
- `primaryInverse` (XML) is the same role as Compose `inversePrimary`. The XML names differ in four ways: `cloudflare`, `doom`, `matrix`, `sapphire` (and `cottoncandy`, `mocha` in the app res) use `primaryInverse`; the others use `inversePrimary`. The View theme (`app/src/main/res/values/themes.xml`) reads `colorPrimaryInverse` from the scheme's `*_primaryInverse` or `*_inversePrimary` key, so both names must be accepted.
- `elevationOverlay` (XML) always points at `primary` (`@color/<scheme>_primary`), for example `@color/doom_primary`. It is a reference, not a literal, and has no Compose counterpart.

Rust model: 38 roles (`Role` enum, `ROLE_COUNT = 38`). The alias is resolved at build time: `InversePrimary` falls back to `PrimaryInverseAlias` when absent, and `ElevationOverlay` falls back to `Primary`.

Two sources exist for every scheme:
1. the XML in `presentation-core/src/main/res/` (and `app/src/main/res/` for cottoncandy and mocha) used by the View theme and by Android resource references;
2. the Kotlin in `app/src/main/java/eu/kanade/presentation/theme/colorscheme/` used by Compose.

The Rust port must have one source. Proposal: the Rust tables are generated from the XML (because the XML carries all 38 keys and the alias), and the Kotlin is checked against them by test (T-49b-039).

---

## 2. Drift between the XML and the Kotlin

Method (reproducible): parse every `<color name="<scheme>_<role>">` in the XML (both folders); parse every `override val darkScheme|lightScheme` block in the Kotlin and capture `role = Color(0x...)`; compare per mode for every role that exists on both sides, with `primaryInverse` mapped to `inversePrimary`.

Result:
- 1000 comparisons, 2 mismatches:
  1. `Monochrome`, light, `surfaceDim`: XML `#000000` vs Kotlin `#FFFFFF`. Dark matches (`#000000`).
  2. `Mocha`, light, `inverseOnSurface`: XML `#FBEEEB` vs Kotlin `#FBEAEB`. A 2-unit green difference. Mocha is in `app/src/main/res/` (not in this scope).
- Not compared (the Kotlin literal does not match the parser): `onSurfaceVariant` in `Cloudflare`, `Doom`, `Matrix`, `Sapphire` (light `0xFFD849454E` vs XML `#D849454E`; dark `0xFFD8FFFFFF` vs XML `#D8FFFFFF`). The Kotlin literal is 10 hex digits with a leading `FF`. Compose's `Color(Long)` overload with that literal must be checked (unverified: whether the high byte is ignored, which would make it equal to the XML value). Test T-49b-039 must assert the rendered result.
- Kotlin-only roles (declared in Kotlin, absent in XML): `surfaceTint` (Cloudflare, Doom, Matrix, Sapphire, Mocha, Cotton Candy, Lavender, Green Apple, Monochrome, Strawberry), `onTertiary` (Cotton Candy dark), `inversePrimary` for the schemes that use `primaryInverse`.
- XML-only roles (declared in XML, not used by Kotlin): `elevationOverlay` (all), `primaryInverse` (Cloudflare, Doom, Matrix, Sapphire, Cotton Candy, Mocha), `onSurfaceVariant` (Cloudflare, Doom, Matrix, Sapphire, see above). These XML values are only read by the View theme.

Conclusion: the two sources agree except for the two values above. Decision for the port: use the XML values, and set Monochrome light `surfaceDim` to the Kotlin value `#FFFFFF` (the Compose value is what the user sees today on Android). Record this in the topic table as a deliberate choice (T-49b-041).

---

## 3. Coverage per scheme

Compose side (Kotlin): which of the 36 M3 roles each scheme sets; the rest take the M3 baseline of the pinned Compose version (`darkColorScheme()` / `lightColorScheme()` defaults). The Kotlin-side check is produced by counting `role = Color(` inside each `darkScheme` and `lightScheme` block.

| Scheme | Mode | Roles set by Kotlin (of 36) | Missing in Kotlin (fall back to M3 baseline) |
|---|---|---|---|
| Default | light | 33 | scrim, surfaceBright, surfaceDim |
| Default | dark | 33 | scrim, surfaceBright, surfaceDim |
| Monochrome | light | 35 | surfaceTint |
| Monochrome | dark | 35 | surfaceTint |
| Yotsuba | light | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Yotsuba | dark | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Tako | light | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Tako | dark | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Cloudflare | light | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Cloudflare | dark | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Doom | light | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Doom | dark | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Lavender | light | 35 | surfaceTint |
| Lavender | dark | 35 | surfaceTint |
| Matrix | light | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Matrix | dark | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Sapphire | light | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Sapphire | dark | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Green Apple | light | 35 | surfaceTint |
| Green Apple | dark | 35 | surfaceTint |
| Midnight Dusk | light | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Midnight Dusk | dark | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Nord | light | 31 | error, outlineVariant, scrim, surfaceBright, surfaceDim |
| Nord | dark | 32 | error, scrim, surfaceBright, surfaceDim |
| Strawberry Daiquiri | light | 35 | surfaceTint |
| Strawberry Daiquiri | dark | 35 | surfaceTint |
| Teal & Turquoise | light | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Teal & Turquoise | dark | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Tidal Wave | light | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Tidal Wave | dark | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Yin & Yang | light | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Yin & Yang | dark | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Cotton Candy | light | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Cotton Candy | dark | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Mocha | light | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Mocha | dark | 23 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |

Reading the table: the Compose schemes for Cloudflare, Doom, Matrix, Sapphire, Cotton Candy and Mocha leave 13 M3 roles to the baseline, including `error` and its three partners. In those schemes the error colours are the Material baseline, not the scheme's own. This is observed behaviour, to be kept for parity (decision recorded in T-49b-004 as "replicate, do not improve").

The XML side has the same pattern. For the XML values, see the coverage table at the end of `p2-data-and-config.md` (Appendix B).

---

## 4. The Monet scheme and the GTK theme on Linux

- Monet has no static table. On Android it is `dynamicLightColorScheme` (SDK 31+) or a Material Color Utilities scheme from the wallpaper seed (SDK 27 to 30). On Linux there is no Android wallpaper API. The Linux replacement is the XDG portal accent colour (behavior file section 1.4; rust design section 5.3).
- `GtkTheme` (Linux-only, decision T-49b-006) has no colour tokens at all. The provider is cleared, so the user's GTK theme (for example Tokyo Night) is what the window shows. Dark or light is still applied through `gtk-application-prefer-dark-theme`.

---

## 5. Contrast observations (for the owner of the palettes)
- Monochrome light: `secondaryContainer` `#888888` with `onSecondaryContainer` `#FFFFFF`: about 3.5 to 1 (below the 4.5 to 1 AA threshold for body text).
- Monochrome dark: `secondaryContainer` `#777777` with `onSecondaryContainer` `#000000`: about 4.7 to 1 (passes). Ratios are WCAG 2.x relative luminance.
- Yin and Yang light: `tertiaryContainer` `#00419E` with `onTertiaryContainer` `#D8E2FF` passes (about 7.2 to 1); the `secondaryContainer` `#717171` with `onSecondaryContainer` `#E4E4E4` is about 3.8 to 1 (below AA).
- These are reported by the contrast test (`p2-edge-cases-and-tests.md` section 4.3), not fixed. Changing a palette is a product decision.

---

## 6. Decisions recorded for the port
1. Source of truth: XML values (T-49b-002), with the `primaryInverse` alias and `elevationOverlay` resolved at build.
2. Monochrome light `surfaceDim`: use `#FFFFFF` (the Compose value), not the XML `#000000` (T-49b-041).
3. Mocha light `inverseOnSurface`: use the Compose value `#FBEAEB` (Mocha is out of this scope, noted for area 49 part 1).
4. Missing roles: replicate the Compose baseline (T-49b-004), not the Default scheme's colours. The earlier draft proposal (use the Default table) is rejected because it would change the error colours of six schemes.
5. Default table completeness: `scrim`, `surfaceBright`, `surfaceDim` are absent in XML and Kotlin for Default; the baseline supplies them (T-49b-004).
6. `onSurfaceVariant` 8-digit colours: parse as ARGB; the Compose `Color(Long)` behaviour must be verified before claiming parity (T-49b-039).
