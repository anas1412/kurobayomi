# Area 41 (part 1): worked examples (test vectors)

Hand-computed examples that an implementer can check against the Kotlin logic and the Rust port. Every expected value below follows from the rules in p1-behavior.md; section numbers are given in brackets.

## A. Extension grouping and query (behavior 1.2-1.4)

Settings: `show_nsfw_source = false`, `source_languages = {all, en}`.

Installed (before filters):
- I1 `Zeta`, pkg `com.a.zeta`, `hasUpdate = true`, `isObsolete = false`, lang `en`, sources `[{id 111, name "Zeta EN"}]`
- I2 `alpha`, pkg `com.a.alpha`, `hasUpdate = false`, `isObsolete = false`, sources `[{id 401, name "Alpha Source"}]`
- I3 `Beta`, pkg `com.a.beta`, `hasUpdate = false`, `isObsolete = true`, sources `[{id 402, name "Beta Source"}]`
- I4 `Adult`, pkg `com.a.adult`, `isNsfw = true`

Untrusted: U1 `Gamma`, pkg `com.a.gamma`.

Available:
- V1 `Multi`, pkg `com.v.multi`, sources `[{201, en, "Multi EN"}, {202, de, "Multi DE"}]`
- V2 `Solo`, pkg `com.v.solo`, sources `[{301, all, "Solo All"}]`
- V3 `Zeta`, pkg `com.a.zeta` (same pkg as I1)
- V4 `Gamma`, pkg `com.a.gamma` (same pkg as U1)

Step by step [1.2]:
1. Installed NSFW filter removes I4. Sort `(!isObsolete, name lowercased)`: I2 `alpha` (not obsolete), I1 `zeta` (not obsolete), I3 `beta` (obsolete). Result `[alpha, Zeta, Beta]`. Partition by `hasUpdate`: updates `[Zeta]`, installed `[alpha, Beta]`.
2. Untrusted `[Gamma]`.
3. Available: V3 and V4 are excluded (pkg matches installed I1 and untrusted U1). V1 keeps only source 201 (`en` enabled, 202 `de` not). V2 keeps 301 (`all` enabled). Synthetic pkgs: `com.v.multi-201`, `com.v.solo-301`. Sorted by name: `Multi EN`, `Solo All`.

Groups with empty query [1.3]:
1. `Resource(ext_updates_pending)`: `[Zeta]`
2. `Resource(ext_installed)`: `[alpha, Beta, Gamma]` (untrusted Gamma last, inside Installed)
3. `Text("Multi")`: `[Solo All]` (language `all` sorts first and displays as "Multi")
4. `Text("English")`: `[Multi EN]`

Query `beta, 301` [1.4]: terms `beta` and `301`.
- Updates: Zeta. Name "Zeta", source "Zeta EN" (id 111): no match. Updates group omitted.
- Installed: alpha ("Alpha Source" id 401) no match; Beta matches by name. Gamma untrusted matches only by name: no.
- Available: Solo matches by id 301. Multi EN: no.

Result: `Installed = [Beta]`, `Multi = [Solo All]`. Headers for empty groups are omitted.

Query `0401`: `"0401".toLongOrNull() == 401` -> matches alpha by source id (exact match only). Query `40`: no match (not equal to 401; not a name substring).

Query `, ,`: all terms empty -> nothing matches, every group empty, list shows the empty state with the search text (search active).

## B. Migration flags (behavior 10, data 5.1)

Saved value `22` (`0b10110`). Options for a Seasons-type old anime with custom cover and custom background, downloads present (no Episodes option because fetch type is Seasons):
- Order: CATEGORIES, CUSTOM_COVER, CUSTOM_BACKGROUND, DELETE_DOWNLOADED.
- Defaults: CATEGORIES `0x02 & 22 = 2 != 0` -> selected; CUSTOM_COVER `0x08 & 22 = 0` -> not selected; CUSTOM_BACKGROUND `0x04 & 22 = 4` -> selected; DELETE_DOWNLOADED `0x10 & 22 = 16` -> selected.
- User keeps defaults: `getSelectedFlagsBitMap` = `0x02 | 0x04 | 0x10 = 22`, saved again as `22`.

Saved value `0x7FFFFFFF` (default), same anime: every option selected, including DELETE_DOWNLOADED. For an Episodes-type anime, EPISODES (`0x01`) is also selected.

`canMigrate` example: old Seasons, new Episodes -> `false`. Flags hidden. Message "Unable to migrate season to episode". Buttons: "Show anime" only (because new is Episodes, "Show seasons" is hidden too).

Old Episodes, new Seasons -> `false`. Message "Unable to migrate episode to season, select season to continue". Buttons: "Show anime", "Show seasons".

## C. Episode transfer (behavior 12 step 2)

Old episodes: ep1 seen, ep2 seen, ep3 unseen, ep4 unseen with bookmark, `date_fetch` 1000; ep5 unseen.
New episodes (recognized): 1, 2, 3, 3.5, 4, 5.

`maxEpisodeSeen = max(seen old episodes) = 2`.

| New | Old match (same recognized number) | seen (new) | bookmark | date_fetch |
|---|---|---|---|---|
| 1 | ep1 | 1 <= 2 -> true | false (old ep1 no bookmark) | old ep1 value |
| 2 | ep2 | 2 <= 2 -> true | old | old |
| 3 | ep3 | 3 > 2 -> false | old (false) | old |
| 3.5 | none | 3.5 > 2 -> false | false | new default |
| 4 | ep4 | false | true (copied) | 1000 (copied) |
| 5 | ep5 | false | old | old |

Important consequence: the rule is max-based. If the old anime had ep5 seen and ep3 unseen, new ep3 becomes seen even though old ep3 was unseen. That is the Kotlin behavior and the spec keeps it.

Proposal E20 adds: `last_second_seen` and `fillermark` copied from the matched old episode. Example: old ep4 `last_second_seen = 1200` -> new ep4 gets 1200.

## D. Source picker sort (behavior 13.1)

Items (name, favorites count, isStub):
- Alpha (5, no), beta (12, no), Gamma (1, stub), delta (12, no)

- ALPHABETICAL ASCENDING: stubs first, then names lowercased: `[Gamma(stub), Alpha, beta, delta]`.
- TOTAL ASCENDING: stubs first, then count ascending: `[Gamma(stub, 1), Alpha(5), beta(12), delta(12)]` (ties keep input order because the sort is stable).
- ALPHABETICAL DESCENDING: `reverseOrder` of the whole comparator -> `[delta, beta, Alpha, Gamma(stub)]`. Stubs end up LAST. (Edge E36; test T45.)
- TOTAL DESCENDING: `[delta(12), beta(12), Alpha(5), Gamma(stub, 1)]`.

Local source (id 0) is never in the list (filter before sort).

## E. Global search visibility (behavior 8.4)

Items after a search (sources with results and errors):
- SrcA: Success with 3 results, pinned
- SrcB: Success with 0 results, unpinned
- SrcC: Error("timeout"), pinned
- SrcD: Loading

Sort [8.4] (`sortComparator`): key 1 is `(result as? Success)?.isEmpty ?: true`. Only a NON-EMPTY Success gets `false`; Loading and Error get `true` like empty results. Key 1 false: SrcA. Key 1 true: SrcC (pinned, key 2 false) first, then SrcB and SrcD (unpinned) by name (name order decides between them). Final order: `[SrcA, SrcC, SrcB, SrcD]` or `[SrcA, SrcC, SrcD, SrcB]` depending on the names; the test fixes the names.

With "Has results" on: `isVisible = !only || (Success && !isEmpty)` -> only SrcA is visible.

Progress: `progress = count(result !is Loading) = 3` (SrcA, SrcB, SrcC), `total = 4`. Bar visible (`1 <= 3 < 4`). After SrcD finishes: `progress = 4`, bar hidden (E40).

## F. Install step map (behavior 1.8)

- Install of `com.v.multi-201` starts: `Pending` -> `Downloading` -> `Installing` -> `Installed` then stream ends.
- Map entries: `{com.v.multi-201: Pending}`, then Downloading, Installing, Installed. On completion the entry is removed.
- Row while Downloading: spinner visible, text "Downloading", cancel button. After completion the row is in the Installed group with the real pkg `com.v.multi`; the synthetic key is gone (E4).

## G. Migrate dialog outcome matrix (behavior 11.3)

| Scenario | Kotlin result | Kurobayomi (Proposal) |
|---|---|---|
| Source missing | returns silently, dialog pops, navigates to new anime | error text "source not installed", buttons stay, nothing written |
| Episode fetch throws | caught, dialog pops, navigates | error text, nothing written (transaction not started) |
| Tracks: enhanced tracker rejects | track dropped silently | track dropped and listed in the outcome (no popup) |
| Download delete throws | caught as Throwable, pops | warning banner after commit, migration kept |
| Success, Copy | old stays favorite | same |
| Success, Migrate | old unfavorited, `date_added = 0` | same |

## H. Query parsing edge table (behavior 1.4)

| Query | Terms | Matches |
|---|---|---|
| `""` | none (empty) | everything |
| `"  "` | `""` after trim -> empty term | nothing |
| `"a,,b"` | `a`, empty, `b` | items matching a OR b |
| `"123"` | 123 | sources with id 123 exactly; names containing "123" |
| `"0123"` | 123 (parsed) | same as "123" for ids; name "0123" also matched by name |
| `"İ"` | Unicode lowercase `i̇` | depends on Unicode folding; test T6 pins the expected result |
