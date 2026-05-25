use super::*;

impl ApiClientApp {
    pub(crate) fn send_request(&mut self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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

        // Clear previous response so the UI shows a clean loading state.
        if let Some(request) = self.active_request_mut() {
            request.response = None;
        }
        self.response_body_inputs.retain(|(id, _), _| *id != request_id);

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_for_task = cancel.clone();

        self.request_started_at = Some(std::time::Instant::now());
        self._request_timer = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let result = this.update(cx, |_, cx| cx.notify());
                if result.is_err() {
                    break;
                }
            }
        }));

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
                    app.request_started_at = None;
                    app._request_timer = None;
                    app.status_line = format!("Cancelled {request_name}.").into();
                    cx.notify();
                });
                return;
            }
            let _ = this.update(cx, |app, cx| {
                app.in_flight_requests.remove(&request_id);
                app.request_started_at = None;
                app._request_timer = None;
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

    pub(crate) fn send_focused_request(
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

    pub(crate) fn submit_dialog_from_action(
        &mut self,
        _: &SubmitDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.submit_active_dialog(window, cx);
    }

    pub(crate) fn cancel_dialog_from_action(
        &mut self,
        _: &CancelDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.cancel_active_dialog(cx) {
            window.focus(&self.focus_handle);
        }
    }

    pub(crate) fn dismiss_dialog_on_backdrop(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_active_dialog(cx);
    }

    pub(crate) fn submit_active_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
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

    pub(crate) fn delete_collection_selection_from_action(
        &mut self,
        _: &DeleteCollectionSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_delete_dialog_for_selected_collection_item(window, cx);
    }

    pub(crate) fn rename_collection_selection_from_action(
        &mut self,
        _: &RenameCollectionSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_rename_dialog_for_selected_collection_item(window, cx);
    }

    pub(crate) fn open_delete_dialog_for_selected_collection_item(
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

    pub(crate) fn open_rename_dialog_for_selected_collection_item(
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

    pub(crate) fn cancel_active_dialog(&mut self, cx: &mut Context<Self>) -> bool {
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

    pub(crate) fn send_active_request(&mut self, cx: &mut Context<Self>) {
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
        // Clear previous response before sending.
        if let Some(request) = self.active_request_mut() {
            request.response = None;
        }

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

    pub(crate) fn cancel_request(
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
            self.request_started_at = None;
            self._request_timer = None;
            self.status_line = "Cancelling request…".into();
            cx.notify();
        }
    }

    pub(crate) fn cancel_active_request(
        &mut self,
        event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.active_request_id {
            self.cancel_request(id, event, window, cx);
        }
    }

    pub(crate) fn is_active_request_in_flight(&self) -> bool {
        self.active_request_id
            .map(|id| self.in_flight_requests.contains_key(&id))
            .unwrap_or(false)
    }
}
