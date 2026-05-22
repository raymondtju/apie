use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use crate::settings::{
    AppSettings, ThemePreference, default_app_settings_path, load_app_settings, save_app_settings,
};
use crate::ui::{
    self, AppTheme, AppTypography, ButtonStyle, CodeInput, IconName, Spacing, SyntaxColors,
    TabPosition, TextInput, ThemeMode, Typography,
};
use apie as domain;
use gpui::{
    AnyElement, App, ClipboardItem, Context, Corner, CursorStyle, Div, Entity, FocusHandle,
    Focusable, InteractiveElement, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, ResizeEdge, ScrollHandle, SharedString,
    StatefulInteractiveElement, Window, WindowControlArea, actions, anchored, deferred, div, point,
    prelude::*, px, rgb,
};

/// Above this byte count, the response panel skips rendering the body
/// inline and shows a "Body too large to render inline" placeholder
/// with a Save to file action. Picked to keep the UI thread under ~16ms
/// even for pathologically minified payloads. Mirrors Zed's
/// `MAX_LINE_LEN_FOR_INLINE_RENDER` style guard from `crates/editor/`.
const MAX_INLINE_RESPONSE_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Cache for the most recently formatted response body. Keyed by the
/// pointer identity of the underlying `SharedString` storage so that
/// reuse across renders is O(1) and never re-runs the JSON parser.
struct FormattedBodyCache {
    request_id: usize,
    mode: BodyViewMode,
    body_ptr: *const u8,
    body_len: usize,
    formatted: SharedString,
}

impl FormattedBodyCache {
    fn matches(&self, request_id: usize, mode: BodyViewMode, body: &SharedString) -> bool {
        self.request_id == request_id
            && self.mode == mode
            && self.body_len == body.len()
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
        RenameCollectionSelection
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
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl Method {
    const ALL: [Self; 6] = [
        Self::Get,
        Self::Post,
        Self::Put,
        Self::Patch,
        Self::Delete,
        Self::Options,
    ];

    fn all() -> &'static [Self] {
        &Self::ALL
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        }
    }

    fn color(self) -> gpui::Hsla {
        match self {
            Self::Get => rgb(0x15803d).into(),
            Self::Post => rgb(0x2563eb).into(),
            Self::Put => rgb(0x7c3aed).into(),
            Self::Patch => rgb(0xc2410c).into(),
            Self::Delete => rgb(0xb91c1c).into(),
            Self::Options => rgb(0x0f766e).into(),
        }
    }

    fn uses_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }

    fn to_domain(self) -> domain::Method {
        match self {
            Self::Get => domain::Method::Get,
            Self::Post => domain::Method::Post,
            Self::Put => domain::Method::Put,
            Self::Patch => domain::Method::Patch,
            Self::Delete => domain::Method::Delete,
            Self::Options => domain::Method::Options,
        }
    }

    fn from_domain(method: domain::Method) -> Self {
        match method {
            domain::Method::Get => Self::Get,
            domain::Method::Post => Self::Post,
            domain::Method::Put => Self::Put,
            domain::Method::Patch => Self::Patch,
            domain::Method::Delete => Self::Delete,
            domain::Method::Options => Self::Options,
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
enum Auth {
    None,
    Basic {
        username: SharedString,
        password: SharedString,
    },
    Bearer {
        label: SharedString,
        secret_ref: SharedString,
    },
    ApiKey {
        name: SharedString,
        secret_ref: SharedString,
        location: AuthLocation,
    },
}

impl Auth {
    fn summary(&self) -> SharedString {
        match self {
            Self::None => "No auth".into(),
            Self::Basic { username, .. } => format!("Basic auth: {username}").into(),
            Self::Bearer { label, secret_ref } => {
                format!("Bearer token: {label} ({secret_ref})").into()
            }
            Self::ApiKey {
                name,
                secret_ref,
                location,
            } => format!("API key: {name} ({secret_ref}) in {}", location.label()).into(),
        }
    }

    fn from_domain(auth: domain::Auth) -> Self {
        match auth {
            domain::Auth::None => Self::None,
            domain::Auth::Basic {
                username_ref,
                password_ref,
            } => Self::Basic {
                username: username_ref.into(),
                password: password_ref.into(),
            },
            domain::Auth::Bearer { token_ref } => Self::Bearer {
                label: "token".into(),
                secret_ref: token_ref.into(),
            },
            domain::Auth::ApiKey {
                name,
                value_ref,
                location,
            } => Self::ApiKey {
                name: name.into(),
                secret_ref: value_ref.into(),
                location: match location {
                    domain::ApiKeyLocation::Header => AuthLocation::Header,
                    domain::ApiKeyLocation::Query => AuthLocation::Query,
                    domain::ApiKeyLocation::Cookie => AuthLocation::Cookie,
                },
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthLocation {
    Header,
    Query,
    Cookie,
}

impl AuthLocation {
    fn label(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Query => "query",
            Self::Cookie => "cookie",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Header => Self::Query,
            Self::Query => Self::Cookie,
            Self::Cookie => Self::Header,
        }
    }
}

#[derive(Clone)]
struct Header {
    name: SharedString,
    value: SharedString,
    enabled: bool,
}

impl Header {
    pub(crate) fn new(name: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
        }
    }

    fn to_domain(&self) -> domain::Header {
        domain::Header {
            name: self.name.to_string(),
            value: self.value.to_string(),
            enabled: self.enabled,
        }
    }

    fn is_blank(&self) -> bool {
        self.name.trim().is_empty() && self.value.trim().is_empty()
    }

    fn from_domain(header: domain::Header) -> Self {
        Self {
            name: header.name.into(),
            value: header.value.into(),
            enabled: header.enabled,
        }
    }
}

#[derive(Clone)]
struct Request {
    id: usize,
    name: SharedString,
    method: Method,
    url: SharedString,
    query: Vec<Header>,
    proxy_url: Option<SharedString>,
    auth: Auth,
    headers: Vec<Header>,
    content_type: SharedString,
    body: SharedString,
    response: Option<ResponseRecord>,
    history: Vec<ResponseRecord>,
    /// When set, the latest response body and history are persisted to
    /// disk. By default response bodies are session-only to keep the
    /// workspace JSON small and writes off the UI thread fast.
    response_pinned: bool,
}

#[derive(Clone)]
enum CollectionItem {
    Folder {
        id: usize,
        name: SharedString,
        items: Vec<CollectionItem>,
    },
    Request(Request),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CollectionSelection {
    Request(usize),
    Folder(usize),
}

impl CollectionItem {
    fn id(&self) -> usize {
        match self {
            Self::Folder { id, .. } => *id,
            Self::Request(request) => request.id,
        }
    }
}

#[derive(Clone)]
struct ResponseRecord {
    status: u16,
    status_text: SharedString,
    duration_ms: u64,
    size_bytes: usize,
    headers: Vec<ResponseHeader>,
    cookies: Vec<ResponseHeader>,
    body: SharedString,
}

#[derive(Clone)]
struct ResponseHeader {
    name: SharedString,
    value: SharedString,
}

impl ResponseHeader {
    fn from_domain(header: domain::Header) -> Self {
        Self {
            name: header.name.into(),
            value: header.value.into(),
        }
    }

    fn to_domain(&self) -> domain::Header {
        domain::Header::new(self.name.to_string(), self.value.to_string())
    }
}

impl ResponseRecord {
    fn from_domain(response: domain::ResponseRecord) -> Self {
        Self {
            status: response.status,
            status_text: response.status_text.into(),
            duration_ms: response.duration_ms,
            size_bytes: response.size_bytes,
            headers: response
                .headers
                .into_iter()
                .map(ResponseHeader::from_domain)
                .collect(),
            cookies: response
                .cookies
                .into_iter()
                .map(ResponseHeader::from_domain)
                .collect(),
            body: response.body.into(),
        }
    }

    fn to_domain(&self) -> domain::ResponseRecord {
        domain::ResponseRecord {
            status: self.status,
            status_text: self.status_text.to_string(),
            duration_ms: self.duration_ms,
            size_bytes: self.size_bytes,
            headers: self.headers.iter().map(ResponseHeader::to_domain).collect(),
            cookies: self.cookies.iter().map(ResponseHeader::to_domain).collect(),
            body: self.body.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResponsePanel {
    Body,
    Headers,
    Cookies,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PairRowKind {
    Param,
    Header,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BodyViewMode {
    Pretty,
    Raw,
}

impl BodyViewMode {
    const ALL: [Self; 2] = [Self::Pretty, Self::Raw];

    fn all() -> &'static [Self] {
        &Self::ALL
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pretty => "Pretty",
            Self::Raw => "Raw",
        }
    }
}

struct Environment {
    name: SharedString,
    variables: Vec<Header>,
}

struct Workspace {
    name: SharedString,
    storage_hint: SharedString,
    active_environment: usize,
    environments: Vec<Environment>,
    items: Vec<CollectionItem>,
    expanded_folders: HashSet<usize>,
}

impl Workspace {
    fn sample() -> Self {
        Self {
            name: "Local API Workspace".into(),
            storage_hint: "~/.local/share/gpui-api-client/workspaces/local.json".into(),
            active_environment: 0,
            environments: vec![
                Environment {
                    name: "Development".into(),
                    variables: vec![
                        Header::new("base_url", "https://api.scalar.com"),
                        Header::new("token", "secret://scalar/dev-token"),
                    ],
                },
                Environment {
                    name: "Production".into(),
                    variables: vec![
                        Header::new("base_url", "https://api.example.com"),
                        Header::new("token", "secret://example/prod-token"),
                    ],
                },
            ],
            items: vec![],
            expanded_folders: HashSet::new(),
        }
    }

    fn from_domain_item(item: domain::CollectionItem) -> CollectionItem {
        match item {
            domain::CollectionItem::Request(request) => {
                let id = request.id.parse().ok().filter(|id| *id > 0).unwrap_or(0);
                CollectionItem::Request(Request::from_domain(id, request))
            }
            domain::CollectionItem::Folder { id, name, items } => {
                let id = id.parse().ok().filter(|id| *id > 0).unwrap_or(0);
                CollectionItem::Folder {
                    id,
                    name: name.into(),
                    items: items.into_iter().map(Self::from_domain_item).collect(),
                }
            }
        }
    }

    fn to_domain_item(item: &CollectionItem) -> domain::CollectionItem {
        match item {
            CollectionItem::Request(request) => {
                domain::CollectionItem::Request(request.to_domain_with_history())
            }
            CollectionItem::Folder { id, name, items } => domain::CollectionItem::Folder {
                id: id.to_string(),
                name: name.to_string(),
                items: items.iter().map(Self::to_domain_item).collect(),
            },
        }
    }

    fn from_domain(workspace: domain::Workspace, storage_hint: SharedString) -> Self {
        let expanded_folders = workspace
            .expanded_folders
            .iter()
            .filter_map(|id| id.parse::<usize>().ok())
            .filter(|id| *id > 0)
            .collect::<HashSet<_>>();
        let environments = if workspace.environments.is_empty() {
            Self::sample().environments
        } else {
            workspace
                .environments
                .into_iter()
                .map(|environment| Environment {
                    name: environment.name.into(),
                    variables: environment
                        .variables
                        .into_iter()
                        .map(Header::from_domain)
                        .collect(),
                })
                .collect()
        };
        let active_environment = environments
            .iter()
            .position(|environment| environment.name.as_ref() == workspace.active_environment)
            .unwrap_or(0);
        Self {
            name: workspace.name.into(),
            storage_hint,
            active_environment,
            environments,
            items: workspace
                .items
                .into_iter()
                .map(Self::from_domain_item)
                .collect(),
            expanded_folders,
        }
    }

    fn normalize_request_ids(&mut self) -> usize {
        let mut used_ids = HashSet::new();
        let mut next_id = 1usize;
        normalize_collection_item_ids(&mut self.items, &mut used_ids, &mut next_id);
        self.expanded_folders
            .retain(|folder_id| find_folder(&self.items, *folder_id).is_some());
        next_id
    }

    fn expanded_folder_ids(&self) -> HashSet<usize> {
        self.expanded_folders.clone()
    }

    fn request_count(&self) -> usize {
        self.request_ids().len()
    }

    fn request_ids(&self) -> Vec<usize> {
        let mut ids = Vec::new();
        collect_request_ids(&self.items, &mut ids);
        ids
    }

    fn first_request_id(&self) -> Option<usize> {
        self.request_ids().first().copied()
    }

    fn request_by_id(&self, request_id: usize) -> Option<&Request> {
        find_request(&self.items, request_id)
    }

    fn request_mut_by_id(&mut self, request_id: usize) -> Option<&mut Request> {
        find_request_mut(&mut self.items, request_id)
    }

    fn request_by_index(&self, index: usize) -> Option<&Request> {
        nth_request(&self.items, index, &mut 0)
    }

    fn insert_root_request(&mut self, request: Request) {
        self.items.push(CollectionItem::Request(request));
    }

    fn insert_request_in_folder(&mut self, folder_id: usize, request: Request) -> bool {
        let Some(items) = find_folder_items_mut(&mut self.items, folder_id) else {
            return false;
        };
        items.push(CollectionItem::Request(request));
        true
    }

    fn insert_root_folder(&mut self, id: usize, name: impl Into<SharedString>) {
        self.items.push(CollectionItem::Folder {
            id,
            name: name.into(),
            items: Vec::new(),
        });
    }

    fn insert_folder_in_folder(
        &mut self,
        parent_id: usize,
        id: usize,
        name: impl Into<SharedString>,
    ) -> bool {
        let Some(items) = find_folder_items_mut(&mut self.items, parent_id) else {
            return false;
        };
        items.push(CollectionItem::Folder {
            id,
            name: name.into(),
            items: Vec::new(),
        });
        true
    }

    fn folder_name(&self, folder_id: usize) -> Option<SharedString> {
        find_folder(&self.items, folder_id).map(|(_, name)| name.clone())
    }

    fn rename_folder(&mut self, folder_id: usize, name: impl Into<SharedString>) -> bool {
        let Some((_, folder_name)) = find_folder_mut(&mut self.items, folder_id) else {
            return false;
        };
        *folder_name = name.into();
        true
    }

    fn remove_request(&mut self, request_id: usize) -> Option<Request> {
        remove_request_from_items(&mut self.items, request_id)
    }

    fn remove_folder(&mut self, folder_id: usize) -> Option<(SharedString, Vec<usize>)> {
        remove_folder_from_items(&mut self.items, folder_id)
    }

    fn move_item_before(&mut self, item_id: usize, target_id: usize) -> bool {
        if item_id == target_id || self.item_contains_id(item_id, target_id) {
            return false;
        }
        let Some(item) = remove_item_from_items(&mut self.items, item_id) else {
            return false;
        };
        insert_item_before(&mut self.items, target_id, item)
    }

    fn move_item_into_folder(&mut self, item_id: usize, folder_id: usize) -> bool {
        if item_id == folder_id || self.item_contains_id(item_id, folder_id) {
            return false;
        }
        let Some(item) = remove_item_from_items(&mut self.items, item_id) else {
            return false;
        };
        let Some(items) = find_folder_items_mut(&mut self.items, folder_id) else {
            self.items.push(item);
            return false;
        };
        items.push(item);
        true
    }

    fn move_item_to_root_end(&mut self, item_id: usize) -> bool {
        let Some(item) = remove_item_from_items(&mut self.items, item_id) else {
            return false;
        };
        self.items.push(item);
        true
    }

    fn item_contains_id(&self, item_id: usize, target_id: usize) -> bool {
        find_item(&self.items, item_id).is_some_and(|item| item_contains_id(item, target_id))
    }

    fn to_domain(&self) -> domain::Workspace {
        let mut expanded_folders = self
            .expanded_folders
            .iter()
            .copied()
            .collect::<Vec<usize>>();
        expanded_folders.sort_unstable();
        domain::Workspace {
            id: "local".to_string(),
            name: self.name.to_string(),
            active_environment: self.environments[self.active_environment].name.to_string(),
            environments: self
                .environments
                .iter()
                .map(|environment| domain::Environment {
                    name: environment.name.to_string(),
                    variables: environment
                        .variables
                        .iter()
                        .map(Header::to_domain)
                        .collect(),
                })
                .collect(),
            items: self.items.iter().map(Self::to_domain_item).collect(),
            expanded_folders: expanded_folders
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }
}

fn normalize_collection_item_ids(
    items: &mut [CollectionItem],
    used_ids: &mut HashSet<usize>,
    next_id: &mut usize,
) {
    for item in items {
        let id = match item {
            CollectionItem::Folder { id, .. } => id,
            CollectionItem::Request(request) => &mut request.id,
        };
        if *id == 0 || !used_ids.insert(*id) {
            while used_ids.contains(next_id) {
                *next_id += 1;
            }
            *id = *next_id;
            used_ids.insert(*id);
        }
        if *id >= *next_id {
            *next_id = *id + 1;
        }
        if let CollectionItem::Folder { items, .. } = item {
            normalize_collection_item_ids(items, used_ids, next_id);
        }
    }
}

fn collect_request_ids(items: &[CollectionItem], out: &mut Vec<usize>) {
    for item in items {
        match item {
            CollectionItem::Request(request) => out.push(request.id),
            CollectionItem::Folder { items, .. } => collect_request_ids(items, out),
        }
    }
}

fn find_item(items: &[CollectionItem], item_id: usize) -> Option<&CollectionItem> {
    for item in items {
        if item.id() == item_id {
            return Some(item);
        }
        if let CollectionItem::Folder { items, .. } = item
            && let Some(found) = find_item(items, item_id)
        {
            return Some(found);
        }
    }
    None
}

fn item_contains_id(item: &CollectionItem, target_id: usize) -> bool {
    match item {
        CollectionItem::Request(request) => request.id == target_id,
        CollectionItem::Folder { id, items, .. } => {
            *id == target_id || items.iter().any(|item| item_contains_id(item, target_id))
        }
    }
}

fn find_request(items: &[CollectionItem], request_id: usize) -> Option<&Request> {
    for item in items {
        match item {
            CollectionItem::Request(request) if request.id == request_id => return Some(request),
            CollectionItem::Folder { items, .. } => {
                if let Some(request) = find_request(items, request_id) {
                    return Some(request);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_request_mut(items: &mut [CollectionItem], request_id: usize) -> Option<&mut Request> {
    for item in items {
        match item {
            CollectionItem::Request(request) if request.id == request_id => return Some(request),
            CollectionItem::Folder { items, .. } => {
                if let Some(request) = find_request_mut(items, request_id) {
                    return Some(request);
                }
            }
            _ => {}
        }
    }
    None
}

fn nth_request<'a>(
    items: &'a [CollectionItem],
    target_index: usize,
    seen: &mut usize,
) -> Option<&'a Request> {
    for item in items {
        match item {
            CollectionItem::Request(request) => {
                if *seen == target_index {
                    return Some(request);
                }
                *seen += 1;
            }
            CollectionItem::Folder { items, .. } => {
                if let Some(request) = nth_request(items, target_index, seen) {
                    return Some(request);
                }
            }
        }
    }
    None
}

fn find_folder(
    items: &[CollectionItem],
    folder_id: usize,
) -> Option<(&[CollectionItem], &SharedString)> {
    for item in items {
        if let CollectionItem::Folder { id, name, items } = item {
            if *id == folder_id {
                return Some((items, name));
            }
            if let Some(folder) = find_folder(items, folder_id) {
                return Some(folder);
            }
        }
    }
    None
}

fn find_folder_mut(
    items: &mut [CollectionItem],
    folder_id: usize,
) -> Option<(&mut Vec<CollectionItem>, &mut SharedString)> {
    for item in items {
        if let CollectionItem::Folder { id, name, items } = item {
            if *id == folder_id {
                return Some((items, name));
            }
            if let Some(folder) = find_folder_mut(items, folder_id) {
                return Some(folder);
            }
        }
    }
    None
}

fn find_folder_items_mut(
    items: &mut [CollectionItem],
    folder_id: usize,
) -> Option<&mut Vec<CollectionItem>> {
    find_folder_mut(items, folder_id).map(|(items, _)| items)
}

fn remove_request_from_items(
    items: &mut Vec<CollectionItem>,
    request_id: usize,
) -> Option<Request> {
    let mut index = 0;
    while index < items.len() {
        match &mut items[index] {
            CollectionItem::Request(request) if request.id == request_id => {
                if let CollectionItem::Request(request) = items.remove(index) {
                    return Some(request);
                }
            }
            CollectionItem::Folder {
                items: child_items, ..
            } => {
                if let Some(request) = remove_request_from_items(child_items, request_id) {
                    return Some(request);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn remove_item_from_items(
    items: &mut Vec<CollectionItem>,
    item_id: usize,
) -> Option<CollectionItem> {
    let mut index = 0;
    while index < items.len() {
        if items[index].id() == item_id {
            return Some(items.remove(index));
        }
        if let CollectionItem::Folder {
            items: child_items, ..
        } = &mut items[index]
            && let Some(item) = remove_item_from_items(child_items, item_id)
        {
            return Some(item);
        }
        index += 1;
    }
    None
}

fn insert_item_before(
    items: &mut Vec<CollectionItem>,
    target_id: usize,
    item: CollectionItem,
) -> bool {
    let mut item = Some(item);
    let inserted = insert_item_before_inner(items, target_id, &mut item);
    if !inserted && let Some(item) = item {
        items.push(item);
    }
    inserted
}

fn insert_item_before_inner(
    items: &mut Vec<CollectionItem>,
    target_id: usize,
    item: &mut Option<CollectionItem>,
) -> bool {
    let mut index = 0;
    while index < items.len() {
        if items[index].id() == target_id {
            if let Some(item) = item.take() {
                items.insert(index, item);
                return true;
            }
            return false;
        }
        if let CollectionItem::Folder {
            items: child_items, ..
        } = &mut items[index]
            && insert_item_before_inner(child_items, target_id, item)
        {
            return true;
        }
        index += 1;
    }
    false
}

fn remove_folder_from_items(
    items: &mut Vec<CollectionItem>,
    folder_id: usize,
) -> Option<(SharedString, Vec<usize>)> {
    let mut index = 0;
    while index < items.len() {
        match &mut items[index] {
            CollectionItem::Folder { id, .. } if *id == folder_id => {
                if let CollectionItem::Folder { name, items, .. } = items.remove(index) {
                    let mut request_ids = Vec::new();
                    collect_request_ids(&items, &mut request_ids);
                    return Some((name, request_ids));
                }
            }
            CollectionItem::Folder {
                items: child_items, ..
            } => {
                if let Some(folder) = remove_folder_from_items(child_items, folder_id) {
                    return Some(folder);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

impl Request {
    fn from_domain(id: usize, request: domain::Request) -> Self {
        let history = request
            .history
            .into_iter()
            .map(ResponseRecord::from_domain)
            .collect::<Vec<_>>();
        let response = history.first().cloned();
        let content_type = match &request.body {
            domain::Body::Raw { content_type, .. } => content_type.clone(),
            domain::Body::Empty => "application/json".to_string(),
        };
        let body = match request.body {
            domain::Body::Raw { value, .. } => value,
            domain::Body::Empty => String::new(),
        };
        Self {
            id,
            name: request.name.into(),
            method: Method::from_domain(request.method),
            url: request.url.into(),
            query: request.query.into_iter().map(Header::from_domain).collect(),
            proxy_url: request.proxy_url.map(Into::into),
            auth: Auth::from_domain(request.auth),
            headers: request
                .headers
                .into_iter()
                .map(Header::from_domain)
                .collect(),
            content_type: content_type.into(),
            body: body.into(),
            response,
            history,
            response_pinned: false,
        }
    }

    fn to_domain(&self) -> domain::Request {
        let mut request = domain::Request::new(
            self.id.to_string(),
            self.name.to_string(),
            self.method.to_domain(),
            self.url.to_string(),
        );
        request.proxy_url = self.proxy_url.as_ref().map(ToString::to_string);
        request.query = self
            .query
            .iter()
            .filter(|header| !header.is_blank())
            .map(Header::to_domain)
            .collect();
        request.headers = self
            .headers
            .iter()
            .filter(|header| !header.is_blank())
            .map(Header::to_domain)
            .collect();
        request.auth = match &self.auth {
            Auth::None => domain::Auth::None,
            Auth::Basic { username, password } => domain::Auth::Basic {
                username_ref: username.to_string(),
                password_ref: password.to_string(),
            },
            Auth::Bearer { secret_ref, .. } => domain::Auth::Bearer {
                token_ref: secret_ref.to_string(),
            },
            Auth::ApiKey {
                name,
                secret_ref,
                location,
            } => domain::Auth::ApiKey {
                name: name.to_string(),
                value_ref: secret_ref.to_string(),
                location: match location {
                    AuthLocation::Header => domain::ApiKeyLocation::Header,
                    AuthLocation::Query => domain::ApiKeyLocation::Query,
                    AuthLocation::Cookie => domain::ApiKeyLocation::Cookie,
                },
            },
        };
        request.body = if !self.method.uses_body() || self.body.is_empty() {
            domain::Body::Empty
        } else {
            domain::Body::Raw {
                content_type: self.content_type.to_string(),
                value: self.body.to_string(),
            }
        };
        request
    }

    fn to_domain_with_history(&self) -> domain::Request {
        let mut request = self.to_domain();
        if self.response_pinned {
            request.history = self.history.iter().map(ResponseRecord::to_domain).collect();
        } else {
            request.history.clear();
        }
        request
    }

    fn to_resolved_domain(&self, environment: &Environment) -> Result<domain::Request, String> {
        let mut request = self.to_domain();
        request.url = resolve_template(&request.url, environment)?;
        request.query = resolve_headers(&request.query, environment)?;
        request.headers = resolve_headers(&request.headers, environment)?;
        request.body = match request.body {
            domain::Body::Empty => domain::Body::Empty,
            domain::Body::Raw {
                content_type,
                value,
            } => domain::Body::Raw {
                content_type,
                value: resolve_template(&value, environment)?,
            },
        };
        request.auth = resolve_auth(&request.auth, environment)?;
        apply_auth(&mut request);
        Ok(request)
    }
}

fn resolve_headers(
    headers: &[domain::Header],
    environment: &Environment,
) -> Result<Vec<domain::Header>, String> {
    headers
        .iter()
        .map(|header| {
            Ok(domain::Header {
                name: resolve_template(&header.name, environment)?,
                value: resolve_template(&header.value, environment)?,
                enabled: header.enabled,
            })
        })
        .collect()
}

fn resolve_auth(auth: &domain::Auth, environment: &Environment) -> Result<domain::Auth, String> {
    Ok(match auth {
        domain::Auth::None => domain::Auth::None,
        domain::Auth::Basic {
            username_ref,
            password_ref,
        } => domain::Auth::Basic {
            username_ref: resolve_template(username_ref, environment)?,
            password_ref: resolve_template(password_ref, environment)?,
        },
        domain::Auth::Bearer { token_ref } => domain::Auth::Bearer {
            token_ref: resolve_template(token_ref, environment)?,
        },
        domain::Auth::ApiKey {
            name,
            value_ref,
            location,
        } => domain::Auth::ApiKey {
            name: resolve_template(name, environment)?,
            value_ref: resolve_template(value_ref, environment)?,
            location: location.clone(),
        },
    })
}

fn resolve_template(value: &str, environment: &Environment) -> Result<String, String> {
    let mut resolved = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("{{") {
        let (prefix, after_start) = rest.split_at(start);
        resolved.push_str(prefix);
        let after_start = &after_start[2..];
        let Some(end) = after_start.find("}}") else {
            return Err(format!("Unclosed environment variable in `{value}`."));
        };
        let key = after_start[..end].trim();
        let Some(variable) = environment
            .variables
            .iter()
            .find(|item| item.enabled && item.name.as_ref() == key)
        else {
            return Err(format!("Missing environment variable `{key}`."));
        };
        resolved.push_str(variable.value.as_ref());
        rest = &after_start[end + 2..];
    }
    resolved.push_str(rest);
    Ok(resolved)
}

fn apply_auth(request: &mut domain::Request) {
    match &request.auth {
        domain::Auth::None => {}
        domain::Auth::Basic {
            username_ref,
            password_ref,
        } => request.headers.push(domain::Header::new(
            "Authorization",
            format!(
                "Basic {}",
                base64_encode(format!("{username_ref}:{password_ref}").as_bytes())
            ),
        )),
        domain::Auth::Bearer { token_ref } => request.headers.push(domain::Header::new(
            "Authorization",
            format!("Bearer {token_ref}"),
        )),
        domain::Auth::ApiKey {
            name,
            value_ref,
            location,
        } => match location {
            domain::ApiKeyLocation::Header => {
                request.headers.push(domain::Header::new(name, value_ref))
            }
            domain::ApiKeyLocation::Query => {
                request.query.push(domain::Header::new(name, value_ref))
            }
            domain::ApiKeyLocation::Cookie => {
                let cookie = format!("{name}={value_ref}");
                if let Some(existing) = request
                    .headers
                    .iter_mut()
                    .find(|header| header.name.eq_ignore_ascii_case("cookie"))
                {
                    existing.value = format!("{}; {cookie}", existing.value);
                } else {
                    request.headers.push(domain::Header::new("Cookie", cookie));
                }
            }
        },
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn response_body_for_mode(body: &str, mode: BodyViewMode) -> SharedString {
    match mode {
        BodyViewMode::Raw => body.to_string().into(),
        BodyViewMode::Pretty => serde_json::from_str::<serde_json::Value>(body)
            .and_then(|value| serde_json::to_string_pretty(&value))
            .unwrap_or_else(|_| body.to_string())
            .into(),
    }
}

fn format_byte_count(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn suggest_response_filename(response: &ResponseRecord) -> String {
    let extension = response
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("content-type"))
        .map(|header| extension_for_content_type(header.value.as_ref()))
        .unwrap_or("txt");
    format!("response-{}.{}", response.status, extension)
}

fn extension_for_content_type(content_type: &str) -> &'static str {
    let lowered = content_type.to_ascii_lowercase();
    let head = lowered.split(';').next().unwrap_or("").trim();
    match head {
        "application/json" | "application/problem+json" => "json",
        "application/xml" | "text/xml" => "xml",
        "text/html" => "html",
        "text/css" => "css",
        "text/javascript" | "application/javascript" => "js",
        "text/csv" => "csv",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "application/octet-stream" => "bin",
        s if s.starts_with("text/") => "txt",
        _ => "txt",
    }
}

fn format_json_body(body: &str) -> Result<String, String> {
    serde_json::from_str::<serde_json::Value>(body)
        .map_err(|error| error.to_string())
        .and_then(|value| serde_json::to_string_pretty(&value).map_err(|error| error.to_string()))
}

fn stable_key_hash(key: &str) -> usize {
    key.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    Params,
    Headers,
    Auth,
    Body,
}

impl Panel {
    fn label(self) -> &'static str {
        match self {
            Self::Params => "Params",
            Self::Headers => "Headers",
            Self::Auth => "Auth",
            Self::Body => "Body",
        }
    }
}

#[derive(Clone, Copy)]
struct RequestContextMenu {
    request_id: usize,
    position: Point<Pixels>,
}

#[derive(Clone, Copy)]
struct FolderContextMenu {
    folder_id: usize,
    position: Point<Pixels>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaneResizeTarget {
    Collection,
    Response,
}

#[derive(Clone, Copy, Debug)]
struct PaneResize {
    target: PaneResizeTarget,
    origin: Point<Pixels>,
    initial_size: Pixels,
}

struct RequestRenameDialog {
    request_id: usize,
    input: Entity<TextInput>,
}

struct FolderRenameDialog {
    folder_id: usize,
    input: Entity<TextInput>,
}

#[derive(Clone)]
struct RequestDeleteDialog {
    request_id: usize,
    request_name: SharedString,
}

#[derive(Clone)]
struct FolderDeleteDialog {
    folder_id: usize,
    folder_name: SharedString,
}

#[derive(Clone)]
struct DraggedRequestTab {
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
struct DraggedCollectionItem {
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

pub(crate) struct ApiClientApp {
    workspace: Workspace,
    focus_handle: FocusHandle,
    url_input: Entity<TextInput>,
    field_inputs: BTreeMap<String, Entity<TextInput>>,
    active_request_id: Option<usize>,
    selected_collection_item: Option<CollectionSelection>,
    open_tabs: Vec<usize>,
    expanded_folders: HashSet<usize>,
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
    method_menu_position: Option<Point<Pixels>>,
    body_view_menu_open: bool,
    body_view_menu_position: Option<Point<Pixels>>,
    request_context_menu: Option<RequestContextMenu>,
    folder_context_menu: Option<FolderContextMenu>,
    rename_request_dialog: Option<RequestRenameDialog>,
    rename_folder_dialog: Option<FolderRenameDialog>,
    delete_request_dialog: Option<RequestDeleteDialog>,
    delete_folder_dialog: Option<FolderDeleteDialog>,
    status_line: SharedString,
    next_item_id: usize,
    suppress_tab_click_selection: bool,
    suppress_collection_click: bool,
    body_inputs: BTreeMap<usize, Entity<CodeInput>>,
    response_body_inputs: BTreeMap<(usize, BodyViewMode), Entity<CodeInput>>,
    response_scroll_handle: ScrollHandle,
    response_scrollbar: Entity<ui::VerticalScrollbar>,
    formatted_body_cache: Option<FormattedBodyCache>,
    in_flight_requests: BTreeMap<usize, InFlightRequest>,
}

struct InFlightRequest {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _task: gpui::Task<()>,
}

impl ApiClientApp {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self::new_with_settings_path(cx, default_app_settings_path())
    }

    pub(crate) fn new_with_settings_path(cx: &mut Context<Self>, settings_path: PathBuf) -> Self {
        let workspace_path = settings_path
            .parent()
            .map(|parent| parent.join("workspaces/local.json"))
            .unwrap_or_else(|| PathBuf::from("workspaces/local.json"));
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
        let settings = load_app_settings(&settings_path).unwrap_or_default();
        let theme_mode = Self::theme_mode_from_settings(settings, cx);
        Self {
            workspace,
            focus_handle: cx.focus_handle(),
            url_input,
            field_inputs: BTreeMap::new(),
            active_request_id: first_request_id,
            selected_collection_item: first_request_id.map(CollectionSelection::Request),
            open_tabs,
            active_panel: Panel::Body,
            active_response_panel: ResponsePanel::Body,
            body_view_mode: BodyViewMode::Pretty,
            theme_mode,
            settings,
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
            method_menu_position: None,
            body_view_menu_open: false,
            body_view_menu_position: None,
            request_context_menu: None,
            folder_context_menu: None,
            rename_request_dialog: None,
            rename_folder_dialog: None,
            delete_request_dialog: None,
            delete_folder_dialog: None,
            status_line: if first_request_id.is_some() {
                "Ready.".into()
            } else {
                "Ready. Create a request to begin.".into()
            },
            next_item_id,
            suppress_tab_click_selection: false,
            suppress_collection_click: false,
            body_inputs: BTreeMap::new(),
            response_body_inputs: BTreeMap::new(),
            response_scroll_handle,
            response_scrollbar,
            expanded_folders,
            formatted_body_cache: None,
            in_flight_requests: BTreeMap::new(),
        }
    }

    fn theme_mode_from_settings(settings: AppSettings, cx: &Context<Self>) -> ThemeMode {
        match settings.theme_preference {
            ThemePreference::System => ThemeMode::from_window_appearance(cx.window_appearance()),
            ThemePreference::ZedDark => ThemeMode::ZedDark,
            ThemePreference::ZedLight => ThemeMode::ZedLight,
        }
    }

    fn active_request(&self) -> Option<&Request> {
        self.active_request_id
            .and_then(|request_id| self.workspace.request_by_id(request_id))
    }

    fn active_request_mut(&mut self) -> Option<&mut Request> {
        let request_id = self.active_request_id?;
        self.workspace.request_mut_by_id(request_id)
    }

    fn sync_url_input_to_active_request(&mut self, cx: &mut Context<Self>) {
        let url = self
            .active_request()
            .map(|request| request.url.clone())
            .unwrap_or_else(|| "".into());
        self.url_input
            .update(cx, |input, cx| input.set_content(url, cx));
    }

    fn field_input(
        &mut self,
        key: impl Into<String>,
        content: impl Into<SharedString>,
        placeholder: &'static str,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let key = key.into();
        let input = self
            .field_inputs
            .entry(key)
            .or_insert_with(|| {
                cx.new(|cx| {
                    TextInput::new_with_selector(cx, content, placeholder, "field-text-input")
                })
            })
            .clone();
        input.update(cx, |input, _| input.set_placeholder(placeholder));
        input
    }

    fn body_input(
        &mut self,
        request_id: usize,
        content: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Entity<CodeInput> {
        let content: SharedString = content.into();
        let input = self
            .body_inputs
            .entry(request_id)
            .or_insert_with(|| {
                cx.new(|cx| CodeInput::new(cx, content.clone(), "Raw JSON, text, or {{variable}}"))
            })
            .clone();
        input.update(cx, |input, _| {
            input.set_placeholder("Raw JSON, text, or {{variable}}");
        });
        input
    }

    fn response_body_input(
        &mut self,
        request_id: usize,
        mode: BodyViewMode,
        content: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Entity<CodeInput> {
        let content: SharedString = content.into();
        let input = self
            .response_body_inputs
            .entry((request_id, mode))
            .or_insert_with(|| {
                cx.new(|cx| {
                    CodeInput::new_read_only(cx, content.clone(), "Response body is empty.")
                })
            })
            .clone();
        input.update(cx, |input, cx| {
            input.set_placeholder("Response body is empty.");
            if !input.content_eq(&content) {
                input.set_content(content.clone(), cx);
            }
        });
        input
    }

    fn syntax_colors_for_theme(theme: AppTheme) -> SyntaxColors {
        SyntaxColors {
            property: theme.accent,
            string: theme.success,
            number: theme.warning,
            keyword: theme.error,
            punctuation: theme.text_muted,
        }
    }

    fn set_active_request_body(&mut self, body: impl Into<SharedString>, cx: &mut Context<Self>) {
        let body: SharedString = body.into();
        if let Some(request) = self.active_request_mut() {
            request.body = body.clone();
        } else {
            return;
        }
        if let Some(request_id) = self.active_request().map(|request| request.id) {
            let input = self.body_input(request_id, body.clone(), cx);
            input.update(cx, |input, cx| input.set_content(body.clone(), cx));
        }
    }

    fn sync_active_request_inputs(&mut self, cx: &mut Context<Self>) {
        let Some(request_id) = self.active_request().map(|request| request.id) else {
            return;
        };
        let value_for = |key: String, inputs: &BTreeMap<String, Entity<TextInput>>| {
            inputs.get(&key).map(|input| input.read(cx).value())
        };
        let mut query_updates = Vec::new();
        let mut header_updates = Vec::new();
        let mut content_type =
            value_for(format!("req:{request_id}:content-type"), &self.field_inputs);
        let mut body = self
            .body_inputs
            .get(&request_id)
            .map(|input| input.read(cx).value());
        let mut auth_updates = BTreeMap::new();
        if let Some(request) = self.active_request() {
            body = body.or_else(|| Some(request.body.to_string()));
            for index in 0..request.query.len() {
                query_updates.push((
                    value_for(
                        format!("req:{request_id}:param:{index}:name"),
                        &self.field_inputs,
                    ),
                    value_for(
                        format!("req:{request_id}:param:{index}:value"),
                        &self.field_inputs,
                    ),
                ));
            }
            for index in 0..request.headers.len() {
                header_updates.push((
                    value_for(
                        format!("req:{request_id}:header:{index}:name"),
                        &self.field_inputs,
                    ),
                    value_for(
                        format!("req:{request_id}:header:{index}:value"),
                        &self.field_inputs,
                    ),
                ));
            }
            for key in ["username", "password", "label", "secret", "name"] {
                if let Some(value) =
                    value_for(format!("req:{request_id}:auth:{key}"), &self.field_inputs)
                {
                    auth_updates.insert(key, value);
                }
            }
        }
        let Some(request) = self.active_request_mut() else {
            return;
        };
        for (index, param) in request.query.iter_mut().enumerate() {
            if let Some(Some(value)) = query_updates.get(index).map(|(name, _)| name.as_ref()) {
                param.name = value.clone().into();
            }
            if let Some(Some(value)) = query_updates.get(index).map(|(_, value)| value.as_ref()) {
                param.value = value.clone().into();
            }
        }
        for (index, header) in request.headers.iter_mut().enumerate() {
            if let Some(Some(value)) = header_updates.get(index).map(|(name, _)| name.as_ref()) {
                header.name = value.clone().into();
            }
            if let Some(Some(value)) = header_updates.get(index).map(|(_, value)| value.as_ref()) {
                header.value = value.clone().into();
            }
        }
        if let Some(value) = content_type.take() {
            request.content_type = value.into();
        }
        if let Some(value) = body.take() {
            request.body = value.into();
        }
        match &mut request.auth {
            Auth::None => {}
            Auth::Basic { username, password } => {
                if let Some(value) = auth_updates.get("username") {
                    *username = value.clone().into();
                }
                if let Some(value) = auth_updates.get("password") {
                    *password = value.clone().into();
                }
            }
            Auth::Bearer { label, secret_ref } => {
                if let Some(value) = auth_updates.get("label") {
                    *label = value.clone().into();
                }
                if let Some(value) = auth_updates.get("secret") {
                    *secret_ref = value.clone().into();
                }
            }
            Auth::ApiKey {
                name, secret_ref, ..
            } => {
                if let Some(value) = auth_updates.get("name") {
                    *name = value.clone().into();
                }
                if let Some(value) = auth_updates.get("secret") {
                    *secret_ref = value.clone().into();
                }
            }
        }
    }

    fn sync_environment_inputs(&mut self, cx: &mut Context<Self>) {
        for (environment_index, environment) in self.workspace.environments.iter_mut().enumerate() {
            for (index, variable) in environment.variables.iter_mut().enumerate() {
                if let Some(input) = self
                    .field_inputs
                    .get(&format!("env:{environment_index}:var:{index}:name"))
                {
                    variable.name = input.read(cx).value().into();
                }
                if let Some(input) = self
                    .field_inputs
                    .get(&format!("env:{environment_index}:var:{index}:value"))
                {
                    variable.value = input.read(cx).value().into();
                }
            }
        }
    }

    fn typography(&self) -> Typography {
        Typography::new(self.settings.ui_font_size, self.settings.buffer_font_size)
    }

    fn persist_settings(&mut self) {
        self.settings = self.settings.clamped();
        match save_app_settings(&self.settings_path, &self.settings) {
            Ok(()) => {
                self.status_line = format!(
                    "Saved settings: UI {:.0}px, buffer {:.0}px.",
                    self.settings.ui_font_size, self.settings.buffer_font_size
                )
                .into();
            }
            Err(error) => {
                self.status_line = format!("Could not save settings: {error}").into();
            }
        }
    }

    fn persist_workspace(&mut self) {
        self.workspace.expanded_folders = self.expanded_folders.clone();
        let workspace_path = self.workspace_path.clone();
        let snapshot = self.workspace.to_domain();
        std::thread::spawn(move || {
            if let Err(error) = domain::save_workspace(&workspace_path, &snapshot) {
                eprintln!("Could not save workspace: {error}");
            }
        });
    }

    fn request_index_by_id(&self, request_id: usize) -> Option<usize> {
        self.open_tabs.iter().position(|id| *id == request_id)
    }

    fn active_request_id(&self) -> Option<usize> {
        self.active_request_id
    }

    fn clear_request_overlays(&mut self) {
        self.request_context_menu = None;
        self.folder_context_menu = None;
        self.rename_request_dialog = None;
        self.rename_folder_dialog = None;
        self.delete_request_dialog = None;
        self.delete_folder_dialog = None;
    }

    fn select_request_by_id(&mut self, request_id: usize, cx: &mut Context<Self>) {
        if self.suppress_collection_click {
            self.suppress_collection_click = false;
            return;
        }
        let Some(request) = self.workspace.request_by_id(request_id) else {
            return;
        };
        if !self.open_tabs.contains(&request_id) {
            self.open_tabs.push(request_id);
        }
        self.active_request_id = Some(request_id);
        self.selected_collection_item = Some(CollectionSelection::Request(request_id));
        self.suppress_tab_click_selection = false;
        self.method_menu_open = false;
        self.request_context_menu = None;
        self.folder_context_menu = None;
        self.url_input
            .update(cx, |input, cx| input.set_content(request.url.clone(), cx));
        self.status_line = format!("Selected {}", request.name).into();
        cx.notify();
    }

    fn select_collection_request_by_id(
        &mut self,
        request_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.select_request_by_id(request_id, cx);
    }

    fn select_collection_folder(
        &mut self,
        folder_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.selected_collection_item = Some(CollectionSelection::Folder(folder_id));
        self.toggle_folder_expanded(folder_id, cx);
    }

    #[allow(dead_code)]
    fn select_request(&mut self, index: usize, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(request_id) = self
            .workspace
            .request_by_index(index)
            .map(|request| request.id)
        {
            self.select_request_by_id(request_id, cx);
        }
    }

    fn set_panel(&mut self, panel: Panel, _: &mut Window, cx: &mut Context<Self>) {
        self.active_panel = panel;
        self.request_context_menu = None;
        cx.notify();
    }

    fn focus_url_input(
        &mut self,
        _event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.method_menu_open = false;
        self.request_context_menu = None;
        let focus_handle = self.url_input.read(cx).focus_handle(cx);
        window.focus(&focus_handle);
        cx.notify();
    }

    fn move_open_tab_to_drop_index(&mut self, request_id: usize, drop_index: usize) -> bool {
        let Some(from_index) = self.request_index_by_id(request_id) else {
            return false;
        };
        let mut insert_index = drop_index.min(self.open_tabs.len());
        if from_index < insert_index {
            insert_index -= 1;
        }
        if from_index == insert_index {
            return false;
        }
        let request_id = self.open_tabs.remove(from_index);
        self.open_tabs.insert(insert_index, request_id);
        true
    }

    fn drop_request_tab_on_tab(
        &mut self,
        target_request_id: usize,
        dragged_tab: &DraggedRequestTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target_index) = self.request_index_by_id(target_request_id) else {
            return;
        };
        let drop_index = if target_index < dragged_tab.source_index {
            target_index
        } else {
            target_index + 1
        };
        if self.move_open_tab_to_drop_index(dragged_tab.request_id, drop_index) {
            self.suppress_tab_click_selection = true;
            cx.notify();
        }
    }

    fn drop_request_tab_at_end(
        &mut self,
        dragged_tab: &DraggedRequestTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.move_open_tab_to_drop_index(dragged_tab.request_id, self.open_tabs.len()) {
            self.suppress_tab_click_selection = true;
            cx.notify();
        }
    }

    fn drop_collection_before_item(
        &mut self,
        target_item_id: usize,
        dragged_item: &DraggedCollectionItem,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .workspace
            .move_item_before(dragged_item.item_id, target_item_id)
        {
            self.suppress_collection_click = true;
            self.status_line = "Moved collection item.".into();
            self.persist_workspace();
            cx.notify();
        }
    }

    fn drop_collection_into_folder(
        &mut self,
        target_folder_id: usize,
        dragged_item: &DraggedCollectionItem,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .workspace
            .move_item_into_folder(dragged_item.item_id, target_folder_id)
        {
            self.suppress_collection_click = true;
            self.expanded_folders.insert(target_folder_id);
            self.status_line = "Moved collection item into folder.".into();
            self.persist_workspace();
            cx.notify();
        }
    }

    fn drop_collection_at_root_end(
        &mut self,
        dragged_item: &DraggedCollectionItem,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace.move_item_to_root_end(dragged_item.item_id) {
            self.suppress_collection_click = true;
            self.status_line = "Moved collection item to root.".into();
            self.persist_workspace();
            cx.notify();
        }
    }

    fn select_request_from_tab_click(
        &mut self,
        request_id: usize,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.suppress_tab_click_selection {
            self.suppress_tab_click_selection = false;
            return;
        }
        self.select_request_by_id(request_id, cx);
    }

    fn toggle_left_dock(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.left_dock_open = !self.left_dock_open;
        self.status_line = if self.left_dock_open {
            "Opened collection dock."
        } else {
            "Closed collection dock."
        }
        .into();
        cx.notify();
    }

    fn toggle_right_dock(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.right_dock_open = !self.right_dock_open;
        self.status_line = if self.right_dock_open {
            "Opened environment dock."
        } else {
            "Closed environment dock."
        }
        .into();
        cx.notify();
    }

    fn clamp_pixels(value: Pixels, min: Pixels, max: Pixels) -> Pixels {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }

    fn start_collection_resize(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_pane_resize = Some(PaneResize {
            target: PaneResizeTarget::Collection,
            origin: event.position,
            initial_size: self.collection_width,
        });
        cx.notify();
    }

    fn start_response_resize(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_pane_resize = Some(PaneResize {
            target: PaneResizeTarget::Response,
            origin: event.position,
            initial_size: self.response_height,
        });
        cx.notify();
    }

    fn update_pane_resize(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(resize) = self.active_pane_resize else {
            return;
        };
        match resize.target {
            PaneResizeTarget::Collection => {
                let delta = event.position.x - resize.origin.x;
                self.collection_width =
                    Self::clamp_pixels(resize.initial_size + delta, px(220.0), px(420.0));
            }
            PaneResizeTarget::Response => {
                let delta = resize.origin.y - event.position.y;
                self.response_height =
                    Self::clamp_pixels(resize.initial_size + delta, px(160.0), px(420.0));
            }
        }
        cx.notify();
    }

    fn finish_pane_resize(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_pane_resize.is_some() {
            self.active_pane_resize = None;
            cx.notify();
        }
    }

    fn next_item_id(&mut self) -> usize {
        let id = self.next_item_id;
        self.next_item_id += 1;
        id
    }

    fn new_request(&mut self) -> Request {
        let id = self.next_item_id();
        Request {
            id,
            name: format!("Untitled request {id}").into(),
            method: Method::Get,
            url: "".into(),
            query: vec![Header::new("", "")],
            proxy_url: None,
            auth: Auth::None,
            headers: vec![Header::new("Accept", "application/json")],
            content_type: "application/json".into(),
            body: "".into(),
            response: None,
            history: vec![],
            response_pinned: false,
        }
    }

    fn add_request(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let request = self.new_request();
        let id = request.id;
        self.method_menu_open = false;
        self.clear_request_overlays();
        self.active_panel = Panel::Params;
        self.workspace.insert_root_request(request);
        self.open_tabs.push(id);
        self.active_request_id = Some(id);
        self.url_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.status_line = "Created a raw request entry.".into();
        self.persist_workspace();
        cx.notify();
    }

    fn add_root_folder(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_item_id();
        self.method_menu_open = false;
        self.clear_request_overlays();
        self.workspace
            .insert_root_folder(id, format!("New folder {id}"));
        self.expanded_folders.insert(id);
        self.status_line = "Created folder.".into();
        self.persist_workspace();
        cx.notify();
    }

    fn add_request_to_context_folder(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.folder_context_menu else {
            return;
        };
        self.add_request_to_folder(menu.folder_id, cx);
    }

    fn add_request_to_context_folder_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.folder_context_menu else {
            return;
        };
        self.add_request_to_folder(menu.folder_id, cx);
    }

    fn add_request_to_folder(&mut self, folder_id: usize, cx: &mut Context<Self>) {
        let request = self.new_request();
        let id = request.id;
        if !self.workspace.insert_request_in_folder(folder_id, request) {
            self.status_line = "Folder no longer exists.".into();
            cx.notify();
            return;
        }
        self.expanded_folders.insert(folder_id);
        self.folder_context_menu = None;
        self.active_panel = Panel::Params;
        self.open_tabs.push(id);
        self.active_request_id = Some(id);
        self.url_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.status_line = "Created request in folder.".into();
        self.persist_workspace();
        cx.notify();
    }

    fn add_folder_to_context_folder(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.folder_context_menu else {
            return;
        };
        self.add_folder_to_folder(menu.folder_id, cx);
    }

    fn add_folder_to_context_folder_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.folder_context_menu else {
            return;
        };
        self.add_folder_to_folder(menu.folder_id, cx);
    }

    fn add_folder_to_folder(&mut self, folder_id: usize, cx: &mut Context<Self>) {
        let id = self.next_item_id();
        if !self
            .workspace
            .insert_folder_in_folder(folder_id, id, format!("New folder {id}"))
        {
            self.status_line = "Folder no longer exists.".into();
            cx.notify();
            return;
        }
        self.expanded_folders.insert(folder_id);
        self.expanded_folders.insert(id);
        self.folder_context_menu = None;
        self.status_line = "Created nested folder.".into();
        self.persist_workspace();
        cx.notify();
    }

    fn send_request(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.method_menu_open = false;
        self.request_context_menu = None;
        self.method_menu_open = false;
        self.request_context_menu = None;
        if self.active_request().is_none() {
            self.status_line = "Create a request before sending.".into();
            cx.notify();
            return;
        }

        let current_url = self.url_input.read(cx).value();
        self.active_request_mut().unwrap().url = current_url.into();
        self.sync_active_request_inputs(cx);
        self.sync_environment_inputs(cx);
        let request = self.active_request().unwrap().clone();
        let request_id = request.id;
        let request_name = request.name.clone();
        let environment = &self.workspace.environments[self.workspace.active_environment];
        let resolved = match request.to_resolved_domain(environment) {
            Ok(request) => request,
            Err(error) => {
                self.status_line = error.into();
                cx.notify();
                return;
            }
        };

        if let Some(prior) = self.in_flight_requests.remove(&request_id) {
            prior
                .cancel
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_for_task = cancel.clone();

        self.status_line = format!("Sending {request_name}…").into();
        cx.notify();

        let task = cx.spawn(async move |this, cx| {
            let cancel_check = cancel_for_task.clone();
            let result = cx
                .background_executor()
                .spawn(async move { domain::send_http_request(&resolved) })
                .await;
            if cancel_check.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = this.update(cx, |app, cx| {
                    app.in_flight_requests.remove(&request_id);
                    app.status_line = format!("Cancelled {request_name}.").into();
                    cx.notify();
                });
                return;
            }
            let _ = this.update(cx, |app, cx| {
                app.in_flight_requests.remove(&request_id);
                match result {
                    Ok(response) => {
                        let response = ResponseRecord::from_domain(response);
                        if let Some(active) = app.workspace.request_mut_by_id(request_id) {
                            active.response = Some(response.clone());
                            active.history.insert(0, response);
                            let status = active.response.as_ref().unwrap().status;
                            let status_text =
                                active.response.as_ref().unwrap().status_text.clone();
                            let history_len = active.history.len();
                            app.status_line = format!(
                                "Sent {request_name} and received {status} {status_text}; history now has {history_len} item(s)."
                            )
                            .into();
                            app.persist_workspace();
                        } else {
                            app.status_line = format!(
                                "Received response for {request_name} but the request was deleted."
                            )
                            .into();
                        }
                    }
                    Err(error) => {
                        app.status_line =
                            format!("Request {request_name} failed: {error}").into();
                    }
                }
                cx.notify();
            });
        });

        self.in_flight_requests.insert(
            request_id,
            InFlightRequest {
                cancel,
                _task: task,
            },
        );
    }

    fn send_focused_request(
        &mut self,
        _: &SendFocusedRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.submit_active_dialog(window, cx) {
            return;
        }
        if self.url_input.read(cx).is_focused(window) {
            self.send_active_request(cx);
            return;
        }
        let focused_body_input = self
            .body_inputs
            .values()
            .find(|input| input.read(cx).is_focused(window))
            .cloned();
        if let Some(input) = focused_body_input {
            input.update(cx, |input, cx| input.insert_newline(cx));
            self.sync_active_request_inputs(cx);
            cx.notify();
        }
    }

    fn submit_dialog_from_action(
        &mut self,
        _: &SubmitDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.submit_active_dialog(window, cx);
    }

    fn cancel_dialog_from_action(
        &mut self,
        _: &CancelDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.cancel_active_dialog(cx) {
            window.focus(&self.focus_handle);
        }
    }

    fn dismiss_dialog_on_backdrop(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_active_dialog(cx);
    }

    fn submit_active_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.rename_request_dialog.is_some() {
            self.confirm_rename_request(&gpui::ClickEvent::default(), window, cx);
            return true;
        }
        if self.rename_folder_dialog.is_some() {
            self.confirm_rename_folder(&gpui::ClickEvent::default(), window, cx);
            return true;
        }
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
            cx.notify();
            return true;
        }
        if self.delete_request_dialog.is_some() {
            self.confirm_delete_request(&gpui::ClickEvent::default(), window, cx);
            return true;
        }
        if self.delete_folder_dialog.is_some() {
            self.confirm_delete_folder(&gpui::ClickEvent::default(), window, cx);
            return true;
        }
        false
    }

    fn delete_collection_selection_from_action(
        &mut self,
        _: &DeleteCollectionSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_delete_dialog_for_selected_collection_item(window, cx);
    }

    fn rename_collection_selection_from_action(
        &mut self,
        _: &RenameCollectionSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_rename_dialog_for_selected_collection_item(window, cx);
    }

    fn open_delete_dialog_for_selected_collection_item(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.rename_request_dialog.is_some()
            || self.rename_folder_dialog.is_some()
            || self.delete_request_dialog.is_some()
            || self.delete_folder_dialog.is_some()
            || self.settings_dialog_open
        {
            return false;
        }

        match self.selected_collection_item {
            Some(CollectionSelection::Request(request_id))
                if self.workspace.request_by_id(request_id).is_some() =>
            {
                self.open_delete_request_dialog(request_id, window, cx);
                true
            }
            Some(CollectionSelection::Folder(folder_id))
                if self.workspace.folder_name(folder_id).is_some() =>
            {
                self.open_delete_folder_dialog(folder_id, window, cx);
                true
            }
            _ => {
                self.status_line = "Select a request or folder before deleting.".into();
                cx.notify();
                false
            }
        }
    }

    fn open_rename_dialog_for_selected_collection_item(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.rename_request_dialog.is_some()
            || self.rename_folder_dialog.is_some()
            || self.delete_request_dialog.is_some()
            || self.delete_folder_dialog.is_some()
            || self.settings_dialog_open
        {
            return false;
        }

        match self.selected_collection_item {
            Some(CollectionSelection::Request(request_id))
                if self.workspace.request_by_id(request_id).is_some() =>
            {
                self.open_rename_request_dialog(request_id, window, cx);
                true
            }
            Some(CollectionSelection::Folder(folder_id))
                if self.workspace.folder_name(folder_id).is_some() =>
            {
                self.open_rename_folder_dialog(folder_id, window, cx);
                true
            }
            _ => {
                self.status_line = "Select a request or folder before renaming.".into();
                cx.notify();
                false
            }
        }
    }

    fn cancel_active_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        if self.rename_request_dialog.take().is_some()
            || self.rename_folder_dialog.take().is_some()
            || self.delete_request_dialog.take().is_some()
            || self.delete_folder_dialog.take().is_some()
        {
            cx.notify();
            return true;
        }
        if self.settings_dialog_open {
            self.settings_dialog_open = false;
            cx.notify();
            return true;
        }
        false
    }

    fn send_active_request(&mut self, cx: &mut Context<Self>) {
        self.method_menu_open = false;
        self.request_context_menu = None;
        if self.active_request().is_none() {
            self.status_line = "Create a request before sending.".into();
            cx.notify();
            return;
        }

        let current_url = self.url_input.read(cx).value();
        self.active_request_mut().unwrap().url = current_url.into();
        self.sync_active_request_inputs(cx);
        self.sync_environment_inputs(cx);
        let request = self.active_request().unwrap().clone();
        let environment = &self.workspace.environments[self.workspace.active_environment];
        let request = match request.to_resolved_domain(environment) {
            Ok(request) => request,
            Err(error) => {
                self.status_line = error.into();
                cx.notify();
                return;
            }
        };
        match domain::send_http_request(&request) {
            Ok(response) => {
                let response = ResponseRecord::from_domain(response);
                let active = self.active_request_mut().unwrap();
                active.response = Some(response.clone());
                active.history.insert(0, response);
                self.status_line = format!(
                    "Sent {} and received {} {}; history now has {} item(s).",
                    active.name,
                    active.response.as_ref().unwrap().status,
                    active.response.as_ref().unwrap().status_text,
                    active.history.len()
                )
                .into();
                self.persist_workspace();
            }
            Err(error) => {
                self.status_line = format!("Request failed: {error}").into();
            }
        }
        cx.notify();
    }

    fn cancel_request(
        &mut self,
        request_id: usize,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(prior) = self.in_flight_requests.remove(&request_id) {
            prior
                .cancel
                .store(true, std::sync::atomic::Ordering::SeqCst);
            self.status_line = "Cancelling request…".into();
            cx.notify();
        }
    }

    fn cancel_active_request(
        &mut self,
        event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.active_request_id {
            self.cancel_request(id, event, window, cx);
        }
    }

    fn is_active_request_in_flight(&self) -> bool {
        self.active_request_id
            .map(|id| self.in_flight_requests.contains_key(&id))
            .unwrap_or(false)
    }

    fn add_param_row(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            request.query.push(Header::new("", ""));
            self.status_line = "Added query parameter row.".into();
            self.persist_workspace();
        }
        cx.notify();
    }

    fn toggle_param_enabled(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if let Some(param) = request.query.get_mut(index) {
                param.enabled = !param.enabled;
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    fn remove_param_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if request.query.len() > 1 && index < request.query.len() {
                request.query.remove(index);
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    fn add_header_row(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            request.headers.push(Header::new("", ""));
            self.status_line = "Added header row.".into();
            self.persist_workspace();
        }
        cx.notify();
    }

    fn toggle_header_enabled(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if let Some(header) = request.headers.get_mut(index) {
                header.enabled = !header.enabled;
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    fn remove_header_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if request.headers.len() > 1 && index < request.headers.len() {
                request.headers.remove(index);
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    fn ensure_param_row(&mut self) {
        if let Some(request) = self.active_request_mut() {
            if request.query.is_empty() {
                request.query.push(Header::new("", ""));
            }
        }
    }

    fn ensure_header_row(&mut self) {
        if let Some(request) = self.active_request_mut() {
            if request.headers.is_empty() {
                request.headers.push(Header::new("", ""));
            }
        }
    }

    fn cycle_auth(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            request.auth = match &request.auth {
                Auth::None => Auth::Bearer {
                    label: "token".into(),
                    secret_ref: "{{token}}".into(),
                },
                Auth::Bearer { .. } => Auth::Basic {
                    username: "user".into(),
                    password: "{{password}}".into(),
                },
                Auth::Basic { .. } => Auth::ApiKey {
                    name: "x-api-key".into(),
                    secret_ref: "{{token}}".into(),
                    location: AuthLocation::Header,
                },
                Auth::ApiKey { .. } => Auth::None,
            };
            self.status_line = "Changed request auth mode.".into();
            self.persist_workspace();
        }
        cx.notify();
    }

    fn cycle_api_key_location(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(request) = self.active_request_mut()
            && let Auth::ApiKey { location, .. } = &mut request.auth
        {
            *location = location.next();
            self.status_line = format!("API key will be sent in {}.", location.label()).into();
            self.persist_workspace();
        }
        cx.notify();
    }

    fn format_body_json(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.sync_active_request_inputs(cx);
        let Some(raw_body) = self
            .active_request()
            .map(|request| request.body.to_string())
        else {
            return;
        };
        let trimmed = raw_body.trim();
        if trimmed.is_empty() {
            self.status_line = "Body is empty; nothing to format.".into();
            cx.notify();
            return;
        }
        match format_json_body(trimmed) {
            Ok(formatted) => {
                if let Some(request) = self.active_request_mut() {
                    request.content_type = "application/json".into();
                }
                self.set_active_request_body(formatted, cx);
                self.status_line = "Formatted request body as JSON.".into();
                self.persist_workspace();
            }
            Err(error) => {
                self.status_line = format!("Format failed: {error}").into();
            }
        }
        cx.notify();
    }

    fn cycle_environment(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.workspace.environments.is_empty() {
            self.workspace.active_environment =
                (self.workspace.active_environment + 1) % self.workspace.environments.len();
            let name = self.workspace.environments[self.workspace.active_environment]
                .name
                .clone();
            self.status_line = format!("Active environment: {name}.").into();
            self.persist_workspace();
        }
        cx.notify();
    }

    fn add_environment_variable(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(environment) = self
            .workspace
            .environments
            .get_mut(self.workspace.active_environment)
        {
            environment.variables.push(Header::new("", ""));
            self.status_line = "Added environment variable.".into();
            self.persist_workspace();
        }
        cx.notify();
    }

    fn set_response_panel(
        &mut self,
        panel: ResponsePanel,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_response_panel = panel;
        self.body_view_menu_open = false;
        self.body_view_menu_position = None;
        cx.notify();
    }

    fn set_body_view_mode(
        &mut self,
        mode: BodyViewMode,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_mode = mode;
        self.body_view_menu_open = false;
        self.body_view_menu_position = None;
        cx.notify();
    }

    fn set_body_view_mode_from_mouse_down(
        &mut self,
        mode: BodyViewMode,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_mode = mode;
        self.body_view_menu_open = false;
        self.body_view_menu_position = None;
        cx.notify();
    }

    fn toggle_body_view_menu(
        &mut self,
        event: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_menu_open = !self.body_view_menu_open;
        self.body_view_menu_position = if self.body_view_menu_open {
            Some(point(
                event.position().x - px(46.0),
                event.position().y + px(16.0),
            ))
        } else {
            None
        };
        cx.notify();
    }

    fn dismiss_body_view_menu(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_menu_open = false;
        self.body_view_menu_position = None;
        cx.notify();
    }

    fn copy_response_header_value(
        &mut self,
        value: SharedString,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
        self.status_line = "Copied response header value.".into();
        cx.notify();
    }

    fn open_settings_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings = self.settings;
        self.settings_dialog_open = true;
        self.method_menu_open = false;
        self.clear_request_overlays();
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn open_settings_from_action(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings = self.settings;
        self.settings_dialog_open = true;
        self.method_menu_open = false;
        self.clear_request_overlays();
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn close_settings_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_dialog_open = false;
        cx.notify();
    }

    fn apply_draft_settings(&mut self, cx: &mut Context<Self>) {
        self.settings = self.draft_settings.clamped();
        self.draft_settings = self.settings;
        self.theme_mode = Self::theme_mode_from_settings(self.settings, cx);
        self.persist_settings();
        cx.notify();
    }

    fn reset_settings_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings = AppSettings::default();
        self.apply_draft_settings(cx);
    }

    fn cycle_draft_theme(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.draft_settings.theme_preference = self.draft_settings.theme_preference.next();
        self.apply_draft_settings(cx);
    }

    fn decrease_draft_ui_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_ui_font_size(-1.0);
        self.apply_draft_settings(cx);
    }

    fn increase_draft_ui_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_ui_font_size(1.0);
        self.apply_draft_settings(cx);
    }

    fn decrease_draft_buffer_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_buffer_font_size(-1.0);
        self.apply_draft_settings(cx);
    }

    fn increase_draft_buffer_font(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft_settings.adjust_buffer_font_size(1.0);
        self.apply_draft_settings(cx);
    }

    fn toggle_method_menu(
        &mut self,
        event: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_request().is_some() {
            self.request_context_menu = None;
            self.method_menu_open = !self.method_menu_open;
            self.method_menu_position = if self.method_menu_open {
                Some(point(
                    event.position().x - px(42.0),
                    event.position().y + px(16.0),
                ))
            } else {
                None
            };
            cx.notify();
        }
    }

    fn set_active_request_method(
        &mut self,
        method: Method,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(request) = self.active_request_mut() {
            request.method = method;
            if !method.uses_body() && self.active_panel == Panel::Body {
                self.active_panel = Panel::Params;
            }
            self.status_line = format!("Changed request method to {}.", method.as_str()).into();
            self.persist_workspace();
        }
        self.method_menu_open = false;
        self.method_menu_position = None;
        self.request_context_menu = None;
        cx.notify();
    }

    fn set_active_request_method_from_mouse_down(
        &mut self,
        method: Method,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(request) = self.active_request_mut() {
            request.method = method;
            if !method.uses_body() && self.active_panel == Panel::Body {
                self.active_panel = Panel::Params;
            }
            self.status_line = format!("Changed request method to {}.", method.as_str()).into();
            self.persist_workspace();
        }
        self.method_menu_open = false;
        self.method_menu_position = None;
        self.request_context_menu = None;
        cx.notify();
    }

    fn dismiss_method_menu(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.method_menu_open = false;
        self.method_menu_position = None;
        cx.notify();
    }

    fn open_request_context_menu(
        &mut self,
        request_id: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.selected_collection_item = Some(CollectionSelection::Request(request_id));
        self.method_menu_open = false;
        self.folder_context_menu = None;
        self.rename_folder_dialog = None;
        self.delete_folder_dialog = None;
        self.request_context_menu = Some(RequestContextMenu {
            request_id,
            position: event.position,
        });
        self.rename_request_dialog = None;
        self.delete_request_dialog = None;
        cx.notify();
    }

    fn open_folder_context_menu(
        &mut self,
        folder_id: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.selected_collection_item = Some(CollectionSelection::Folder(folder_id));
        self.method_menu_open = false;
        self.request_context_menu = None;
        self.rename_request_dialog = None;
        self.delete_request_dialog = None;
        self.folder_context_menu = Some(FolderContextMenu {
            folder_id,
            position: event.position,
        });
        self.rename_folder_dialog = None;
        self.delete_folder_dialog = None;
        cx.notify();
    }

    fn dismiss_request_context_menu(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_context_menu = None;
        self.folder_context_menu = None;
        cx.notify();
    }

    fn toggle_folder_expanded(&mut self, folder_id: usize, cx: &mut Context<Self>) {
        if self.suppress_collection_click {
            self.suppress_collection_click = false;
            return;
        }
        if !self.expanded_folders.insert(folder_id) {
            self.expanded_folders.remove(&folder_id);
        }
        self.folder_context_menu = None;
        self.persist_workspace();
        cx.notify();
    }

    fn start_rename_request_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_rename_request_dialog(menu.request_id, window, cx);
        }
    }

    fn start_rename_request_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_rename_request_dialog(menu.request_id, window, cx);
        }
    }

    fn start_close_tab_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.close_request_tab(menu.request_id, cx);
        }
    }

    fn start_close_tab_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.close_request_tab(menu.request_id, cx);
        }
    }

    fn open_rename_request_dialog(
        &mut self,
        request_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.workspace.request_by_id(request_id).cloned() else {
            return;
        };
        let input = cx.new(|cx| {
            TextInput::new_with_selector(
                cx,
                request.name.clone(),
                "Request name",
                "rename-request-input",
            )
        });
        self.rename_request_dialog = Some(RequestRenameDialog { request_id, input });
        self.request_context_menu = None;
        if let Some(dialog) = self.rename_request_dialog.as_ref() {
            let focus_handle = dialog.input.read(cx).focus_handle(cx);
            window.focus(&focus_handle);
        }
        cx.notify();
    }

    fn close_rename_request_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rename_request_dialog = None;
        cx.notify();
    }

    fn confirm_rename_request(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.rename_request_dialog.take() else {
            return;
        };
        let next_name = dialog.input.read(cx).value().trim().to_string();
        if next_name.is_empty() {
            self.status_line = "Request name cannot be empty.".into();
            self.rename_request_dialog = Some(dialog);
            cx.notify();
            return;
        }
        let Some(request) = self.workspace.request_mut_by_id(dialog.request_id) else {
            self.status_line = "Request no longer exists.".into();
            cx.notify();
            return;
        };
        request.name = next_name.clone().into();
        self.status_line = format!("Renamed request to {next_name}.").into();
        self.persist_workspace();
        cx.notify();
    }

    fn start_rename_folder_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_rename_folder_dialog(menu.folder_id, window, cx);
        }
    }

    fn start_rename_folder_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_rename_folder_dialog(menu.folder_id, window, cx);
        }
    }

    fn open_rename_folder_dialog(
        &mut self,
        folder_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.workspace.folder_name(folder_id) else {
            return;
        };
        let input = cx
            .new(|cx| TextInput::new_with_selector(cx, name, "Folder name", "rename-folder-input"));
        self.rename_folder_dialog = Some(FolderRenameDialog { folder_id, input });
        self.folder_context_menu = None;
        if let Some(dialog) = self.rename_folder_dialog.as_ref() {
            let focus_handle = dialog.input.read(cx).focus_handle(cx);
            window.focus(&focus_handle);
        }
        cx.notify();
    }

    fn close_rename_folder_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rename_folder_dialog = None;
        cx.notify();
    }

    fn confirm_rename_folder(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.rename_folder_dialog.take() else {
            return;
        };
        let next_name = dialog.input.read(cx).value().trim().to_string();
        if next_name.is_empty() {
            self.status_line = "Folder name cannot be empty.".into();
            self.rename_folder_dialog = Some(dialog);
            cx.notify();
            return;
        }
        if !self
            .workspace
            .rename_folder(dialog.folder_id, next_name.clone())
        {
            self.status_line = "Folder no longer exists.".into();
            cx.notify();
            return;
        }
        self.status_line = format!("Renamed folder to {next_name}.").into();
        self.persist_workspace();
        cx.notify();
    }

    fn start_delete_request_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_delete_request_dialog(menu.request_id, window, cx);
        }
    }

    fn start_delete_request_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_delete_request_dialog(menu.request_id, window, cx);
        }
    }

    fn start_delete_folder_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_delete_folder_dialog(menu.folder_id, window, cx);
        }
    }

    fn start_delete_folder_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_delete_folder_dialog(menu.folder_id, window, cx);
        }
    }

    fn open_delete_request_dialog(
        &mut self,
        request_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.workspace.request_by_id(request_id) else {
            return;
        };
        self.delete_request_dialog = Some(RequestDeleteDialog {
            request_id,
            request_name: request.name.clone(),
        });
        self.request_context_menu = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn close_delete_request_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_request_dialog = None;
        cx.notify();
    }

    fn open_delete_folder_dialog(
        &mut self,
        folder_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(folder_name) = self.workspace.folder_name(folder_id) else {
            return;
        };
        self.delete_folder_dialog = Some(FolderDeleteDialog {
            folder_id,
            folder_name,
        });
        self.folder_context_menu = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn close_delete_folder_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_folder_dialog = None;
        cx.notify();
    }

    fn confirm_delete_folder(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.delete_folder_dialog.take() else {
            return;
        };
        let Some((folder_name, request_ids)) = self.workspace.remove_folder(dialog.folder_id)
        else {
            self.status_line = "Folder no longer exists.".into();
            cx.notify();
            return;
        };
        for request_id in &request_ids {
            self.body_inputs.remove(request_id);
            self.response_body_inputs
                .retain(|(response_request_id, _), _| response_request_id != request_id);
        }
        self.open_tabs.retain(|id| !request_ids.contains(id));
        self.expanded_folders.remove(&dialog.folder_id);
        if self
            .active_request_id
            .is_some_and(|id| request_ids.contains(&id))
        {
            self.active_request_id = self.open_tabs.first().copied();
            self.sync_url_input_to_active_request(cx);
        }
        self.selected_collection_item = self.active_request_id.map(CollectionSelection::Request);
        self.clear_request_overlays();
        self.status_line = format!("Deleted folder {folder_name}.").into();
        self.persist_workspace();
        cx.notify();
    }

    fn close_request_tab_from_click(
        &mut self,
        request_id: usize,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_request_tab(request_id, cx);
    }

    fn close_request_tab(&mut self, request_id: usize, cx: &mut Context<Self>) {
        let Some(index) = self.request_index_by_id(request_id) else {
            return;
        };
        self.open_tabs.remove(index);
        self.request_context_menu = None;
        if self.active_request_id == Some(request_id) {
            self.active_request_id = self
                .open_tabs
                .get(index)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|previous| self.open_tabs.get(previous))
                })
                .copied();
            self.sync_url_input_to_active_request(cx);
        }
        self.status_line = "Closed request tab.".into();
        cx.notify();
    }

    fn confirm_delete_request(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.delete_request_dialog.take() else {
            return;
        };
        let request_id = dialog.request_id;
        let Some(deleted_request) = self.workspace.remove_request(request_id) else {
            self.status_line = "Request no longer exists.".into();
            cx.notify();
            return;
        };
        let closed_name = deleted_request.name.clone();
        self.open_tabs.retain(|id| *id != request_id);
        self.body_inputs.remove(&request_id);
        self.response_body_inputs
            .retain(|(response_request_id, _), _| *response_request_id != request_id);
        self.method_menu_open = false;
        self.clear_request_overlays();
        if matches!(
            self.selected_collection_item,
            Some(CollectionSelection::Request(id)) if id == request_id
        ) {
            self.selected_collection_item = None;
        }
        if matches!(
            self.selected_collection_item,
            Some(CollectionSelection::Folder(folder_id))
                if self.workspace.folder_name(folder_id).is_none()
        ) {
            self.selected_collection_item = None;
        }

        if self.workspace.request_count() == 0 || self.open_tabs.is_empty() {
            self.active_request_id = None;
            self.url_input
                .update(cx, |input, cx| input.set_content("", cx));
            self.status_line = format!("Deleted {closed_name}; no request selected.").into();
            self.persist_workspace();
            cx.notify();
            return;
        }

        if self.active_request_id == Some(request_id) {
            self.active_request_id = self.open_tabs.first().copied();
        }
        if self.selected_collection_item.is_none() {
            self.selected_collection_item =
                self.active_request_id.map(CollectionSelection::Request);
        }
        self.sync_url_input_to_active_request(cx);
        self.status_line = format!("Deleted {closed_name}.").into();
        self.persist_workspace();
        cx.notify();
    }

    fn render_button(
        label: impl Into<SharedString>,
        active: bool,
        style: ButtonStyle,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let id = label.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        ui::button_base(("button", id), label, active, style, theme).on_click(cx.listener(on_click))
    }

    fn render_scoped_button(
        scope: &'static str,
        label: impl Into<SharedString>,
        active: bool,
        style: ButtonStyle,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let id = label.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        let scope_id = scope.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        let scoped_id = scope_id.wrapping_mul(1_000_003).wrapping_add(id);
        ui::button_base(("button", scoped_id), label, active, style, theme)
            .on_click(cx.listener(on_click))
    }

    fn render_scoped_button_with_selector(
        scope: &'static str,
        selector: &'static str,
        label: impl Into<SharedString>,
        active: bool,
        style: ButtonStyle,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let id = label.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        let scope_id = scope.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        let scoped_id = scope_id.wrapping_mul(1_000_003).wrapping_add(id);
        ui::button_base(("button", scoped_id), label, active, style, theme)
            .debug_selector(move || selector.into())
            .on_click(cx.listener(on_click))
    }

    fn render_toolbar_button(
        label: impl Into<SharedString>,
        active: bool,
        style: ButtonStyle,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let id = label.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        ui::button_base_with_size(
            ("toolbar-button", id),
            label,
            active,
            style,
            ui::ButtonSize::Medium,
            theme,
        )
        .on_click(cx.listener(on_click))
    }

    fn render_window_control_button(
        id: &'static str,
        label: &'static str,
        control_area: WindowControlArea,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let debug_selector = format!("window-control-{id}");
        div()
            .id(id)
            .debug_selector(move || debug_selector.clone())
            .window_control_area(control_area)
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(24.0))
            .rounded_sm()
            .text_color(theme.text_muted)
            .child(label)
            .hover(move |this| {
                let background = if matches!(control_area, WindowControlArea::Close) {
                    theme.error.opacity(0.18)
                } else {
                    theme.ghost_element_hover
                };
                this.bg(background).text_color(theme.text)
            })
            .active(move |this| this.bg(theme.element_active))
            .on_click(cx.listener(on_click))
    }

    fn minimize_window(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.minimize_window();
    }

    fn toggle_maximize_window(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.zoom_window();
        cx.notify();
    }

    fn close_window(&mut self, _: &gpui::ClickEvent, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }

    fn render_bottom_icon_button(
        id: &'static str,
        icon: IconName,
        active: bool,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let icon_color = if active {
            theme.accent
        } else {
            theme.icon_muted
        };
        ui::button_like(
            id,
            false,
            ButtonStyle::Transparent,
            ui::ButtonSize::Default,
            theme,
        )
        .w(ui::ButtonSize::Default.height())
        .child(ui::icon(icon, ui::IconSize::Small, icon_color))
        .debug_selector(move || id.into())
        .on_click(cx.listener(on_click))
    }

    fn render_collection_resize_handle(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.active_pane_resize.map(|resize| resize.target)
            == Some(PaneResizeTarget::Collection);

        div()
            .id("collection-resize-handle")
            .debug_selector(|| "collection-resize-handle".into())
            .w(px(6.0))
            .h_full()
            .flex()
            .flex_none()
            .justify_end()
            .cursor(CursorStyle::ResizeLeftRight)
            .bg(theme.panel_background)
            .child(div().w(px(1.0)).h_full().bg(if active {
                theme.border_focused
            } else {
                theme.border_variant
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(Self::start_collection_resize),
            )
    }

    fn render_response_resize_handle(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active =
            self.active_pane_resize.map(|resize| resize.target) == Some(PaneResizeTarget::Response);

        div()
            .id("response-resize-handle")
            .debug_selector(|| "response-resize-handle".into())
            .h(px(6.0))
            .w_full()
            .flex()
            .flex_col()
            .flex_none()
            .justify_end()
            .cursor(CursorStyle::ResizeUpDown)
            .bg(theme.surface_background)
            .child(div().h(px(1.0)).w_full().bg(if active {
                theme.border_focused
            } else {
                theme.border_variant
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::start_response_resize))
    }

    fn render_resize_hitbox(
        id: &'static str,
        edge: ResizeEdge,
        cursor: CursorStyle,
        inset: Pixels,
        thickness: Pixels,
    ) -> impl IntoElement {
        let debug_selector = format!("window-resize-{id}");
        let hitbox = div()
            .id(("window-resize", stable_key_hash(id)))
            .debug_selector(move || debug_selector.clone())
            .absolute()
            .cursor(cursor)
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                window.start_window_resize(edge);
            });

        match edge {
            ResizeEdge::Top => hitbox.top_0().left(inset).right(inset).h(thickness),
            ResizeEdge::Bottom => hitbox.bottom_0().left(inset).right(inset).h(thickness),
            ResizeEdge::Left => hitbox.left_0().top(inset).bottom(inset).w(thickness),
            ResizeEdge::Right => hitbox.right_0().top(inset).bottom(inset).w(thickness),
            ResizeEdge::TopLeft => hitbox.top_0().left_0().size(inset),
            ResizeEdge::TopRight => hitbox.top_0().right_0().size(inset),
            ResizeEdge::BottomLeft => hitbox.bottom_0().left_0().size(inset),
            ResizeEdge::BottomRight => hitbox.bottom_0().right_0().size(inset),
        }
    }

    fn render_resize_hitboxes() -> [AnyElement; 8] {
        let edge = px(10.0);
        let thickness = px(6.0);
        [
            Self::render_resize_hitbox(
                "top-left",
                ResizeEdge::TopLeft,
                CursorStyle::ResizeUpLeftDownRight,
                edge,
                thickness,
            )
            .into_any_element(),
            Self::render_resize_hitbox(
                "top-right",
                ResizeEdge::TopRight,
                CursorStyle::ResizeUpRightDownLeft,
                edge,
                thickness,
            )
            .into_any_element(),
            Self::render_resize_hitbox(
                "bottom-left",
                ResizeEdge::BottomLeft,
                CursorStyle::ResizeUpRightDownLeft,
                edge,
                thickness,
            )
            .into_any_element(),
            Self::render_resize_hitbox(
                "bottom-right",
                ResizeEdge::BottomRight,
                CursorStyle::ResizeUpLeftDownRight,
                edge,
                thickness,
            )
            .into_any_element(),
            Self::render_resize_hitbox(
                "top",
                ResizeEdge::Top,
                CursorStyle::ResizeUpDown,
                edge,
                thickness,
            )
            .into_any_element(),
            Self::render_resize_hitbox(
                "bottom",
                ResizeEdge::Bottom,
                CursorStyle::ResizeUpDown,
                edge,
                thickness,
            )
            .into_any_element(),
            Self::render_resize_hitbox(
                "left",
                ResizeEdge::Left,
                CursorStyle::ResizeLeftRight,
                edge,
                thickness,
            )
            .into_any_element(),
            Self::render_resize_hitbox(
                "right",
                ResizeEdge::Right,
                CursorStyle::ResizeLeftRight,
                edge,
                thickness,
            )
            .into_any_element(),
        ]
    }

    fn render_collection_items(
        &self,
        items: &[CollectionItem],
        depth: usize,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let spacing = Spacing::app();
        let selected_collection_item = self.selected_collection_item;
        let mut rows = Vec::new();
        for item in items {
            match item {
                CollectionItem::Request(request) => {
                    let request_id = request.id;
                    let row_debug_selector = format!("collection-request-row-{request_id}");
                    let active = matches!(
                        selected_collection_item,
                        Some(CollectionSelection::Request(id)) if id == request_id
                    );
                    let dragged_item = DraggedCollectionItem {
                        item_id: request_id,
                        label: request.name.clone(),
                        detail: "Request".into(),
                        is_folder: false,
                        theme,
                        typography,
                    };
                    rows.push(
                        ui::list_item(("request-row", request.id), active, theme)
                            .pr_0()
                            .debug_selector(move || row_debug_selector.clone())
                            .on_drag(dragged_item, |dragged_item, _, _, cx| {
                                cx.new(|_| dragged_item.clone())
                            })
                            .drag_over::<DraggedCollectionItem>(move |row, dragged_item, _, _| {
                                if dragged_item.item_id == request_id {
                                    row
                                } else {
                                    row.bg(theme.element_selected)
                                        .border_color(theme.border_focused)
                                }
                            })
                            .on_drop(cx.listener(
                                move |this, dragged_item: &DraggedCollectionItem, window, cx| {
                                    this.drop_collection_before_item(
                                        request_id,
                                        dragged_item,
                                        window,
                                        cx,
                                    )
                                },
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_collection_request_by_id(request_id, window, cx);
                            }))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event, window, cx| {
                                    this.open_request_context_menu(request_id, event, window, cx)
                                }),
                            )
                            .pl(px(4.0 + (depth as f32) * 14.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(spacing.cluster_gap())
                                    .child(
                                        div()
                                            .w(px(42.0))
                                            .text_ui_xs(typography)
                                            .text_color(request.method.color())
                                            .child(request.method.as_str()),
                                    )
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .truncate()
                                            .child(request.name.clone()),
                                    ),
                            )
                            .into_any_element(),
                    );
                }
                CollectionItem::Folder { id, name, items } => {
                    let folder_id = *id;
                    let row_debug_selector = format!("collection-folder-row-{folder_id}");
                    let expanded = self.expanded_folders.contains(id);
                    let selected = matches!(
                        selected_collection_item,
                        Some(CollectionSelection::Folder(id)) if id == folder_id
                    );
                    let dragged_item = DraggedCollectionItem {
                        item_id: folder_id,
                        label: name.clone(),
                        detail: "Folder".into(),
                        is_folder: true,
                        theme,
                        typography,
                    };
                    rows.push(
                        ui::list_item(("folder-row", folder_id), selected, theme)
                            .pr_0()
                            .debug_selector(move || row_debug_selector.clone())
                            .relative()
                            .on_drag(dragged_item, |dragged_item, _, _, cx| {
                                cx.new(|_| dragged_item.clone())
                            })
                            .drag_over::<DraggedCollectionItem>(move |row, dragged_item, _, _| {
                                if dragged_item.item_id == folder_id {
                                    row
                                } else {
                                    row.bg(theme.element_selected)
                                        .border_color(theme.border_focused)
                                }
                            })
                            .on_drop(cx.listener(
                                move |this, dragged_item: &DraggedCollectionItem, window, cx| {
                                    this.drop_collection_into_folder(
                                        folder_id,
                                        dragged_item,
                                        window,
                                        cx,
                                    )
                                },
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_collection_folder(folder_id, window, cx);
                            }))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event, window, cx| {
                                    this.open_folder_context_menu(folder_id, event, window, cx)
                                }),
                            )
                            .pl(px(4.0 + (depth as f32) * 14.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(spacing.cluster_gap())
                                    .child(
                                        div()
                                            .w(px(14.0))
                                            .text_ui_xs(typography)
                                            .text_color(theme.text_muted)
                                            .child(if expanded { "v" } else { ">" }),
                                    )
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .truncate()
                                            .child(name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right_0()
                                    .h(px(8.0))
                                    .drag_over::<DraggedCollectionItem>(
                                        move |target, dragged_item, _, _| {
                                            if dragged_item.item_id == folder_id {
                                                target
                                            } else {
                                                target
                                                    .border_t_2()
                                                    .border_color(theme.border_focused)
                                            }
                                        },
                                    )
                                    .on_drop(cx.listener(
                                        move |this,
                                              dragged_item: &DraggedCollectionItem,
                                              window,
                                              cx| {
                                            this.drop_collection_before_item(
                                                folder_id,
                                                dragged_item,
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                            )
                            .into_any_element(),
                    );
                    if expanded {
                        rows.extend(self.render_collection_items(
                            items,
                            depth + 1,
                            theme,
                            typography,
                            cx,
                        ));
                    }
                }
            }
        }
        rows
    }

    fn render_collection_root_tail_target(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("collection-root-tail-drop-target")
            .min_h(px(22.0))
            .rounded_sm()
            .drag_over::<DraggedCollectionItem>(move |target, _, _, _| {
                target.bg(theme.element_selected).border_1()
            })
            .on_drop(cx.listener(
                move |this, dragged_item: &DraggedCollectionItem, window, cx| {
                    this.drop_collection_at_root_end(dragged_item, window, cx)
                },
            ))
            .into_any_element()
    }

    fn render_sidebar(&self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let request_rows =
            self.render_collection_items(&self.workspace.items, 0, theme, typography, cx);

        ui::dock_panel("collection-dock", theme)
            .w(self.collection_width)
            .flex_none()
            .child(
                ui::panel_surface(theme)
                    .rounded_none()
                    .border_0()
                    .border_b_1()
                    .border_color(theme.border_variant)
                    .bg(theme.panel_background)
                    .p(spacing.base12())
                    .child(
                        ui::label(self.workspace.name.clone(), theme)
                            .text_ui_lg(typography)
                            .font_weight(gpui::FontWeight::BOLD),
                    ),
            )
            .child(
                ui::toolbar("collection-toolbar", theme)
                    .h(px(34.0))
                    .p(spacing.base08())
                    .child(Self::render_button(
                        "+ Request",
                        false,
                        ButtonStyle::Subtle,
                        theme,
                        Self::add_request,
                        cx,
                    ))
                    .child(Self::render_button(
                        "+ Folder",
                        false,
                        ButtonStyle::Subtle,
                        theme,
                        Self::add_root_folder,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("request-list")
                    .flex_1()
                    .overflow_scroll()
                    .flex()
                    .flex_col()
                    .children(request_rows)
                    .child(self.render_collection_root_tail_target(theme, cx)),
            )
    }

    fn render_request_tabs(&self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let active_request_id = self.active_request_id();
        let open_tabs = self
            .open_tabs
            .iter()
            .filter_map(|request_id| self.workspace.request_by_id(*request_id))
            .collect::<Vec<_>>();
        let tab_len = open_tabs.len();
        let tabs = open_tabs.into_iter().enumerate().map(|(index, request)| {
            let debug_selector = format!("request-tab-{}", request.id);
            let position = match (index, tab_len) {
                (_, 1) => TabPosition::Only,
                (0, _) => TabPosition::First,
                (last, len) if last + 1 == len => TabPosition::Last,
                _ => TabPosition::Middle,
            };
            let request_id = request.id;
            let close_debug_selector = format!("request-tab-close-{request_id}");
            let label = request.name.clone();
            let selected = Some(request_id) == active_request_id;
            let tab_hover_group = format!("request-tab-hover-{request_id}");
            let dragged_tab = DraggedRequestTab {
                request_id,
                source_index: index,
                label: label.clone(),
                selected,
                theme,
                typography: self.typography(),
            };
            ui::tab_shell(("request-tab", request_id), selected, position, theme)
                .debug_selector(move || debug_selector.clone())
                .group(tab_hover_group.clone())
                .on_drag(dragged_tab, |dragged_tab, _, _, cx| {
                    cx.new(|_| dragged_tab.clone())
                })
                .drag_over::<DraggedRequestTab>(move |tab, dragged_tab, _, _| {
                    if dragged_tab.request_id == request_id {
                        return tab;
                    }
                    let mut tab = tab
                        .bg(theme.element_selected)
                        .border_color(theme.border_focused)
                        .border_0();
                    if index < dragged_tab.source_index {
                        tab = tab.border_l_2();
                    } else if index > dragged_tab.source_index {
                        tab = tab.border_r_2();
                    }
                    tab
                })
                .on_drop(
                    cx.listener(move |this, dragged_tab: &DraggedRequestTab, window, cx| {
                        this.drop_request_tab_on_tab(request_id, dragged_tab, window, cx)
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event, window, cx| {
                        this.open_request_context_menu(request_id, event, window, cx)
                    }),
                )
                .child(
                    ui::tab_label(("request-tab-label", request_id), label).on_click(cx.listener(
                        move |this, event, window, cx| {
                            this.select_request_from_tab_click(request_id, event, window, cx)
                        },
                    )),
                )
                .child(
                    ui::tab_close_button(("request-tab-close", request_id), selected, theme)
                        .debug_selector(move || close_debug_selector.clone())
                        .group_hover(tab_hover_group, |style| style.opacity(1.0))
                        .on_click(cx.listener(move |this, event, window, cx| {
                            this.close_request_tab_from_click(request_id, event, window, cx)
                        })),
                )
                .into_any_element()
        });

        ui::tab_bar(theme)
            .children(tabs)
            .child(
                div()
                    .id("tab-bar-tail-drop-target")
                    .flex_1()
                    .border_b_1()
                    .border_color(theme.border_variant)
                    .drag_over::<DraggedRequestTab>(move |bar, _, _, _| {
                        bar.bg(theme.element_selected)
                    })
                    .on_drop(cx.listener(
                        move |this, dragged_tab: &DraggedRequestTab, window, cx| {
                            this.drop_request_tab_at_end(dragged_tab, window, cx)
                        },
                    )),
            )
            .child(
                ui::icon_button_base("new-request-tab", IconName::Plus, false, theme)
                    .on_click(cx.listener(Self::add_request)),
            )
    }

    fn render_request_context_menu(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(menu) = self.request_context_menu else {
            return div();
        };
        let typography = self.typography();

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(Self::dismiss_request_context_menu),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(Self::dismiss_request_context_menu),
                    ),
            )
            .child(
                deferred(
                    anchored()
                        .anchor(Corner::TopLeft)
                        .position(menu.position)
                        .child(
                            ui::context_menu_panel("tab-context-menu", theme)
                                .debug_selector(|| "tab-context-menu".into())
                                .child(
                                    ui::context_menu_item(
                                        "tab-context-close",
                                        "Close Tab",
                                        theme,
                                        typography,
                                    )
                                    .debug_selector(|| "tab-context-close".into())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            Self::start_close_tab_from_context_menu_mouse_down,
                                        ),
                                    )
                                    .on_click(cx.listener(Self::start_close_tab_from_context_menu)),
                                )
                                .child(
                                    ui::context_menu_item(
                                        "tab-context-rename",
                                        "Rename...",
                                        theme,
                                        typography,
                                    )
                                    .debug_selector(|| "tab-context-rename".into())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            Self::start_rename_request_from_context_menu_mouse_down,
                                        ),
                                    )
                                    .on_click(
                                        cx.listener(Self::start_rename_request_from_context_menu),
                                    ),
                                )
                                .child(
                                    ui::context_menu_item(
                                        "tab-context-delete",
                                        "Delete",
                                        theme,
                                        typography,
                                    )
                                    .debug_selector(|| "tab-context-delete".into())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            Self::start_delete_request_from_context_menu_mouse_down,
                                        ),
                                    )
                                    .on_click(
                                        cx.listener(Self::start_delete_request_from_context_menu),
                                    ),
                                ),
                        ),
                )
                .with_priority(3),
            )
    }

    fn render_folder_context_menu(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(menu) = self.folder_context_menu else {
            return div();
        };
        let typography = self.typography();

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(Self::dismiss_request_context_menu),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(Self::dismiss_request_context_menu),
                    ),
            )
            .child(
                deferred(
                    anchored()
                        .anchor(Corner::TopLeft)
                        .position(menu.position)
                        .child(
                            ui::context_menu_panel("folder-context-menu", theme)
                                .debug_selector(|| "folder-context-menu".into())
                                .child(
                                    ui::context_menu_item(
                                        "folder-context-new-request",
                                        "New Request",
                                        theme,
                                        typography,
                                    )
                                    .debug_selector(|| "folder-context-new-request".into())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(Self::add_request_to_context_folder_mouse_down),
                                    )
                                    .on_click(cx.listener(Self::add_request_to_context_folder)),
                                )
                                .child(
                                    ui::context_menu_item(
                                        "folder-context-new-folder",
                                        "New Folder",
                                        theme,
                                        typography,
                                    )
                                    .debug_selector(|| "folder-context-new-folder".into())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(Self::add_folder_to_context_folder_mouse_down),
                                    )
                                    .on_click(cx.listener(Self::add_folder_to_context_folder)),
                                )
                                .child(
                                    ui::context_menu_item(
                                        "folder-context-rename",
                                        "Rename...",
                                        theme,
                                        typography,
                                    )
                                    .debug_selector(|| "folder-context-rename".into())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            Self::start_rename_folder_from_context_menu_mouse_down,
                                        ),
                                    )
                                    .on_click(
                                        cx.listener(Self::start_rename_folder_from_context_menu),
                                    ),
                                )
                                .child(
                                    ui::context_menu_item(
                                        "folder-context-delete",
                                        "Delete",
                                        theme,
                                        typography,
                                    )
                                    .debug_selector(|| "folder-context-delete".into())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            Self::start_delete_folder_from_context_menu_mouse_down,
                                        ),
                                    )
                                    .on_click(
                                        cx.listener(Self::start_delete_folder_from_context_menu),
                                    ),
                                ),
                        ),
                )
                .with_priority(3),
            )
    }

    fn render_url_input_field(
        &self,
        theme: AppTheme,
        typography: Typography,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.url_input.update(cx, |input, _| {
            input.set_placeholder_color(theme.text_placeholder)
        });
        let focus_handle = self.url_input.read(cx).focus_handle(cx);
        let focused = self.url_input.read(cx).is_focused(window);
        ui::input_field_shell("url-input-shell", focused, theme, typography)
            .track_focus(&focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::focus_url_input))
            .child(self.url_input.clone())
    }

    fn render_field_input(
        id: impl Into<gpui::ElementId>,
        input: Entity<TextInput>,
        theme: AppTheme,
        typography: Typography,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Self::render_field_input_with_background(
            id,
            input,
            theme,
            theme.editor_background,
            typography,
            window,
            cx,
        )
    }

    fn render_field_input_with_background(
        id: impl Into<gpui::ElementId>,
        input: Entity<TextInput>,
        theme: AppTheme,
        background: gpui::Hsla,
        typography: Typography,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        input.update(cx, |input, _| {
            input.set_placeholder_color(theme.text_placeholder)
        });
        let focus_handle = input.read(cx).focus_handle(cx);
        let focused = input.read(cx).is_focused(window);
        let focus_input = input.clone();
        ui::input_field_shell_with_selector(id, "field-input-shell", focused, theme, typography)
            .bg(background)
            .track_focus(&focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_, _event: &MouseDownEvent, window, cx| {
                    let focus_handle = focus_input.read(cx).focus_handle(cx);
                    window.focus(&focus_handle);
                    cx.notify();
                }),
            )
            .child(input)
    }

    fn render_method_select(
        &self,
        request: &Request,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current_method = request.method;

        div().relative().flex_none().child(
            ui::select_trigger_with_size(
                "method-select-trigger",
                current_method.as_str(),
                current_method.color(),
                self.method_menu_open,
                ui::ButtonSize::Medium,
                theme,
                typography,
            )
            .debug_selector(|| "method-select-trigger".into())
            .on_click(cx.listener(Self::toggle_method_menu)),
        )
    }

    fn render_method_menu_overlay(
        &self,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.method_menu_open {
            return div().into_any_element();
        }
        let Some(request) = self.active_request() else {
            return div().into_any_element();
        };
        let spacing = Spacing::app();
        let current_method = request.method;
        let position = self
            .method_menu_position
            .unwrap_or_else(|| point(px(0.0), px(0.0)));
        let items = Method::all()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, method)| {
                let debug_selector = format!("method-option-{}", method.as_str());
                ui::select_menu_item(
                    ("method-option", index),
                    method.as_str(),
                    method.color(),
                    method == current_method,
                    theme,
                    typography,
                )
                .debug_selector(move || debug_selector.clone())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event, window, cx| {
                        this.set_active_request_method_from_mouse_down(method, event, window, cx)
                    }),
                )
                .on_click(cx.listener(move |this, event, window, cx| {
                    this.set_active_request_method(method, event, window, cx)
                }))
                .into_any_element()
            })
            .collect::<Vec<_>>();

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .bg(theme.ghost_element_background)
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_method_menu))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::dismiss_method_menu)),
            )
            .child(
                deferred(
                    anchored().anchor(Corner::TopLeft).position(position).child(
                        div()
                            .id("method-select-menu")
                            .debug_selector(|| "method-select-menu".into())
                            .w(px(112.0))
                            .p(spacing.base04())
                            .rounded_sm()
                            .border_1()
                            .border_color(theme.panel_focused_border)
                            .bg(theme.panel_overlay_background)
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .gap(spacing.base04())
                            .children(items),
                    ),
                )
                .with_priority(3),
            )
            .into_any_element()
    }

    fn render_body_view_select(
        &self,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().relative().flex_none().child(
            ui::select_trigger_with_size(
                "body-view-select-trigger",
                self.body_view_mode.label(),
                theme.accent,
                self.body_view_menu_open,
                ui::ButtonSize::Default,
                theme,
                typography,
            )
            .debug_selector(|| "body-view-select-trigger".into())
            .on_click(cx.listener(Self::toggle_body_view_menu)),
        )
    }

    fn render_body_view_menu_overlay(
        &self,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.body_view_menu_open {
            return div().into_any_element();
        }
        let spacing = Spacing::app();
        let position = self
            .body_view_menu_position
            .unwrap_or_else(|| point(px(0.0), px(0.0)));
        let items = BodyViewMode::all()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, mode)| {
                let debug_selector =
                    format!("body-view-option-{}", mode.label().to_ascii_lowercase());
                ui::select_menu_item(
                    ("body-view-option", index),
                    mode.label(),
                    theme.accent,
                    mode == self.body_view_mode,
                    theme,
                    typography,
                )
                .debug_selector(move || debug_selector.clone())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event, window, cx| {
                        this.set_body_view_mode_from_mouse_down(mode, event, window, cx)
                    }),
                )
                .on_click(cx.listener(move |this, event, window, cx| {
                    this.set_body_view_mode(mode, event, window, cx)
                }))
                .into_any_element()
            })
            .collect::<Vec<_>>();

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .bg(theme.ghost_element_background)
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_body_view_menu))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(Self::dismiss_body_view_menu),
                    ),
            )
            .child(
                deferred(
                    anchored().anchor(Corner::TopLeft).position(position).child(
                        div()
                            .id("body-view-select-menu")
                            .debug_selector(|| "body-view-select-menu".into())
                            .w(px(112.0))
                            .p(spacing.base04())
                            .rounded_sm()
                            .border_1()
                            .border_color(theme.panel_focused_border)
                            .bg(theme.panel_overlay_background)
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .gap(spacing.base04())
                            .children(items),
                    ),
                )
                .with_priority(3),
            )
            .into_any_element()
    }

    fn render_request_editor(
        &mut self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let Some(request) = self.active_request().cloned() else {
            return ui::panel_surface(theme)
                .flex_1()
                .m(spacing.base16())
                .p(spacing.base16())
                .items_center()
                .justify_center()
                .text_color(theme.text_muted)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(spacing.component_gap())
                        .child(
                            ui::label("No request selected", theme)
                                .text_ui_lg(typography)
                                .font_weight(gpui::FontWeight::BOLD),
                        )
                        .child(
                            ui::muted_label("Create a request from the collection dock.", theme)
                                .text_ui_sm(typography),
                        )
                        .child(Self::render_button(
                            "+ Request",
                            false,
                            ButtonStyle::Subtle,
                            theme,
                            Self::add_request,
                            cx,
                        )),
                )
                .into_any_element();
        };
        if !request.method.uses_body() && self.active_panel == Panel::Body {
            self.active_panel = Panel::Params;
        }

        let mut panels = vec![
            Self::render_scoped_button(
                "request-panel",
                Panel::Params.label(),
                self.active_panel == Panel::Params,
                ButtonStyle::Transparent,
                theme,
                |this, _, window, cx| this.set_panel(Panel::Params, window, cx),
                cx,
            )
            .into_any_element(),
            Self::render_scoped_button(
                "request-panel",
                Panel::Headers.label(),
                self.active_panel == Panel::Headers,
                ButtonStyle::Transparent,
                theme,
                |this, _, window, cx| this.set_panel(Panel::Headers, window, cx),
                cx,
            )
            .into_any_element(),
            Self::render_scoped_button(
                "request-panel",
                Panel::Auth.label(),
                self.active_panel == Panel::Auth,
                ButtonStyle::Transparent,
                theme,
                |this, _, window, cx| this.set_panel(Panel::Auth, window, cx),
                cx,
            )
            .into_any_element(),
        ];

        if request.method.uses_body() {
            panels.push(
                Self::render_scoped_button(
                    "request-panel",
                    Panel::Body.label(),
                    self.active_panel == Panel::Body,
                    ButtonStyle::Transparent,
                    theme,
                    |this, _, window, cx| this.set_panel(Panel::Body, window, cx),
                    cx,
                )
                .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.surface_background)
            .child(
                ui::toolbar("request-editor-toolbar", theme)
                    .h(px(40.0))
                    .border_0()
                    .bg(theme.surface_background)
                    .px(spacing.base12())
                    .gap(spacing.component_gap())
                    .child(self.render_method_select(&request, theme, typography, cx))
                    .child(self.render_url_input_field(theme, typography, window, cx))
                    .child(if self.is_active_request_in_flight() {
                        Self::render_toolbar_button(
                            "Cancel",
                            true,
                            ButtonStyle::Tinted(ui::TintColor::Error),
                            theme,
                            Self::cancel_active_request,
                            cx,
                        )
                        .into_any_element()
                    } else {
                        Self::render_toolbar_button(
                            "Send",
                            true,
                            ButtonStyle::Filled,
                            theme,
                            Self::send_request,
                            cx,
                        )
                        .into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(spacing.cluster_gap())
                    .px(spacing.base12())
                    .py(spacing.base04())
                    .border_b_1()
                    .border_color(theme.border_variant)
                    .bg(theme.toolbar_background)
                    .children(panels),
            )
            .child(self.render_active_panel(theme, window, cx))
            .into_any_element()
    }

    fn render_active_panel(
        &mut self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let Some(request) = self.active_request().cloned() else {
            return ui::panel_surface(theme)
                .p(spacing.base12())
                .text_color(theme.text_muted)
                .child("No request selected.")
                .into_any_element();
        };
        match self.active_panel {
            Panel::Params => {
                self.ensure_param_row();
                let request = self.active_request().cloned().unwrap();
                let request_id = request.id;
                let row_count = request.query.len();
                let can_remove = row_count > 1;
                let rows = request
                    .query
                    .iter()
                    .enumerate()
                    .map(|(index, param)| {
                        self.render_editable_pair_row_with_controls(
                            format!("req:{request_id}:param:{index}"),
                            param,
                            index,
                            PairRowKind::Param,
                            can_remove,
                            "Query parameter",
                            "Value",
                            theme,
                            typography,
                            window,
                            cx,
                        )
                    })
                    .collect::<Vec<_>>();
                ui::flat_section(theme)
                    .flex_1()
                    .min_h_0()
                    .children(rows)
                    .child(
                        div()
                            .px(spacing.base12())
                            .py(spacing.base08())
                            .bg(theme.surface_background)
                            .child(Self::render_button(
                                "+ Param",
                                false,
                                ButtonStyle::Subtle,
                                theme,
                                Self::add_param_row,
                                cx,
                            )),
                    )
                    .into_any_element()
            }
            Panel::Headers => {
                self.ensure_header_row();
                let request = self.active_request().cloned().unwrap();
                let request_id = request.id;
                let row_count = request.headers.len();
                let can_remove = row_count > 1;
                let rows = request
                    .headers
                    .iter()
                    .enumerate()
                    .map(|(index, header)| {
                        self.render_editable_pair_row_with_controls(
                            format!("req:{request_id}:header:{index}"),
                            header,
                            index,
                            PairRowKind::Header,
                            can_remove,
                            "Header name",
                            "Header value",
                            theme,
                            typography,
                            window,
                            cx,
                        )
                    })
                    .collect::<Vec<_>>();
                ui::flat_section(theme)
                    .flex_1()
                    .min_h_0()
                    .children(rows)
                    .child(
                        div()
                            .px(spacing.base12())
                            .py(spacing.base08())
                            .bg(theme.surface_background)
                            .child(Self::render_button(
                                "+ Header",
                                false,
                                ButtonStyle::Subtle,
                                theme,
                                Self::add_header_row,
                                cx,
                            )),
                    )
                    .into_any_element()
            }
            Panel::Auth => {
                let auth_inputs = self.render_auth_inputs(&request, theme, typography, window, cx);
                ui::flat_section(theme)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .text_color(theme.text)
                    .child(
                        ui::panel_header(
                            "Authentication",
                            request.auth.summary(),
                            theme,
                            typography,
                        )
                        .bg(theme.surface_background),
                    )
                    .children(auth_inputs)
                    .child(
                        div()
                            .px(spacing.base12())
                            .py(spacing.base08())
                            .flex()
                            .items_center()
                            .gap(spacing.cluster_gap())
                            .bg(theme.surface_background)
                            .child(Self::render_button(
                                "Cycle Auth Mode",
                                false,
                                ButtonStyle::Subtle,
                                theme,
                                Self::cycle_auth,
                                cx,
                            )),
                    )
                    .when(matches!(request.auth, Auth::ApiKey { .. }), |this| {
                        this.child(
                            div()
                                .px(spacing.base12())
                                .pb(spacing.base08())
                                .bg(theme.surface_background)
                                .child(Self::render_button(
                                    "Cycle API Key Location",
                                    false,
                                    ButtonStyle::Transparent,
                                    theme,
                                    Self::cycle_api_key_location,
                                    cx,
                                )),
                        )
                    })
                    .child(
                        div()
                            .px(spacing.base12())
                            .pb(spacing.base12())
                            .text_ui_sm(typography)
                            .text_color(theme.text_muted)
                            .bg(theme.surface_background)
                            .child("Auth values may use environment variables like {{token}}."),
                    )
                    .into_any_element()
            }
            Panel::Body if request.method.uses_body() => {
                let request_id = request.id;
                let content_type_input = self.field_input(
                    format!("req:{request_id}:content-type"),
                    request.content_type.clone(),
                    "application/json",
                    cx,
                );
                let body_input = self.body_input(request_id, request.body.clone(), cx);
                body_input.update(cx, |input, _cx| {
                    input.set_placeholder("Raw JSON, text, or {{variable}}");
                    input.set_placeholder_color(theme.text_placeholder);
                    input.set_syntax_colors(Self::syntax_colors_for_theme(theme));
                });
                let body_focus_input = body_input.clone();
                ui::flat_section(theme)
                    .flex_1()
                    .min_h_0()
                    .child(
                        ui::panel_header(
                            "Body editor",
                            format!("Raw {}", request.content_type),
                            theme,
                            typography,
                        )
                        .bg(theme.surface_background),
                    )
                    .child(
                        div()
                            .p(spacing.base12())
                            .flex()
                            .flex_col()
                            .gap(spacing.component_gap())
                            .bg(theme.surface_background)
                            .child(ui::muted_label("Content-Type", theme).text_ui_sm(typography))
                            .child(Self::render_field_input_with_background(
                                (
                                    "field-input",
                                    stable_key_hash(&format!("req:{request_id}:content-type")),
                                ),
                                content_type_input,
                                theme,
                                theme.surface_background,
                                typography,
                                window,
                                cx,
                            ))
                            .child(ui::muted_label("Body", theme).text_ui_sm(typography))
                            .child(
                                div()
                                    .id(("body-input-shell", request_id))
                                    .debug_selector(|| "body-input-shell".into())
                                    .relative()
                                    .h(px(220.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(if body_input.read(cx).is_focused(window) {
                                        theme.border_focused
                                    } else {
                                        theme.border_variant
                                    })
                                    .bg(theme.surface_background)
                                    .track_focus(&body_input.read(cx).focus_handle(cx))
                                    .cursor(CursorStyle::IBeam)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |_, _event: &MouseDownEvent, window, cx| {
                                                let focus_handle =
                                                    body_focus_input.read(cx).focus_handle(cx);
                                                window.focus(&focus_handle);
                                                cx.notify();
                                            },
                                        ),
                                    )
                                    .child(
                                        div()
                                            .id(("body-code-editor-scroll", request_id))
                                            .flex()
                                            .flex_col()
                                            .h_full()
                                            .overflow_scroll()
                                            .p(spacing.base08())
                                            .child(body_input),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .top(spacing.base08())
                                            .right(spacing.base08())
                                            .child(
                                                ui::icon_button_base(
                                                    "format-body-json",
                                                    IconName::Code,
                                                    false,
                                                    theme,
                                                )
                                                .debug_selector(|| "format-body-json".into())
                                                .on_click(cx.listener(Self::format_body_json)),
                                            ),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
            Panel::Body => ui::flat_section(theme)
                .flex_1()
                .min_h_0()
                .p(spacing.base12())
                .text_color(theme.text_muted)
                .child("This method does not use a request body.")
                .into_any_element(),
        }
    }

    fn render_response(&mut self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let request = self.active_request();
        let active_request_id = request.map(|request| request.id);
        let response = request.and_then(|request| request.response.clone());
        let status_meta = response.as_ref().map(|response| {
            let status_color = if response.status < 300 {
                theme.success
            } else if response.status < 500 {
                theme.warning
            } else {
                theme.error
            };
            (
                status_color,
                format!("{} {}", response.status, response.status_text),
                format!("{} ms", response.duration_ms),
                format!("{} bytes", response.size_bytes),
            )
        });
        div()
            .flex()
            .flex_col()
            .h(self.response_height)
            .flex_none()
            .min_h_0()
            .bg(theme.surface_background)
            .text_color(theme.text)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(spacing.component_gap())
                    .px(spacing.base12())
                    .py(spacing.base04())
                    .border_b_1()
                    .border_color(theme.border_variant)
                    .bg(theme.toolbar_background)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(spacing.cluster_gap())
                            .child(Self::response_tab_button(
                                "Body",
                                self.active_response_panel == ResponsePanel::Body,
                                ResponsePanel::Body,
                                theme,
                                cx,
                            ))
                            .child(Self::response_tab_button(
                                "Headers",
                                self.active_response_panel == ResponsePanel::Headers,
                                ResponsePanel::Headers,
                                theme,
                                cx,
                            ))
                            .child(Self::response_tab_button(
                                "Cookies",
                                self.active_response_panel == ResponsePanel::Cookies,
                                ResponsePanel::Cookies,
                                theme,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(spacing.cluster_gap())
                            .text_ui_sm(typography)
                            .child(self.render_body_view_select(theme, typography, cx))
                            .child(self.render_pin_response_button(theme, cx))
                            .when_some(
                                status_meta,
                                |this, (status_color, status_text, duration, size)| {
                                    this.child(
                                        div()
                                            .debug_selector(|| "response-status-meta".into())
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(status_color)
                                            .child(status_text),
                                )
                                .child(
                                    div()
                                        .debug_selector(|| "response-time-meta".into())
                                        .text_color(theme.text_muted)
                                        .child(duration),
                                )
                                .child(
                                    div()
                                        .debug_selector(|| "response-size-meta".into())
                                        .text_color(theme.text_muted)
                                        .child(size),
                                )
                            }),
                    ),
            )
            .child(match (self.active_response_panel, response) {
                (ResponsePanel::Body, Some(response)) => self.render_response_body(
                    active_request_id.unwrap_or_default(),
                    &response,
                    theme,
                    cx,
                ),
                (ResponsePanel::Headers, Some(response)) => self.render_response_header_list(
                    &response.headers,
                    "No response headers.",
                    theme,
                    cx,
                ),
                (ResponsePanel::Cookies, Some(response)) => self.render_response_header_list(
                    &response.cookies,
                    "No response cookies.",
                    theme,
                    cx,
                ),
                (_, None) => div()
                    .flex_1()
                    .min_h_0()
                    .p(spacing.base12())
                    .text_color(theme.text_muted)
                    .child("Send the request to populate status, headers, body, timing, and size.")
                    .into_any_element(),
            })
    }

    fn response_tab_button(
        label: &'static str,
        active: bool,
        panel: ResponsePanel,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = label.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        let debug_selector = format!("response-tab-{}", label.to_ascii_lowercase());
        ui::button_base(
            ("response-tab-button", id),
            label,
            active,
            ButtonStyle::Transparent,
            theme,
        )
        .debug_selector(move || debug_selector.clone())
        .on_click(cx.listener(move |this, event, window, cx| {
            this.set_response_panel(panel, event, window, cx)
        }))
    }

    fn formatted_body_for(&mut self, request_id: usize, response: &ResponseRecord) -> SharedString {
        let mode = self.body_view_mode;
        if let Some(cache) = self.formatted_body_cache.as_ref() {
            if cache.matches(request_id, mode, &response.body) {
                return cache.formatted.clone();
            }
        }
        let formatted = response_body_for_mode(response.body.as_ref(), mode);
        self.formatted_body_cache = Some(FormattedBodyCache {
            request_id,
            mode,
            body_ptr: response.body.as_ref().as_ptr(),
            body_len: response.body.len(),
            formatted: formatted.clone(),
        });
        formatted
    }

    fn render_pin_response_button(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pinned = self
            .active_request()
            .map(|request| request.response_pinned)
            .unwrap_or(false);
        let label = if pinned { "Pinned" } else { "Pin" };
        Self::render_scoped_button(
            "pin-response",
            label,
            pinned,
            if pinned {
                ButtonStyle::Tinted(ui::TintColor::Accent)
            } else {
                ButtonStyle::Transparent
            },
            theme,
            Self::toggle_pin_active_response,
            cx,
        )
    }

    fn toggle_pin_active_response(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(request) = self.active_request_mut() {
            request.response_pinned = !request.response_pinned;
            let pinned = request.response_pinned;
            self.status_line = if pinned {
                "Pinned response. Future history will persist to disk.".into()
            } else {
                "Unpinned response. Bodies will be session-only.".into()
            };
            self.persist_workspace();
            cx.notify();
        }
    }

    fn render_response_body(
        &mut self,
        request_id: usize,
        response: &ResponseRecord,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let typography = self.typography();
        let spacing = Spacing::app();

        if response.body.len() > MAX_INLINE_RESPONSE_BODY_BYTES {
            return self.render_oversize_response_placeholder(response, theme, cx);
        }

        let body = self.formatted_body_for(request_id, response);
        let response_code_input =
            self.response_body_input(request_id, self.body_view_mode, body.clone(), cx);
        response_code_input.update(cx, |input, _cx| {
            input.set_placeholder_color(theme.text_placeholder);
            input.set_syntax_colors(Self::syntax_colors_for_theme(theme));
        });
        let scroll_handle = self.response_scroll_handle.clone();
        self.response_scrollbar.update(cx, |scrollbar, _cx| {
            scrollbar.set_colors(theme.border_variant, theme.text_muted);
            scrollbar.set_code_input(response_code_input.downgrade());
        });
        let scrollbar = ui::vertical_scrollbar(&self.response_scrollbar);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.surface_background)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("response-body-code-scroll")
                            .debug_selector(|| "response-body-code-scroll".into())
                            .flex()
                            .flex_col()
                            .h_full()
                            .overflow_scroll()
                            .track_scroll(&scroll_handle)
                            .p(spacing.base08())
                            .font_family(ui::JETBRAINS_FONT_FAMILY)
                            .text_buffer(typography)
                            .text_color(theme.editor_text)
                            .child(response_code_input),
                    )
                    .child(scrollbar),
            )
            .into_any_element()
    }

    fn render_oversize_response_placeholder(
        &self,
        response: &ResponseRecord,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let size_label = format_byte_count(response.body.len());
        let body = response.body.clone();
        let suggested_name = suggest_response_filename(response);
        div()
            .debug_selector(|| "response-body-oversize-placeholder".into())
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(spacing.component_gap())
            .flex_1()
            .min_h_0()
            .p(spacing.base12())
            .bg(theme.surface_background)
            .child(
                div()
                    .text_color(theme.text)
                    .text_ui(typography)
                    .child(SharedString::from(format!(
                        "Body too large to render inline ({size_label})."
                    ))),
            )
            .child(
                div()
                    .text_color(theme.text_muted)
                    .text_ui_sm(typography)
                    .child(SharedString::from(
                        "Save the body to a file to inspect it with another tool.",
                    )),
            )
            .child(Self::render_button(
                "Save to file",
                false,
                ButtonStyle::Filled,
                theme,
                move |this, _event, _window, cx| {
                    this.save_response_body_to_file(body.clone(), suggested_name.clone(), cx);
                },
                cx,
            ))
            .into_any_element()
    }

    fn save_response_body_to_file(
        &self,
        body: SharedString,
        suggested_name: String,
        cx: &mut Context<Self>,
    ) {
        let receiver =
            cx.prompt_for_new_path(std::path::Path::new(""), Some(suggested_name.as_str()));
        cx.spawn(async move |this, cx| {
            let Ok(result) = receiver.await else {
                return;
            };
            let Ok(Some(path)) = result else {
                return;
            };
            let write_result = std::fs::write(&path, body.as_bytes());
            let _ = this.update(cx, |app, cx| {
                app.status_line = match write_result {
                    Ok(()) => format!("Saved response body to {}", path.display()).into(),
                    Err(error) => format!("Failed to save response body: {error}").into(),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn render_response_header_list(
        &self,
        headers: &[ResponseHeader],
        empty_label: &'static str,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let spacing = Spacing::app();
        div()
            .flex_1()
            .min_h_0()
            .bg(theme.surface_background)
            .child(
                div()
                    .id("response-kv-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .children(if headers.is_empty() {
                        vec![
                            div()
                                .p(spacing.base12())
                                .text_color(theme.text_muted)
                                .child(empty_label)
                                .into_any_element(),
                        ]
                    } else {
                        headers
                            .iter()
                            .enumerate()
                            .map(move |(index, header)| {
                                self.render_header_row(index, header, theme, cx)
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    }),
            )
            .into_any_element()
    }

    fn render_header_row(
        &self,
        index: usize,
        header: &ResponseHeader,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + '_ {
        let spacing = Spacing::app();
        let value = header.value.clone();
        let copy_selector = format!("response-header-copy-{index}");
        div()
            .flex()
            .items_start()
            .border_b_1()
            .border_color(theme.border_variant)
            .child(
                div()
                    .w(px(180.0))
                    .flex_none()
                    .px(spacing.base12())
                    .py(spacing.base08())
                    .text_color(theme.text)
                    .child(header.name.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(spacing.base12())
                    .py(spacing.base08())
                    .text_color(theme.text_muted)
                    .child(header.value.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .px(spacing.base08())
                    .py(spacing.base06())
                    .child(
                        ui::icon_button_base(
                            ("response-header-copy", index),
                            IconName::Copy,
                            false,
                            theme,
                        )
                        .debug_selector(move || copy_selector.clone())
                        .on_click(cx.listener(
                            move |this, event, window, cx| {
                                this.copy_response_header_value(value.clone(), event, window, cx)
                            },
                        )),
                    ),
            )
    }

    fn render_editable_pair_row(
        &mut self,
        key_prefix: String,
        item: &Header,
        name_placeholder: &'static str,
        value_placeholder: &'static str,
        theme: AppTheme,
        typography: Typography,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let spacing = Spacing::app();
        let name_key = format!("{key_prefix}:name");
        let value_key = format!("{key_prefix}:value");
        let name_input =
            self.field_input(name_key.clone(), item.name.clone(), name_placeholder, cx);
        let value_input =
            self.field_input(value_key.clone(), item.value.clone(), value_placeholder, cx);
        div()
            .flex()
            .gap(spacing.component_gap())
            .border_b_1()
            .border_color(theme.border_variant)
            .bg(theme.surface_background)
            .px(spacing.base12())
            .py(spacing.base06())
            .child(
                div()
                    .w(px(180.0))
                    .child(Self::render_field_input_with_background(
                        ("field-input", stable_key_hash(&name_key)),
                        name_input,
                        theme,
                        theme.surface_background,
                        typography,
                        window,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .child(Self::render_field_input_with_background(
                        ("field-input", stable_key_hash(&value_key)),
                        value_input,
                        theme,
                        theme.surface_background,
                        typography,
                        window,
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn render_editable_pair_row_with_controls(
        &mut self,
        key_prefix: String,
        item: &Header,
        index: usize,
        kind: PairRowKind,
        can_remove: bool,
        name_placeholder: &'static str,
        value_placeholder: &'static str,
        theme: AppTheme,
        typography: Typography,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let spacing = Spacing::app();
        let name_key = format!("{key_prefix}:name");
        let value_key = format!("{key_prefix}:value");
        let name_input =
            self.field_input(name_key.clone(), item.name.clone(), name_placeholder, cx);
        let value_input =
            self.field_input(value_key.clone(), item.value.clone(), value_placeholder, cx);
        let enabled = item.enabled;
        let toggle_id = format!("pair-toggle:{key_prefix}");
        let remove_id = format!("pair-remove:{key_prefix}");
        let toggle_hash = stable_key_hash(&toggle_id);
        let remove_hash = stable_key_hash(&remove_id);
        let check_color = if enabled {
            theme.icon
        } else {
            theme.icon_disabled
        };
        let checkbox_border = if enabled {
            theme.border_variant
        } else {
            theme.border_variant
        };
        let remove_color = if can_remove {
            theme.icon_muted
        } else {
            theme.icon_disabled
        };
        let input_height = ui::ButtonSize::Medium.height();
        div()
            .flex()
            .items_center()
            .gap(spacing.component_gap())
            .border_b_1()
            .border_color(theme.border_variant)
            .bg(theme.surface_background)
            .px(spacing.base12())
            .py(spacing.base06())
            .child(
                div()
                    .id(("pair-toggle", toggle_hash))
                    .w(input_height)
                    .h(input_height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(checkbox_border)
                    .cursor(CursorStyle::PointingHand)
                    .child(ui::icon(IconName::Check, ui::IconSize::Small, check_color))
                    .when(!enabled, |this| this.opacity(0.35))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| match kind {
                            PairRowKind::Param => this.toggle_param_enabled(index, cx),
                            PairRowKind::Header => this.toggle_header_enabled(index, cx),
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(spacing.component_gap())
                    .child(
                        div()
                            .w(px(180.0))
                            .child(Self::render_field_input_with_background(
                                ("field-input", stable_key_hash(&name_key)),
                                name_input,
                                theme,
                                theme.surface_background,
                                typography,
                                window,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Self::render_field_input_with_background(
                                ("field-input", stable_key_hash(&value_key)),
                                value_input,
                                theme,
                                theme.surface_background,
                                typography,
                                window,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .id(("pair-remove", remove_hash))
                    .w(px(20.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .text_color(remove_color)
                    .when(can_remove, |this| {
                        this.cursor(CursorStyle::PointingHand)
                            .hover(move |this| this.bg(theme.element_hover))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| match kind {
                                    PairRowKind::Param => this.remove_param_row(index, cx),
                                    PairRowKind::Header => this.remove_header_row(index, cx),
                                }),
                            )
                    })
                    .child(ui::icon(IconName::Trash, ui::IconSize::Small, remove_color)),
            )
            .into_any_element()
    }

    fn render_auth_inputs(
        &mut self,
        request: &Request,
        theme: AppTheme,
        typography: Typography,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let spacing = Spacing::app();
        let request_id = request.id;
        let mut row =
            |label: &'static str, key: String, value: SharedString, placeholder: &'static str| {
                let input = self.field_input(key.clone(), value, placeholder, cx);
                div()
                    .flex()
                    .items_center()
                    .gap(spacing.component_gap())
                    .px(spacing.base12())
                    .py(spacing.base06())
                    .bg(theme.surface_background)
                    .child(div().w(px(120.0)).text_color(theme.text_muted).child(label))
                    .child(
                        div()
                            .flex_1()
                            .child(Self::render_field_input_with_background(
                                ("field-input", stable_key_hash(&key)),
                                input,
                                theme,
                                theme.surface_background,
                                typography,
                                window,
                                cx,
                            )),
                    )
                    .into_any_element()
            };
        match &request.auth {
            Auth::None => Vec::new(),
            Auth::Basic { username, password } => vec![
                row(
                    "Username",
                    format!("req:{request_id}:auth:username"),
                    username.clone(),
                    "username",
                ),
                row(
                    "Password",
                    format!("req:{request_id}:auth:password"),
                    password.clone(),
                    "password or {{password}}",
                ),
            ],
            Auth::Bearer { label, secret_ref } => vec![
                row(
                    "Label",
                    format!("req:{request_id}:auth:label"),
                    label.clone(),
                    "token",
                ),
                row(
                    "Token",
                    format!("req:{request_id}:auth:secret"),
                    secret_ref.clone(),
                    "{{token}}",
                ),
            ],
            Auth::ApiKey {
                name, secret_ref, ..
            } => vec![
                row(
                    "Name",
                    format!("req:{request_id}:auth:name"),
                    name.clone(),
                    "x-api-key",
                ),
                row(
                    "Value",
                    format!("req:{request_id}:auth:secret"),
                    secret_ref.clone(),
                    "{{token}}",
                ),
            ],
        }
    }

    fn render_environment(
        &mut self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let environment_index = self.workspace.active_environment;
        let environment_name = self.workspace.environments[environment_index].name.clone();
        let variables = self.workspace.environments[environment_index]
            .variables
            .clone();
        let request = self.active_request();
        let history_len = request
            .map(|request| request.history.len())
            .unwrap_or_default();
        let history_rows: Vec<_> = request
            .into_iter()
            .flat_map(|request| request.history.iter().take(5))
            .map(|item| {
                let status_color = if item.status < 300 {
                    theme.success
                } else if item.status < 500 {
                    theme.warning
                } else {
                    theme.error
                };
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme.border_variant)
                    .bg(theme.element_background)
                    .p(spacing.base08())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(status_color)
                                    .child(format!("{} {}", item.status, item.status_text)),
                            )
                            .child(
                                div()
                                    .text_ui_xs(typography)
                                    .text_color(theme.text_muted)
                                    .child(format!("{} ms", item.duration_ms)),
                            ),
                    )
                    .child(
                        div()
                            .text_ui_xs(typography)
                            .text_color(theme.text_muted)
                            .child(format!("{} bytes", item.size_bytes)),
                    )
                    .into_any_element()
            })
            .collect();
        ui::dock_panel("environment-dock", theme)
            .w(px(272.0))
            .border_l_1()
            .child(
                ui::panel_header("Environment", environment_name.clone(), theme, typography).child(
                    Self::render_button(
                        "Switch",
                        false,
                        ButtonStyle::Transparent,
                        theme,
                        Self::cycle_environment,
                        cx,
                    ),
                ),
            )
            .child(
                div()
                    .p(spacing.base12())
                    .flex()
                    .flex_col()
                    .gap(spacing.component_gap())
                    .child(
                        div()
                            .border_1()
                            .border_color(theme.border_variant)
                            .rounded_sm()
                            .p(spacing.base12())
                            .bg(theme.element_background)
                            .text_color(theme.text)
                            .child(environment_name),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(ui::muted_label("Variables", theme).text_ui_sm(typography))
                            .child(Self::render_button(
                                "+ Var",
                                false,
                                ButtonStyle::Subtle,
                                theme,
                                Self::add_environment_variable,
                                cx,
                            )),
                    )
                    .children(
                        variables
                            .iter()
                            .enumerate()
                            .map(|(index, item)| {
                                self.render_editable_pair_row(
                                    format!("env:{environment_index}:var:{index}"),
                                    item,
                                    "Variable name",
                                    "Variable value",
                                    theme,
                                    typography,
                                    window,
                                    cx,
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                    .child(
                        div()
                            .mt(spacing.base12())
                            .text_ui_xs(typography)
                            .text_color(theme.text_muted)
                            .child(format!("Storage: {}", self.workspace.storage_hint)),
                    )
                    .child(
                        div()
                            .mt(spacing.base16())
                            .pt(spacing.base12())
                            .border_t_1()
                            .border_color(theme.border)
                            .child(ui::label("History", theme).font_weight(gpui::FontWeight::BOLD))
                            .child(
                                ui::muted_label(
                                    format!("{history_len} run(s) for active request"),
                                    theme,
                                )
                                .text_ui_xs(typography),
                            ),
                    )
                    .children(history_rows),
            )
    }

    fn render_rename_request_dialog(
        &self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.rename_request_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "rename-request-dialog".into())
                    .absolute()
                    .top(px(110.0))
                    .left(px(360.0))
                    .right(px(360.0))
                    .bg(theme.panel_overlay_background)
                    .border_1()
                    .border_color(theme.panel_focused_border)
                    .rounded_sm()
                    .shadow_lg()
                    .occlude()
                    .p(spacing.base12())
                    .flex()
                    .flex_col()
                    .gap(spacing.component_gap())
                    .child(ui::panel_header("Rename Request", "", theme, typography))
                    .child(Self::render_field_input(
                        "rename-request-input-shell",
                        dialog.input.clone(),
                        theme,
                        typography,
                        window,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(spacing.component_gap())
                            .child(Self::render_button(
                                "Cancel",
                                false,
                                ButtonStyle::Transparent,
                                theme,
                                Self::close_rename_request_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Rename",
                                false,
                                ButtonStyle::Filled,
                                theme,
                                Self::confirm_rename_request,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_delete_request_dialog(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.delete_request_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "delete-request-dialog".into())
                    .absolute()
                    .top(px(126.0))
                    .left(px(360.0))
                    .right(px(360.0))
                    .bg(theme.panel_overlay_background)
                    .border_1()
                    .border_color(theme.panel_focused_border)
                    .rounded_sm()
                    .shadow_lg()
                    .occlude()
                    .p(spacing.base12())
                    .flex()
                    .flex_col()
                    .gap(spacing.component_gap())
                    .child(ui::panel_header("Delete Request", "", theme, typography))
                    .child(
                        ui::muted_label(
                            format!("Delete '{}' permanently?", dialog.request_name),
                            theme,
                        )
                        .text_ui_sm(typography),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(spacing.component_gap())
                            .child(Self::render_button(
                                "Cancel",
                                false,
                                ButtonStyle::Transparent,
                                theme,
                                Self::close_delete_request_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Delete",
                                false,
                                ButtonStyle::Tinted(ui::TintColor::Error),
                                theme,
                                Self::confirm_delete_request,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_rename_folder_dialog(
        &self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.rename_folder_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "rename-folder-dialog".into())
                    .absolute()
                    .top(px(110.0))
                    .left(px(360.0))
                    .right(px(360.0))
                    .bg(theme.panel_overlay_background)
                    .border_1()
                    .border_color(theme.panel_focused_border)
                    .rounded_sm()
                    .shadow_lg()
                    .occlude()
                    .p(spacing.base12())
                    .flex()
                    .flex_col()
                    .gap(spacing.component_gap())
                    .child(ui::panel_header("Rename Folder", "", theme, typography))
                    .child(Self::render_field_input(
                        "rename-folder-input-shell",
                        dialog.input.clone(),
                        theme,
                        typography,
                        window,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(spacing.component_gap())
                            .child(Self::render_button(
                                "Cancel",
                                false,
                                ButtonStyle::Transparent,
                                theme,
                                Self::close_rename_folder_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Rename",
                                false,
                                ButtonStyle::Filled,
                                theme,
                                Self::confirm_rename_folder,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_delete_folder_dialog(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.delete_folder_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "delete-folder-dialog".into())
                    .absolute()
                    .top(px(126.0))
                    .left(px(360.0))
                    .right(px(360.0))
                    .bg(theme.panel_overlay_background)
                    .border_1()
                    .border_color(theme.panel_focused_border)
                    .rounded_sm()
                    .shadow_lg()
                    .occlude()
                    .p(spacing.base12())
                    .flex()
                    .flex_col()
                    .gap(spacing.component_gap())
                    .child(ui::panel_header("Delete Folder", "", theme, typography))
                    .child(
                        ui::muted_label(
                            format!(
                                "Delete '{}' and all nested requests permanently?",
                                dialog.folder_name
                            ),
                            theme,
                        )
                        .text_ui_sm(typography),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(spacing.component_gap())
                            .child(Self::render_button(
                                "Cancel",
                                false,
                                ButtonStyle::Transparent,
                                theme,
                                Self::close_delete_folder_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Delete",
                                false,
                                ButtonStyle::Tinted(ui::TintColor::Error),
                                theme,
                                Self::confirm_delete_folder,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_dialog_backdrop(cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(Self::dismiss_dialog_on_backdrop),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(Self::dismiss_dialog_on_backdrop),
            )
    }

    fn render_settings_dialog(&self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.settings_dialog_open {
            return div().into_any_element();
        }
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "settings-dialog".into())
                    .absolute()
                    .top(px(92.0))
                    .left(px(360.0))
                    .right(px(360.0))
                    .bg(theme.panel_overlay_background)
                    .border_1()
                    .border_color(theme.panel_focused_border)
                    .rounded_sm()
                    .shadow_lg()
                    .occlude()
                    .p(spacing.base12())
                    .flex()
                    .flex_col()
                    .gap(spacing.component_gap())
                    .child(ui::panel_header(
                        "Settings",
                        "Appearance - auto-saved",
                        theme,
                        typography,
                    ))
                    .child(
                        Self::settings_row(
                            "Theme",
                            self.draft_settings.theme_preference.label(),
                            theme,
                            typography,
                        )
                        .child(Self::render_scoped_button_with_selector(
                            "settings-theme-cycle",
                            "settings-theme-cycle",
                            "Cycle",
                            false,
                            ButtonStyle::Subtle,
                            theme,
                            Self::cycle_draft_theme,
                            cx,
                        )),
                    )
                    .child(
                        Self::settings_row("UI font size", "Auto-saved", theme, typography).child(
                            Self::settings_stepper(
                                "settings-ui-font",
                                "settings-ui-font-decrease",
                                "settings-ui-font-increase",
                                format!("{:.0}px", self.draft_settings.ui_font_size),
                                cx,
                                theme,
                                typography,
                                Self::decrease_draft_ui_font,
                                Self::increase_draft_ui_font,
                            ),
                        ),
                    )
                    .child(
                        Self::settings_row("Buffer font size", "Auto-saved", theme, typography)
                            .child(Self::settings_stepper(
                                "settings-buffer-font",
                                "settings-buffer-font-decrease",
                                "settings-buffer-font-increase",
                                format!("{:.0}px", self.draft_settings.buffer_font_size),
                                cx,
                                theme,
                                typography,
                                Self::decrease_draft_buffer_font,
                                Self::increase_draft_buffer_font,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(spacing.component_gap())
                            .child(Self::render_scoped_button_with_selector(
                                "settings-reset",
                                "settings-reset",
                                "Reset",
                                false,
                                ButtonStyle::Transparent,
                                theme,
                                Self::reset_settings_dialog,
                                cx,
                            ))
                            .child(Self::render_scoped_button_with_selector(
                                "settings-done",
                                "settings-done",
                                "Done",
                                false,
                                ButtonStyle::Filled,
                                theme,
                                Self::close_settings_dialog,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn settings_stepper(
        scope: &'static str,
        decrease_selector: &'static str,
        increase_selector: &'static str,
        value: impl Into<SharedString>,
        cx: &mut Context<Self>,
        theme: AppTheme,
        typography: Typography,
        on_decrease: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        on_increase: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
    ) -> Div {
        let spacing = Spacing::app();
        div()
            .flex()
            .items_center()
            .gap(spacing.base04())
            .rounded_sm()
            .border_1()
            .border_color(theme.border_variant)
            .bg(theme.ghost_element_background)
            .p(spacing.base04())
            .child(Self::render_scoped_button_with_selector(
                scope,
                decrease_selector,
                "-",
                false,
                ButtonStyle::Transparent,
                theme,
                on_decrease,
                cx,
            ))
            .child(
                div()
                    .min_w(px(48.0))
                    .h(ui::ButtonSize::Default.height())
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .bg(theme.element_background)
                    .text_ui_sm(typography)
                    .text_color(theme.text)
                    .child(value.into()),
            )
            .child(Self::render_scoped_button_with_selector(
                scope,
                increase_selector,
                "+",
                false,
                ButtonStyle::Transparent,
                theme,
                on_increase,
                cx,
            ))
    }

    fn settings_row(
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        theme: AppTheme,
        typography: Typography,
    ) -> Div {
        let spacing = Spacing::app();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(spacing.component_gap())
            .p(spacing.base08())
            .rounded_sm()
            .bg(theme.element_background)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .flex_1()
                    .child(ui::label(label, theme).text_ui(typography))
                    .child(ui::muted_label(value, theme).text_ui_sm(typography)),
            )
    }
}

impl Render for ApiClientApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = AppTheme::for_mode(self.theme_mode);
        let typography = self.typography();
        let spacing = Spacing::app();

        div()
            .relative()
            .size_full()
            .when(!window.is_maximized() && !window.is_fullscreen(), |this| {
                this.rounded_md()
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme.border)
            })
            .bg(theme.background)
            .text_color(theme.text)
            .text_ui(typography)
            .key_context("ApiClient")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::send_focused_request))
            .on_action(cx.listener(Self::open_settings_from_action))
            .on_action(cx.listener(Self::submit_dialog_from_action))
            .on_action(cx.listener(Self::cancel_dialog_from_action))
            .on_action(cx.listener(Self::delete_collection_selection_from_action))
            .on_action(cx.listener(Self::rename_collection_selection_from_action))
            .on_mouse_move(cx.listener(Self::update_pane_resize))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_pane_resize))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_pane_resize))
            .flex()
            .flex_col()
            .child(
                ui::toolbar("title-toolbar", theme)
                    .h(px(30.0))
                    .justify_between()
                    .bg(theme.title_bar_background)
                    .px(spacing.base12())
                    .child(
                        div()
                            .id("window-drag-region")
                            .debug_selector(|| "window-drag-region".into())
                            .window_control_area(WindowControlArea::Drag)
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move();
                            })
                            .flex()
                            .items_center()
                            .flex_1()
                            .h_full(),
                    )
                    .child(
                        div().flex().items_center().child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(2.0))
                                .child(Self::render_window_control_button(
                                    "minimize",
                                    "-",
                                    WindowControlArea::Min,
                                    theme,
                                    Self::minimize_window,
                                    cx,
                                ))
                                .child(Self::render_window_control_button(
                                    "maximize",
                                    "[]",
                                    WindowControlArea::Max,
                                    theme,
                                    Self::toggle_maximize_window,
                                    cx,
                                ))
                                .child(Self::render_window_control_button(
                                    "close",
                                    "x",
                                    WindowControlArea::Close,
                                    theme,
                                    Self::close_window,
                                    cx,
                                )),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.left_dock_open, |this| {
                        this.child(self.render_sidebar(theme, cx))
                            .child(self.render_collection_resize_handle(theme, cx))
                    })
                    .child(
                        ui::pane("request-workspace-pane", theme)
                            .child(self.render_request_tabs(theme, cx))
                            .child(self.render_request_editor(theme, window, cx))
                            .child(self.render_response_resize_handle(theme, cx))
                            .child(self.render_response(theme, cx)),
                    )
                    .when(self.right_dock_open, |this| {
                        this.child(self.render_environment(theme, window, cx))
                    }),
            )
            .child(
                ui::status_bar(theme, typography)
                    .debug_selector(|| "bottom-bar".into())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(spacing.base04())
                            .child(Self::render_bottom_icon_button(
                                "bottom-toggle-collections",
                                IconName::PanelLeft,
                                self.left_dock_open,
                                theme,
                                Self::toggle_left_dock,
                                cx,
                            ))
                            .child(Self::render_bottom_icon_button(
                                "bottom-open-settings",
                                IconName::Settings,
                                self.settings_dialog_open,
                                theme,
                                Self::open_settings_dialog,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .ml(spacing.component_gap())
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .child(self.status_line.clone()),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .flex()
                            .items_center()
                            .gap(spacing.cluster_gap())
                            .child(format!(
                                "{} request(s) | {} | UI {:.0}px Buf {:.0}px",
                                self.workspace.request_count(),
                                self.theme_mode.label(),
                                self.settings.ui_font_size,
                                self.settings.buffer_font_size
                            ))
                            .child(Self::render_bottom_icon_button(
                                "bottom-toggle-environment",
                                IconName::PanelRight,
                                self.right_dock_open,
                                theme,
                                Self::toggle_right_dock,
                                cx,
                            )),
                    ),
            )
            .child(self.render_settings_dialog(theme, cx))
            .child(self.render_rename_request_dialog(theme, window, cx))
            .child(self.render_delete_request_dialog(theme, cx))
            .child(self.render_rename_folder_dialog(theme, window, cx))
            .child(self.render_delete_folder_dialog(theme, cx))
            .child(self.render_request_context_menu(theme, cx))
            .child(self.render_folder_context_menu(theme, cx))
            .child(self.render_method_menu_overlay(theme, typography, cx))
            .child(self.render_body_view_menu_overlay(theme, typography, cx))
            .when(!window.is_maximized() && !window.is_fullscreen(), |this| {
                this.children(Self::render_resize_hitboxes())
            })
    }
}

#[cfg(test)]
mod tests;
