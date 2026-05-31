# Architecture

This repository is a Rust GPUI desktop API client. It is intentionally small: the app should remain easy for agents to inspect, run, and modify locally.

## Current Structure
The app is split into a reusable core, a GPUI shell, and local UI primitives:

```text
src/
  lib.rs                 public facade for domain + HTTP
  main.rs                GPUI process startup, assets, key bindings, window creation
  settings.rs            persisted theme, UI font size, and buffer font size
  tests.rs               lib-level integration tests
  domain/
    models.rs            serializable core request, response, auth, environment, workspace models
    workspace.rs         workspace operations and send helpers
    persistence.rs       JSON load/save and default workspace path
    preview.rs           domain-only synthetic response helper for tests/demo paths
    error.rs             core error/result types
    secrets.rs           SecretStore trait, MemorySecretStore, and secret key helpers
  http/mod.rs            custom hyper+tokio-rustls HTTP stack with instrumented per-phase timing (DNS, TCP Connect, TLS, TTFB, Transfer)
  app/mod.rs             ApiClientApp struct definition + constructor + re-exports
  app/types.rs           GPUI-local types: Method, Auth, Header, Request, Workspace, etc.
  app/resolve.rs         template resolution, auth application, body formatting helpers
  app/state.rs           input management helpers (field_input, body_input, sync, persist)
  app/actions/
    mod.rs               module barrel
    crud.rs              collection CRUD, tab management, param/header/body controls
    send.rs              request sending, cancel, dialog action handlers
    settings.rs          settings dialog, method menu, clipboard
    dialogs.rs           rename/delete dialog state machines, context menus
  app/render.rs          all render_* methods + impl Render for ApiClientApp
  app/tests.rs           GPUI interaction and smoke tests
  ui/mod.rs              module root + re-exports (was src/ui.rs)
  ui/
    code_input.rs        multiline code input with line numbers, folding, syntax color
    input.rs             single-line editor-like text input
    scrollbar.rs         custom vertical and horizontal scrollbar primitives
    controls.rs          buttons/select/menu building blocks
    layout.rs            shell, toolbar, panel, and section primitives
    tabs.rs              tab rendering and close affordances
    theme.rs             Zed Dark/Light theme tokens
    typography.rs        UI and buffer font helpers
    spacing.rs           compact spacing scale
    icons.rs             SVG icon loading helpers
    tests.rs             UI primitive tests
```

Supporting folders:

- `assets/icons/`: SVG icons **embedded at compile time** (`include_bytes!` in `src/ui/icons.rs`) and served by the `AssetSource` implementation in `src/main.rs`. (This replaced the previous runtime fs loader that depended on `CARGO_MANIFEST_DIR` and caused missing icons after project moves.)
- `docs/design-docs/`: durable UI/product design notes.
- `docs/product-specs/`: product behavior specs.
- `docs/exec-plans/`: active/completed plan scaffolding and tech debt tracking.
- `docs/references/`: local references such as Zed UI and Harness Engineering notes.

There is no active `src/import/` module. cURL/Postman/Insomnia import work is deferred until a future implementation.

## Request Lifecycle
Users create requests and folders inside the local workspace. The app renders the active request, binds the URL field to a custom GPUI text input, and updates request state through explicit UI handlers. Sending a request resolves environment variables, applies params/headers/auth/body, records real response status, headers, cookies, body, duration, and size, then inserts successful responses into request history.

The HTTP layer uses a custom hyper+tokio-rustls stack instead of reqwest to instrument per-phase timing. Each request measures DNS lookup, TCP connect, TLS handshake (for HTTPS), time to first byte (TTFB), and transfer time separately. The response meta area (status/time/size) is clickable when timing data is present, opening a popover with the per-phase breakdown and proportional colored bars.

Request Params, Headers, Auth, and Body are view panels over the active request state. They render as flat full-width sections with internal row padding rather than nested cards. Params and Header panels auto-ensure at least one row exists, display an enabled/disabled checkbox and a remove button per row (disabled when only one row remains), and place the add button below the list. Horizontal scrolling in the code editor works via GPUI's native `overflow_scroll` system enabled by the `min_w_full()` wrapper on `CodeInput`, which allows content to grow beyond the viewport.

Response tabs (Body/Headers/Cookies) are always visible — the tab strip renders unconditionally, even without a response. Status/time/size meta appear only when a response is present. Response Body supports Pretty and Raw modes through the shared read-only `CodeInput`, which uses a fast `LineIndex` + viewport-only shaping to handle very large bodies (tens of MB+) smoothly. Bodies above ~10 MiB show a warning banner (and default to Raw for performance). A shared `src/ui/` vertical scrollbar and a matching horizontal scrollbar overlay the response body (both driven by the same 2D `ScrollHandle`). The `CodeInput` wrapper uses `min_w_full` (not `w_full`) so that wide content can grow past the viewport, letting the parent `overflow_scroll` container detect real horizontal overflow and translate child bounds via `with_element_offset`; `CodeElement` paints text at the (already scroll-shifted) `bounds.left()` and re-anchors the gutter to `content_mask.bounds.left()` for a sticky line-number column. The horizontal scrollbar's `max_scroll()` calculation uses only `scroll_handle.max_offset().width` (which comes from GPUI's `clamp_scroll_position()` and accurately reports the actual scrollable range), NOT `forced_content_width` (which represents total content width and would incorrectly add false scrollable space). Response Headers and Cookies render as flat rows with wrapping values and per-value copy controls. The "Save to file" action is available for large responses and all responses, while the hard 2 MiB cap has been removed in favor of soft thresholds. Preview/browser embedding is intentionally not part of the current response model. For concrete public endpoints that reliably produce multi-MB and many-line bodies suitable for exercising these paths (and the 5 MiB pretty-print guard), see `docs/references/large-response-test-endpoints.md`.

The horizontal scrollbar's `max_scroll()` calculation uses only `scroll_handle.max_offset().width` (which comes from GPUI's `clamp_scroll_position()` and accurately reports the actual scrollable range), NOT `forced_content_width` (which represents total content width and would incorrectly add false scrollable space). The scrollbar thumb width is also limited by `max_scroll` to prevent it from extending beyond the actual scrollable range.

All buttons and inputs use a unified 24px height. Filled button hover darkens the accent color rather than switching to grey. The collection tab bar is 28px; individual tabs and the tab shell use 24px. Request and response tab strip padding uses `base04` (4px) for compact spacing.

## Collections and Tabs
The collection is the persistent request tree and may contain folders. Open tabs are app-local view state keyed by request id. Closing a tab must only remove that id from the open tab list; deleting a request or folder is the only action that removes collection data from the saved workspace.

The reusable domain layer stores ids as strings for persistence. The GPUI app layer converts that into local numeric request/folder ids to simplify hit testing, tabs, drag/drop, selection, rename/delete dialogs, and expanded-folder state. Keep this split explicit when moving behavior between `src/domain/` and `src/app/`.

## UI and Settings Flow
The window starts from the system theme unless settings choose a fixed Zed Dark or Zed Light mode. `AppSettings` stores theme preference, `ui_font_size`, and `buffer_font_size`; UI text and editor/body text consume those sizes separately through the Settings dialog.

The bottom bar owns compact global controls for collection visibility, settings, and environment visibility. The collection dock width and response panel height are local app view state, adjusted through thin drag handles. The Settings dialog auto-saves changes.

## Boundaries
Keep reusable data and persistence behavior in `src/domain/`, real network execution in `src/http/`, GPUI interaction in `src/app/`, user settings in `src/settings.rs`, startup in `src/main.rs`, and visual/input primitives in `src/ui/`. Do not import or copy Zed UI code; reproduce only the small local behavior needed by this app.
