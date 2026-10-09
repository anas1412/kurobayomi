# Topic: browse buckets, reconciliation, languages, and update flags

Deep dive for area:19-extensions-anime. Covers how the installed, available, and untrusted lists are computed from the three state maps, how an installed package is matched to a store entry, and how the update count is derived. Sources are under `/home/blackbox/aniyomi-src`. Behaviour sections are in `p1-behavior.md` 5.7, 5.12, 13.

## 1. State maps (inputs)

| Map | Key | Written by | Read by |
|---|---|---|---|
| `installedExtensionsMapFlow` | `pkgName` | load at start (MGR:130-142), listener (MGR:336-359), reconciliation (MGR:198-234), trust (MGR:298-300) | buckets, update count, sources lookup |
| `untrustedExtensionsMapFlow` | `pkgName` | load at start, listener, trust | buckets |
| `availableExtensionsMapFlow` | `pkgName` | `findAvailableExtensions` (MGR:158), last entry wins | buckets, update check (`updateExists()` default) |
| `preferences.animeExtensionUpdatesCount` | none | MGR:202, 384 | Home screen badge (outside scope) |

## 2. Reconciliation: the rule for one installed package

Runs once per `findAvailableExtensions` with the fresh list `L` (behavior 5.7).

Given installed `I` with key `pkg`:

```
hit = first element of L with pkg
if hit is none:
    I.isObsolete = true            (only if it was false; Aniyomi never sets it back)
else:
    I.hasUpdate = hit.versionCode > I.versionCode  or  hit.libVersion > I.libVersion   (Aniyomi, MGR:216)
    I.store = hit.store
    I.isObsolete stays as Aniyomi leaves it (defect: sticky)
```

Kurobayomi rule (design 7.4):

```
hit = first element of L with pkg
if hit is none:
    I.isObsolete = true
    I.hasUpdate = false
else:
    I.isObsolete = false                                             (fix)
    I.hasUpdate  = hit.versionCode > I.versionCode
                   or (hit.abi is some and hit.abi > I.abi)          (tuple compare)
    I.store = hit.store
```

Differences, with reasons:

| Aspect | Aniyomi | Kurobayomi | Reason |
|---|---|---|---|
| Obsolete reset | never | reset when found | Sticky obsolete hides a package that is back (E-REC-01) |
| Lib comparison | `Double` (`libVersion`) | tuple `AbiVersion` | `Double` cannot compare `1.10` and `1.1` (E-LDR-15). Anime Aniyomi lib values are integers and compare correctly as Double, but Kurobayomi packages use the abi tuple |
| Empty list | update count set to 0, map untouched (MGR:201-204) | same | Keeps the installed list when the network fails |
| Count with obsolete | counted (MGR:383) | not counted | Obsolete packages cannot be updated (E-REC-02) |

## 3. Update count

Aniyomi: `count = installed.values.count { hasUpdate }`, stored in `animeext_updates_count` (MGR:382-388). Dismisses the notification at zero.

Kurobayomi (design 7.4 step 3): `count = installed.values.filter(|i| i.has_update && !i.is_obsolete).count()`, kept in memory, recomputed after every state change. Not persisted. The Home badge (outside scope) reads the in-memory value.

Notification: shown only when the count increases or when names change (Proposal, avoids a notification on every refresh). Withdrawn at zero.

## 4. Browse buckets (outputs)

Computed from the three maps plus `show_nsfw_source` and `source_languages` (behavior 13.1).

```
installed_all  = installed.values
installed_vis  = installed_all.filter(show_nsfw || !nsfw)
sorted_inst    = sort_by(!is_obsolete, name lowercase)   // obsolete first
updates        = sorted_inst.filter(has_update)
installed_list = sorted_inst.filter(!has_update)

untrusted_list = untrusted.values.sort_by(name lowercase)  // no NSFW filter (kept)

avail = available.values
        .filter(pkg not in installed and pkg not in untrusted and (show_nsfw || !nsfw))
avail_rows = avail.flat_map(ext =>
        if ext.sources is empty: [Row { ext, source: None, key: pkg }]           // Kurobayomi fix
        else ext.sources.filter(lang in enabled).map(src =>
                 Row { ext, source: Some(src), key: pkg + "-" + src.id }))
avail_list = avail_rows.sort_by(name lowercase)
```

The sort key for an available split row is the **source** name (behavior 13.1 step 2 copies `name = it.name`). The Browse list shows source names for split rows.

Aniyomi drops zero-source extensions from `avail_rows` (E-REC-04). Kurobayomi keeps them. This is a visible change. It is listed in section 7 of the edge file.

## 5. Languages

Aniyomi (behavior 13.2): `distinct(source.lang over available)`, sorted by `(lang not in enabled, LocaleHelper.comparator)`. `"all"` sorts first inside the comparator.

Kurobayomi: same rule, with the comparator from area:02-preferences-system. Zero-source extensions contribute nothing (kept as Aniyomi).

Default enabled set: `{"all", "en", device language}` (LocaleHelper.kt:72-74). First-run enablement for device sub-languages (behavior 5.6) runs once and is marked done in the state file.

## 6. Source toggles

Aniyomi (behavior 13.3): `enabled = source.id.toString() !in hidden_anime_catalogues`. The label rule:
- single-source extension → language label
- multi-source, all names equal → language label
- multi-source, names differ → name label

Kurobayomi: same. The set is keyed by the decimal id as text. Source ids are 64-bit, so the key must not go through `f64` (E-STO-14).

## 7. Lookup by source id

Used when a source id needs an extension (the Browse source list, the source screen, downloads):
- an installed package with that source id → its handle
- else an available package with that source id → a stub handle (behavior 5.4)
- else none

Aniyomi's `getExtensionPackage` returns the first installed match (MGR:84-89). The stub map is replaced whenever the list is non-empty (MGR:116-123). Kurobayomi rebuilds the stub index from the current list on every refresh, so an empty list gives an empty index (E-REP-05).

## 8. Ordering and determinism

- Aniyomi's maps are `Map` instances from `toMutableMap` and `associate`. Iteration order is insertion order, which changes after updates.
- Kurobayomi uses `BTreeMap` for the maps (sorted by pkg) and sorts every bucket explicitly. The order of the Browse list does not depend on the order of the network responses.
- The store list order comes from the table (rowid order; design section 14, pitfall 12). Used only for the "first match wins" rule.

## 9. What the user sees after common actions

| Action | Effect on buckets | Effect on count |
|---|---|---|
| Add a store | new available rows after refresh | unchanged until refresh |
| Remove a store | installed packages only it listed become obsolete (sorted first) | recomputed |
| Refresh (K-5) | reconciliation over the new list | recomputed |
| Install a package | moves from available to installed | recomputed |
| Install an update | `has_update` cleared after reconciliation | decremented |
| Uninstall | removed from installed, back to available if still listed | recomputed |
| Trust an untrusted package | moves to installed after reload; `has_update` not set until next reconciliation (E-TRU-06 in Aniyomi). Kurobayomi runs reconciliation after the trust reload | recomputed |
| Change `show_nsfw_source` | buckets rebuilt | unchanged |
| Change `source_languages` | available rows rebuilt | unchanged |
| Toggle a source | that source's row shown or hidden | unchanged |

## 10. Test anchors

TST-19-064 to 074 and TST-19-092 to 097 (see `p1-edge-cases-and-tests.md`). The reconciliation and bucket functions are pure, so they are tested without GTK (design section 15).
