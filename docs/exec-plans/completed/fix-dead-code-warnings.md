# Fix Dead-Code Warnings

## Goal
Eliminate the 2 compiler dead-code warnings so `cargo check` stays completely clean.

## Success Criteria
- `cargo check` produces zero warnings.
- No behavioral change — all existing tests still pass.

## Current Warnings

1. **`render_tab_button` unused** — `src/app/render.rs:19`
   ```rust
   fn render_tab_button(...) { ... }
   ```
   A helper method in `impl ApiClientApp` that is never called. It appears to be a tab rendering utility that was replaced or never wired up.

2. **`event_id` and `timestamp_ms` fields never read** — `src/app/types.rs:403-406`
   ```rust
   pub(crate) struct StreamMessage {
       pub(crate) event_id: Option<SharedString>,  // never read
       pub(crate) timestamp_ms: u64,               // never read
       ...
   }
   ```
   These fields are populated from the domain `StreamMessage` but never referenced in the UI layer.

## Files / Modules Likely Affected
- `src/app/render.rs` — remove or annotate `render_tab_button`
- `src/app/types.rs` — add `#[allow(dead_code)]` or remove fields from `StreamMessage`

## Implementation Steps

1. **`render_tab_button`**: Check whether this method is truly unused (grep for calls). If definitely unused, either:
   - Remove it entirely (and the associated `button_base` + `render_button` dependency if no other callers remain), OR
   - Add `#[allow(dead_code)]` if it's kept for future use.
   
   Preferred: remove it. The `render_button` and `button_base` helpers exist separately and are used — `render_tab_button` is a wrapper that never got callers.

2. **`event_id` / `timestamp_ms`**: Two options:
   - **Option A**: Remove the fields from the app-level `StreamMessage` struct and the assignment in `from_domain()`. They're useful metadata but currently unused in the UI.
   - **Option B**: Add `#[allow(dead_code)]` on the struct if they're expected to be used soon (e.g. in streaming inspector display).
   
   Preferred: Option A (remove fields), since dead fields are dead weight. They remain available in the domain model if needed later.

3. Run `cargo check` to confirm zero warnings.

## Verification
```bash
cargo check 2>&1 | grep -c "warning:"  # must output 0
cargo test
```

## Open Questions / Blockers
None — straightforward cleanup.
