# 46 Player controls UI, part 1: topic, system volume and brightness on Linux

Deep dive for the parts of `p1-behavior.md` section 8 and `p1-data-and-config.md` section 5 that have no Linux counterpart. Scope: the device volume and screen brightness that the Kotlin player changes through `AudioManager` and `Window.attributes`. Mpv boost (the mpv `volume` property above 100) is in area 45 and is only referenced here.

Kotlin source of truth:
- `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` lines 286-299 (initial values), 752-800 (writers), 1011-1020 (none).
- `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt` lines 225-301 (drag math, which calls these writers).
- `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt` line 258 (`volume-max` option).

Verified on the target machine (this session): `wpctl` and `pactl` are installed; `brightnessctl` is not; `/sys/class/backlight` contains `intel_backlight`; systemd-logind exposes `org.freedesktop.login1.Session.SetBrightness(in s subsystem, in u brightness)` on the system bus. The signature was read from `gdbus introspect` on `/org/freedesktop/login1/session/auto`.

---

## 1. What the Kotlin does, in one table

| Concern | Kotlin call | Range | Stored where | Read when |
|---|---|---|---|---|
| Device volume read | `AudioManager.getStreamVolume(STREAM_MUSIC)` (VM:292) | 0..`getStreamMaxVolume` | `VM.currentVolume` | VM init; each drag start |
| Device volume max | `getStreamMaxVolume(STREAM_MUSIC)` (VM:765) | device, typically 15 | `VM.maxVolume` | VM init |
| Device volume write | `setStreamVolume(STREAM_MUSIC, v, 0)` (VM:779-787) | `0..max` (coerced) | `VM.currentVolume` updated after the write | every vertical drag event on the right half |
| Brightness read | `Settings.System.getFloat(SCREEN_BRIGHTNESS)` normalised 0..255 to 0..1 (VM:286-291) | 0..1, on failure 0 | `VM.currentBrightness` | VM init |
| Brightness write | `window.attributes.screenBrightness = clamp(b, 0, 1)` (VM:752-759) | 0..1 | `VM.currentBrightness` = clamp(b, -0.75, 1) | every vertical drag event on the left half |
| Software dim | `BrightnessOverlay` black with alpha = -b when b < 0 (BrightnessOverlay.kt:18-31) | 0..0.75 | none | every recomposition |
| Boost | mpv `volume` property (VM:789-791), cap `pref_audio_volume_boost_cap` (default 30), mpv `volume-max` = cap + 100 (AniyomiMPVView.kt:258) | 100..cap+100 | `VM.currentMPVVolume` | drag, observer |

Key semantic facts that the Linux port must keep:
1. Device volume and boost are two separate layers: boost is only entered when the device layer is already at its maximum and the user keeps dragging up (GH:251-265, VM:766-778).
2. Brightness is the window's brightness, not the panel's: on Android the value applies only to this window. Linux has no per-window brightness, so the port must choose a global backlight write, a software dim, or both.
3. Negative brightness (down to -0.75) is purely a software dim. It never reaches the hardware.

---

## 2. Linux device volume

### 2.1 Backend choice

Proposal: PipeWire through `wpctl` (already present), with `pactl` as fallback when `wpctl` is missing. Reasons: `wpctl` addresses the default sink without a PulseAudio compatibility layer, and it takes a percentage with an optional limit.

Commands (verify on the machine before relying on them; these are the documented forms of `wpctl`):

```
wpctl get-volume @DEFAULT_AUDIO_SINK@        # prints "Volume: 0.45" or "Volume: 0.45 [MUTED]"
wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.45   # absolute, linear 0.0..1.0
wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 0.45   # with a limit of 100%
```

Mapping from Aniyomi steps to PipeWire linear volume:
- `max_steps = 15` (Kurobayomi constant, `SystemVolume::max_steps`).
- `linear = step / max_steps` (0.0 to 1.0).
- Read: `step = round(linear * max_steps)` clamped to 0..max_steps.
- Write: `set-volume <sink> <step / max_steps>`, using the same `step` rounding as the Kotlin `coerceIn(0..max)`.

Gesture sensitivity (GH:226): `volumeGestureSens = 0.001 * max_steps = 0.015`. One step equals about 67 px of drag (the Int truncation in `calculateNewVerticalGestureValue` at GH:349-351 decides the step). Keep the value.

Muted state: `wpctl get-volume` prints `[MUTED]`. The Kotlin has no mute concept; a muted sink is shown as volume 0 (icon Off), and a drag up unmutes implicitly through `set-volume` (PipeWire clears mute on a volume set? verify on the machine; if not, call `wpctl set-mute @DEFAULT_AUDIO_SINK@ 0` before the first write).

### 2.2 Rust trait

```rust
pub trait SystemVolume: Send + Sync {
    /// Device step 0..=max_steps, or None when the backend is unavailable.
    fn get_step(&self) -> Option<i32>;
    /// Writes a step; clamps to 0..=max_steps.
    fn set_step(&self, step: i32) -> Result<(), VolumeError>;
    fn max_steps(&self) -> i32 { 15 }
}

#[derive(Debug)]
pub enum VolumeError { NoBackend, Command(String) }

pub struct WpctlVolume;   // runs `wpctl` through std::process::Command on the blocking pool
pub struct PactlVolume;   // fallback: `pactl set-sink-volume @DEFAULT_SINK@ <pct>%`
```

Threading: `Command::output()` blocks for about 10 to 30 ms. Never call it on the GTK thread. The drag handler sends `VolumeCmd::Set(step)` to a worker thread that coalesces to the latest value (E-46-081 rule) and writes at most every 30 ms.

Startup read: one `get_step()` on the worker thread before the first draw. Until it returns, the pill shows the last known step (0) and the drag uses it; this matches the Kotlin's synchronous read in VM init only in outcome, not timing (documented deviation).

### 2.3 Failure handling

| Failure | Behaviour |
|---|---|
| `wpctl` missing and `pactl` missing | `get_step` returns `None`; drag right half still changes boost, not device volume; log once (E-46-091). |
| Command exits non-zero | `VolumeError::Command`; keep the in-app step; log at warn. |
| Output unparsable | treat as `None`. |
| Device has no default sink | `wpctl` prints an error; treated as command failure. |

---

## 3. Linux brightness

### 3.1 Options

| Option | Mechanism | Works without root | Notes |
|---|---|---|---|
| A. sysfs backlight | write `/sys/class/backlight/<dev>/brightness` (integer 0..max_brightness) | only if the file is writable (udev rule or group) | Needs permission. `intel_backlight` on this machine; check writability at runtime. |
| B. logind | `org.freedesktop.login1.Session.SetBrightness(s subsystem, u brightness)` on the system bus, subsystem `backlight`, name = device name | yes, for the active session | Verified present on this machine. Brightness is an absolute integer, not a fraction. Read the maximum from `max_brightness` in sysfs. |
| C. brightnessctl | external command | depends | Not installed here. Not used. |
| D. software veil | full-window black with alpha | yes | Always available; mirrors the Kotlin negative-range overlay. |

Proposal order: B (logind, no extra permission), then A (if writable, avoids the DBus round trip), then D only. Read the current value from sysfs `brightness` and `max_brightness` (read is always allowed); fall back to `None`.

Mapping: `hardware = round(fraction * max_brightness)` for fraction 0..1. Kotlin's window brightness is 0..1; the hardware equivalent is linear in the logind integer. Do not apply a gamma curve (Android does not either for window brightness).

### 3.2 Rust trait

```rust
pub trait SystemBrightness: Send + Sync {
    /// Current hardware fraction 0.0..=1.0, or None when no backlight is readable.
    fn get(&self) -> Option<f32>;
    /// Writes a hardware fraction; values are already clamped to 0.0..=1.0 by the caller.
    fn set(&self, fraction: f32) -> Result<(), BrightnessError>;
}

#[derive(Debug)]
pub enum BrightnessError { NoBacklight, Permission, Dbus(String) }

pub struct LogindBrightness { device: String, max: u32 }   // zbus or gio DBus call
pub struct SysfsBrightness { path: std::path::PathBuf, max: u32 }
```

DBus binding: `gio::DBusConnection::call_sync` on the system bus (the `gio` crate is already available through `gtk4`). Method call: `org.freedesktop.login1` at `/org/freedesktop/login1/session/auto` (use `auto` for the current session, which logind resolves), interface `org.freedesktop.login1.Session`, method `SetBrightness` with parameters `("backlight", device_name, value)`. Verify the exact tuple shape with `gdbus call --system --dest org.freedesktop.login1 --object-path /org/freedesktop/login1/session/auto --method org.freedesktop.login1.Session.SetBrightness backlight intel_backlight 100` before coding; the introspection output shows `in s subsystem, in u brightness` and an object-path form may be needed for the device name (verify).

### 3.3 Combined behaviour (the part the user sees)

The Kotlin brightness state is a single float in -0.75..1. Map it:

| State value `b` | Hardware write | Software veil alpha |
|---|---|---|
| `b >= 0` | `set(b)` (0..1) | 0 |
| `b < 0` | `set(0)` (floor, matches window `coerceIn(0,1)` in VM:752-759) | `-b` (0 to 0.75) |

This matches the Kotlin: the window never goes below 0, and negative values darken the content only (BrightnessOverlay). Keep the same mapping so that a user who drags below zero sees the veil, not a hardware change.

Initial state: `b = get().unwrap_or(0.0)` (VM:286-291). If the hardware is readable, start at its value. The Kotlin reads the window value, which can be "auto" (-1) on Android; Linux has no auto, so no sentinel is needed.

Write rate: a drag produces many events; coalesce hardware writes to 1 per 50 ms on the worker thread (logind calls are about 1 to 5 ms, sysfs writes are immediate; the 50 ms limit avoids flicker on slow drivers). The veil updates every frame (cheap).

### 3.4 Failure handling

| Failure | Behaviour |
|---|---|
| No backlight device (desktop with external monitor only) | `get()` is `None`; `set()` returns `NoBacklight`; brightness pill still works as software veil only; log once (E-46-090). |
| logind refuses (not the active session) | `Dbus` error; fall back to sysfs; if that fails treat as `NoBacklight`. |
| Permission denied on sysfs | `Permission`; try logind next. |

---

## 4. Pill display rules (what the user sees)

- Volume pill: device percentage or integer; boost text above; icon thresholds as in behaviour 8.1 (off at 0, mute 1..30, down 31..60, up 61..100). Percent uses the Rust formatter (D-9).
- Brightness pill: integer `b*100` (can be negative); icon by fraction; error-coloured overflow bar for negative values.
- Both pills hide 2000 ms after the last drag event (behaviour 8.2).

Linux icon names (freedesktop, all in the Adwaita/Papirus sets; verify they exist in the user's icon theme before using):
- volume: `audio-volume-muted-symbolic`, `audio-volume-low-symbolic`, `audio-volume-medium-symbolic`, `audio-volume-high-symbolic`.
- brightness: `display-brightness-symbolic` for the base, `night-light-symbolic` for the 0 state, `display-brightness-low-symbolic`, `display-brightness-medium-symbolic`, `display-brightness-high-symbolic`.

---

## 5. Testing

| Test | Type | Expectation |
|---|---|---|
| Step-to-linear mapping at 0, 1, 7, 15 | unit | 0.0, 0.0667, 0.4667, 1.0 |
| Step read rounding from linear 0.45 | unit | step 7 (0.45*15 = 6.75 → 7) |
| Veil alpha for -0.75 | unit | 0.75 |
| Hardware write for negative brightness | unit with fake backend | writes 0.0, veil 0.3 for -0.3 |
| Coalescing writes within 50 ms | unit with fake clock | one write for three events |
| `wpctl` absent | unit with PATH override | `NoBackend`, pill still works for boost |
| Live sink change | manual | change volume in the KDE tray, reopen the player, the initial pill matches |
| Live brightness change | manual | KDE brightness slider, reopen the player, initial pill matches |

---

## 6. Open items (must be verified on the machine before implementation)

1. `wpctl set-volume` clears mute: test with `wpctl set-mute @DEFAULT_AUDIO_SINK@ 1` then set a volume.
2. Exact logind `SetBrightness` argument form for the device object (the subsystem string and the device name `intel_backlight`).
3. Whether `/sys/class/backlight/intel_backlight/brightness` is writable by the user (expected not).
4. Whether KDE's own brightness changes are visible to logind reads at startup (they are sysfs changes, so yes, but verify).
5. Icon names exist in the user's icon theme (Papirus or Breeze; check with `gtk-query-settings` or `gtk4-icon-browser`).
