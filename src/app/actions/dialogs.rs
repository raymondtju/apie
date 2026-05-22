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

    pub(crate) fn open_folder_context_menu(
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

    pub(crate) fn dismiss_request_context_menu(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_context_menu = None;
        self.folder_context_menu = None;
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
        self.selected_collection_item = self.active_request_id.map(CollectionSelection::Request);
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
}
