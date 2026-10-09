# Topic: mpv bridge (libmpv embedding, Lua protocol, key input)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerObserver.kt`, `app/src/main/assets/aniyomi.lua`, and the mpv parts of `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`. Detailed behaviour is in `p1-behavior.md` sections 2, 5, 6 and 8; data in `p1-data-and-config.md` sections 3.3-3.5 and 4.

## 1. What the Android side is

- `AniyomiMPVView` extends `is.xyz.mpv.BaseMPVView` (external library `com.github.aniyomiorg:aniyomi-mpv-lib:1.18.n`, from `gradle/aniyomi.versions.toml:2`). That library is not in this repository. Its `initialize`, `destroy`, `setVo`, `initOptions`, `observeProperties`, `postInitOptions` hooks are called from here; their internals are inferred from call sites only.
- `MPVLib` is the JNI wrapper: `setOptionString`, `setPropertyString|Int|Boolean|Double`, `getProperty*`, `command`, `observeProperty(name, format)`, `addObserver`, `addLogObserver`, `removeObserver`, `event` callbacks.
- The mpv instance is created in `PlayerActivity.setupPlayerMPV` (`PlayerActivity.kt:424-448`), with `configDir` = `<filesDir>/mpv`, `cacheDir` = app cache dir, log level `info` (verbose) or `warn`.

## 2. Rust replacement

- Crate: `libmpv2` (safe bindings with the render API). Verify the current version on crates.io and check that the render feature is enabled.
- Output: render API into a `gtk::GLArea` (`p1-rust-design.md` sections 3.5 and 9). `vo` must be `libmpv`. The Kotlin `gpu` / `gpu-next` choice is kept as a renderer preference (see section 4 below).
- Threading: the handle stays on the GTK main thread (`!Send` wrapper). Events come from `mpv_set_wakeup_callback` into a glib source.

## 3. Option lifecycle (Kotlin order, Rust must keep it)

1. Create handle and set `config-dir`, `log` level before init (the Kotlin passes these to `initialize`).
2. Apply startup options (`Option` list, `p1-data-and-config.md` section 4 golden list).
3. Apply `Property` settings (`pause=yes`, `audio-channels`).
4. Initialise.
5. Observe properties (34 entries).
6. Post-init: stats page script bindings when `pref_player_statistics_page != 0`.
7. Add log observer and event observer.

Per-window: options are re-applied on every open because the window owns a fresh handle (behaviour 2.1). Rust may keep one handle per process only if the episode switch path is kept fast; proposal: one handle per window (matches Kotlin, simpler teardown).

## 4. The `gpu` vs `gpu-next` question

- Kotlin: `setVo(if (gpuNext) "gpu-next" else "gpu")`.
- Linux (proposal): keep the preference. Apply it through the renderer selection that the libmpv render output uses, verified at runtime with the mpv version found. If the installed libmpv rejects the value, fall back to the default renderer and log a warning. This is a verification item in `p1-edge-cases-and-tests.md` section 7 (item 4).

## 5. The `user-data/aniyomi` protocol

- Lua functions in `app/src/main/assets/aniyomi.lua` call `mp.set_property("user-data/aniyomi/<name>", value)`. The app observes these properties (14 of them, behaviour section 5).
- Rust: `bridge.rs` receives the `Prop(Str)` event, matches the property name, parses the value (pipe-separated for `*_with_text` and `launch_int_picker`), and calls the part-2 model method.
- Keep `aniyomi.lua` as a file for custom-button scripts. Custom buttons compile to Lua (`custombuttons.lua`, behaviour 2.1). This keeps existing user scripts working.
- Reset semantics: the Kotlin does not clear the property after use. Proposal: clear to an empty string after dispatch so an identical repeated command still triggers. This requires the observer to ignore the empty value.

## 6. Key input (AniyomiMPVView.onKey)

- Input is GTK `EventControllerKey` on the window. Output is `keydown <name>` / `keyup <name>` mpv commands.
- Mapping table and modifier order: `p1-rust-design.md` section 8; behaviour section 8.
- Repeat events are swallowed (mpv does its own repeat).
- The mapping table is a Linux proposal. The Android `KeyMapping.map` is external.

## 7. Observers

- The 34 observed properties and their formats are in `p1-behavior.md` section 6.1. The Rust `OBSERVED` table reproduces them.
- The Kotlin gates every observer on `isExiting`. Rust keeps that gate, with the PLAYBACK_RESTART exception (behaviour 6.4).
- Log lines: map mpv levels to Rust levels, store the last `HTTP error` line for end-file messages (behaviour 13.3).

## 8. Known defects not carried over

- `vf` overwrite between debanding and yuv420p (behaviour 2.2).
- `trackId` `toInt()` exception (behaviour 6.6).
- `onKeyUp` `event!!`.
- `createPipParams` `!!` and zero-width Rational.
- `FLAG`-stuck `isExiting` (behaviour 6.4).

## 9. Test coverage

- Options golden list: `p1-edge-cases-and-tests.md` section 5.1.
- Keys: section 5.2.
- Bridge: section 5.3.
- Controller gate and order: section 5.9.
- Real mpv and GL: section 7.
