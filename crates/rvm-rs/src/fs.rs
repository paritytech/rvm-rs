use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use fs4::fs_std::FileExt;
use semver::Version;

use crate::{errors::Error, Build};

pub(crate) trait FsPaths {
    fn new() -> Result<Self, Error>
    where
        Self: Sized;

    fn path(&self) -> &Path;

    fn install_version(&self, build: &Build, binary_blob: &[u8]) -> Result<(), Error> {
        let version = &build.version;
        let binary_path = &build.name;
        let folder = self.path().join(version.to_string());
        let _lock_file = self.create_lock_file(version)?;

        fs::create_dir_all(&folder)?;

        let mut f = fs::File::create_new(folder.join(binary_path))?;
        let metadata = fs::File::create_new(folder.join("build.json"))?;
        serde_json::to_writer(metadata, &build)?;
        f.flush()?;
        #[cfg(target_family = "unix")]
        {
            use std::{fs::Permissions, os::unix::fs::PermissionsExt};
            f.set_permissions(Permissions::from_mode(0o755))?;
        }

        f.write_all(binary_blob).map_err(Into::into)
    }

    fn get_default_version(&self) -> Result<Version, Error> {
        std::fs::read_to_string(self.path().join(".default_version"))
            .map_err(Into::into)
            .and_then(|str| Version::parse(str.trim_matches('/')).map_err(Into::into))
    }

    fn remove_default(&self) -> Result<(), Error> {
        std::fs::remove_file(self.path().join(".default_version")).map_err(Into::into)
    }

    fn set_default_version(&self, version: &Version) -> Result<(), Error> {
        let _lock_file = self.create_lock_file(&Version::new(0, 0, 0))?;

        std::fs::File::create(self.path().join(".default_version"))?
            .write_all(version.to_string().as_bytes())
            .map_err(Into::into)
    }

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
                let file = entry.path().join("build.json");
                let file = std::fs::read_to_string(file).ok()?;
                serde_json::from_str::<Build>(&file).ok()
            })
            .collect::<Vec<Build>>();
        Ok(files)
    }

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
