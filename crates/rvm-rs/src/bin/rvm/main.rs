//! Main Resolc version manager entrypoint  

use std::time::Duration;

use clap::{Parser, Subcommand};
use indicatif::ProgressBar;
use rvm::{Binary, Error, VersionManager};
use semver::Version;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser)]
#[clap(
    name = "resolc version manager",
    version = VERSION,
)]
struct Cli {
    /// Run in offline mode
    #[arg(short, long, default_value_t = false)]
    offline: bool,
    #[clap(subcommand)]
    command: Rvm,
}

/// Resolc version manager.
#[derive(Debug, Subcommand)]
enum Rvm {
    /// Install given version of Resolc
    Install {
        /// Resolc version
        version: Version,
        /// Use as default Resolc version,
        #[arg(long, default_value_t = false)]
        set_default: bool,
    },
    /// Uninstall given version of Resolc
    Remove(WithVersion),
    /// Print path to the installed Resolc version
    Which(WithVersion),
    /// Set a default Resolc version to use
    Use {
        /// Resolc version
        version: Version,
        /// Install Resolc binary if it's not already installed
        #[arg(long, default_value_t = false)]
        install: bool,
    },
    /// List all available and installed versions of Resolc.
    /// Also prints default Resolc version if it's present.
    List,
}
#[allow(missing_docs)]
#[derive(Debug, Parser, Clone)]
pub struct WithVersion {
    /// Resolc version
    version: Version,
}

fn spinner(msg: String) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(150));
    spinner.set_message(msg);
    spinner
}

fn exec(is_offline: bool, rvm: Rvm, manager: VersionManager) -> anyhow::Result<(), anyhow::Error> {
    match rvm {
        Rvm::Install {
            version,
            set_default,
        } => {
            if is_offline {
                return Err(Error::CantInstallOffline.into());
            }

            if manager.is_installed(&version) {
                println!("Resolc v{} is already installed", version);
                return Ok(());
            }

            let spinner = spinner(format!("Downloading and installing Resolc v{}", version));
            manager.get_or_install(&version, None)?;
            spinner.finish_with_message(format!("Resolc v{} is installed succesfully", version));
            if set_default {
                manager.set_default(&version)?;
                println!("Succesfully set Resolc v{} as default", version)
            }
        }
        Rvm::Remove(WithVersion { version }) => {
            manager.remove(&version)?;
            println!("Resolc v{} is removed succesfully", version);
        }
        Rvm::List => {
            let versions = manager.list_available(None)?;
            if let Ok(default) = manager.get_default() {
                println!("Default version of Resolc is: {}", default.version())
            }
            println!(
                "Available to install Resolc version: {:?}",
                versions
                    .iter()
                    .filter_map(|x| match x {
                        Binary::Remote(binary_info) => Some(binary_info.version.to_string()),
                        _ => None,
                    })
                    .collect::<Vec<String>>()
            );
            println!(
                "Already installed Resolc versions: {:?}",
                versions
                    .iter()
                    .filter_map(|x| match x {
                        Binary::Local { info, .. } => Some(info.version.to_string()),
                        _ => None,
                    })
                    .collect::<Vec<String>>()
            )
        }
        Rvm::Use { version, install } => {
            if !is_offline && install && manager.get(&version, None).is_err() {
                let spinner = spinner(format!("Downloading and installing Resolc v{}", version));
                manager.get_or_install(&version, None)?;
                spinner
                    .finish_with_message(format!("Resolc v{} is installed succesfully", version));
            }
            manager.set_default(&version)?;
            println!("Succesfully set Resolc v{} as default", version)
        }
        Rvm::Which(WithVersion { version }) => {
            let build = manager.get(&version, None)?;
            println!(
                "Path to the requested binary version of Resolc: {}",
                build.local().expect("Can't happen").to_string_lossy()
            );
        }
    };
    Ok(())
}

fn main() -> anyhow::Result<(), anyhow::Error> {
    let rvm = Cli::parse();
    let manager = VersionManager::new(rvm.offline).unwrap();
    exec(rvm.offline, rvm.command, manager)
}
