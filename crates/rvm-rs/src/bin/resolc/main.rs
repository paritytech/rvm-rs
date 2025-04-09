//! resolc wrapper

use anyhow::Context;
use rvm::VersionManager;
use std::io;
use std::process::{Command, ExitStatus, Stdio};

fn main() {
    let code = match runner() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("rvm: error: {err}");
            1
        }
    };
    std::process::exit(code);
}

fn runner() -> anyhow::Result<i32> {
    let mut args = std::env::args_os().skip(1).peekable();
    let manager = VersionManager::new(true)?;
    let bin = 'v: {
        if let Some(arg) = args.peek() {
            if let Some(arg) = arg.to_str() {
                if let Some(stripped) = arg.strip_prefix('+') {
                    let version = stripped
                        .parse::<semver::Version>()
                        .context("failed to parse version specifier")?;
                    args.next();
                    break 'v manager.get(&version, None)?;
                }
            }
        }
        manager.get_default()?
    };

    let bin_path = bin.local().expect("should not fail");

    if !bin_path.exists() {
        anyhow::bail!(
            "Resolc version {} is not installed or does not exist; looked at {}",
            bin.version(),
            bin_path.display()
        );
    }

    let mut cmd = Command::new(bin_path);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    Ok(exec(&mut cmd)?.code().unwrap_or(-1))
}

fn exec(cmd: &mut Command) -> io::Result<ExitStatus> {
    #[cfg(unix)]
    {
        use std::os::unix::prelude::*;
        Err(cmd.exec())
    }
    #[cfg(not(unix))]
    {
        cmd.status()
    }
}
