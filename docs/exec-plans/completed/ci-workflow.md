# CI Workflow

## Goal
Set up a GitHub Actions CI pipeline that runs `cargo fmt --check`, `cargo check`, and `cargo test` on every push and PR to the main branch, catching regressions before they land.

## Success Criteria
- CI triggers on `push` and `pull_request` to `main`.
- Pipeline runs `cargo fmt --check`, `cargo check`, and `cargo test`.
- Pipeline passes on the current HEAD commit.
- CI badge in `README.md` (optional).
- Failing pipeline blocks merge (branch protection set separately by the user).

## Files / Modules Likely Affected
- `.github/workflows/ci.yml` (new file)
- `rust-toolchain.toml` or `rust-toolchain` (new file — pin Rust version for reproducible CI)
- `README.md` (add CI badge)

## Implementation Steps

1. Create `.github/workflows/ci.yml` with:
   - Trigger: `push` and `pull_request` on `main`.
   - Job `check` running on `ubuntu-latest`.
   - Steps: checkout, install Rust (stable via `actions-rust-lang/setup-rust-toolchain`), cache cargo, run `cargo fmt --check`, `cargo check`, `cargo test`.
   - Note: the project uses `clang` + `mold` linker locally; CI should use the default Rust linker (no special linker setup needed for basic compilation).
2. Add `rust-toolchain.toml` pinning the stable channel so local and CI builds agree.
3. Run `cargo fmt --check` locally first to confirm formatting passes — if not, `cargo fmt` first.
4. Push and verify the workflow runs green on GitHub.
5. (Optional) Add a `[![CI](...))](...)` badge to `README.md`.

## Verification
```bash
# Locally simulate CI checks
cargo fmt --check
cargo check
cargo test
```

## Open Questions / Blockers
- Does the project need `GNOME` / `x11` / `wayland` or other system dependencies on CI for GPUI test support? GPUI tests may need `libxkbcommon` and other system libs — check GPUI's own CI setup.
- GPUI smoke tests may require a virtual framebuffer (`xvfb`). If so, add `sudo apt install libxkbcommon-x11-dev libwayland-dev xvfb` and wrap `cargo test` with `xvfb-run`.
