//! # Revive Version Manager library code
//!
//! Used by the `rvm` binary, but can be used separately in libraries and applications
#![cfg_attr(
    not(any(test, feature = "cli", feature = "resolc")),
    warn(unused_crate_dependencies)
)]

use constants::Platform;
use fs::FsPaths;
use semver::Version;
use std::collections::{BTreeMap, BTreeSet};

mod constants;
mod errors;
mod fs;
mod releases;
pub use errors::Error;
pub use releases::{Binary, BinaryInfo};
use releases::{Build, Releases};

/// Version manager responsible for handling Resolc installation.
pub struct VersionManager {
    pub(crate) fs: Box<dyn FsPaths>,
    releases: Releases,
    offline: bool,
}

impl VersionManager {
    /// Instantiate the version manager
    ///
    /// # Arguments
    ///
    /// * `offline` - run in offline mode.
    pub fn new(offline: bool) -> Result<Self, Error> {
        let fspaths = fs::DataDir::new()?;
        let releases = if offline {
            Self::get_releases_offline(&fspaths)?
        } else {
            Self::get_releases()?
        };
        Ok(Self {
            offline,
            fs: Box::new(fspaths),
            releases,
        })
    }

    #[cfg(test)]
    /// For use in tests
    pub fn new_in_temp() -> Self {
        use test::TempDir;
        let releases = Self::get_releases().expect("no network");

        VersionManager {
            offline: false,
            fs: Box::new(TempDir::new().unwrap()),
            releases,
        }
    }

    fn get_releases() -> Result<Releases, Error> {
        let url = Platform::get()?.download_url()?;
        Releases::new(url)
    }

    fn get_releases_offline(data: &impl FsPaths) -> Result<Releases, Error> {
        let installed = data.installed_versions()?;
        if installed.is_empty() {
            return Err(Error::NoVersionsInstalled);
        }
        let releases = BTreeMap::from_iter(installed.iter().map(|data| {
            (
                data.version.clone(),
                format!("{}+{}", data.name, data.long_version),
            )
        }));

        let latest_release = installed
            .iter()
            .max_by(|a, b| a.version.cmp(&b.version))
            .map(|x| &x.version)
            .cloned()
            .expect("Cant be empty");

        Ok(Releases {
            builds: installed,
            releases,
            latest_release,
        })
    }

    /// Returns an already present Resolc binary
    ///
    /// # Arguments
    ///
    /// * `resolc_version` - required Resolc version
    /// * `solc_version` - optional `solc` version requirement, passing this will also check the compatibility between the two compiler versions
    pub fn get(
        &self,
        resolc_version: &Version,
        solc_version: Option<Version>,
    ) -> Result<Binary, Error> {
        let releases = &self.releases;
        let build = releases.get_build(resolc_version)?;

        if let Some(solc_version) = solc_version {
            build.check_solc_compat(&solc_version)?;
        };

        if self
            .fs
            .path()
            .to_path_buf()
            .join(resolc_version.to_string())
            .join(&build.name)
            .exists()
        {
            Ok(build.clone().into_local(self.fs.path()))
        } else {
            Err(Error::NotInstalled {
                version: resolc_version.clone(),
            })
        }
    }

    /// Returns an already present binary or installs the requested Resolc version
    ///
    /// # Arguments
    ///
    /// * `resolc_version` - required Resolc version
    /// * `solc_version` - optional `solc` version requirement, passing this will also check the compatibility between the two compiler versions
    pub fn get_or_install(
        &self,
        resolc_version: &Version,
        solc_version: Option<Version>,
    ) -> Result<Binary, Error> {
        if let bin @ Ok(_) = self.get(resolc_version, solc_version) {
            return bin;
        }

        if self.offline {
            return Err(Error::CantInstallOffline);
        }

        let build = self.releases.get_build(resolc_version)?;

        let binary = build.download_binary()?;

        self.fs.install_version(build, &binary)?;

        Ok(build.clone().into_local(self.fs.path()))
    }

    /// Uninstall the listed version if it exists in path
    pub fn remove(&self, version: &Version) -> Result<(), Error> {
        if !self
            .fs
            .path()
            .to_path_buf()
            .join(version.to_string())
            .exists()
        {
            return Err(Error::NotInstalled {
                version: version.clone(),
            });
        }

        self.fs.remove_version(version)
    }

    /// Returns the version used by default
    pub fn get_default(&self) -> Result<Binary, Error> {
        let version = self.fs.get_default_version().map_err(|e| match e {
            Error::IoError(_) => Error::DefaultVersionNotSet,
            e => e,
        })?;

        self.get(&version, None)
    }

    /// Sets the default used version
    pub fn set_default(&self, version: &Version) -> Result<(), Error> {
        let _ = self.get(version, None)?;
        self.fs.set_default_version(version)
    }

    /// Lists all installed and available Resolc versions
    ///
    /// # Arguments
    ///
    /// * `solc_version` - optional `solc` version requirement, passing this will only return compilers that support given `solc` version.
    pub fn list_available(&self, solc_version: Option<Version>) -> Result<Vec<Binary>, Error> {
        let releases = &self.releases;
        let mut installed_versions = BTreeSet::new();

        let installed: Result<Vec<Binary>, Error> = self
            .fs
            .installed_versions()?
            .into_iter()
            .filter_map(|build| {
                if let Some(solc_version) = &solc_version {
                    build.check_solc_compat(solc_version).ok()?;
                    Some(build)
                } else {
                    Some(build)
                }
            })
            .map(|x| {
                installed_versions.insert(x.version.clone());
                Ok::<releases::Binary, Error>(x.into_local(self.fs.path()))
            })
            .collect();

        let mut available: Vec<Binary> = releases
            .builds
            .iter()
            .filter(|build| !installed_versions.contains(&build.version))
            .cloned()
            .map(|build| build.into_remote())
            .collect();
        let mut installed = installed?;
        installed.append(&mut available);
        installed.sort();
        Ok(installed)
    }
}

#[cfg(test)]
mod test {
    use std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
    };

    use expect_test::expect;
    use semver::Version;

    use crate::{Binary, Error, FsPaths, VersionManager};

    /// Temp directory storage
    #[derive(Clone)]
    pub struct TempDir {
        path: PathBuf,
    }

    impl FsPaths for TempDir {
        fn new() -> Result<Self, Error> {
            use tempfile::tempdir;
            let path = tempdir()?.into_path();

            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            self.path.as_path()
        }
    }

    pub fn get_version_for_path(path: &Path) -> String {
        let mut cmd = Command::new(path);
        cmd.arg("--version")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped());
        let output = cmd.output().expect("Should not fail");
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn install() {
        let manager = VersionManager::new_in_temp();

        if let Binary::Local { path, .. } = manager
            .get_or_install(&Version::parse("0.1.0-dev.13").unwrap(), None)
            .expect("should be installed")
        {
            let version = get_version_for_path(&path);
            let expected = expect![[r#"
                Solidity frontend for the revive compiler version 0.1.0-dev.13+commit.ad33153.llvm-18.1.8
            "#]];
            expected.assert_eq(&version);
        } else {
            panic!()
        }
    }

    #[test]
    fn set_default_and_remove() {
        let manager = VersionManager::new_in_temp();
        let bin = manager
            .get_or_install(&Version::parse("0.1.0-dev.13").unwrap(), None)
            .unwrap();

        manager
            .set_default(bin.version())
            .expect("should be installed");

        manager
            .remove(bin.version())
            .expect("removed default version");

        expect!["Default version of Resolc is not set"].assert_eq(&format!(
            "{}",
            manager.get_default().expect_err("error should happen")
        ));
    }

    #[test]
    fn get_set_default() {
        let manager = VersionManager::new_in_temp();
        let bin = manager
            .get_or_install(&Version::parse("0.1.0-dev.13").unwrap(), None)
            .unwrap();

        manager
            .set_default(bin.version())
            .expect("should be installed");
        if let Binary::Local { path, .. } = manager.get_default().expect("should be installed") {
            let version = get_version_for_path(&path);
            let expected = expect![[r#"
                Solidity frontend for the revive compiler version 0.1.0-dev.13+commit.ad33153.llvm-18.1.8
            "#]];
            expected.assert_eq(&version);
        } else {
            panic!()
        }
    }

    #[test]
    fn list_available() {
        let manager = VersionManager::new_in_temp();

        let result = manager.list_available(None).unwrap();
        let expected = expect![[r#"
            [
                Remote {
                    version: "0.1.0-dev.13",
                    solc_req: ">=0.8.0, <=0.8.29",
                },
            ]"#]];

        expected.assert_eq(&format!("{result:#?}"));
        manager
            .get_or_install(&Version::parse("0.1.0-dev.13").unwrap(), None)
            .unwrap();
        manager
            .set_default(&Version::parse("0.1.0-dev.13").unwrap())
            .expect("should be installed");

        let mut result = manager.list_available(None).unwrap();

        for bin in result.iter_mut() {
            if let Binary::Local { path, .. } = bin {
                *path = PathBuf::new();
            }
        }

        let expected = expect![[r#"
            [
                Installed {
                    path: "",
                    version: "0.1.0-dev.13",
                    solc_req: ">=0.8.0, <=0.8.29",
                },
            ]"#]];

        expected.assert_eq(&format!("{result:#?}"));
    }
}
