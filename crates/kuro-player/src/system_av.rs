//! System volume and brightness backends for the vertical drag (area 46 `PlayerViewModel.kt` volume
//! and brightness code, T-46a-049). Linux replacements for the Android calls: the default PipeWire
//! sink through `wpctl` in 15 steps, and the writable sysfs backlight. Every command here blocks, so
//! the caller runs it on a blocking task and never on the GTK thread. When neither works, only the
//! software veil is used. The logind brightness fallback is a `SystemBrightness` the platform layer
//! provides; `choose_brightness` takes it as an argument.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Device volume steps, as on Android (`maxVolume`).
pub const VOLUME_STEPS: i32 = 15;
/// PipeWire's name for the default output.
pub const DEFAULT_SINK: &str = "@DEFAULT_AUDIO_SINK@";

/// Parses `wpctl get-volume` output, `Volume: 0.45` or `Volume: 0.45 [MUTED]`, into steps. Muted
/// output reads as 0 steps, so the pill shows silence while muted.
pub fn parse_wpctl_volume(output: &str) -> Option<i32> {
    let rest = output
        .lines()
        .find_map(|l| l.strip_prefix("Volume:"))?
        .trim();
    let value: f32 = rest.split_whitespace().next()?.parse().ok()?;
    if rest.contains("[MUTED]") {
        return Some(0);
    }
    Some((value * VOLUME_STEPS as f32).round() as i32)
}

/// Arguments for `wpctl set-volume` that set `step` of `VOLUME_STEPS`.
pub fn volume_args(step: i32) -> Vec<String> {
    let fraction = step.clamp(0, VOLUME_STEPS) as f32 / VOLUME_STEPS as f32;
    vec![
        "set-volume".to_owned(),
        DEFAULT_SINK.to_owned(),
        format!("{fraction:.2}"),
    ]
}

/// Device volume in steps.
pub trait SystemVolume {
    fn read(&self) -> io::Result<i32>;
    fn write(&self, step: i32) -> io::Result<()>;
}

/// The default sink through `wpctl`.
pub struct Wpctl;

impl SystemVolume for Wpctl {
    fn read(&self) -> io::Result<i32> {
        let out = Command::new("wpctl")
            .args(["get-volume", DEFAULT_SINK])
            .output()?;
        let text = String::from_utf8_lossy(&out.stdout);
        parse_wpctl_volume(&text)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unreadable wpctl output"))
    }

    fn write(&self, step: i32) -> io::Result<()> {
        let status = Command::new("wpctl").args(volume_args(step)).status()?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| io::Error::other("wpctl set-volume failed"))
    }
}

/// Parses `pactl get-sink-volume` output into steps. The first `NN%` value is the sink volume, for
/// example `Volume: front-left: 9830 /  15% / -49.00 dB, ...`.
pub fn parse_pactl_volume(output: &str) -> Option<i32> {
    let line = output
        .lines()
        .find(|l| l.trim_start().starts_with("Volume:"))?;
    let percent: f32 = line
        .split_whitespace()
        .find_map(|token| token.strip_suffix('%'))?
        .parse()
        .ok()?;
    Some((percent / 100.0 * VOLUME_STEPS as f32).round() as i32)
}

/// The default sink through `pactl`, the PulseAudio fallback when `wpctl` is missing or fails.
pub struct Pactl;

impl SystemVolume for Pactl {
    fn read(&self) -> io::Result<i32> {
        let out = Command::new("pactl")
            .args(["get-sink-volume", DEFAULT_SINK])
            .output()?;
        let text = String::from_utf8_lossy(&out.stdout);
        parse_pactl_volume(&text)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unreadable pactl output"))
    }

    fn write(&self, step: i32) -> io::Result<()> {
        let percent = step.clamp(0, VOLUME_STEPS) * 100 / VOLUME_STEPS;
        let status = Command::new("pactl")
            .args(["set-sink-volume", DEFAULT_SINK, &format!("{percent}%")])
            .status()?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| io::Error::other("pactl set-sink-volume failed"))
    }
}

/// Tries the first backend and falls back to the second when it fails (PipeWire, then PulseAudio).
pub struct Fallback<P, Q>(pub P, pub Q);

impl<P: SystemVolume, Q: SystemVolume> SystemVolume for Fallback<P, Q> {
    fn read(&self) -> io::Result<i32> {
        self.0.read().or_else(|_| self.1.read())
    }

    fn write(&self, step: i32) -> io::Result<()> {
        self.0.write(step).or_else(|_| self.1.write(step))
    }
}

/// The last good volume reading. A failed read keeps the last value and logs once (E-46-091).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeReading {
    last: i32,
    logged: bool,
}

impl VolumeReading {
    pub fn new(start: i32) -> Self {
        Self {
            last: start,
            logged: false,
        }
    }

    pub fn update(&mut self, result: io::Result<i32>) -> i32 {
        match result {
            Ok(step) => self.last = step,
            Err(error) if !self.logged => {
                tracing::warn!(%error, "system volume read failed; keeping the last value");
                self.logged = true;
            }
            Err(_) => {}
        }
        self.last
    }
}

/// A sysfs backlight directory with `brightness` and `max_brightness`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlight {
    dir: PathBuf,
}

fn read_u32(path: &Path) -> io::Result<u32> {
    fs::read_to_string(path)?
        .trim()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not a number"))
}

/// Screen brightness as 0..=1 of the hardware maximum.
pub trait SystemBrightness: Send {
    fn read(&self) -> io::Result<f32>;
    fn write(&self, fraction: f32) -> io::Result<()>;
}

impl Backlight {
    /// The first backlight under `root` (normally `/sys/class/backlight`) that has both files.
    pub fn find(root: &Path) -> Option<Self> {
        let mut dirs: Vec<PathBuf> = fs::read_dir(root)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.join("brightness").is_file() && p.join("max_brightness").is_file())
            .collect();
        dirs.sort();
        dirs.into_iter().next().map(|dir| Self { dir })
    }

    /// Whether the `brightness` file can be opened for writing by this user.
    fn is_writable(&self) -> bool {
        fs::OpenOptions::new()
            .write(true)
            .open(self.dir.join("brightness"))
            .is_ok()
    }
}

impl SystemBrightness for Backlight {
    /// Brightness as 0..=1 of the hardware maximum.
    fn read(&self) -> io::Result<f32> {
        let raw = read_u32(&self.dir.join("brightness"))?;
        let max = read_u32(&self.dir.join("max_brightness"))?;
        if max == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "zero maximum"));
        }
        Ok((raw as f32 / max as f32).min(1.0))
    }

    /// Writes a fraction of the hardware maximum. Values outside 0..=1 are clamped.
    fn write(&self, fraction: f32) -> io::Result<()> {
        let max = read_u32(&self.dir.join("max_brightness"))?;
        let raw = (fraction.clamp(0.0, 1.0) * max as f32).round() as u32;
        fs::write(self.dir.join("brightness"), raw.to_string())
    }
}

/// How brightness is changed: by a device backend, or only by the software veil (E-46-090).
pub enum BrightnessMode {
    Device(Box<dyn SystemBrightness>),
    VeilOnly,
}

/// The writable sysfs backlight under `sysfs_root` first, then `logind` (the platform layer's
/// backend, `None` when it could not connect). A backend that cannot be read does not count.
pub fn choose_brightness(
    sysfs_root: &Path,
    logind: Option<Box<dyn SystemBrightness>>,
) -> BrightnessMode {
    let sysfs = Backlight::find(sysfs_root).filter(|b| b.is_writable() && b.read().is_ok());
    if let Some(backlight) = sysfs {
        return BrightnessMode::Device(Box::new(backlight));
    }
    match logind.filter(|b| b.read().is_ok()) {
        Some(backend) => BrightnessMode::Device(backend),
        None => {
            tracing::info!("no brightness backend works; using the software veil only");
            BrightnessMode::VeilOnly
        }
    }
}

/// The mpv property that caps boost: `volume-max` is the boost cap plus 100 (E-46-025). Set it
/// before the first playback and whenever the cap changes.
pub fn volume_max_write(boost_cap: i32) -> (&'static str, i64) {
    ("volume-max", i64::from(boost_cap) + 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Logind(io::Result<f32>);

    impl SystemBrightness for Logind {
        fn read(&self) -> io::Result<f32> {
            self.0
                .as_ref()
                .map(|v| *v)
                .map_err(|_| io::Error::other("no logind"))
        }
        fn write(&self, _: f32) -> io::Result<()> {
            Ok(())
        }
    }

    fn sysfs_with_backlight() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let dev = root.path().join("intel_backlight");
        fs::create_dir(&dev).unwrap();
        fs::write(dev.join("brightness"), "50").unwrap();
        fs::write(dev.join("max_brightness"), "200").unwrap();
        root
    }

    #[test]
    fn sysfs_is_preferred_then_logind_then_only_the_veil() {
        let root = sysfs_with_backlight();
        let logind = || Some(Box::new(Logind(Ok(0.9))) as Box<dyn SystemBrightness>);
        let BrightnessMode::Device(b) = choose_brightness(root.path(), logind()) else {
            panic!("sysfs expected");
        };
        assert_eq!(b.read().unwrap(), 0.25);
        let empty = tempfile::tempdir().unwrap();
        let BrightnessMode::Device(b) = choose_brightness(empty.path(), logind()) else {
            panic!("logind expected");
        };
        assert_eq!(b.read().unwrap(), 0.9);
        let broken =
            Some(Box::new(Logind(Err(io::Error::other("x")))) as Box<dyn SystemBrightness>);
        assert!(matches!(
            choose_brightness(empty.path(), broken),
            BrightnessMode::VeilOnly
        ));
        assert!(matches!(
            choose_brightness(empty.path(), None),
            BrightnessMode::VeilOnly
        ));
    }

    #[test]
    fn wpctl_output_becomes_steps() {
        assert_eq!(parse_wpctl_volume("Volume: 0.45\n"), Some(7));
        assert_eq!(parse_wpctl_volume("Volume: 1.00"), Some(15));
        assert_eq!(parse_wpctl_volume("Volume: 0.45 [MUTED]"), Some(0));
        assert_eq!(parse_wpctl_volume("nonsense"), None);
    }

    #[test]
    fn set_arguments_use_the_fraction_of_fifteen_steps() {
        assert_eq!(volume_args(15), vec!["set-volume", DEFAULT_SINK, "1.00"]);
        assert_eq!(volume_args(-3)[2], "0.00");
    }

    #[test]
    fn a_failed_read_keeps_the_last_value() {
        let mut reading = VolumeReading::new(6);
        assert_eq!(reading.update(Ok(9)), 9);
        assert_eq!(reading.update(Err(io::Error::other("no wpctl"))), 9);
        assert_eq!(reading.update(Err(io::Error::other("still none"))), 9);
    }

    #[test]
    fn backlight_reads_and_writes_the_fraction() {
        let root = tempfile::tempdir().unwrap();
        let dev = root.path().join("intel_backlight");
        fs::create_dir(&dev).unwrap();
        fs::write(dev.join("brightness"), "50\n").unwrap();
        fs::write(dev.join("max_brightness"), "200\n").unwrap();
        let backlight = Backlight::find(root.path()).expect("backlight found");
        assert_eq!(backlight.read().unwrap(), 0.25);
        backlight.write(0.5).unwrap();
        assert_eq!(fs::read_to_string(dev.join("brightness")).unwrap(), "100");
        backlight.write(3.0).unwrap();
        assert_eq!(fs::read_to_string(dev.join("brightness")).unwrap(), "200");
    }

    #[test]
    fn no_backlight_means_none() {
        let root = tempfile::tempdir().unwrap();
        assert!(Backlight::find(root.path()).is_none());
        assert!(Backlight::find(&root.path().join("absent")).is_none());
    }

    #[test]
    fn pactl_percent_becomes_steps() {
        let out =
            "Volume: front-left: 9830 /  15% / -49.00 dB,   front-right: 9830 /  15% / -49.00 dB\n";
        assert_eq!(parse_pactl_volume(out), Some(2));
        assert_eq!(parse_pactl_volume("nothing here"), None);
    }

    struct Backend(io::Result<i32>, std::cell::Cell<bool>);

    impl SystemVolume for Backend {
        fn read(&self) -> io::Result<i32> {
            match &self.0 {
                Ok(v) => Ok(*v),
                Err(e) => Err(io::Error::new(e.kind(), "down")),
            }
        }
        fn write(&self, _: i32) -> io::Result<()> {
            self.1.set(true);
            if self.0.is_ok() {
                Ok(())
            } else {
                Err(io::Error::other("down"))
            }
        }
    }

    #[test]
    fn fallback_uses_the_second_backend_when_the_first_fails() {
        let first = Backend(
            Err(io::Error::other("no wpctl")),
            std::cell::Cell::new(false),
        );
        let second = Backend(Ok(7), std::cell::Cell::new(false));
        let both = Fallback(first, second);
        assert_eq!(both.read().unwrap(), 7);
        assert!(both.write(3).is_ok());
        assert!(both.1.1.get());
    }

    #[test]
    fn fallback_keeps_the_first_backend_when_it_works() {
        let first = Backend(Ok(4), std::cell::Cell::new(false));
        let second = Backend(Ok(9), std::cell::Cell::new(false));
        let both = Fallback(first, second);
        assert_eq!(both.read().unwrap(), 4);
        assert!(!both.1.1.get());
    }

    #[test]
    fn volume_max_is_the_cap_plus_one_hundred() {
        assert_eq!(volume_max_write(30), ("volume-max", 130));
    }
}
