# Startup benchmark

`scripts/bench-startup.sh` times how long the app takes to show its first library frame. It replaces the Android
startup benchmark. It needs [hyperfine](https://github.com/sharkdp/hyperfine) (system package `hyperfine`).

    scripts/bench-startup.sh [--mode cold|warm|hot] <binary> [runs]

- `<binary>` is a path, for example `target/dist/kurobayomi`. Because the binary is an argument, the benchmark
  cannot measure the wrong program (the Android benchmark used a package name that did not match the build).
- `runs` defaults to 10.
- The result is JSON in `target/bench-startup.json`. Set `BENCH_OUT` to write somewhere else.
- Each run executes `env KB_TRACE_STARTUP=1 <binary> --exit-after-first-frame`.

## Modes

The three Android startup modes map like this:

| Android mode | Mode here | What the script does |
|---|---|---|
| warm | `warm` (default) | one warm-up run, then the timed runs; the page cache is primed |
| hot | `hot` | three warm-up runs, so caches and the app's own files are all in place |
| cold | `cold` | drops the page cache before each timed run (`sync; echo 3 > /proc/sys/vm/drop_caches` through `sudo`) |

Cold mode needs root and changes system-wide cache state, so it is never the default and never runs in CI.
The Android compilation modes (no compilation, baseline profile, full compilation) do not exist for a native binary
and are dropped.

## What the app writes

With `KB_TRACE_STARTUP=1` the app writes three lines to stderr, each with a monotonic timestamp in microseconds:

    startup main_entry <us>
    startup window_presented <us>
    startup first_library_render <us>

`--exit-after-first-frame` makes the app quit once the first library frame is drawn, so a run ends by itself.
Read the lines to see where the time goes: `main_entry` to `window_presented` is GTK and window setup,
`window_presented` to `first_library_render` is the library query and the first draw.

**Known gap (T-54a-023, app side not implemented).** The three trace lines and the `--exit-after-first-frame` flag
belong in `src/main.rs` and the library screen, which this folder does not own, and they are not there yet. Today the
binary does not know the flag (the launch parser rejects it and exits non-zero), so hyperfine reports a failed run and
the script exits non-zero against the real binary. The smoke test passes only because it uses a stub binary and a fake
hyperfine. The script itself has not been run against a real hyperfine here (not installed). Close the gap in the
application crate: add the flag and the three `startup <name> <us>` stderr lines, with a test.

## Smoke test

`scripts/tests/bench-startup-smoke.sh` runs the script with one run against `scripts/tests/stub-binary.sh` and a
fake hyperfine, and checks the modes, the trace variable and the error cases. It needs no extra package.

## Screen walk

The Android benchmark also walked the tabs Library, Updates, History, More and Settings to record a profile.
There is no profile to record in a native binary. A GTK smoke test that opens those five screens in a test window
replaces the walk; it belongs with the GUI tests (`crate::testing::with_display`), not with this script.
