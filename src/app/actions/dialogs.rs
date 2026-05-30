use super::*;
use std::hash::{Hash, Hasher};

impl ApiClientApp {
    pub(crate) fn open_request_context_menu(
        &mut self,
        request_id: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        let collection_id = self
            .workspace
            .collections
            .iter()
            .find(|c| find_request(&c.items, request_id).is_some())
            .map(|c| c.id)
            .unwrap_or(1);
        self.selected_collection_item = Some(CollectionSelection::Request {
            collection_id,
            item_id: request_id,
        });
        self.method_menu_open = false;
        self.folder_context_menu = None;
        self.collection_context_menu = None;
        self.rename_folder_dialog = None;
        self.delete_folder_dialog = None;
        self.rename_collection_dialog = None;
        self.delete_collection_dialog = None;
        self.request_context_menu = Some(RequestContextMenu {
            request_id,
            position: event.position,
        });
        self.rename_request_dialog = None;
        self.delete_request_dialog = None;
        cx.notify();
    }

    pub(crate) fn open_folder_context_menu(
        &mut self,
        folder_id: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        let collection_id = self
            .workspace
            .collections
            .iter()
            .find(|c| find_folder(&c.items, folder_id).is_some())
            .map(|c| c.id)
            .unwrap_or(1);
        self.selected_collection_item = Some(CollectionSelection::Folder {
            collection_id,
            item_id: folder_id,
        });
        self.method_menu_open = false;
        self.request_context_menu = None;
        self.collection_context_menu = None;
        self.rename_request_dialog = None;
        self.delete_request_dialog = None;
        self.rename_collection_dialog = None;
        self.delete_collection_dialog = None;
        self.folder_context_menu = Some(FolderContextMenu {
            folder_id,
            position: event.position,
        });
        self.rename_folder_dialog = None;
        self.delete_folder_dialog = None;
        cx.notify();
    }

    pub(crate) fn open_collection_context_menu(
        &mut self,
        collection_id: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.selected_collection_item = None;
        self.method_menu_open = false;
        self.request_context_menu = None;
        self.folder_context_menu = None;
        self.rename_request_dialog = None;
        self.delete_request_dialog = None;
        self.rename_folder_dialog = None;
        self.delete_folder_dialog = None;
        self.rename_collection_dialog = None;
        self.delete_collection_dialog = None;
        self.collection_context_menu = Some(CollectionContextMenu {
            collection_id,
            position: event.position,
        });
        cx.notify();
    }

    pub(crate) fn dismiss_request_context_menu(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_context_menu = None;
        self.folder_context_menu = None;
        self.collection_context_menu = None;
        cx.notify();
    }

    pub(crate) fn toggle_folder_expanded(&mut self, folder_id: usize, cx: &mut Context<Self>) {
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

    pub(crate) fn toggle_collection_expanded(
        &mut self,
        collection_id: usize,
        cx: &mut Context<Self>,
    ) {
        if self.suppress_collection_click {
            self.suppress_collection_click = false;
            return;
        }
        if !self.expanded_collections.insert(collection_id) {
            self.expanded_collections.remove(&collection_id);
        }
        self.collection_context_menu = None;
        self.persist_workspace();
        cx.notify();
    }

    pub(crate) fn start_rename_request_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_rename_request_dialog(menu.request_id, window, cx);
        }
    }

    pub(crate) fn start_rename_request_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_rename_request_dialog(menu.request_id, window, cx);
        }
    }

    pub(crate) fn start_close_tab_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.close_request_tab(menu.request_id, cx);
        }
    }

    pub(crate) fn start_close_tab_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.close_request_tab(menu.request_id, cx);
        }
    }

    pub(crate) fn open_rename_request_dialog(
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

    pub(crate) fn close_rename_request_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rename_request_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_rename_request(
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

    pub(crate) fn start_rename_folder_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_rename_folder_dialog(menu.folder_id, window, cx);
        }
    }

    pub(crate) fn start_rename_folder_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_rename_folder_dialog(menu.folder_id, window, cx);
        }
    }

    pub(crate) fn open_rename_folder_dialog(
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

    pub(crate) fn close_rename_folder_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rename_folder_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_rename_folder(
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

    pub(crate) fn start_delete_request_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_delete_request_dialog(menu.request_id, window, cx);
        }
    }

    pub(crate) fn start_delete_request_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.request_context_menu {
            self.open_delete_request_dialog(menu.request_id, window, cx);
        }
    }

    pub(crate) fn start_delete_folder_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_delete_folder_dialog(menu.folder_id, window, cx);
        }
    }

    pub(crate) fn start_delete_folder_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.folder_context_menu {
            self.open_delete_folder_dialog(menu.folder_id, window, cx);
        }
    }

    pub(crate) fn open_delete_request_dialog(
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

    pub(crate) fn close_delete_request_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_request_dialog = None;
        cx.notify();
    }

    pub(crate) fn open_delete_folder_dialog(
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

    pub(crate) fn close_delete_folder_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_folder_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_delete_folder(
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
        self.selected_collection_item =
            self.active_request_id
                .map(|id| CollectionSelection::Request {
                    collection_id: 1,
                    item_id: id,
                });
        self.clear_request_overlays();
        self.status_line = format!("Deleted folder {folder_name}.").into();
        self.persist_workspace();
        cx.notify();
    }

    pub(crate) fn close_request_tab_from_click(
        &mut self,
        request_id: usize,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_request_tab(request_id, cx);
    }

    pub(crate) fn close_request_tab(&mut self, request_id: usize, cx: &mut Context<Self>) {
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

    pub(crate) fn confirm_delete_request(
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
            Some(CollectionSelection::Request { item_id: id, .. }) if id == request_id
        ) {
            self.selected_collection_item = None;
        }
        if matches!(
            self.selected_collection_item,
            Some(CollectionSelection::Folder { item_id: folder_id, .. })
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
            self.selected_collection_item = self.active_request_id.map(|id| {
                let collection_id = self
                    .workspace
                    .collections
                    .iter()
                    .find(|c| find_request(&c.items, id).is_some())
                    .map(|c| c.id)
                    .unwrap_or(1);
                CollectionSelection::Request {
                    collection_id,
                    item_id: id,
                }
            });
        }
        self.sync_url_input_to_active_request(cx);
        self.status_line = format!("Deleted {closed_name}.").into();
        self.persist_workspace();
        cx.notify();
    }

    pub(crate) fn start_rename_collection_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.collection_context_menu {
            self.open_rename_collection_dialog(menu.collection_id, window, cx);
        }
    }

    pub(crate) fn start_rename_collection_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.collection_context_menu {
            self.open_rename_collection_dialog(menu.collection_id, window, cx);
        }
    }

    pub(crate) fn open_rename_collection_dialog(
        &mut self,
        collection_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(collection) = self
            .workspace
            .collections
            .iter()
            .find(|c| c.id == collection_id)
            .cloned()
        else {
            return;
        };
        let input = cx.new(|cx| {
            TextInput::new_with_selector(
                cx,
                collection.name.clone(),
                "Collection name",
                "rename-collection-input",
            )
        });
        self.rename_collection_dialog = Some(CollectionRenameDialog {
            collection_id,
            input,
        });
        self.collection_context_menu = None;
        if let Some(dialog) = self.rename_collection_dialog.as_ref() {
            let focus_handle = dialog.input.read(cx).focus_handle(cx);
            window.focus(&focus_handle);
        }
        cx.notify();
    }

    pub(crate) fn close_rename_collection_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rename_collection_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_rename_collection(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.rename_collection_dialog.take() else {
            return;
        };
        let next_name = dialog.input.read(cx).value().trim().to_string();
        if next_name.is_empty() {
            self.status_line = "Collection name cannot be empty.".into();
            self.rename_collection_dialog = Some(dialog);
            cx.notify();
            return;
        }
        if self
            .workspace
            .rename_collection(dialog.collection_id, next_name.clone())
        {
            self.status_line = format!("Renamed collection to {next_name}.").into();
            self.persist_workspace();
        } else {
            self.status_line = "Collection no longer exists.".into();
        }
        cx.notify();
    }

    pub(crate) fn open_create_workspace_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            TextInput::new_with_selector(
                cx,
                "".to_string(),
                "Workspace name",
                "create-workspace-input",
            )
        });
        self.create_workspace_dialog = Some(WorkspaceCreateDialog { input });
        self.workspace_menu_open = false;
        if let Some(dialog) = self.create_workspace_dialog.as_ref() {
            let focus_handle = dialog.input.read(cx).focus_handle(cx);
            window.focus(&focus_handle);
        }
        cx.notify();
    }

    pub(crate) fn close_create_workspace_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_workspace_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_create_workspace(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.create_workspace_dialog.take() else {
            return;
        };
        let name = dialog.input.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.status_line = "Workspace name cannot be empty.".into();
            self.create_workspace_dialog = Some(dialog);
            cx.notify();
            return;
        }

        let mut slug = name
            .chars()
            .map(|c| if c.is_whitespace() { '-' } else { c })
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_lowercase();
        if slug.is_empty() {
            slug = "workspace".to_string();
        }

        let workspaces_dir = self
            .settings_path
            .parent()
            .map(|parent| parent.join("workspaces"))
            .unwrap_or_else(|| PathBuf::from("workspaces"));

        let mut path = workspaces_dir.join(format!("{slug}.json"));
        let mut counter = 1;
        while path.exists() {
            counter += 1;
            path = workspaces_dir.join(format!("{slug}-{counter}.json"));
        }

        let new_workspace = domain::Workspace::new(slug.clone(), name.clone());
        if let Err(error) = domain::save_workspace(&path, &new_workspace) {
            self.status_line = format!("Could not create workspace file: {error}").into();
            self.create_workspace_dialog = Some(dialog);
            cx.notify();
            return;
        }

        self.switch_to_workspace(path, cx);
    }

    pub(crate) fn start_delete_collection_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.collection_context_menu {
            self.open_delete_collection_dialog(menu.collection_id, window, cx);
        }
    }

    pub(crate) fn start_delete_collection_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.collection_context_menu {
            self.open_delete_collection_dialog(menu.collection_id, window, cx);
        }
    }

    pub(crate) fn open_delete_collection_dialog(
        &mut self,
        collection_id: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(collection) = self
            .workspace
            .collections
            .iter()
            .find(|c| c.id == collection_id)
        else {
            return;
        };
        self.delete_collection_dialog = Some(CollectionDeleteDialog {
            collection_id,
            collection_name: collection.name.clone(),
        });
        self.collection_context_menu = None;
        cx.notify();
    }

    pub(crate) fn close_delete_collection_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_collection_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_delete_collection(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.delete_collection_dialog.take() else {
            return;
        };
        let collection_id = dialog.collection_id;

        let mut request_ids_to_close = Vec::new();
        if let Some(collection) = self
            .workspace
            .collections
            .iter()
            .find(|c| c.id == collection_id)
        {
            fn collect_ids(items: &[CollectionItem], ids: &mut Vec<usize>) {
                for item in items {
                    match item {
                        CollectionItem::Request(r) => ids.push(r.id),
                        CollectionItem::Folder { items, .. } => collect_ids(items, ids),
                    }
                }
            }
            collect_ids(&collection.items, &mut request_ids_to_close);
        }

        if let Some(deleted_collection) = self.workspace.remove_collection(collection_id) {
            let closed_name = deleted_collection.name.clone();

            // Clean up any watch for this collection
            self.watched_imports.retain(|w| w.collection_id != collection_id);

            for req_id in request_ids_to_close {
                self.open_tabs.retain(|id| *id != req_id);
                self.body_inputs.remove(&req_id);
                self.response_body_inputs
                    .retain(|(response_request_id, _), _| *response_request_id != req_id);
            }

            self.method_menu_open = false;
            self.clear_request_overlays();

            if self.workspace.request_count() == 0 || self.open_tabs.is_empty() {
                self.active_request_id = None;
                self.url_input
                    .update(cx, |input, cx| input.set_content("", cx));
                self.status_line =
                    format!("Deleted collection {closed_name}; no request selected.").into();
                self.persist_workspace();
                cx.notify();
                return;
            }

            if let Some(active_id) = self.active_request_id {
                if !self.open_tabs.contains(&active_id) {
                    self.active_request_id = self.open_tabs.first().copied();
                }
            } else {
                self.active_request_id = self.open_tabs.first().copied();
            }

            self.sync_url_input_to_active_request(cx);
            self.status_line = format!("Deleted collection {closed_name}.").into();
            self.persist_workspace();
        } else {
            self.status_line = "Collection no longer exists.".into();
        }
        cx.notify();
    }

    pub(crate) fn add_request_to_context_collection(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.collection_context_menu else {
            return;
        };
        self.add_request_to_collection(menu.collection_id, cx);
    }

    pub(crate) fn add_request_to_context_collection_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.collection_context_menu else {
            return;
        };
        self.add_request_to_collection(menu.collection_id, cx);
    }

    pub(crate) fn add_request_to_collection(
        &mut self,
        collection_id: usize,
        cx: &mut Context<Self>,
    ) {
        let request = self.new_request();
        let id = request.id;
        self.workspace
            .insert_request_in_collection(collection_id, request);
        self.collection_context_menu = None;
        self.active_panel = Panel::Params;
        self.open_tabs.push(id);
        self.active_request_id = Some(id);
        self.selected_collection_item = Some(CollectionSelection::Request {
            collection_id,
            item_id: id,
        });
        self.url_input
            .update(cx, |input, cx| input.set_content("", cx));
        self.status_line = "Created request in collection.".into();
        self.persist_workspace();
        cx.notify();
    }

    pub(crate) fn add_folder_to_context_collection(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.collection_context_menu else {
            return;
        };
        self.add_folder_to_collection(menu.collection_id, cx);
    }

    pub(crate) fn add_folder_to_context_collection_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.collection_context_menu else {
            return;
        };
        self.add_folder_to_collection(menu.collection_id, cx);
    }

    pub(crate) fn add_folder_to_collection(
        &mut self,
        collection_id: usize,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_item_id();
        let name = format!("New folder {id}");
        if let Some(collection) = self
            .workspace
            .collections
            .iter_mut()
            .find(|c| c.id == collection_id)
        {
            collection.items.push(CollectionItem::Folder {
                id,
                name: name.clone().into(),
                items: Vec::new(),
            });
            self.expanded_folders.insert(id);
            self.selected_collection_item = Some(CollectionSelection::Folder {
                collection_id,
                item_id: id,
            });
            self.status_line = "Created folder in collection.".into();
            self.persist_workspace();
        } else {
            self.status_line = "Collection no longer exists.".into();
        }
        self.collection_context_menu = None;
        cx.notify();
    }

    // ---------------------------------------------------------------
    // Import OpenAPI Spec
    // ---------------------------------------------------------------

    pub(crate) fn start_import_openapi_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(_menu) = self.collection_context_menu {
            self.open_import_openapi_dialog(window, cx);
        }
    }

    pub(crate) fn start_import_openapi_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(_menu) = self.collection_context_menu {
            self.open_import_openapi_dialog(window, cx);
        }
    }

    pub(crate) fn open_import_openapi_dialog(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            TextInput::new_with_selector(
                cx,
                SharedString::default(),
                "Path or URL to OpenAPI JSON/YAML spec",
                "import-openapi-input",
            )
        });
        let auth_token_input = cx.new(|cx| {
            TextInput::new_with_selector(cx, SharedString::default(), "token or {{token}}", "import-auth-token-input")
        });
        let auth_username_input = cx.new(|cx| {
            TextInput::new_with_selector(cx, SharedString::default(), "username", "import-auth-username-input")
        });
        let auth_password_input = cx.new(|cx| {
            TextInput::new_with_selector(cx, SharedString::default(), "password or {{password}}", "import-auth-password-input")
        });
        let auth_name_input = cx.new(|cx| {
            TextInput::new_with_selector(cx, SharedString::default(), "x-api-key", "import-auth-name-input")
        });
        let auth_value_input = cx.new(|cx| {
            TextInput::new_with_selector(cx, SharedString::default(), "value or {{value}}", "import-auth-value-input")
        });
        self.import_openapi_dialog = Some(ImportOpenApiDialog {
            input,
            auth_type: Auth::None,
            auth_menu_open: false,
            auth_token_input,
            auth_username_input,
            auth_password_input,
            auth_name_input,
            auth_value_input,
            watch_enabled: false,
        });
        self.collection_context_menu = None;
        cx.notify();
    }

    pub(crate) fn close_import_openapi_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.import_openapi_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_import_openapi(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.import_openapi_dialog.take() else {
            return;
        };
        let path = dialog.input.read(cx).value().to_string();
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() {
            self.status_line = "File path cannot be empty.".into();
            self.import_openapi_dialog = Some(dialog);
            cx.notify();
            return;
        }

        let is_url = trimmed.starts_with("http://") || trimmed.starts_with("https://");
        let (content, url_domain_auth) = if is_url {
            // Build domain auth from dialog's auth inputs
            let domain_auth = match &dialog.auth_type {
                Auth::None => domain::Auth::None,
                Auth::Bearer { .. } => domain::Auth::Bearer {
                    token_ref: dialog.auth_token_input.read(cx).value().to_string(),
                },
                Auth::Basic { .. } => domain::Auth::Basic {
                    username_ref: dialog.auth_username_input.read(cx).value().to_string(),
                    password_ref: dialog.auth_password_input.read(cx).value().to_string(),
                },
                Auth::ApiKey { location, .. } => domain::Auth::ApiKey {
                    name: dialog.auth_name_input.read(cx).value().to_string(),
                    value_ref: dialog.auth_value_input.read(cx).value().to_string(),
                    location: match location {
                        AuthLocation::Header => domain::ApiKeyLocation::Header,
                        AuthLocation::Query => domain::ApiKeyLocation::Query,
                        AuthLocation::Cookie => domain::ApiKeyLocation::Cookie,
                    },
                },
            };
            let mut fetch_req = domain::Request::new("", "", domain::Method::Get, &trimmed);
            fetch_req.auth = domain_auth.clone();
            apply_auth(&mut fetch_req);
            match domain::send_http_request(&fetch_req) {
                Ok(response) => {
                    if response.status >= 400 {
                        self.status_line = format!(
                            "Failed to fetch OpenAPI spec: HTTP {}",
                            response.status
                        )
                        .into();
                        self.import_openapi_dialog = Some(dialog);
                        cx.notify();
                        return;
                    }
                    (response.body, Some(domain_auth))
                }
                Err(e) => {
                    self.status_line = format!("Failed to fetch OpenAPI spec: {e}").into();
                    self.import_openapi_dialog = Some(dialog);
                    cx.notify();
                    return;
                }
            }
        } else {
            match std::fs::read_to_string(&trimmed) {
                Ok(content) => (content, None),
                Err(e) => {
                    self.status_line = format!("Could not read file: {e}").into();
                    self.import_openapi_dialog = Some(dialog);
                    cx.notify();
                    return;
                }
            }
        };

        match domain::openapi::parse_openapi_spec(&content) {
                Ok(domain_collection) => {
                    // Count requests recursively (matches the ID-generation count)
                    fn count_requests(items: &[domain::CollectionItem]) -> usize {
                        items
                            .iter()
                            .map(|item| match item {
                                domain::CollectionItem::Request(_) => 1,
                                domain::CollectionItem::Folder { items: ch, .. } => {
                                    count_requests(ch)
                                }
                            })
                            .sum()
                    }
                    let imported_count = count_requests(&domain_collection.items);
                    let spec_name = domain_collection.name.clone();

                    // Convert domain collection items to app collection items
                    let app_collection = Workspace::from_domain_collection(domain_collection);
                    let mut items = app_collection.items;

                    // Pre-generate enough IDs for all items (including nested)
                    fn count_items(items: &[CollectionItem]) -> usize {
                        items
                            .iter()
                            .map(|i| match i {
                                CollectionItem::Request(_) => 1,
                                CollectionItem::Folder { items: ch, .. } => 1 + count_items(ch),
                            })
                            .sum()
                    }
                    let new_ids: Vec<usize> = (0..count_items(&items))
                        .map(|_| self.next_item_id())
                        .collect();
                    let mut id_iter = new_ids.into_iter();
                    Self::reassign_item_ids(&mut items, &mut || id_iter.next().unwrap());

                    // Create a new collection named after the spec title
                    let new_collection_id = self.next_item_id();
                    self.workspace.collections.push(Collection {
                        id: new_collection_id,
                        name: spec_name.into(),
                        items,
                    });
                    self.status_line =
                        format!("Imported {imported_count} requests into new collection.").into();
                    self.persist_workspace();

                    if dialog.watch_enabled {
                        if let Some(auth) = url_domain_auth {
                            let body_hash = {
                                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                                content.hash(&mut hasher);
                                hasher.finish()
                            };
                            let source = ImportSource::Url {
                                url: trimmed,
                                auth,
                                body_hash,
                            };
                            self.start_watching_import(new_collection_id, source, cx);
                        } else {
                            let source = ImportSource::File(std::path::PathBuf::from(&trimmed));
                            self.start_watching_import(new_collection_id, source, cx);
                        }
                        self.status_line = format!(
                            "Imported {imported_count} requests into new collection. Watching for changes..."
                        )
                        .into();
                    }
                }
                Err(e) => {
                    self.status_line = format!("Import failed: {e}").into();
                }
            }
        cx.notify();
    }

    // Import OpenAPI Dialog — Auth handlers

    pub(crate) fn toggle_import_auth_menu(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.import_openapi_dialog.as_mut() {
            dialog.auth_menu_open = !dialog.auth_menu_open;
            cx.notify();
        }
    }

    pub(crate) fn set_import_auth_type(
        &mut self,
        auth: &Auth,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.import_openapi_dialog.as_mut() {
            dialog.auth_type = auth.clone();
            dialog.auth_menu_open = false;
            cx.notify();
        }
    }

    pub(crate) fn toggle_import_watch(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.import_openapi_dialog.as_mut() {
            dialog.watch_enabled = !dialog.watch_enabled;
            cx.notify();
        }
    }

    pub(crate) fn start_watching_import(
        &mut self,
        collection_id: usize,
        source: ImportSource,
        cx: &mut Context<Self>,
    ) {
        // Cancel any existing watch for this collection
        self.watched_imports.retain(|w| w.collection_id != collection_id);

        let file_modified = match &source {
            ImportSource::File(path) => std::fs::metadata(path).ok().and_then(|m| m.modified().ok()),
            ImportSource::Url { .. } => None,
        };

        let task = cx.spawn(async move |this, cx| {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                let _ = this.update(cx, |app, cx| {
                    app.reimport_watched_spec(collection_id, cx);
                });
            }
        });

        self.watched_imports.push(WatchedImport {
            collection_id,
            source,
            file_modified,
            _watch_task: task,
        });
    }

    pub(crate) fn reimport_watched_spec(
        &mut self,
        collection_id: usize,
        cx: &mut Context<Self>,
    ) {
        let watch_idx = match self
            .watched_imports
            .iter()
            .position(|w| w.collection_id == collection_id)
        {
            Some(idx) => idx,
            None => return,
        };

        let (content, is_changed) = match &self.watched_imports[watch_idx].source {
            ImportSource::File(path) => {
                let current_mtime = match std::fs::metadata(path).and_then(|m| m.modified()) {
                    Ok(t) => t,
                    Err(_) => return, // file gone temporarily — keep watching
                };
                let last = self.watched_imports[watch_idx].file_modified;
                if let Some(last) = last {
                    if current_mtime <= last {
                        return; // no change
                    }
                }
                match std::fs::read_to_string(path) {
                    Ok(c) => {
                        self.watched_imports[watch_idx].file_modified = Some(current_mtime);
                        (c, true)
                    }
                    Err(e) => {
                        self.status_line = format!("Watch re-import failed: {e}").into();
                        cx.notify();
                        return;
                    }
                }
            }
            ImportSource::Url {
                url,
                auth,
                body_hash: old_hash,
            } => {
                let mut req = domain::Request::new("", "", domain::Method::Get, url);
                req.auth = auth.clone();
                apply_auth(&mut req);
                match domain::send_http_request(&req) {
                    Ok(response) if response.status < 400 => {
                        let body = response.body;
                        let new_hash = {
                            let mut hasher = std::collections::hash_map::DefaultHasher::new();
                            body.hash(&mut hasher);
                            hasher.finish()
                        };
                        if new_hash == *old_hash {
                            return; // no change
                        }
                        if let ImportSource::Url { body_hash, .. } =
                            &mut self.watched_imports[watch_idx].source
                        {
                            *body_hash = new_hash;
                        }
                        (body, true)
                    }
                    Ok(response) => {
                        self.status_line =
                            format!("Watch re-import failed: HTTP {}", response.status).into();
                        cx.notify();
                        return;
                    }
                    Err(e) => {
                        self.status_line = format!("Watch re-import failed: {e}").into();
                        cx.notify();
                        return;
                    }
                }
            }
        };

        if !is_changed {
            return;
        }

        match domain::openapi::parse_openapi_spec(&content) {
            Ok(domain_collection) => {
                if let Some(collection) =
                    self.workspace.collections.iter_mut().find(|c| c.id == collection_id)
                {
                    let app_collection = Workspace::from_domain_collection(domain_collection);
                    collection.name = app_collection.name;
                    collection.items = app_collection.items;
                    self.status_line = "Re-imported watched spec.".into();
                    self.persist_workspace();
                    cx.notify();
                }
            }
            Err(e) => {
                self.status_line = format!("Watch re-import failed (invalid spec): {e}").into();
                cx.notify();
            }
        }
    }

    // ---------------------------------------------------------------
    // Export OpenAPI Spec
    // ---------------------------------------------------------------

    pub(crate) fn start_export_openapi_from_context_menu(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.collection_context_menu {
            self.open_export_openapi_dialog(menu.collection_id, window, cx);
        }
    }

    pub(crate) fn start_export_openapi_from_context_menu_mouse_down(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.collection_context_menu {
            self.open_export_openapi_dialog(menu.collection_id, window, cx);
        }
    }

    pub(crate) fn open_export_openapi_dialog(
        &mut self,
        collection_id: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            TextInput::new_with_selector(
                cx,
                SharedString::default(),
                "Output path for OpenAPI JSON",
                "export-openapi-input",
            )
        });
        self.export_openapi_dialog = Some(ExportOpenApiDialog {
            collection_id,
            input,
        });
        self.collection_context_menu = None;
        cx.notify();
    }

    pub(crate) fn close_export_openapi_dialog(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.export_openapi_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_export_openapi(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.export_openapi_dialog.take() else {
            return;
        };
        let path = dialog.input.read(cx).value().to_string();
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() {
            self.status_line = "File path cannot be empty.".into();
            self.export_openapi_dialog = Some(dialog);
            cx.notify();
            return;
        }

        // Export only the selected collection, not the entire workspace.
        let collection = self
            .workspace
            .collections
            .iter()
            .find(|c| c.id == dialog.collection_id);
        let Some(collection) = collection else {
            self.status_line = "Collection not found.".into();
            cx.notify();
            return;
        };
        let domain_collection = Workspace::to_domain_collection(collection);
        let domain_workspace = domain::Workspace {
            id: "local".to_string(),
            name: domain_collection.name.clone(),
            active_environment: String::new(),
            environments: vec![],
            items: vec![domain_collection],
            expanded_folders: vec![],
            expanded_collections: vec![],
        };

        match domain::export_openapi::workspace_to_openapi_spec(&domain_workspace) {
            Ok(json) => match std::fs::write(&trimmed, &json) {
                Ok(_) => {
                    self.status_line = format!("Exported OpenAPI spec to {trimmed}.").into();
                }
                Err(e) => {
                    self.status_line = format!("Could not write file: {e}").into();
                }
            },
            Err(e) => {
                self.status_line = format!("Export failed: {e}").into();
            }
        }
        cx.notify();
    }

    /// Reassign unique IDs to imported collection items recursively.
    fn reassign_item_ids(items: &mut [CollectionItem], id_gen: &mut impl FnMut() -> usize) {
        for item in items.iter_mut() {
            match item {
                CollectionItem::Request(req) => {
                    req.id = id_gen();
                }
                CollectionItem::Folder { id, items, .. } => {
                    *id = id_gen();
                    Self::reassign_item_ids(items, id_gen);
                }
            }
        }
    }
}
