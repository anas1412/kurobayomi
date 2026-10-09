import re, os, sys
SRC = "/home/blackbox/aniyomi-src/app/src/main/java/eu/kanade/presentation/theme/colorscheme"
TOKENS = ["primary","onPrimary","primaryContainer","onPrimaryContainer","inversePrimary","secondary","onSecondary","secondaryContainer","onSecondaryContainer","tertiary","onTertiary","tertiaryContainer","onTertiaryContainer","background","onBackground","surface","onSurface","surfaceVariant","onSurfaceVariant","surfaceTint","inverseSurface","inverseOnSurface","error","onError","errorContainer","onErrorContainer","outline","outlineVariant","scrim","surfaceBright","surfaceContainer","surfaceContainerHigh","surfaceContainerHighest","surfaceContainerLow","surfaceContainerLowest","surfaceDim","primaryFixed","primaryFixedDim","onPrimaryFixed","onPrimaryFixedVariant","secondaryFixed","secondaryFixedDim","onSecondaryFixed","onSecondaryFixedVariant","tertiaryFixed","tertiaryFixedDim","onTertiaryFixed","onTertiaryFixedVariant"]
files = sorted(f for f in os.listdir(SRC) if f.endswith("ColorScheme.kt") and f not in ("BaseColorScheme.kt","MonetColorScheme.kt"))
out = []
out.append("| Theme (object) | Source file | Mode | Explicit tokens | Omitted tokens (fall back to androidx M3 baseline) |")
out.append("|---|---|---|---|---|")
detail = []
for f in files:
    text = open(os.path.join(SRC, f), encoding="utf-8").read()
    name = re.search(r"internal object (\w+)", text).group(1)
    for mode, fn in (("dark","darkScheme"),("light","lightScheme")):
        m = re.search(r"override val %s = (?:dark|light)ColorScheme\((.*?)\n    \)" % fn, text, re.S)
        body = m.group(1)
        pairs = re.findall(r"(\w+)\s*=\s*Color\((0x[0-9A-Fa-f]{8})\)", body)
        explicit = {k: v.upper().replace("0X","0x") for k, v in pairs}
        omitted = [t for t in TOKENS if t not in explicit and t not in ("primaryFixed","primaryFixedDim","onPrimaryFixed","onPrimaryFixedVariant","secondaryFixed","secondaryFixedDim","onSecondaryFixed","onSecondaryFixedVariant","tertiaryFixed","tertiaryFixedDim","onTertiaryFixed","onTertiaryFixedVariant")]
        out.append(f"| {name} | app/src/main/java/eu/kanade/presentation/theme/colorscheme/{f} | {mode} | {len(explicit)} | {len(omitted)} |")
        detail.append(f"\n#### {name} / {mode}\n")
        detail.append("| Token | Value | Source comment |\n|---|---|---|")
        for line in body.splitlines():
            mm = re.match(r"\s*(\w+)\s*=\s*Color\((0x[0-9A-Fa-f]{8})\),?\s*(//\s*(.*))?$", line)
            if mm:
                detail.append(f"| {mm.group(1)} | {mm.group(2).upper().replace('0X','0x')} | {(mm.group(4) or '').strip()} |")
        detail.append("\nOmitted: " + ", ".join(omitted) if omitted else "\nOmitted: none")
print("\n".join(out))
print("\n".join(detail))
