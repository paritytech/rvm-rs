# Resolc compiler version manager

This project provides a (limited)cross-platform support for managing Resolc compiler versions.

[info on supported platforms](https://contracts.polkadot.io/revive_compiler/installation#resolcbinary-releases)
- Linux (MUSL)
- MacOS (uinversal)
- Windows x86_64

## Install

From [crates.io](https://crates.io):

```bash
cargo install rvm-rs
```

Or from the repository:

```bash
cargo install --locked --git https://github.com/paritytech/rvm-rs.git
```

This will install both `rvm` and `resolc` binaries.

## `rvm` Usage

```bash
Resolc version manager

Usage: rvm [OPTIONS] <COMMAND>

Commands:
  install  Install given version of Resolc
  remove   Uninstall given version of Resolc
  which    Print path to the installed Resolc version
  use      Set a default Resolc version to use
  list     List all available and installed versions of Resolc. Also prints default Resolc version if it's present
  help     Print this message or the help of the given subcommand(s)

Options:
  -o, --offline  Run in offline mode
  -h, --help     Print help
  -V, --version  Print version
```

## `resolc` Usage

Please refer to [this page](https://contracts.polkadot.io/revive_compiler/usage)

Wrapper installed by this project also provides additional options: 

* `resolc +<version>` - where `+<version>` is any version that is installed on the system. Otherwise globally set default version will be used.