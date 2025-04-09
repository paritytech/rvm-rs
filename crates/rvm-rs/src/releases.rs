use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use semver::{Comparator, Prerelease, Version};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{constants::MIN_VERSION, errors::Error};

/// Resolc equivalent of `list.json` of `solc` releases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Releases {
    pub(crate) builds: Vec<Build>,
    pub(crate) releases: BTreeMap<Version, String>,
    #[serde(rename = "latestRelease")]
    pub(crate) latest_release: Version,
}

impl Releases {
    /// Grabs all releases from the remote `url`.
    pub fn new(url: url::Url) -> Result<Releases, Error> {
        reqwest::blocking::get(url)?.json().map_err(Into::into)
    }

    /// Returns a build by Resolc version if it's present
    pub fn get_build(&self, version: &Version) -> Result<&Build, Error> {
        self.releases
            .get(version)
            .and_then(|_| self.builds.iter().find(|item| item.version == *version))
            .ok_or_else(|| Error::UnknownVersion {
                version: version.clone(),
            })
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Basic information about Resolc binary
pub struct BinaryInfo {
    /// Resolc version
    pub version: Version,
    /// first supported `solc` version
    pub first_supported_solc_version: Version,
    /// last supported `solc` version
    pub last_supported_solc_version: Version,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Basic information about Resolc binary including whether or not it's already installed
pub enum Binary {
    /// Resolc binaries that are installed locally
    Local {
        /// Path to the installed binary
        path: PathBuf,
        /// Basic info about Resolc library
        info: BinaryInfo,
    },
    /// Resolc binaries that are available and can be downloaded
    Remote(BinaryInfo),
}

impl std::fmt::Debug for Binary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Binary::Local { path, info } => f
                .debug_struct("Installed")
                .field("path", path)
                .field("version", &info.version.to_string())
                .field(
                    "solc_req",
                    &semver::VersionReq {
                        comparators: vec![
                            Comparator {
                                op: semver::Op::GreaterEq,
                                major: info.first_supported_solc_version.major,
                                minor: Some(info.first_supported_solc_version.minor),
                                patch: Some(info.first_supported_solc_version.patch),
                                pre: Prerelease::default(),
                            },
                            Comparator {
                                op: semver::Op::LessEq,
                                major: info.last_supported_solc_version.major,
                                minor: Some(info.last_supported_solc_version.minor),
                                patch: Some(info.last_supported_solc_version.patch),
                                pre: Prerelease::default(),
                            },
                        ],
                    }
                    .to_string(),
                )
                .finish(),
            Binary::Remote(info) => f
                .debug_struct("Remote")
                .field("version", &info.version.to_string())
                .field(
                    "solc_req",
                    &semver::VersionReq {
                        comparators: vec![
                            Comparator {
                                op: semver::Op::GreaterEq,
                                major: info.first_supported_solc_version.major,
                                minor: Some(info.first_supported_solc_version.minor),
                                patch: Some(info.first_supported_solc_version.patch),
                                pre: Prerelease::default(),
                            },
                            Comparator {
                                op: semver::Op::LessEq,
                                major: info.last_supported_solc_version.major,
                                minor: Some(info.last_supported_solc_version.minor),
                                patch: Some(info.last_supported_solc_version.patch),
                                pre: Prerelease::default(),
                            },
                        ],
                    }
                    .to_string(),
                )
                .finish(),
        }
    }
}

impl Binary {
    /// Returns the version for the given `Binary`
    pub fn version(&self) -> &Version {
        match self {
            Binary::Local { info, .. } => &info.version,
            Binary::Remote(info) => &info.version,
        }
    }
    /// Returns the path for the given `Binary`
    pub fn local(&self) -> Option<&Path> {
        match self {
            Binary::Local { path, .. } => Some(path.as_ref()),
            Binary::Remote(_) => None,
        }
    }
}

/// Basic information about Resolc build that is available to be installed
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    pub(crate) name: String,
    pub(crate) version: Version,
    #[serde(rename = "longVersion")]
    pub(crate) long_version: String,
    pub(crate) url: Url,
    #[serde(rename = "firstSolcVersion")]
    pub(crate) first_supported_solc_version: Version,
    #[serde(rename = "lastSolcVersion")]
    pub(crate) last_supported_solc_version: Version,
}

impl Build {
    /// Checks compatibility between selected Resolc and `solc` versions
    ///
    /// # Arguments
    ///
    /// * `solc_version` -  `solc` version requirement this will allow check the compatibility between the two compiler versions
    pub fn check_solc_compat(&self, solc_version: &Version) -> Result<(), Error> {
        let version_req = semver::VersionReq {
            comparators: vec![
                Comparator {
                    op: semver::Op::GreaterEq,
                    major: self.first_supported_solc_version.major,
                    minor: Some(self.first_supported_solc_version.minor),
                    patch: Some(self.first_supported_solc_version.patch),
                    pre: Prerelease::default(),
                },
                Comparator {
                    op: semver::Op::LessEq,
                    major: self.last_supported_solc_version.major,
                    minor: Some(self.last_supported_solc_version.minor),
                    patch: Some(self.last_supported_solc_version.patch),
                    pre: Prerelease::default(),
                },
            ],
        };
        if version_req.matches(solc_version) && solc_version >= &MIN_VERSION {
            Ok(())
        } else {
            Err(Error::SolcVersionNotSupported {
                solc_version: solc_version.clone(),
                resolc_version: self.version.clone(),
                supported_range: version_req,
            })
        }
    }

    /// Downloads the binary for the given version
    pub fn download_binary(&self) -> Result<Vec<u8>, Error> {
        let binary = reqwest::blocking::ClientBuilder::new()
            .timeout(Duration::from_secs(300))
            .build()?
            .get(self.url.as_ref())
            .send()?
            .error_for_status()?;

        Ok(binary.bytes().map(|x| x.to_vec())?)
    }

    pub(crate) fn into_local(self, path: &Path) -> Binary {
        Binary::Local {
            path: path.join(self.version.to_string()).join(self.name),
            info: BinaryInfo {
                version: self.version,
                first_supported_solc_version: self.first_supported_solc_version,
                last_supported_solc_version: self.last_supported_solc_version,
            },
        }
    }

    pub(crate) fn into_remote(self) -> Binary {
        Binary::Remote(BinaryInfo {
            version: self.version,
            first_supported_solc_version: self.first_supported_solc_version,
            last_supported_solc_version: self.last_supported_solc_version,
        })
    }
}

#[cfg(test)]
mod test {
    use semver::Version;

    use super::{Build, Releases};

    fn release() -> &'static str {
        r#"{
            "builds": [
                {
                    "path": "resolc-x86_64-unknown-linux-musl+0.1.0-dev.17+commit.6bcd04b2",
                    "name": "resolc-x86_64-unknown-linux-musl",
                    "version": "0.1.0-dev.17",
                    "build": "commit.6bcd04b2",
                    "longVersion": "0.1.0-dev.17+commit.6bcd04b2",
                    "url": "https://github.com/paritytech/revive-workflow-test/releases/download/v0.1.0-dev.17/resolc-x86_64-unknown-linux-musl",
                    "firstSolcVersion": "0.8.0",
                    "lastSolcVersion": "0.8.29"
                },
                {
                    "path": "resolc-x86_64-unknown-linux-musl+0.1.0-dev.18+commit.f1e51fda",
                    "name": "resolc-x86_64-unknown-linux-musl",
                    "version": "0.1.0-dev.18",
                    "build": "commit.f1e51fda",
                    "longVersion": "0.1.0-dev.18+commit.f1e51fda",
                    "url": "https://github.com/paritytech/revive-workflow-test/releases/download/v0.1.0-dev.18/resolc-x86_64-unknown-linux-musl",
                    "firstSolcVersion": "0.8.0",
                    "lastSolcVersion": "0.8.29"
                }
            ],
            "releases": {
                "0.1.0-dev.17": "resolc-x86_64-unknown-linux-musl+0.1.0-dev.17+commit.6bcd04b2",
                "0.1.0-dev.18": "resolc-x86_64-unknown-linux-musl+0.1.0-dev.18+commit.f1e51fda"
            },
            "latestRelease": "0.1.0-dev.18"
        }"#
    }

    #[test]
    fn find_version() {
        let release: Releases = serde_json::from_str(&release()).unwrap();
        release
            .get_build(&Version::parse("0.1.0-dev.17").unwrap())
            .unwrap()
            .check_solc_compat(&Version::new(0, 8, 0))
            .unwrap()
    }

    #[test]
    fn solc_version_support() {
        let build = r#"
        {
            "path": "resolc-x86_64-unknown-linux-musl+0.1.0-dev.17+commit.6bcd04b2",
            "name": "resolc-x86_64-unknown-linux-musl",
            "version": "0.1.0-dev.17",
            "build": "commit.6bcd04b2",
            "longVersion": "0.1.0-dev.17+commit.6bcd04b2",
            "url": "https://github.com/paritytech/revive-workflow-test/releases/download/v0.1.0-dev.17/resolc-x86_64-unknown-linux-musl",
            "firstSolcVersion": "0.8.0",
            "lastSolcVersion": "0.8.29"
        }
        "#;

        let build: Build = serde_json::from_str(build).unwrap();

        assert_eq!(
            r#"
            Unsupported version of `solc` - v0.3.4 for Resolc v0.1.0-dev.17. Only versions ">=0.8.0, <=0.8.29" is supported by this version of Resolc
            "#.trim(),
            build
                .check_solc_compat(&Version::new(0, 3, 4))
                .expect_err("Expecting error")
                .to_string()
        );
    }
}
