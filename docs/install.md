# Installing

## The one-line installer

```
curl -fsSL https://raw.githubusercontent.com/DrianeDiojanPerez/lazyssh/refs/heads/master/install.sh | bash
```

It runs these steps:

1. Works out your system and CPU from `uname`.
2. Asks GitHub for the newest release.
3. If lazyssh is already installed and up to date, asks whether to install it again anyway.
4. Downloads `lazyssh_<System>_<arch>.tar.gz` (a `.zip` on Windows) and unpacks it.
5. Puts the binary in `/usr/local/bin/lazyssh`. If that folder needs root, it asks for your sudo password. On Windows, under Git Bash, it uses `~/bin/lazyssh.exe` and tells you if `~/bin` isn't in your `PATH`.

| System | Builds |
|---|---|
| Linux | x86_64, arm64 |
| Windows | x86_64 |

There's no macOS build yet, so the installer fails there.

## By hand

The steps are in the [README](../README.md#manual-install).

## Build from source

Install [Rust](https://rustup.rs), then:

```
git clone https://github.com/DrianeDiojanPerez/lazyssh.git
cd lazyssh
cargo build --release
sudo install -m 755 target/release/lazyssh /usr/local/bin/lazyssh
```

`cargo test` runs the tests. Some of them start real processes on a pseudo-terminal, so they need `bash`.
