# Topic: settings search index and matching

Covers T-47a-004 and T-47a-007. Source: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSearchScreen.kt` (lines 173-238, 288-341), `app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt` (lines 32-42, 85-97), `app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt` (lines 89-120).

## 1. Index contents

| Index | Screens, in order |
|---|---|
| Main index (`settingScreens`) | Appearance, Library, Reader, Downloads, Tracking, Browse, Data and storage, Security and privacy, Advanced |
| Player index (`playerSettingScreens`) | Player, Gestures, Decoder, Subtitles, Audio, Torrent, Advanced (player) |

Appended order: main index first, then player index (when not in player search). The About screen and the main-menu screens (Appearance is included, About is not) — About is not indexed. Rust keeps this list exactly (behaviour §15).

## 2. Per-screen flatten

```
for screen in index:
    title  = screen.title
    items  = screen.preferences()           # evaluated fresh each search
    for entry in items:
        if entry is Group:
            if !entry.enabled: continue     # disabled group contributes nothing
            for item in entry.items:
                yield (group = entry.title, item)
        else:                               # top-level item
            yield (group = null, entry)

filter:  item.enabled && item.title != ""   (after group filtering)
filter:  item is not InfoPreference
filter:  item.title.contains(q, ignoreCase) || (item.subtitle?.contains(q, ignoreCase) ?: false)
map:     Hit(route = screen, title = item.title,
             breadcrumb = [player-prefix?] + [screen.title] + [group.title?] joined by " > ",
             highlightKey = item.title)
take(10)
```

Filter details (`SettingsSearchScreen.kt:190-236`):
- The `enabled` test on a group (line 198) is applied before its items; the item test (`it.enabled && it.title.isNotBlank()`, line 200) is applied again per item.
- Top-level entries are filtered with `enabled && title.isNotBlank()` (line 193) before the group split.
- `TrackerPreference` has `title = ""` and is dropped here.

## 3. Matching

- `contains(query, ignoreCase = true)` on title and subtitle. Subtitles may be `null`.
- Query is the raw field text, untrimmed. The empty test is `query.is_empty()`, as in Android (`SettingsSearchScreen.kt:182`); a spaces-only query is searched (edge case 5.2).
- Unicode case folding: Rust uses `str::to_lowercase()` on both haystack and needle; additionally compares `to_uppercase()` to cover the Turkish dotted/dotless i edge case (5.4).

## 4. Ordering

Results are **not ranked**. They are in index order, then within a screen in the order of `getPreferences()`, and cut at 10. Consequence: a broad query shows only the first screen's matches (documented parity behaviour; Proposal for a later improvement: rank title matches above subtitle matches).

## 5. Breadcrumb format

- LTR: `join(" > ")`. RTL: `reversed().join(" < ")` (`SettingsSearchScreen.kt:311-319`).
- Player prefix: when the hit comes from the player index during a non-player search, the first node is the Player settings label (`label_player_settings`, "Player settings").

## 6. Navigation and highlight

1. On click: `SearchableSettings.highlightKey = hit.highlightKey`, then `navigator.replace(hit.route)`.
2. The target screen's `PreferenceScreen` reads the key. If the key is found (by title) in the flattened tree, wait 500 ms, then `animateScrollToItem(index)`.
3. The key is cleared immediately after the check (line 40), so a later visit does not re-highlight.
4. Highlight: the row background animates to `surfaceTint` at 0.12 alpha, five pulses (200 ms each, first pulse after 600 ms), total flag time 3 s.

Rust design:
- `SearchIndex` is built once per settings open and rebuilt when any indexed pref changes (cheap: ~230 rows).
- The search is synchronous; no `produceState` equivalent.
- `highlight_key` is a `RefCell<Option<String>>` on the settings context; the target screen takes it on build.

## 7. Test vectors

See `p1-edge-cases-and-tests.md` section 5 (cases 5.1–5.11) and fixture F-6.
