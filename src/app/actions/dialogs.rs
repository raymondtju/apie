use super::*;

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
}
