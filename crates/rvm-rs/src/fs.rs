use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
    thread::sleep,
    time::Duration,
};

use semver::Version;

use crate::{errors::Error, Build};

const BUILD_FILE_NAME: &str = "build.json";

/// Trait to store and retrieve binaries and their metadata from the filesystem.
///
/// global default version of Resolc is stored in `.default_version` in the installation folder.
///
/// each Resolc version will installed into `<installation_folder>/<binary version >/<binary|build.json>`
pub(crate) trait FsPaths {
    fn new() -> Result<Self, Error>
    where
        Self: Sized;

    /// Path to the storage folder
    fn path(&self) -> &Path;

    fn default_version_path(&self) -> &'static Path {
        static ONCE: OnceLock<PathBuf> = OnceLock::new();
        ONCE.get_or_init(|| self.path().join(".default_version"))
    }

    /// installs the provided binary into `<Self::path>/<binary version>/<stored artifacts>`
    ///
    /// # Stored artifacts
    /// * `binary` - binary itself
    /// * `build` - binary metadata from the releases file.
    fn install_version(&self, build: &Build, binary_blob: &[u8]) -> Result<(), Error> {
        match self.install_inner(build, binary_blob) {
            ok @ Ok(_) => return ok,
            Err(Error::IoError(err)) if err.kind() == ErrorKind::AlreadyExists => {
                return Ok(());
            }
            e => return e,
        }
    }

    fn install_inner(&self, build: &Build, binary_blob: &[u8]) -> Result<(), Error> {
        let version = &build.version;
        let binary_path = &build.name;
        let folder = self.path().join(version.to_string());
        match self.create_lock_file(version) {
            Ok(_) => {}
            Err(Error::IoError(err)) if err.kind() == ErrorKind::AlreadyExists => {
                sleep(Duration::from_millis(250));
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        fs::create_dir_all(&folder)?;

        let mut f = fs::File::create_new(folder.join(binary_path))?;
        let metadata = fs::File::create_new(folder.join(BUILD_FILE_NAME))?;
        serde_json::to_writer(metadata, &build)?;
        f.flush()?;
        #[cfg(target_family = "unix")]
        {
            use std::{fs::Permissions, os::unix::fs::PermissionsExt};
            f.set_permissions(Permissions::from_mode(0o755))?;
        }

        f.write_all(binary_blob).map_err(Into::into)
    }
    /// Retrieve default version of Resolc for use if it's present.
    fn get_default_version(&self) -> Result<Version, Error> {
        std::fs::read_to_string(self.default_version_path())
            .map_err(Into::into)
            .and_then(|str| Version::parse(str.trim_matches('/')).map_err(Into::into))
    }
    /// Remove default Resolc version
    fn remove_default(&self) -> Result<(), Error> {
        let _lock_file = self.create_lock_file(&Version::new(0, 0, 0))?;

        std::fs::remove_file(self.default_version_path()).map_err(Into::into)
    }

    /// Sets a default version of Resolc to be used globally
    fn set_default_version(&self, version: &Version) -> Result<(), Error> {
        let _lock_file = self.create_lock_file(&Version::new(0, 0, 0))?;

        std::fs::File::create(self.default_version_path())?
            .write_all(version.to_string().as_bytes())
            .map_err(Into::into)
    }

    /// Build a list of installed binaries using the `build.json` metadata that is stored alongside them.
    fn installed_versions(&self) -> Result<Vec<Build>, Error> {
        let files = std::fs::read_dir(self.path())?
            .filter_map(|e| e.ok())
            .filter_map(|entry| {
                if entry.metadata().is_ok_and(|data| data.is_file()) {
                    return None;
                };
                Some(entry)
            })
            .filter_map(|entry| {
                let entry = entry;
                let file = entry.path().join(BUILD_FILE_NAME);
                let file = std::fs::read_to_string(file).ok()?;
                serde_json::from_str::<Build>(&file).ok()
            })
            .collect::<Vec<Build>>();
        Ok(files)
    }

    /// Will delete the version provided from the filesystem
    ///
    /// also unsets the default version if it's the version that is removed
    fn remove_version(&self, version: &Version) -> Result<(), Error> {
        let path = self.path().join(version.to_string());

        if !path.exists() {
            return Ok(());
        };
        let _lock_file = self.create_lock_file(version)?;
        if let Ok(default_version) = self.get_default_version() {
            if default_version == *version {
                self.remove_default()?
            }
        }

        std::fs::remove_dir_all(path).map_err(Into::into)
    }

    fn create_lock_file(&self, version: &Version) -> Result<LockFile, Error> {
        use fs4::fs_std::FileExt;

        let path = self.path().join(format!(".lock-{version}"));
        let _file = std::fs::File::options()
            .read(true)
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        _file.lock_exclusive()?;
        Ok(LockFile { _file, path })
    }
}

pub(crate) struct LockFile {
    _file: std::fs::File,
    path: PathBuf,
}

impl Drop for LockFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Implementation used by default.
///
///
#[derive(Clone)]
pub struct DataDir {
    path: PathBuf,
}

fn create_dir(path: &PathBuf) -> Result<(), Error> {
    match fs::create_dir_all(path) {
        Err(err) if matches!(err.kind(), std::io::ErrorKind::AlreadyExists) => Ok(()),
        any => any,
    }?;

    if !path.is_dir() {
        return Err(Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            format!("{} is not a directory", path.display()),
        )));
    };
    Ok(())
}

impl FsPaths for DataDir {
    fn new() -> Result<Self, Error> {
        let home_dir = dirs::home_dir()
            .ok_or(Error::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "$USER directory doesn't exist".to_owned(),
            )))
            .map(|x| x.join(".rvm"));

        let data_dir = dirs::data_dir().ok_or(Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Data directory doesn't exist".to_owned(),
        )));

        let path = match (&home_dir, data_dir) {
            (Ok(dir), Ok(data_dir)) if !dir.exists() => Ok(data_dir.join("rvm")),
            _ => home_dir,
        }?;

        create_dir(&path)?;

        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        self.path.as_path()
    }
}
