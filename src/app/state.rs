use super::*;

impl ApiClientApp {
    pub(crate) fn theme_mode_from_settings(settings: AppSettings, cx: &Context<Self>) -> ThemeMode {
        match settings.theme_preference {
            ThemePreference::System => ThemeMode::from_window_appearance(cx.window_appearance()),
            ThemePreference::ZedDark => ThemeMode::ZedDark,
            ThemePreference::ZedLight => ThemeMode::ZedLight,
        }
    }

    pub(crate) fn active_request(&self) -> Option<&Request> {
        self.active_request_id
            .and_then(|request_id| self.workspace.request_by_id(request_id))
    }

    pub(crate) fn active_request_mut(&mut self) -> Option<&mut Request> {
        let request_id = self.active_request_id?;
        self.workspace.request_mut_by_id(request_id)
    }

    pub(crate) fn sync_url_input_to_active_request(&mut self, cx: &mut Context<Self>) {
        let url = self
            .active_request()
            .map(|request| request.url.clone())
            .unwrap_or_else(|| "".into());
        self.url_input
            .update(cx, |input, cx| input.set_content(url, cx));
    }

    pub(crate) fn field_input(
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

    pub(crate) fn body_input(
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

    pub(crate) fn response_body_input(
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

    pub(crate) fn syntax_colors_for_theme(theme: AppTheme) -> SyntaxColors {
        SyntaxColors {
            property: theme.accent,
            string: theme.success,
            number: theme.warning,
            keyword: theme.error,
            punctuation: theme.text_muted,
        }
    }

    pub(crate) fn set_active_request_body(&mut self, body: impl Into<SharedString>, cx: &mut Context<Self>) {
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

    pub(crate) fn sync_active_request_inputs(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn sync_environment_inputs(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn typography(&self) -> Typography {
        Typography::new(self.settings.ui_font_size, self.settings.buffer_font_size)
    }

    pub(crate) fn persist_settings(&mut self) {
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

    pub(crate) fn persist_workspace(&mut self) {
        self.workspace.expanded_folders = self.expanded_folders.clone();
        let workspace_path = self.workspace_path.clone();
        let snapshot = self.workspace.to_domain();
        std::thread::spawn(move || {
            if let Err(error) = domain::save_workspace(&workspace_path, &snapshot) {
                eprintln!("Could not save workspace: {error}");
            }
        });
    }
}
