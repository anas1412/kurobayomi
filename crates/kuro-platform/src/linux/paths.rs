//! XDG base directories for Linux.

use std::path::{Path, PathBuf};

use directories::{ProjectDirs, UserDirs};

use crate::paths::Paths;

/// App directory name for the user's downloads folder.
const APP_DIR: &str = "Kurobayomi";

/// XDG directory name (`~/.config/kurobayomi` and so on), lowercase as plans/ARCHITECTURE.md says.
const XDG_APP_DIR: &str = "kurobayomi";

/// Base directories that [`LinuxPaths`] derives every other path from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
    pub data: PathBuf,
    pub config: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
    pub downloads: PathBuf,
}

/// Failure to resolve the XDG directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("no home directory: cannot resolve XDG directories")]
    NoHome,
}

/// Paths derived from XDG base directories under the app name `kurobayomi`.
#[derive(Debug, Clone)]
pub struct LinuxPaths {
    roots: Roots,
}

impl LinuxPaths {
    /// Builds paths from explicit roots. Tests pass directories under a temp dir.
    pub fn new(roots: Roots) -> Self {
        Self { roots }
    }

    /// Builds paths from the real XDG environment. Computes paths only; creates nothing.
    pub fn system() -> Result<Self, PathError> {
        let project =
            ProjectDirs::from_path(PathBuf::from(XDG_APP_DIR)).ok_or(PathError::NoHome)?;
        let user = UserDirs::new().ok_or(PathError::NoHome)?;
        let downloads = user
            .download_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| user.home_dir().join("Downloads"));
        let state = project.state_dir().ok_or(PathError::NoHome)?.to_path_buf();
        Ok(Self::new(Roots {
            data: project.data_dir().to_path_buf(),
            config: project.config_dir().to_path_buf(),
            cache: project.cache_dir().to_path_buf(),
            state,
            downloads,
        }))
    }
}

impl Paths for LinuxPaths {
    fn data_dir(&self) -> PathBuf {
        self.roots.data.clone()
    }

    fn config_dir(&self) -> PathBuf {
        self.roots.config.clone()
    }

    fn state_dir(&self) -> PathBuf {
        self.roots.state.clone()
    }

    fn cache_dir(&self) -> PathBuf {
        self.roots.cache.clone()
    }

    fn log_dir(&self) -> PathBuf {
        self.roots.state.join("logs")
    }

    fn temp_dir(&self) -> PathBuf {
        self.roots.cache.join("tmp")
    }

    fn crash_dir(&self) -> PathBuf {
        self.roots.state.join("crashes")
    }

    fn default_downloads_dir(&self) -> PathBuf {
        self.roots.downloads.join(APP_DIR)
    }

    fn default_backups_dir(&self) -> PathBuf {
        self.roots.data.join("backups")
    }

    fn covers_dir(&self) -> PathBuf {
        self.roots.cache.join("covers")
    }

    fn plugin_dir(&self) -> PathBuf {
        self.roots.data.join("plugins")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::FolderProvider;

    fn roots_under(base: &Path) -> Roots {
        Roots {
            data: base.join("data"),
            config: base.join("config"),
            cache: base.join("cache"),
            state: base.join("state"),
            downloads: base.join("Downloads"),
        }
    }

    #[test]
    fn overridden_roots_drive_every_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let paths = LinuxPaths::new(roots_under(base));
        assert_eq!(paths.data_dir(), base.join("data"));
        assert_eq!(paths.config_dir(), base.join("config"));
        assert_eq!(paths.cache_dir(), base.join("cache"));
        assert_eq!(paths.state_dir(), base.join("state"));
        assert_eq!(paths.log_dir(), base.join("state/logs"));
        assert_eq!(paths.temp_dir(), base.join("cache/tmp"));
        assert_eq!(paths.crash_dir(), base.join("state/crashes"));
        assert_eq!(paths.default_downloads_dir(), base.join("Downloads/Kurobayomi"));
        assert_eq!(paths.default_backups_dir(), base.join("data/backups"));
        assert_eq!(paths.covers_dir(), base.join("cache/covers"));
        assert_eq!(paths.plugin_dir(), base.join("data/plugins"));
    }

    #[test]
    fn resolving_paths_creates_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = LinuxPaths::new(roots_under(tmp.path()));
        let _ = (
            paths.log_dir(),
            paths.temp_dir(),
            paths.crash_dir(),
            paths.covers_dir(),
            paths.plugin_dir(),
        );
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn folder_provider_url_is_file_url_of_downloads() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = LinuxPaths::new(roots_under(tmp.path()));
        let folder = FolderProvider::new(&paths);
        assert_eq!(folder.directory(), tmp.path().join("Downloads/Kurobayomi"));
        let url = folder.url().unwrap().to_string();
        assert!(url.starts_with("file://"), "{url}");
        assert!(url.ends_with("Kurobayomi"), "{url}");
    }

    #[test]
    fn first_run_creates_the_four_base_directories_private() {
        use crate::paths::create_app_dirs;
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let paths = LinuxPaths::new(roots_under(tmp.path()));
        create_app_dirs(&paths).unwrap();
        for dir in [paths.config_dir(), paths.state_dir(), paths.data_dir(), paths.cache_dir()] {
            assert!(dir.is_dir(), "{}", dir.display());
            let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{}", dir.display());
        }
        assert!(!paths.log_dir().exists(), "log dir is created by the logger, not here");
    }

    #[test]
    fn first_run_keeps_an_existing_directory_and_its_mode() {
        use crate::paths::create_app_dirs;
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let paths = LinuxPaths::new(roots_under(tmp.path()));
        std::fs::create_dir_all(paths.data_dir()).unwrap();
        std::fs::set_permissions(paths.data_dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
        create_app_dirs(&paths).unwrap();
        let mode = std::fs::metadata(paths.data_dir()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[test]
    fn first_run_failure_names_the_blocking_path() {
        use crate::paths::create_app_dirs;
        let tmp = tempfile::tempdir().unwrap();
        let paths = LinuxPaths::new(roots_under(tmp.path()));
        std::fs::write(paths.cache_dir(), b"not a directory").unwrap();
        let err = create_app_dirs(&paths).unwrap_err();
        assert_eq!(err.path, paths.cache_dir());
        assert!(err.to_string().starts_with(&format!("cannot create {}", paths.cache_dir().display())));
    }

    #[test]
    fn system_paths_end_in_app_name() {
        // Reads the real environment but writes nothing.
        let paths = LinuxPaths::system().unwrap();
        assert!(paths.data_dir().ends_with("kurobayomi"));
        assert!(paths.config_dir().ends_with("kurobayomi"));
        assert!(paths.cache_dir().ends_with("kurobayomi"));
        assert!(paths.state_dir().ends_with("kurobayomi"));
        assert!(paths.log_dir().ends_with("kurobayomi/logs"));
    }
}
