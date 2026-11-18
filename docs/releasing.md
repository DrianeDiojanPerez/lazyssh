# Releasing

You need `goreleaser`, `zig`, and a `GITHUB_TOKEN` in your environment. The release hooks install `cargo-zigbuild` and the Rust targets themselves.

1. Bump the version and tag it:
   ```
   just add-version 0.0.9
   ```
   It updates `Cargo.toml` and `Cargo.lock`, commits `release v0.0.9`, and makes an annotated tag.
2. Push the commit and the tag:
   ```
   git push --follow-tags
   ```
3. Build and publish:
   ```
   just release
   ```
   It runs the tests first and stops if any fail. Then it builds Linux x86_64, Linux arm64, and Windows x86_64 with `cargo zigbuild`, and uploads the archives and a checksums file to the GitHub release.

`just dry-release` builds everything into `dist/` without publishing, which is a good check before a real release.

All three targets build with zig, so no Docker is needed. The Windows build used to go through `cross`, but its MinGW image is too old for Rust 1.100 and fails to link with `cannot find -lsynchronization`.
