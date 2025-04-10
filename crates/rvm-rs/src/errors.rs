use semver::Version;
use thiserror::Error as DeriveError;

/// Errors that can occur when using Resolc version manager
#[derive(DeriveError, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Default version of Resolc is not set")]
    DefaultVersionNotSet,
    #[error("Can't install new Resolc versions in offline mode")]
    CantInstallOffline,
    #[error("No versions are installed")]
    NoVersionsInstalled,
    #[error("Unknown version of Resolc v{}.", version)]
    UnknownVersion { version: Version },
    #[error("Version of Resolc v{} is not installed.", version)]
    NotInstalled { version: Version },
    #[error(
        "Checksum validation error occured when checking binary. Expected: {expected}, got: {actual}"
    )]
    ChecksumValidationError { expected: String, actual: String },
    #[error(
        "Unsupported version of `solc` - v{} for Resolc v{}. Only versions \"{}\" is supported by this version of Resolc",
        solc_version,
        resolc_version,
        supported_range
    )]
    SolcVersionNotSupported {
        solc_version: Version,
        resolc_version: Version,
        supported_range: semver::VersionReq,
    },
    #[error("Unsupported platform {os}_{target}")]
    PlatformNotSupported { os: String, target: String },
    #[error(transparent)]
    SemverError(#[from] semver::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    UrlError(#[from] url::ParseError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    HexDecodign(#[from] hex::FromHexError),
}
