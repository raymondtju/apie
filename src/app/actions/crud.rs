use super::*;

impl ApiClientApp {
    pub(crate) fn request_index_by_id(&self, request_id: usize) -> Option<usize> {
        self.open_tabs.iter().position(|id| *id == request_id)
    }

    pub(crate) fn active_request_id(&self) -> Option<usize> {
        self.active_request_id
    }

    pub(crate) fn clear_request_overlays(&mut self) {
        self.request_context_menu = None;
        self.folder_context_menu = None;
        self.rename_request_dialog = None;
        self.rename_folder_dialog = None;
        self.delete_request_dialog = None;
        self.delete_folder_dialog = None;
    }

    pub(crate) fn select_request_by_id(&mut self, request_id: usize, cx: &mut Context<Self>) {
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

    pub(crate) fn select_collection_request_by_id(
        &mut self,
        request_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.select_request_by_id(request_id, cx);
    }

    pub(crate) fn select_collection_folder(
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
    pub(crate) fn select_request(&mut self, index: usize, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(request_id) = self
            .workspace
            .request_by_index(index)
            .map(|request| request.id)
        {
            self.select_request_by_id(request_id, cx);
        }
    }

    pub(crate) fn set_panel(&mut self, panel: Panel, _: &mut Window, cx: &mut Context<Self>) {
        self.active_panel = panel;
        self.request_context_menu = None;
        cx.notify();
    }

    pub(crate) fn focus_url_input(
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

    pub(crate) fn move_open_tab_to_drop_index(&mut self, request_id: usize, drop_index: usize) -> bool {
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

    pub(crate) fn drop_request_tab_on_tab(
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

    pub(crate) fn drop_request_tab_at_end(
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

    pub(crate) fn drop_collection_before_item(
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

    pub(crate) fn drop_collection_into_folder(
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

    pub(crate) fn drop_collection_at_root_end(
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

    pub(crate) fn select_request_from_tab_click(
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

    pub(crate) fn toggle_left_dock(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.left_dock_open = !self.left_dock_open;
        self.status_line = if self.left_dock_open {
            "Opened collection dock."
        } else {
            "Closed collection dock."
        }
        .into();
        cx.notify();
    }

    pub(crate) fn toggle_right_dock(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.right_dock_open = !self.right_dock_open;
        self.status_line = if self.right_dock_open {
            "Opened environment dock."
        } else {
            "Closed environment dock."
        }
        .into();
        cx.notify();
    }

    pub(crate) fn clamp_pixels(value: Pixels, min: Pixels, max: Pixels) -> Pixels {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }

    pub(crate) fn start_collection_resize(
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

    pub(crate) fn start_response_resize(
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

    pub(crate) fn update_pane_resize(
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

    pub(crate) fn finish_pane_resize(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_pane_resize.is_some() {
            self.active_pane_resize = None;
            cx.notify();
        }
    }

    pub(crate) fn next_item_id(&mut self) -> usize {
        let id = self.next_item_id;
        self.next_item_id += 1;
        id
    }

    pub(crate) fn new_request(&mut self) -> Request {
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

    pub(crate) fn add_request(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn add_root_folder(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn add_request_to_context_folder(
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

    pub(crate) fn add_request_to_context_folder_mouse_down(
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

    pub(crate) fn add_request_to_folder(&mut self, folder_id: usize, cx: &mut Context<Self>) {
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

    pub(crate) fn add_folder_to_context_folder(
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

    pub(crate) fn add_folder_to_context_folder_mouse_down(
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

    pub(crate) fn add_folder_to_folder(&mut self, folder_id: usize, cx: &mut Context<Self>) {
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
    pub(crate) fn add_param_row(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            request.query.push(Header::new("", ""));
            self.status_line = "Added query parameter row.".into();
            self.persist_workspace();
        }
        cx.notify();
    }

    pub(crate) fn toggle_param_enabled(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if let Some(param) = request.query.get_mut(index) {
                param.enabled = !param.enabled;
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    pub(crate) fn remove_param_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if request.query.len() > 1 && index < request.query.len() {
                request.query.remove(index);
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    pub(crate) fn add_header_row(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            request.headers.push(Header::new("", ""));
            self.status_line = "Added header row.".into();
            self.persist_workspace();
        }
        cx.notify();
    }

    pub(crate) fn toggle_header_enabled(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if let Some(header) = request.headers.get_mut(index) {
                header.enabled = !header.enabled;
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    pub(crate) fn remove_header_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(request) = self.active_request_mut() {
            if request.headers.len() > 1 && index < request.headers.len() {
                request.headers.remove(index);
                self.persist_workspace();
            }
        }
        cx.notify();
    }

    pub(crate) fn ensure_param_row(&mut self) {
        if let Some(request) = self.active_request_mut() {
            if request.query.is_empty() {
                request.query.push(Header::new("", ""));
            }
        }
    }

    pub(crate) fn ensure_header_row(&mut self) {
        if let Some(request) = self.active_request_mut() {
            if request.headers.is_empty() {
                request.headers.push(Header::new("", ""));
            }
        }
    }

    pub(crate) fn cycle_auth(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn cycle_api_key_location(
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

    pub(crate) fn format_body_json(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn cycle_environment(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn add_environment_variable(
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

    pub(crate) fn set_response_panel(
        &mut self,
        panel: ResponsePanel,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_response_panel = panel;
        self.body_view_menu_open = false;
        cx.notify();
    }

    pub(crate) fn set_body_view_mode(
        &mut self,
        mode: BodyViewMode,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_mode = mode;
        self.body_view_menu_open = false;
        cx.notify();
    }

    pub(crate) fn set_body_view_mode_from_mouse_down(
        &mut self,
        mode: BodyViewMode,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_mode = mode;
        self.body_view_menu_open = false;
        cx.notify();
    }

    pub(crate) fn toggle_body_view_menu(
        &mut self,
        _event: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_menu_open = !self.body_view_menu_open;
        cx.notify();
    }

    pub(crate) fn dismiss_body_view_menu(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.body_view_menu_open = false;
        cx.notify();
    }

    pub(crate) fn dismiss_response_meta_popover(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.response_meta_popover = false;
        cx.notify();
    }

    pub(crate) fn copy_response_header_value(
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

    pub(crate) fn copy_response_header_value_on_mouse_down(
        &mut self,
        value: SharedString,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
        self.status_line = "Copied response header value.".into();
        cx.notify();
    }
}
