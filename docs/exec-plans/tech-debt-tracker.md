# Tech Debt Tracker

Track small known gaps here instead of scattering them through chat.

| Item | Area | Status | Notes |
| --- | --- | --- | --- |
| Add screenshot-based visual regression | UI | Open | Useful once layout stabilizes. |
| Add CI workflow | Tooling | Completed | `.github/workflows/ci.yml` and `rust-toolchain.toml` created. |
| OS keychain secret storage | Security | Partial | Phases 1-2, 4-5 complete (`SecretStore` trait, domain Auth rename, resolve wiring, dead-code cleanup). Phase 3 (platform keyring backend) deferred — needs system libraries. See `docs/exec-plans/active/keychain-secret-storage.md`. |
| Fix dead-code warnings (3 items) | Code Quality | Completed | Removed `render_tab_button`, `event_id`/`timestamp_ms` fields, `Auth::summary()`, `AuthLocation::next()`. `cargo check` now zero warnings. |
| Add release packaging | Distribution | Open | Needed before external desktop use. |
| Reuse shared scrollbar beyond response body | UI | Open | Response Body uses `src/ui/scrollbar.rs` (vertical + horizontal); evaluate reuse for other scrollable panes if they need visible custom scrollbars. |

## Recently Resolved
- Runtime asset loading fragility (`CARGO_MANIFEST_DIR` baked paths causing missing icons on `cargo run` after project moves) — fixed by embedding all 12 SVGs via `include_bytes!` (see `src/ui/icons.rs` + `src/main.rs`). No longer a source of silent dev-time failures. (2026-05)
- HTTPS support — fully implemented in `src/http/mod.rs:84-128` using `tokio_rustls::TlsConnector` with `rustls::ClientConfig` and `webpki_roots` cert store. TLS handshake timing is instrumented separately in the timing popover. (2026-01)
