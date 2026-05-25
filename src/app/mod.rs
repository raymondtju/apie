use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    time::Instant,
};

pub(crate) use crate::settings::{
    AppSettings, ThemePreference, default_app_settings_path, load_app_settings, save_app_settings,
};
pub(crate) use crate::ui::{
    self, AppTheme, AppTypography, ButtonStyle, CodeInput, IconName, Spacing, SyntaxColors,
    TabPosition, TextInput, ThemeMode, Typography,
};
pub(crate) use apie as domain;
pub(crate) use gpui::{
    AnyElement, App, ClipboardItem, Context, Corner, CursorStyle, Div, Entity, FocusHandle,
    Focusable, InteractiveElement, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, ResizeEdge, ScrollHandle, SharedString,
    StatefulInteractiveElement, Window, WindowControlArea, actions, anchored, deferred, div,
    prelude::*, px, rgb,
};

pub(crate) mod actions;
pub(crate) mod render;
pub(crate) mod resolve;
pub(crate) mod state;
pub(crate) mod types;
pub(crate) use resolve::*;
pub(crate) use types::*;

/// Response bodies above this size trigger a warning banner in the UI.
/// We still attempt to render them (Raw by default), but warn the user
/// that Pretty mode may be slow or memory-intensive.
pub(crate) const LARGE_RESPONSE_WARNING_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

/// Above this size we skip automatic JSON pretty-printing to protect
/// UI responsiveness and memory usage. The user can still explicitly
/// request Pretty mode for the current response.
pub(crate) const PRETTY_PRINT_GUARD_BYTES: usize = 5 * 1024 * 1024; // 5 MiB

// Historical note: We previously had a hard 2 MiB cap
// (`MAX_INLINE_RESPONSE_BODY_BYTES`) that completely bypassed CodeInput.
// This has been removed in favor of the soft thresholds above.

/// Cache for the most recently formatted response body. Keyed by the
/// pointer identity of the underlying `SharedString` storage so that
/// reuse across renders is O(1) and never re-runs the JSON parser.
pub(crate) struct FormattedBodyCache {
    request_id: usize,
    mode: BodyViewMode,
    body_ptr: *const u8,
    formatted: SharedString,
}

impl FormattedBodyCache {
    fn matches(&self, request_id: usize, mode: BodyViewMode, body: &SharedString) -> bool {
        self.request_id == request_id
            && self.mode == mode
            && std::ptr::eq(self.body_ptr, body.as_ref().as_ptr())
    }
}

actions!(
    api_client,
    [
        SendFocusedRequest,
        OpenSettings,
        SubmitDialog,
        CancelDialog,
        DeleteCollectionSelection,
        RenameCollectionSelection,
        NewTab,
        CloseTab
    ]
);

pub(crate) fn bind_app_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", SendFocusedRequest, Some("TextInput")),
        KeyBinding::new("enter", SendFocusedRequest, Some("CodeInput")),
        KeyBinding::new("enter", SubmitDialog, Some("ApiClient")),
        KeyBinding::new("escape", CancelDialog, None),
        KeyBinding::new("delete", DeleteCollectionSelection, Some("ApiClient")),
        KeyBinding::new("f2", RenameCollectionSelection, Some("ApiClient")),
        KeyBinding::new("ctrl-,", OpenSettings, None),
        KeyBinding::new("ctrl-t", NewTab, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Panel {
    Params,
    Path,
    Headers,
    Auth,
    Body,
}

impl Panel {
    fn label(self) -> &'static str {
        match self {
            Self::Params => "Params",
            Self::Path => "Path",
            Self::Headers => "Headers",
            Self::Auth => "Auth",
            Self::Body => "Body",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RequestContextMenu {
    request_id: usize,
    position: Point<Pixels>,
}

#[derive(Clone, Copy)]
pub(crate) struct FolderContextMenu {
    folder_id: usize,
    position: Point<Pixels>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaneResizeTarget {
    Collection,
    Response,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneResize {
    target: PaneResizeTarget,
    origin: Point<Pixels>,
    initial_size: Pixels,
}

pub(crate) struct RequestRenameDialog {
    request_id: usize,
    input: Entity<TextInput>,
}

pub(crate) struct FolderRenameDialog {
    folder_id: usize,
    input: Entity<TextInput>,
}

#[derive(Clone)]
pub(crate) struct RequestDeleteDialog {
    request_id: usize,
    request_name: SharedString,
}

#[derive(Clone)]
pub(crate) struct FolderDeleteDialog {
    folder_id: usize,
    folder_name: SharedString,
}

pub(crate) struct CollectionRenameDialog {
    pub(crate) collection_id: usize,
    pub(crate) input: Entity<TextInput>,
}

#[derive(Clone)]
pub(crate) struct CollectionDeleteDialog {
    pub(crate) collection_id: usize,
    pub(crate) collection_name: SharedString,
}

#[derive(Clone, Copy)]
pub(crate) struct CollectionContextMenu {
    pub(crate) collection_id: usize,
    pub(crate) position: Point<Pixels>,
}

pub(crate) struct WorkspaceCreateDialog {
    pub(crate) input: Entity<TextInput>,
}

#[derive(Clone)]
pub(crate) struct DraggedRequestTab {
    request_id: usize,
    source_index: usize,
    label: SharedString,
    selected: bool,
    theme: AppTheme,
    typography: Typography,
}

impl Render for DraggedRequestTab {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        ui::tab_shell(
            "dragged-request-tab-preview",
            self.selected,
            TabPosition::Only,
            self.theme,
        )
        .shadow_lg()
        .child(ui::tab_label(
            "dragged-request-tab-preview-label",
            self.label.clone(),
        ))
        .text_ui_sm(self.typography)
    }
}

#[derive(Clone)]
pub(crate) struct DraggedCollectionItem {
    item_id: usize,
    label: SharedString,
    detail: SharedString,
    is_folder: bool,
    theme: AppTheme,
    typography: Typography,
}

impl Render for DraggedCollectionItem {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let spacing = Spacing::app();
        ui::list_item("dragged-collection-item-preview", false, self.theme)
            .shadow_lg()
            .bg(self.theme.panel_overlay_background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(spacing.cluster_gap())
                    .child(
                        div()
                            .w(px(42.0))
                            .text_ui_xs(self.typography)
                            .text_color(self.theme.text_muted)
                            .child(if self.is_folder { "DIR" } else { "REQ" }),
                    )
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(self.theme.text)
                            .truncate()
                            .child(self.label.clone()),
                    ),
            )
            .child(
                div()
                    .text_ui_xs(self.typography)
                    .text_color(self.theme.text_muted)
                    .truncate()
                    .child(self.detail.clone()),
            )
    }
}

#[derive(Clone)]
pub(crate) struct DraggedCollection {
    pub(crate) collection_id: usize,
    pub(crate) label: SharedString,
    pub(crate) theme: AppTheme,
    pub(crate) typography: Typography,
}

impl Render for DraggedCollection {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let spacing = Spacing::app();
        ui::list_item("dragged-collection-preview", false, self.theme)
            .shadow_lg()
            .bg(self.theme.panel_overlay_background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(spacing.cluster_gap())
                    .child(
                        div()
                            .w(px(42.0))
                            .text_ui_xs(self.typography)
                            .text_color(self.theme.text_muted)
                            .child("COLL"),
                    )
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(self.theme.text)
                            .truncate()
                            .child(self.label.clone()),
                    ),
            )
    }
}

pub(crate) struct ApiClientApp {
    workspace: Workspace,
    focus_handle: FocusHandle,
    url_input: Entity<TextInput>,
    field_inputs: BTreeMap<String, Entity<TextInput>>,
    active_request_id: Option<usize>,
    selected_collection_item: Option<CollectionSelection>,
    open_tabs: Vec<usize>,
    expanded_folders: HashSet<usize>,
    expanded_collections: HashSet<usize>,
    active_panel: Panel,
    active_response_panel: ResponsePanel,
    body_view_mode: BodyViewMode,
    theme_mode: ThemeMode,
    settings: AppSettings,
    draft_settings: AppSettings,
    settings_path: PathBuf,
    workspace_path: PathBuf,
    left_dock_open: bool,
    right_dock_open: bool,
    collection_width: Pixels,
    response_height: Pixels,
    active_pane_resize: Option<PaneResize>,
    settings_dialog_open: bool,
    method_menu_open: bool,
    body_view_menu_open: bool,
    auth_menu_open: bool,
    auth_location_menu_open: bool,
    response_meta_popover: bool,
    request_context_menu: Option<RequestContextMenu>,
    folder_context_menu: Option<FolderContextMenu>,
    collection_context_menu: Option<CollectionContextMenu>,
    rename_request_dialog: Option<RequestRenameDialog>,
    rename_folder_dialog: Option<FolderRenameDialog>,
    rename_collection_dialog: Option<CollectionRenameDialog>,
    delete_request_dialog: Option<RequestDeleteDialog>,
    delete_folder_dialog: Option<FolderDeleteDialog>,
    delete_collection_dialog: Option<CollectionDeleteDialog>,
    create_workspace_dialog: Option<WorkspaceCreateDialog>,
    workspace_menu_open: bool,
    workspaces_list: Vec<(String, PathBuf)>,
    status_line: SharedString,
    next_item_id: usize,
    suppress_tab_click_selection: bool,
    suppress_collection_click: bool,
    body_inputs: BTreeMap<usize, Entity<CodeInput>>,
    response_body_inputs: BTreeMap<(usize, BodyViewMode), Entity<CodeInput>>,
    response_scroll_handle: ScrollHandle,
    response_scrollbar: Entity<ui::VerticalScrollbar>,
    response_horizontal_scrollbar: Entity<ui::HorizontalScrollbar>,
    formatted_body_cache: Option<FormattedBodyCache>,
    in_flight_requests: BTreeMap<usize, InFlightRequest>,
    request_started_at: Option<Instant>,
    _request_timer: Option<gpui::Task<()>>,
}

pub(crate) struct InFlightRequest {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _task: gpui::Task<()>,
}

impl ApiClientApp {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self::new_with_settings_path(cx, default_app_settings_path())
    }

    pub(crate) fn new_with_settings_path(cx: &mut Context<Self>, settings_path: PathBuf) -> Self {
        let settings = load_app_settings(&settings_path).unwrap_or_default();
        let workspace_path = settings
            .active_workspace
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                settings_path
                    .parent()
                    .map(|parent| parent.join("workspaces/local.json"))
                    .unwrap_or_else(|| PathBuf::from("workspaces/local.json"))
            });
        let mut workspace = domain::load_workspace(&workspace_path)
            .map(|workspace| {
                Workspace::from_domain(
                    workspace,
                    workspace_path.to_string_lossy().to_string().into(),
                )
            })
            .unwrap_or_else(|_| Workspace::sample());
        let next_item_id = workspace.normalize_request_ids();
        let first_request_id = workspace.first_request_id();
        let open_tabs = first_request_id.into_iter().collect::<Vec<_>>();
        let expanded_folders = workspace.expanded_folder_ids();
        let expanded_collections = workspace.expanded_collection_ids();
        let initial_url = first_request_id
            .and_then(|id| {
                workspace
                    .request_by_id(id)
                    .map(|request| request.url.clone())
            })
            .unwrap_or_else(|| "".into());
        let url_input =
            cx.new(|cx| TextInput::new(cx, initial_url, "Paste URL or use {{base_url}}/path"));
        let response_scroll_handle = ScrollHandle::new();
        let response_scrollbar =
            cx.new(|_| ui::VerticalScrollbar::new(response_scroll_handle.clone()));
        let response_horizontal_scrollbar =
            cx.new(|_| ui::HorizontalScrollbar::new(response_scroll_handle.clone()));
        let settings = load_app_settings(&settings_path).unwrap_or_default();
        let theme_mode = Self::theme_mode_from_settings(settings.clone(), cx);
        let mut app = Self {
            workspace,
            focus_handle: cx.focus_handle(),
            url_input,
            field_inputs: BTreeMap::new(),
            active_request_id: first_request_id,
            selected_collection_item: first_request_id.map(|id| CollectionSelection::Request {
                collection_id: 1,
                item_id: id,
            }),
            open_tabs,
            active_panel: Panel::Body,
            active_response_panel: ResponsePanel::Body,
            body_view_mode: BodyViewMode::Pretty,
            theme_mode,
            settings: settings.clone(),
            draft_settings: settings,
            settings_path,
            workspace_path,
            left_dock_open: true,
            right_dock_open: true,
            collection_width: px(288.0),
            response_height: px(260.0),
            active_pane_resize: None,
            settings_dialog_open: false,
            method_menu_open: false,
            body_view_menu_open: false,
            auth_menu_open: false,
            auth_location_menu_open: false,
            response_meta_popover: false,
            request_context_menu: None,
            folder_context_menu: None,
            collection_context_menu: None,
            rename_request_dialog: None,
            rename_folder_dialog: None,
            rename_collection_dialog: None,
            delete_request_dialog: None,
            delete_folder_dialog: None,
            delete_collection_dialog: None,
            create_workspace_dialog: None,
            workspace_menu_open: false,
            workspaces_list: Vec::new(),
            status_line: "Welcome to the API Client".into(),
            next_item_id: next_item_id,
            suppress_tab_click_selection: false,
            suppress_collection_click: false,
            body_inputs: BTreeMap::new(),
            response_body_inputs: BTreeMap::new(),
            response_scrollbar,
            response_horizontal_scrollbar,
            response_scroll_handle,
            formatted_body_cache: None,
            in_flight_requests: BTreeMap::new(),
            _request_timer: None,
            expanded_folders,
            expanded_collections,
            request_started_at: None,
        };
        app.refresh_workspaces_list();
        app
    }
}

#[cfg(test)]
mod tests;
