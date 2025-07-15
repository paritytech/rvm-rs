use semver::Version;

use crate::errors::Error;

/// Repository URL for list.json files with supported Resolc versions
pub const REPO_URL: &str = "https://paritytech.github.io/resolc-bin/";

/// Minimum supported `solc` version.
pub(crate) const MIN_VERSION: Version = semver::Version::new(0, 8, 0);

#[derive(Eq, PartialEq, PartialOrd, Ord)]
pub(crate) enum Platform {
    Linux,
    Macos,
    Windows,
}

impl Platform {
    pub(crate) fn get() -> Result<Self, Error> {
        let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => Self::Linux,
            ("macos", "aarch64") | ("macos", "x86_64") => Self::Macos,
            ("windows", "x86_64") => Self::Windows,
            (os, target) => {
                return Err(Error::PlatformNotSupported {
                    os: os.to_string(),
                    target: target.to_string(),
                })
            }
        };
        Ok(platform)
    }

    pub(crate) fn download_url(&self, nightly: bool) -> Result<url::Url, Error> {
        let platform_path = match self {
            Platform::Linux => "linux",
            Platform::Macos => "macos",
            Platform::Windows => "windows",
        };
        let url = if nightly {
            format!("{REPO_URL}/nightly/{platform_path}/list.json")
        } else {
            format!("{REPO_URL}/{platform_path}/list.json")
        };
        let url = url::Url::parse(&url)?;
        Ok(url)
    }
}
