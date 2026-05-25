use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
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

    pub(crate) fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        }
    }

    pub(crate) fn color(self) -> gpui::Hsla {
        match self {
            Self::Get => rgb(0x15803d).into(),
            Self::Post => rgb(0x2563eb).into(),
            Self::Put => rgb(0x7c3aed).into(),
            Self::Patch => rgb(0xc2410c).into(),
            Self::Delete => rgb(0xb91c1c).into(),
            Self::Options => rgb(0x0f766e).into(),
        }
    }

    pub(crate) fn uses_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }

    pub(crate) fn to_domain(self) -> domain::Method {
        match self {
            Self::Get => domain::Method::Get,
            Self::Post => domain::Method::Post,
            Self::Put => domain::Method::Put,
            Self::Patch => domain::Method::Patch,
            Self::Delete => domain::Method::Delete,
            Self::Options => domain::Method::Options,
        }
    }

    pub(crate) fn from_domain(method: domain::Method) -> Self {
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
pub(crate) enum Auth {
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
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::None => "No Auth",
            Self::Basic { .. } => "Basic Auth",
            Self::Bearer { .. } => "Bearer Token",
            Self::ApiKey { .. } => "API Key",
        }
    }

    #[allow(dead_code)]
    pub(crate) fn summary(&self) -> SharedString {
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

    pub(crate) fn from_domain(auth: domain::Auth) -> Self {
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
pub(crate) enum AuthLocation {
    Header,
    Query,
    Cookie,
}

impl AuthLocation {
    const ALL: [Self; 3] = [Self::Header, Self::Query, Self::Cookie];

    pub(crate) fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Query => "query",
            Self::Cookie => "cookie",
        }
    }

    #[allow(dead_code)]
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Header => Self::Query,
            Self::Query => Self::Cookie,
            Self::Cookie => Self::Header,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Header {
    pub(crate) name: SharedString,
    pub(crate) value: SharedString,
    pub(crate) enabled: bool,
}

impl Header {
    pub(crate) fn new(name: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
        }
    }

    pub(crate) fn to_domain(&self) -> domain::Header {
        domain::Header {
            name: self.name.to_string(),
            value: self.value.to_string(),
            enabled: self.enabled,
        }
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.name.trim().is_empty() && self.value.trim().is_empty()
    }

    pub(crate) fn from_domain(header: domain::Header) -> Self {
        Self {
            name: header.name.into(),
            value: header.value.into(),
            enabled: header.enabled,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Request {
    pub(crate) id: usize,
    pub(crate) name: SharedString,
    pub(crate) method: Method,
    pub(crate) url: SharedString,
    pub(crate) query: Vec<Header>,
    pub(crate) path_params: Vec<Header>,
    pub(crate) proxy_url: Option<SharedString>,
    pub(crate) auth: Auth,
    pub(crate) headers: Vec<Header>,
    pub(crate) content_type: SharedString,
    pub(crate) body: SharedString,
    pub(crate) response: Option<ResponseRecord>,
    pub(crate) history: Vec<ResponseRecord>,
    /// When set, the latest response body and history are persisted to
    /// disk. By default response bodies are session-only to keep the
    /// workspace JSON small and writes off the UI thread fast.
    pub(crate) response_pinned: bool,
}

#[derive(Clone)]
pub(crate) enum CollectionItem {
    Folder {
        id: usize,
        name: SharedString,
        items: Vec<CollectionItem>,
    },
    Request(Request),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CollectionSelection {
    Request {
        collection_id: usize,
        item_id: usize,
    },
    Folder {
        collection_id: usize,
        item_id: usize,
    },
}

impl CollectionItem {
    pub(crate) fn id(&self) -> usize {
        match self {
            Self::Folder { id, .. } => *id,
            Self::Request(request) => request.id,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ResponseTiming {
    pub(crate) dns_lookup_ms: u64,
    pub(crate) connect_ms: u64,
    pub(crate) tls_handshake_ms: u64,
    pub(crate) time_to_first_byte_ms: u64,
    pub(crate) transfer_ms: u64,
}

impl ResponseTiming {
    pub(crate) fn from_domain(timing: domain::ResponseTiming) -> Self {
        Self {
            dns_lookup_ms: timing.dns_lookup_ms,
            connect_ms: timing.connect_ms,
            tls_handshake_ms: timing.tls_handshake_ms,
            time_to_first_byte_ms: timing.time_to_first_byte_ms,
            transfer_ms: timing.transfer_ms,
        }
    }

    pub(crate) fn total_ms(&self) -> u64 {
        self.dns_lookup_ms
            + self.connect_ms
            + self.tls_handshake_ms
            + self.time_to_first_byte_ms
            + self.transfer_ms
    }
}

#[derive(Clone)]
pub(crate) struct ResponseRecord {
    pub(crate) status: u16,
    pub(crate) status_text: SharedString,
    pub(crate) duration_ms: u64,
    pub(crate) size_bytes: usize,
    pub(crate) headers: Vec<ResponseHeader>,
    pub(crate) cookies: Vec<ResponseHeader>,
    pub(crate) body: SharedString,
    pub(crate) timing: Option<ResponseTiming>,
}

#[derive(Clone)]
pub(crate) struct ResponseHeader {
    pub(crate) name: SharedString,
    pub(crate) value: SharedString,
}

impl ResponseHeader {
    pub(crate) fn from_domain(header: domain::Header) -> Self {
        Self {
            name: header.name.into(),
            value: header.value.into(),
        }
    }

    pub(crate) fn to_domain(&self) -> domain::Header {
        domain::Header::new(self.name.to_string(), self.value.to_string())
    }
}

impl ResponseRecord {
    pub(crate) fn from_domain(response: domain::ResponseRecord) -> Self {
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
            timing: response.timing.map(ResponseTiming::from_domain),
        }
    }

    pub(crate) fn to_domain(&self) -> domain::ResponseRecord {
        domain::ResponseRecord {
            status: self.status,
            status_text: self.status_text.to_string(),
            duration_ms: self.duration_ms,
            size_bytes: self.size_bytes,
            headers: self.headers.iter().map(ResponseHeader::to_domain).collect(),
            cookies: self.cookies.iter().map(ResponseHeader::to_domain).collect(),
            body: self.body.to_string(),
            timing: self.timing.map(|t| domain::ResponseTiming {
                dns_lookup_ms: t.dns_lookup_ms,
                connect_ms: t.connect_ms,
                tls_handshake_ms: t.tls_handshake_ms,
                time_to_first_byte_ms: t.time_to_first_byte_ms,
                transfer_ms: t.transfer_ms,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResponsePanel {
    Body,
    Headers,
    Cookies,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PairRowKind {
    Param,
    Header,
    Path,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BodyViewMode {
    Pretty,
    Raw,
}

impl BodyViewMode {
    const ALL: [Self; 2] = [Self::Pretty, Self::Raw];

    pub(crate) fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Pretty => "Pretty",
            Self::Raw => "Raw",
        }
    }
}

#[derive(Clone)]
pub(crate) struct Environment {
    pub(crate) name: SharedString,
    pub(crate) variables: Vec<Header>,
}

#[derive(Clone)]
pub(crate) struct Collection {
    pub(crate) id: usize,
    pub(crate) name: SharedString,
    pub(crate) items: Vec<CollectionItem>,
}

#[derive(Clone)]
pub(crate) struct Workspace {
    pub(crate) name: SharedString,
    pub(crate) storage_hint: SharedString,
    pub(crate) active_environment: usize,
    pub(crate) environments: Vec<Environment>,
    pub(crate) collections: Vec<Collection>,
    pub(crate) expanded_folders: HashSet<usize>,
    pub(crate) expanded_collections: HashSet<usize>,
}

// ── helpers to find the right collection ──────────────────────────

fn find_collection_mut(
    collections: &mut [Collection],
    collection_id: usize,
) -> Option<&mut Collection> {
    collections.iter_mut().find(|c| c.id == collection_id)
}

fn find_collection_by_item_mut(
    collections: &mut [Collection],
    item_id: usize,
) -> Option<(usize, &mut Vec<CollectionItem>)> {
    for collection in collections.iter_mut() {
        if find_item(&collection.items, item_id).is_some() {
            return Some((collection.id, &mut collection.items));
        }
    }
    None
}

fn find_items_mut_in_collections(
    collections: &mut [Collection],
    folder_id: usize,
) -> Option<&mut Vec<CollectionItem>> {
    for collection in collections.iter_mut() {
        if let Some(items) = find_folder_items_mut(&mut collection.items, folder_id) {
            return Some(items);
        }
    }
    None
}

impl Workspace {
    pub(crate) fn sample() -> Self {
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
            collections: vec![],
            expanded_folders: HashSet::new(),
            expanded_collections: HashSet::new(),
        }
    }

    pub(crate) fn from_domain_collection(collection: domain::Collection) -> Collection {
        Collection {
            id: collection.id.parse().ok().filter(|id| *id > 0).unwrap_or(0),
            name: collection.name.into(),
            items: collection
                .items
                .into_iter()
                .map(Self::from_domain_item)
                .collect(),
        }
    }

    pub(crate) fn from_domain_item(item: domain::CollectionItem) -> CollectionItem {
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

    pub(crate) fn to_domain_item(item: &CollectionItem) -> domain::CollectionItem {
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

    pub(crate) fn to_domain_collection(collection: &Collection) -> domain::Collection {
        domain::Collection {
            id: collection.id.to_string(),
            name: collection.name.to_string(),
            items: collection.items.iter().map(Self::to_domain_item).collect(),
        }
    }

    pub(crate) fn from_domain(workspace: domain::Workspace, storage_hint: SharedString) -> Self {
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
        let expanded_collections = workspace
            .expanded_collections
            .iter()
            .filter_map(|id| id.parse::<usize>().ok())
            .filter(|id| *id > 0)
            .collect::<HashSet<_>>();
        Self {
            name: workspace.name.into(),
            storage_hint,
            active_environment,
            environments,
            collections: workspace
                .items
                .into_iter()
                .map(Self::from_domain_collection)
                .collect(),
            expanded_folders,
            expanded_collections,
        }
    }

    // ── helpers to iterate/slice through all collections ──────────

    fn for_each_items_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Vec<CollectionItem>),
    {
        for collection in &mut self.collections {
            f(&mut collection.items);
        }
    }

    // ── public API ────────────────────────────────────────────────

    pub(crate) fn normalize_request_ids(&mut self) -> usize {
        let mut used_ids = HashSet::new();
        let mut next_id = 1usize;
        self.for_each_items_mut(|items| {
            normalize_collection_item_ids(items, &mut used_ids, &mut next_id);
        });
        // Clean up stale folder references across all collections.
        let valid_ids: Vec<usize> = self
            .expanded_folders
            .iter()
            .copied()
            .filter(|folder_id| self.folder_exists(*folder_id))
            .collect();
        self.expanded_folders.clear();
        self.expanded_folders.extend(valid_ids);
        next_id
    }

    fn folder_exists(&self, folder_id: usize) -> bool {
        self.collections
            .iter()
            .any(|c| find_folder(&c.items, folder_id).is_some())
    }

    pub(crate) fn expanded_folder_ids(&self) -> HashSet<usize> {
        self.expanded_folders.clone()
    }

    pub(crate) fn expanded_collection_ids(&self) -> HashSet<usize> {
        if self.expanded_collections.is_empty() {
            self.collections.iter().map(|c| c.id).collect()
        } else {
            self.expanded_collections
                .iter()
                .copied()
                .filter(|id| self.collections.iter().any(|c| c.id == *id))
                .collect()
        }
    }

    pub(crate) fn request_count(&self) -> usize {
        self.request_ids().len()
    }

    pub(crate) fn request_ids(&self) -> Vec<usize> {
        let mut ids = Vec::new();
        for collection in &self.collections {
            collect_request_ids(&collection.items, &mut ids);
        }
        ids
    }

    pub(crate) fn first_request_id(&self) -> Option<usize> {
        // First non-empty collection's first request.
        for collection in &self.collections {
            let mut ids = Vec::new();
            collect_request_ids(&collection.items, &mut ids);
            if let Some(&id) = ids.first() {
                return Some(id);
            }
        }
        None
        // self.request_ids().first().copied() would also work
    }

    pub(crate) fn request_by_id(&self, request_id: usize) -> Option<&Request> {
        for collection in &self.collections {
            if let Some(request) = find_request(&collection.items, request_id) {
                return Some(request);
            }
        }
        None
    }

    pub(crate) fn request_mut_by_id(&mut self, request_id: usize) -> Option<&mut Request> {
        for collection in &mut self.collections {
            if let Some(request) = find_request_mut(&mut collection.items, request_id) {
                return Some(request);
            }
        }
        None
    }

    pub(crate) fn request_by_index(&self, index: usize) -> Option<&Request> {
        let mut seen = 0;
        for collection in &self.collections {
            if let Some(request) = nth_request(&collection.items, index, &mut seen) {
                return Some(request);
            }
        }
        None
    }

    pub(crate) fn insert_request_in_collection(&mut self, collection_id: usize, request: Request) {
        if let Some(collection) = find_collection_mut(&mut self.collections, collection_id) {
            collection.items.push(CollectionItem::Request(request));
        }
    }

    /// Insert request at the root level (first collection, or create one).
    pub(crate) fn insert_root_request(&mut self, request: Request) {
        if let Some(collection) = self.collections.first_mut() {
            collection.items.push(CollectionItem::Request(request));
        } else {
            let id = self.next_collection_id();
            self.collections.push(Collection {
                id,
                name: "Default".into(),
                items: vec![CollectionItem::Request(request)],
            });
        }
    }

    pub(crate) fn insert_request_in_folder(&mut self, folder_id: usize, request: Request) -> bool {
        let Some(items) = find_items_mut_in_collections(&mut self.collections, folder_id) else {
            return false;
        };
        items.push(CollectionItem::Request(request));
        true
    }

    pub(crate) fn next_collection_id(&self) -> usize {
        self.collections.iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    pub(crate) fn insert_root_collection(&mut self, id: usize, name: impl Into<SharedString>) {
        self.collections.push(Collection {
            id,
            name: name.into(),
            items: Vec::new(),
        });
    }

    pub(crate) fn insert_root_folder(&mut self, id: usize, name: impl Into<SharedString>) {
        // Add folder to the first collection, or create a collection.
        if let Some(collection) = self.collections.first_mut() {
            collection.items.push(CollectionItem::Folder {
                id,
                name: name.into(),
                items: Vec::new(),
            });
        } else {
            let cid = self.next_collection_id();
            self.collections.push(Collection {
                id: cid,
                name: "Default".into(),
                items: vec![CollectionItem::Folder {
                    id,
                    name: name.into(),
                    items: Vec::new(),
                }],
            });
        }
    }

    pub(crate) fn insert_folder_in_folder(
        &mut self,
        parent_id: usize,
        id: usize,
        name: impl Into<SharedString>,
    ) -> bool {
        let Some(items) = find_items_mut_in_collections(&mut self.collections, parent_id) else {
            return false;
        };
        items.push(CollectionItem::Folder {
            id,
            name: name.into(),
            items: Vec::new(),
        });
        true
    }

    pub(crate) fn folder_name(&self, folder_id: usize) -> Option<SharedString> {
        for collection in &self.collections {
            if let Some(result) = find_folder(&collection.items, folder_id) {
                return Some(result.1.clone());
            }
        }
        None
    }

    pub(crate) fn rename_folder(
        &mut self,
        folder_id: usize,
        name: impl Into<SharedString>,
    ) -> bool {
        for collection in &mut self.collections {
            if let Some((_, folder_name)) = find_folder_mut(&mut collection.items, folder_id) {
                *folder_name = name.into();
                return true;
            }
        }
        false
    }

    pub(crate) fn remove_request(&mut self, request_id: usize) -> Option<Request> {
        for collection in &mut self.collections {
            if let Some(request) = remove_request_from_items(&mut collection.items, request_id) {
                return Some(request);
            }
        }
        None
    }

    pub(crate) fn remove_folder(&mut self, folder_id: usize) -> Option<(SharedString, Vec<usize>)> {
        for collection in &mut self.collections {
            if let Some(result) = remove_folder_from_items(&mut collection.items, folder_id) {
                return Some(result);
            }
        }
        None
    }

    pub(crate) fn remove_collection(&mut self, collection_id: usize) -> Option<Collection> {
        let index = self
            .collections
            .iter()
            .position(|c| c.id == collection_id)?;
        Some(self.collections.remove(index))
    }

    pub(crate) fn rename_collection(
        &mut self,
        collection_id: usize,
        name: impl Into<SharedString>,
    ) -> bool {
        let Some(collection) = find_collection_mut(&mut self.collections, collection_id) else {
            return false;
        };
        collection.name = name.into();
        true
    }

    pub(crate) fn move_item_before(&mut self, item_id: usize, target_id: usize) -> bool {
        if item_id == target_id || self.item_contains_id(item_id, target_id) {
            return false;
        }
        let Some((_, items)) = find_collection_by_item_mut(&mut self.collections, item_id) else {
            return false;
        };
        let Some(item) = remove_item_from_items(items, item_id) else {
            return false;
        };
        insert_item_before(items, target_id, item)
    }

    pub(crate) fn move_item_between_collections(
        &mut self,
        item_id: usize,
        target_collection_id: usize,
    ) -> bool {
        // Find and remove item from its current collection.
        let item = {
            let items = find_collection_by_item_mut(&mut self.collections, item_id);
            if items.is_none() {
                return false;
            }
            let (_, src_items) = items.unwrap();
            remove_item_from_items(src_items, item_id)
        };
        let Some(item) = item else {
            return false;
        };
        // Insert into target collection's items.
        let Some(target) = find_collection_mut(&mut self.collections, target_collection_id) else {
            return false;
        };
        target.items.push(item);
        true
    }

    pub(crate) fn move_item_into_folder(&mut self, item_id: usize, folder_id: usize) -> bool {
        if item_id == folder_id || self.item_contains_id(item_id, folder_id) {
            return false;
        }
        // Find the item in any collection.
        let item = {
            let items = find_collection_by_item_mut(&mut self.collections, item_id);
            if items.is_none() {
                return false;
            }
            let (_, src_items) = items.unwrap();
            remove_item_from_items(src_items, item_id)
        };
        let Some(item) = item else {
            return false;
        };
        // Find the target folder in any collection.
        let Some(items) = find_items_mut_in_collections(&mut self.collections, folder_id) else {
            // Target folder not found; push to first collection's root.
            if let Some(c) = self.collections.first_mut() {
                c.items.push(item);
            }
            return false;
        };
        items.push(item);
        true
    }

    pub(crate) fn move_item_to_root_end(&mut self, item_id: usize) -> bool {
        let item = {
            let items = find_collection_by_item_mut(&mut self.collections, item_id);
            if items.is_none() {
                return false;
            }
            let (_, src_items) = items.unwrap();
            remove_item_from_items(src_items, item_id)
        };
        let Some(item) = item else {
            return false;
        };
        // Push to first collection's root.
        if let Some(c) = self.collections.first_mut() {
            c.items.push(item);
        } else {
            let id = self.next_collection_id();
            self.collections.push(Collection {
                id,
                name: "Default".into(),
                items: vec![item],
            });
        }
        true
    }

    pub(crate) fn item_contains_id(&self, item_id: usize, target_id: usize) -> bool {
        for collection in &self.collections {
            if find_item(&collection.items, item_id)
                .is_some_and(|item| item_contains_id(item, target_id))
            {
                return true;
            }
        }
        false
    }

    pub(crate) fn to_domain(&self) -> domain::Workspace {
        let mut expanded_folders = self
            .expanded_folders
            .iter()
            .copied()
            .collect::<Vec<usize>>();
        expanded_folders.sort_unstable();
        let mut expanded_collections = self
            .expanded_collections
            .iter()
            .copied()
            .collect::<Vec<usize>>();
        expanded_collections.sort_unstable();
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
            items: self
                .collections
                .iter()
                .map(Self::to_domain_collection)
                .collect(),
            expanded_folders: expanded_folders
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            expanded_collections: expanded_collections
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }
}

pub(crate) fn normalize_collection_item_ids(
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

pub(crate) fn collect_request_ids(items: &[CollectionItem], out: &mut Vec<usize>) {
    for item in items {
        match item {
            CollectionItem::Request(request) => out.push(request.id),
            CollectionItem::Folder { items, .. } => collect_request_ids(items, out),
        }
    }
}

pub(crate) fn find_item(items: &[CollectionItem], item_id: usize) -> Option<&CollectionItem> {
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

pub(crate) fn item_contains_id(item: &CollectionItem, target_id: usize) -> bool {
    match item {
        CollectionItem::Request(request) => request.id == target_id,
        CollectionItem::Folder { id, items, .. } => {
            *id == target_id || items.iter().any(|item| item_contains_id(item, target_id))
        }
    }
}

pub(crate) fn find_request(items: &[CollectionItem], request_id: usize) -> Option<&Request> {
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

pub(crate) fn find_request_mut(
    items: &mut [CollectionItem],
    request_id: usize,
) -> Option<&mut Request> {
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

pub(crate) fn nth_request<'a>(
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

pub(crate) fn find_folder(
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

pub(crate) fn find_folder_mut(
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

pub(crate) fn find_folder_items_mut(
    items: &mut [CollectionItem],
    folder_id: usize,
) -> Option<&mut Vec<CollectionItem>> {
    find_folder_mut(items, folder_id).map(|(items, _)| items)
}

pub(crate) fn remove_request_from_items(
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

pub(crate) fn remove_item_from_items(
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

pub(crate) fn insert_item_before(
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

pub(crate) fn insert_item_before_inner(
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

pub(crate) fn remove_folder_from_items(
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
    pub(crate) fn from_domain(id: usize, request: domain::Request) -> Self {
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
            url: request.url.clone().into(),
            query: request.query.into_iter().map(Header::from_domain).collect(),
            path_params: vec![],
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

    pub(crate) fn to_domain(&self) -> domain::Request {
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

    pub(crate) fn to_domain_with_history(&self) -> domain::Request {
        let mut request = self.to_domain();
        if self.response_pinned {
            request.history = self.history.iter().map(ResponseRecord::to_domain).collect();
        } else {
            request.history.clear();
        }
        request
    }

    pub(crate) fn to_resolved_domain(
        &self,
        environment: &Environment,
    ) -> Result<domain::Request, String> {
        let mut request = self.to_domain();
        request.url = resolve_template(&request.url, environment)?;
        request.url = resolve_path_params(&request.url, &self.path_params)?;
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
