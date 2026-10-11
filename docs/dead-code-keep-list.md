# Unwired plan code kept on purpose

The phone-feature removal deleted code that served only the removed features. These modules have no production caller yet
but belong to a kept feature (keyboard, empty states, tabbed shells, tracker work, link handling). They stay until their
screens are wired or the owner decides to cut them (design principle 1: wire it or delete it, never hide it).

| Item | Why it stays | Wiring or decision still open |
|---|---|---|
| `src/browse/deep_link.rs` `resolve`, `anime_search_intent`, `open_anime_search_link` | pasted links and the `kurobayomi://animesearch` link (T-20a-031, T-41b-037) | the next source lane wires `resolve` to the sources that implement `ResolvableSource` |
| `src/browse/pending.rs` `park_intent`, `open_search_intent` | the parked search the link above opens | same |
| `src/entry_core/anime/airing_load.rs` | AniChart airing lookups through the AniList and MAL trackers (T-39b-036, T-39b-037) | the runner calls it |
| `src/browse/keys.rs` `help_entries` | the help list for the browse keys, with its test that every key has a line | the shortcut help window reads it |
| `src/shell/appbar.rs` | shared app bar rules and builders (T-36a-037, T-36a-038); Escape in selection mode is handled today by `entry_core/selection.rs` (`key_command`, tested), `library/keys.rs` and `updates/updates_screen.rs` | `docs/audit/p0-07.md` rows T-36a-037 and T-36a-038 |
| `src/widgets/refresh.rs` | the header refresh button and `win.refresh` on F5 and Ctrl+R (T-49b-019) | pages adopt it |
| `src/shell/empty.rs` | empty screen of T-36a-048 | candidate to delete: `browse::look::empty_state` and `theme/empty_state.rs` do the same job |
| `src/widgets/keyboard.rs`, `src/widgets/adaptive_sheet.rs` | Tab trapping and Escape for sheets; `keyboard.rs` is used only by `adaptive_sheet` | decide with the sheet work |

Not in this list because they have live callers (the warning count over-reports them): `settings_ui/jobs.rs` (worker info
page), `library/category_screen.rs` (`library/t2/catscr.rs`), `entry_core/live.rs` (`entry_core/page.rs`),
`pref_model/more.rs` (`screens.rs`).
