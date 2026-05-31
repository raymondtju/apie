# OS Keychain Secret Storage

> **Status**: Partial — Phases 1-2, 4-5 complete. Phase 3 (platform backend) deferred.

## Goal
Replace plaintext credential storage in workspace JSON with OS-native keychain-backed secret storage. Currently, auth values in `domain::Auth` are stored as plain strings in the workspace file. This plan moves secrets to the OS keychain (Linux: Secret Service / libsecret, macOS: Keychain, Windows: Credential Manager) and stores only opaque references in the JSON.

## Success Criteria (Completed)
- `SecretStore` trait with `MemorySecretStore` implementation + tests. ✅
- `domain::Auth` fields renamed from `_ref` to `_key` with serde `alias` for backward compatibility. ✅
- Auth flow passes secret store through `resolve_auth()` for key lookup at send-time. ✅
- `MemorySecretStore` wired as the default store in `ApiClientApp`, falling back to template-only resolution. ✅
- `cargo check` produces zero warnings. ✅
- `cargo test --lib` passes (39/39). ✅

## Remaining Work (Phase 3 — Platform Backend)
The `keyring` crate (v3+) provides OS-native credential storage but requires system libraries (`libsecret-1` on Linux, Security framework on macOS). To complete:

1. Add `keyring = { version = "3" }` to `Cargo.toml` with platform features.
2. Implement `PlatformSecretStore` wrapping `keyring::Entry`.
3. During `ApiClientApp::new()`, try platform store first; fall back to `MemorySecretStore`.
4. Update persistence layer to store literal secrets (not template refs like `{{token}}`) in keychain at save time, with load-time migration for existing inline secrets.

## Success Criteria
- New `SecretStore` abstraction with platform-native backends.
- `domain::Auth` stores secret references (lookup keys) instead of plaintext values.
- Existing workspace files with inline secrets are migrated on first load (backward compatible).
- Auth flow reads secrets from the keychain at send-time.
- Sensitive values are never written to disk as plaintext in workspace JSON.
- `cargo test` passes with a mock/in-memory backend that works in CI.

## Files / Modules Likely Affected
- `Cargo.toml` — add keyring crate dependency.
- `src/domain/secrets.rs` (new) — `SecretStore` trait + `MemorySecretStore` (testing) + platform impl.
- `src/domain/models.rs` — update `Auth` enum to hold opaque secret keys, add migration for inline values.
- `src/domain/persistence.rs` — call migration logic during workspace load.
- `src/app/types.rs` — update `Auth` app-layer mapping to work with async secret resolution.
- `src/app/resolve.rs` — resolve environment and secret variables before sending.
- `src/app/state.rs` — may need async-aware secret write-through on auth changes.
- `src/app/actions/settings.rs` or auth handlers — write secrets to keychain when user edits auth fields.
- `src/tests.rs` / `src/app/tests.rs` — update tests to use `MemorySecretStore`.

## Implementation Steps

### Phase 1 — Abstraction
1. Create `src/domain/secrets.rs` with:
   ```rust
   pub trait SecretStore {
       fn store(&mut self, key: &str, value: &str) -> Result<()>;
       fn get(&self, key: &str) -> Result<Option<String>>;
       fn delete(&mut self, key: &str) -> Result<()>;
   }

   pub struct MemorySecretStore { /* HashMap */ }
   impl SecretStore for MemorySecretStore { ... }
   ```
2. Wire `MemorySecretStore` into the app for initial testing.

### Phase 2 — Domain Model Migration
3. Change `domain::Auth::Basic` fields from `username_ref: String, password_ref: String` to `username_key: String, password_key: String` (or clearer naming).
4. Add a load-time migration that detects inline secrets in older workspace files, stores them into the secret store, and replaces the values with generated keys.
5. Ensure serialization skips the old plaintext fields (use `#[serde(skip)]` or a versioned struct).

### Phase 3 — Platform Backend
6. Add `keyring` crate to `Cargo.toml`.
7. Implement `PlatformSecretStore` using the `keyring` crate, with fallback to `MemorySecretStore` when the keychain is unavailable (headless/CI).
8. On app startup, instantiate the platform store; if it fails, log a warning and use memory store.

### Phase 4 — Wire Through App
9. Update `src/app/resolve.rs` to call `secret_store.get()` when resolving auth values.
10. Update auth field edit handlers in `src/app/actions/` to call `secret_store.store()` on changes.
11. Remove the old `Summary` display that leaks secret values (or mask them).

### Phase 5 — Cleanup
12. Remove `#[allow(dead_code)]` on `Auth::summary()` now that it's used.
13. Update `ARCHITECTURE.md` to document `src/domain/secrets.rs`.

## Verification
```bash
cargo check
cargo test
# Manual: launch app, set a Bearer token, inspect workspace JSON — token should not appear in plaintext.
```

## Open Questions / Blockers
- GPUI is single-threaded synchronous for rendering; keychain access may be blocking (disk/DBus). Need to decide: spawn blocking task via `cx.background_spawn()` or use an in-memory cache with async write-back.
- The `keyring` crate requires platform-specific runtime deps (libsecret on Linux, etc.). CI setup may need `sudo apt install libsecret-1-dev`.
- Secret key naming scheme: use a prefix like `apie/{workspace_id}/{request_id}/auth/{field}` to avoid collisions.
- What about environment variable secrets (e.g. `{{token}}` in env variables)? Those are also stored in workspace JSON plaintext currently — should they go through the keychain too?
