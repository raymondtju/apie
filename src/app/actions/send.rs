use super::*;

impl ApiClientApp {
    pub(crate) fn send_request(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_request_impl(cx);
    }

    fn send_request_impl(&mut self, cx: &mut Context<Self>) {
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
        let resolved = match request.to_resolved_domain(environment, Some(&*self.secret_store)) {
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
        self.response_body_inputs
            .retain(|(id, _), _| *id != request_id);

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
            self.send_request_impl(cx);
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

    pub(crate) fn submit_active_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.rename_request_dialog.is_some() {
            self.confirm_rename_request(&gpui::ClickEvent::default(), window, cx);
            return true;
        }
        if self.rename_folder_dialog.is_some() {
            self.confirm_rename_folder(&gpui::ClickEvent::default(), window, cx);
            return true;
        }
        if self.rename_collection_dialog.is_some() {
            self.confirm_rename_collection(&gpui::ClickEvent::default(), window, cx);
            return true;
        }
        if self.create_workspace_dialog.is_some() {
            self.confirm_create_workspace(&gpui::ClickEvent::default(), window, cx);
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
        if self.delete_collection_dialog.is_some() {
            self.confirm_delete_collection(&gpui::ClickEvent::default(), window, cx);
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

    pub(crate) fn new_tab_from_action(
        &mut self,
        _: &NewTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub(crate) fn close_tab_from_action(
        &mut self,
        _: &CloseTab,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(request_id) = self.active_request_id {
            self.close_request_tab(request_id, cx);
        }
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
            Some(CollectionSelection::Request {
                collection_id: _,
                item_id: request_id,
            }) if self.workspace.request_by_id(request_id).is_some() => {
                self.open_delete_request_dialog(request_id, window, cx);
                true
            }
            Some(CollectionSelection::Folder {
                collection_id: _,
                item_id: folder_id,
            }) if self.workspace.folder_name(folder_id).is_some() => {
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
            || self.rename_collection_dialog.is_some()
            || self.delete_request_dialog.is_some()
            || self.delete_folder_dialog.is_some()
            || self.delete_collection_dialog.is_some()
            || self.create_workspace_dialog.is_some()
            || self.settings_dialog_open
        {
            return false;
        }

        match self.selected_collection_item {
            Some(CollectionSelection::Request {
                collection_id: _,
                item_id: request_id,
            }) if self.workspace.request_by_id(request_id).is_some() => {
                self.open_rename_request_dialog(request_id, window, cx);
                true
            }
            Some(CollectionSelection::Folder {
                collection_id: _,
                item_id: folder_id,
            }) if self.workspace.folder_name(folder_id).is_some() => {
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
            || self.rename_collection_dialog.take().is_some()
            || self.delete_request_dialog.take().is_some()
            || self.delete_folder_dialog.take().is_some()
            || self.delete_collection_dialog.take().is_some()
            || self.create_workspace_dialog.take().is_some()
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

    #[allow(dead_code)]
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
        let request = match request.to_resolved_domain(environment, Some(&*self.secret_store)) {
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

    pub(crate) fn start_stream(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(request_id) = self.active_request_id else {
            return;
        };
        let Some(request) = self.workspace.request_by_id(request_id) else {
            return;
        };
        let url = request.url.to_string();
        let is_websocket = url.starts_with("ws://") || url.starts_with("wss://");
        if !url.starts_with("http://") && !url.starts_with("https://") && !is_websocket {
            self.status_line = "Invalid URL for streaming".into();
            cx.notify();
            return;
        }

        // Enforce a global cap on concurrent streams.
        let active_count = self
            .stream_state
            .values()
            .filter(|s| s.status == StreamStatus::Connected || s.status == StreamStatus::Connecting)
            .count();
        let already_active = self.stream_state.get(&request_id).is_some_and(|s| {
            s.status == StreamStatus::Connected || s.status == StreamStatus::Connecting
        });
        if !already_active && active_count >= MAX_CONCURRENT_STREAMS {
            self.status_line = format!("Max {MAX_CONCURRENT_STREAMS} concurrent streams").into();
            cx.notify();
            return;
        }

        // Cancel any existing stream for this request.
        if let Some(cancel) = self.stream_cancel.remove(&request_id) {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if let Some(_task) = self.stream_tasks.remove(&request_id) {
            // Task will be dropped and cancelled
        }

        let stream_state = self
            .stream_state
            .entry(request_id)
            .or_insert_with(StreamState::new);
        if stream_state.status == StreamStatus::Connected
            || stream_state.status == StreamStatus::Connecting
        {
            return;
        }
        stream_state.status = StreamStatus::Connecting;
        stream_state.messages.clear();
        self.stream_expanded_messages
            .retain(|(req_id, _)| *req_id != request_id);
        self.stream_list_state.reset(0);
        stream_state.error = None;
        stream_state.started_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        );
        stream_state.last_received_at = None;

        let (message_tx, mut message_rx) =
            tokio::sync::mpsc::channel::<domain::StreamMessage>(1000);
        let cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.stream_cancel.insert(request_id, cancel_token.clone());

        let stream_task = cx.spawn(async move |this, cx| {
            let connect_result = cx
                .background_executor()
                .spawn(async move {
                    if is_websocket {
                        domain::connect_ws(url, message_tx, cancel_token)
                    } else {
                        domain::connect_sse(url, message_tx, cancel_token)
                    }
                })
                .await;

            if let Err(e) = connect_result {
                let _ = this.update(cx, |app, cx| {
                    if let Some(state) = app.stream_state.get_mut(&request_id) {
                        state.status = StreamStatus::Disconnected;
                        state.error = Some(e.to_string().into());
                    }
                    app.status_line = format!("Stream error: {e}").into();
                    cx.notify();
                });
            } else {
                let _ = this.update(cx, |app, cx| {
                    if let Some(state) = app.stream_state.get_mut(&request_id) {
                        state.status = StreamStatus::Disconnected;
                    }
                    app.status_line = "Stream disconnected".into();
                    cx.notify();
                });
            }
        });
        self.stream_tasks.insert(request_id, stream_task);

        // Process incoming messages in batches.
        // Wait for the first message, then drain all accumulated messages
        // and trigger a single cx.notify() per batch. This caps re-renders
        // at ~60fps even when SSE events arrive much faster.
        // Remove any existing message task before spawning a new one (dropping the task cancels it).
        self.stream_msg_tasks.remove(&request_id);
        let msg_task = cx.spawn(async move |this, cx| {
            loop {
                // Block until at least one message arrives.
                let first = match message_rx.recv().await {
                    Some(msg) => msg,
                    None => break, // Channel closed.
                };

                // Drain any additional messages that arrived in the meantime.
                let mut batch = vec![first];
                while let Ok(msg) = message_rx.try_recv() {
                    batch.push(msg);
                }

                let _ = this.update(cx, |app, cx| {
                    if let Some(state) = app.stream_state.get_mut(&request_id) {
                        if state.status == StreamStatus::Connecting {
                            state.status = StreamStatus::Connected;
                        }
                        let old_count = state.messages.len();
                        let mut count = 0usize;
                        let should_scroll = state.pin_to_bottom;
                        for msg in batch {
                            let app_msg = StreamMessage::from_domain(msg);
                            // Skip the zero-data handshake message from connect_sse.
                            if !app_msg.data.is_empty() {
                                state.last_received_at = Some(app_msg.received_at);
                                state.messages.push(app_msg);
                                count += 1;
                            }
                        }
                        if count > 0 {
                            app.stream_list_state.splice(old_count..old_count, count);
                            if should_scroll {
                                let last = state.messages.len().saturating_sub(1);
                                app.stream_list_state.scroll_to(gpui::ListOffset {
                                    item_ix: last,
                                    offset_in_item: gpui::Pixels::ZERO,
                                });
                            }
                        }
                        // Evict oldest messages when the cap is exceeded.
                        let len = state.messages.len();
                        if len > MAX_STREAM_MESSAGES {
                            let excess = len - MAX_STREAM_MESSAGES;
                            state.messages.drain(0..excess);
                            // Shift expanded-message indices down and drop stale entries.
                            let old = std::mem::take(&mut app.stream_expanded_messages);
                            for (req_id, idx) in old {
                                if req_id == request_id {
                                    if idx >= excess {
                                        app.stream_expanded_messages.insert((req_id, idx - excess));
                                    }
                                } else {
                                    app.stream_expanded_messages.insert((req_id, idx));
                                }
                            }
                            app.stream_list_state.reset(state.messages.len());
                            if should_scroll {
                                let last = state.messages.len().saturating_sub(1);
                                app.stream_list_state.scroll_to(gpui::ListOffset {
                                    item_ix: last,
                                    offset_in_item: gpui::Pixels::ZERO,
                                });
                            }
                        }
                        if count > 0 {
                            cx.notify();
                        }
                    }
                });
            }
        });
        self.stream_msg_tasks.insert(request_id, msg_task);

        self.status_line = "Connecting to stream…".into();
        cx.notify();
    }

    pub(crate) fn stop_stream(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(request_id) = self.active_request_id else {
            return;
        };
        if let Some(cancel) = self.stream_cancel.remove(&request_id) {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if let Some(_task) = self.stream_tasks.remove(&request_id) {
            // Task will be dropped and cancelled
        }
        if let Some(state) = self.stream_state.get_mut(&request_id) {
            state.status = StreamStatus::Disconnected;
            state.started_at = None;
            state.last_received_at = None;
        }
        self.status_line = "Stream stopped".into();
        cx.notify();
    }

    /// Save all stream messages for a request to a plain-text file.
    pub(crate) fn save_stream_session(&mut self, request_id: usize, cx: &mut Context<Self>) {
        let Some(state) = self.stream_state.get(&request_id) else {
            self.status_line = "No stream session to save.".into();
            cx.notify();
            return;
        };
        if state.messages.is_empty() {
            self.status_line = "No messages to save.".into();
            cx.notify();
            return;
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let filename = format!("stream-session-{request_id}-{timestamp}.txt");

        let mut output = String::new();
        for (i, msg) in state.messages.iter().enumerate() {
            let dir = match msg.direction {
                domain::StreamDirection::Sent => "SENT",
                domain::StreamDirection::Received => "RECV",
            };
            let ts = format_wall_clock_time(msg.received_at)
                .map(|s| s.to_string())
                .unwrap_or_else(|| "--:--:--".to_string());
            output.push_str(&format!(
                "[#{}] {} {} ({} bytes)\n",
                i + 1,
                ts,
                dir,
                msg.size_bytes
            ));
            if let Some(event_type) = &msg.event_type {
                output.push_str(&format!("  event: {event_type}\n"));
            }
            output.push_str(&format!("  {}\n\n", msg.data));
        }

        match std::fs::write(&filename, &output) {
            Ok(_) => {
                self.status_line = format!("Stream session saved to {filename}").into();
            }
            Err(e) => {
                self.status_line = format!("Failed to save stream session: {e}").into();
            }
        }
        cx.notify();
    }
}

fn format_wall_clock_time(timestamp_ms: u64) -> Option<String> {
    let secs = timestamp_ms / 1000;
    if secs > 0 {
        let hours = (secs / 3600) % 24;
        let minutes = (secs / 60) % 60;
        let seconds = secs % 60;
        Some(format!("{:02}:{:02}:{:02}", hours, minutes, seconds))
    } else {
        None
    }
}
