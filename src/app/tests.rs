use super::*;
use crate::ui::{bind_code_input_keys, bind_text_input_keys};
use gpui::{ClickEvent, Modifiers, ScrollWheelEvent, TestAppContext, point};
use std::collections::HashSet;
use std::io::{Read, Write};

#[test]
fn resolves_environment_params_headers_auth_and_body_before_send() {
    let environment = Environment {
        name: "Local".into(),
        variables: vec![
            Header::new("base_url", "https://api.example.test"),
            Header::new("token", "abc"),
        ],
    };
    let request = Request {
        id: 1,
        name: "Resolved".into(),
        method: Method::Post,
        url: "{{base_url}}/users".into(),
        query: vec![Header::new("q", "{{token}}")],
        proxy_url: None,
        path_params: vec![],
        auth: Auth::ApiKey {
            name: "x-api-key".into(),
            secret_ref: "{{token}}".into(),
            location: AuthLocation::Header,
        },
        headers: vec![Header::new("Accept", "application/json")],
        content_type: "application/json".into(),
        body: "{\"token\":\"{{token}}\"}".into(),
        response: None,
        history: vec![],
        response_pinned: false,
    };

    let resolved = request.to_resolved_domain(&environment).unwrap();

    assert_eq!(resolved.url, "https://api.example.test/users");
    assert_eq!(resolved.query[0].value, "abc");
    assert!(
        resolved
            .headers
            .iter()
            .any(|header| header.name == "x-api-key" && header.value == "abc")
    );
    assert!(matches!(
        resolved.body,
        domain::Body::Raw { ref value, .. } if value == "{\"token\":\"abc\"}"
    ));
}

#[test]
fn body_is_only_emitted_for_body_methods() {
    let request = Request {
        id: 1,
        name: "Hidden body".into(),
        method: Method::Get,
        url: "https://api.example.test/users".into(),
        query: vec![],
        proxy_url: None,
        path_params: vec![],
        auth: Auth::None,
        headers: vec![],
        content_type: "application/json".into(),
        body: "{\"stale\":true}".into(),
        response: None,
        history: vec![],
        response_pinned: false,
    };

    assert!(matches!(request.to_domain().body, domain::Body::Empty));

    let mut post_request = request.clone();
    post_request.method = Method::Post;
    assert!(matches!(
        post_request.to_domain().body,
        domain::Body::Raw { ref value, .. } if value == "{\"stale\":true}"
    ));
}

#[test]
fn blank_editable_rows_are_not_sent() {
    let request = Request {
        id: 1,
        name: "Blank rows".into(),
        method: Method::Get,
        url: "https://api.example.test/users".into(),
        query: vec![Header::new("", ""), Header::new("page", "1")],
        proxy_url: None,
        path_params: vec![],
        auth: Auth::None,
        headers: vec![
            Header::new("", ""),
            Header::new("Accept", "application/json"),
        ],
        content_type: "application/json".into(),
        body: "".into(),
        response: None,
        history: vec![],
        response_pinned: false,
    };

    let domain_request = request.to_domain();

    assert_eq!(domain_request.query.len(), 1);
    assert_eq!(domain_request.query[0].name, "page");
    assert_eq!(domain_request.headers.len(), 1);
    assert_eq!(domain_request.headers[0].name, "Accept");
}

#[test]
fn workspace_normalizes_duplicate_and_invalid_request_ids() {
    let workspace = domain::Workspace {
        id: "local".to_string(),
        name: "Workspace".to_string(),
        active_environment: "dev".to_string(),
        environments: vec![domain::Environment {
            name: "dev".to_string(),
            variables: vec![],
        }],
        expanded_folders: Vec::new(),
        expanded_collections: Vec::new(),
        items: vec![domain::Collection {
            id: "0".to_string(),
            name: "Default".into(),
            items: vec![
                domain::CollectionItem::Request(domain::Request::new(
                    "alpha",
                    "Request A",
                    domain::Method::Get,
                    "",
                )),
                domain::CollectionItem::Request(domain::Request::new(
                    "1",
                    "Request B",
                    domain::Method::Get,
                    "",
                )),
                domain::CollectionItem::Request(domain::Request::new(
                    "1",
                    "Request C",
                    domain::Method::Get,
                    "",
                )),
            ],
        }],
    };

    let mut workspace = Workspace::from_domain(workspace, "test".into());
    let next_id = workspace.normalize_request_ids();
    let ids = workspace.request_ids();

    assert_eq!(ids.len(), 3);
    assert!(ids.iter().all(|id| *id > 0));
    assert_eq!(HashSet::<usize>::from_iter(ids.iter().copied()).len(), 3);
    assert_eq!(next_id, ids.iter().copied().max().unwrap() + 1);
}

#[test]
fn workspace_preserves_folder_tree_round_trip() {
    let workspace = domain::Workspace {
        id: "local".to_string(),
        name: "Workspace".to_string(),
        active_environment: "dev".to_string(),
        environments: vec![domain::Environment {
            name: "dev".to_string(),
            variables: vec![],
        }],
        expanded_folders: vec!["10".to_string()],
        expanded_collections: Vec::new(),
        items: vec![domain::Collection {
            id: "0".to_string(),
            name: "Default".into(),
            items: vec![domain::CollectionItem::Folder {
                id: "10".to_string(),
                name: "Users".to_string(),
                items: vec![domain::CollectionItem::Request(domain::Request::new(
                    "11",
                    "List users",
                    domain::Method::Get,
                    "/users",
                ))],
            }],
        }],
    };

    let mut workspace = Workspace::from_domain(workspace, "test".into());
    workspace.normalize_request_ids();
    let domain_workspace = workspace.to_domain();

    assert_eq!(domain_workspace.expanded_folders, vec!["10".to_string()]);
    match &domain_workspace.items[0].items[0] {
        domain::CollectionItem::Folder { id, name, items } => {
            assert_eq!(id, "10");
            assert_eq!(name, "Users");
            assert_eq!(items.len(), 1);
        }
        domain::CollectionItem::Request(_) => panic!("folder should round trip"),
    }
}

fn request_ids(workspace: &Workspace) -> Vec<usize> {
    workspace.request_ids()
}

fn test_request(id: usize) -> Request {
    Request {
        id,
        name: format!("Request {id}").into(),
        method: Method::Get,
        url: "".into(),
        query: vec![],
        proxy_url: None,
        path_params: vec![],
        auth: Auth::None,
        headers: vec![],
        content_type: "application/json".into(),
        body: "".into(),
        response: None,
        history: vec![],
        response_pinned: false,
    }
}

#[test]
fn workspace_moves_collection_request_before_target() {
    let mut workspace = Workspace::sample();
    workspace.insert_root_request(test_request(1));
    workspace.insert_root_request(test_request(2));
    workspace.insert_root_request(test_request(3));

    assert!(workspace.move_item_before(3, 1));
    assert_eq!(workspace.request_ids(), vec![3, 1, 2]);
}

#[test]
fn workspace_moves_collection_request_into_folder() {
    let mut workspace = Workspace::sample();
    workspace.insert_root_folder(10, "Folder");
    workspace.insert_root_request(test_request(1));

    assert!(workspace.move_item_into_folder(1, 10));

    let domain = workspace.to_domain();
    match &domain.items[0].items[0] {
        domain::CollectionItem::Folder { items, .. } => {
            assert_eq!(items.len(), 1);
            assert!(matches!(
                &items[0],
                domain::CollectionItem::Request(request) if request.id == "1"
            ));
        }
        domain::CollectionItem::Request(_) => panic!("request should move into folder"),
    }
}

#[test]
fn workspace_prevents_moving_folder_into_own_descendant() {
    let mut workspace = Workspace::sample();
    workspace.insert_root_folder(1, "Parent");
    assert!(workspace.insert_folder_in_folder(1, 2, "Child"));

    assert!(!workspace.move_item_into_folder(1, 2));
    assert_eq!(workspace.to_domain().items.len(), 1);
}

#[test]
fn workspace_moves_nested_collection_item_to_root_end() {
    let mut workspace = Workspace::sample();
    workspace.insert_root_folder(10, "Folder");
    workspace.insert_root_request(test_request(1));
    assert!(workspace.insert_request_in_folder(10, test_request(2)));

    assert!(workspace.move_item_to_root_end(2));
    assert_eq!(workspace.request_ids(), vec![1, 2]);
    let domain = workspace.to_domain();
    match &domain.items[0].items[0] {
        domain::CollectionItem::Folder { items, .. } => assert!(items.is_empty()),
        domain::CollectionItem::Request(_) => panic!("folder should remain first"),
    }
}

#[test]
fn formats_response_body_modes() {
    let raw = "{\"ok\":true}";

    assert_eq!(response_body_for_mode(raw, BodyViewMode::Raw), raw);
    assert!(response_body_for_mode(raw, BodyViewMode::Pretty).contains("\"ok\": true"));
}

#[test]
fn formats_request_body_json_when_valid() {
    let formatted = format_json_body("{\"ok\":true,\"count\":2}").expect("json should format");
    assert!(formatted.contains("\"ok\": true"));
    assert!(formatted.contains("\"count\": 2"));
    assert!(formatted.contains('\n'));
}

#[test]
fn format_request_body_json_reports_error_when_invalid() {
    let error = format_json_body("{not json").expect_err("invalid json should fail");
    assert!(!error.trim().is_empty());
}

#[gpui::test]
fn zed_shell_visual_smoke(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, _| window.activate_window());
    cx.debug_bounds("window-control-minimize")
        .expect("minimize window control should render");
    cx.debug_bounds("window-control-maximize")
        .expect("maximize window control should render");
    cx.debug_bounds("window-control-close")
        .expect("close window control should render");
    cx.debug_bounds("window-drag-region")
        .expect("window drag region should render");
    cx.debug_bounds("bottom-bar")
        .expect("bottom bar should render");
    cx.debug_bounds("bottom-toggle-collections")
        .expect("collection bottom toggle should render");
    cx.debug_bounds("bottom-open-settings")
        .expect("settings bottom button should render");
    cx.debug_bounds("bottom-toggle-environment")
        .expect("environment bottom toggle should render");
    cx.debug_bounds("window-resize-top")
        .expect("top resize hitbox should render");
    cx.debug_bounds("window-resize-bottom")
        .expect("bottom resize hitbox should render");
    cx.debug_bounds("window-resize-left")
        .expect("left resize hitbox should render");
    cx.debug_bounds("window-resize-right")
        .expect("right resize hitbox should render");

    app.read_with(cx, |app, cx| {
        assert_eq!(app.workspace.request_count(), 0);
        assert_eq!(app.url_input.read(cx).value(), "");
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });
    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.request_count(), 1);
        assert_eq!(app.active_request_id, Some(1));
        assert_eq!(app.open_tabs, vec![1]);
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.query.len(), 1);
        assert_eq!(request.query[0].name, "");
        assert_eq!(request.query[0].value, "");
        assert_eq!(app.active_panel, Panel::Params);
    });

    let url_shell_bounds = cx
        .debug_bounds("url-input-shell")
        .expect("URL input shell should render with a debug selector");
    cx.simulate_click(url_shell_bounds.center(), Modifiers::default());

    cx.debug_bounds("url-text-input")
        .expect("URL input should render with a debug selector");
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("https://smoke.test/zed");
    app.read_with(cx, |app, cx| {
        assert_eq!(app.url_input.read(cx).value(), "https://smoke.test/zed");
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.toggle_left_dock(&ClickEvent::default(), window, cx);
            app.toggle_right_dock(&ClickEvent::default(), window, cx);
            app.open_settings_dialog(&ClickEvent::default(), window, cx);
            app.increase_draft_ui_font(&ClickEvent::default(), window, cx);
            app.decrease_draft_buffer_font(&ClickEvent::default(), window, cx);
            app.cycle_draft_theme(&ClickEvent::default(), window, cx);
            app.close_settings_dialog(&ClickEvent::default(), window, cx);
        });
    });
    app.read_with(cx, |app, _| {
        assert!(!app.left_dock_open);
        assert!(!app.right_dock_open);
        assert!(!app.settings_dialog_open);
        assert_eq!(app.settings.ui_font_size, 15.0);
        assert_eq!(app.settings.buffer_font_size, 13.0);
        assert_eq!(app.settings.theme_preference, ThemePreference::ZedDark);
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_settings_dialog(&ClickEvent::default(), window, cx);
            app.reset_settings_dialog(&ClickEvent::default(), window, cx);
            app.close_settings_dialog(&ClickEvent::default(), window, cx);
        });
    });
    app.read_with(cx, |app, _| {
        assert_eq!(app.settings, AppSettings::default());
        assert_eq!(
            load_app_settings(&app.settings_path).unwrap(),
            AppSettings::default()
        );
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.send_request(&ClickEvent::default(), window, cx);
        });
    });
    cx.run_until_parked();
    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.url, "https://smoke.test/zed");
        assert!(request.response.is_none());
        assert_eq!(request.history.len(), 0);
        eprintln!("STATUS_LINE: {:?}", app.status_line);
        assert!(app.status_line.contains("failed:"));
    });
}

#[gpui::test]
fn ui_send_records_real_http_response(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(request.starts_with("GET /users HTTP/1.1"));
        stream
            .write_all(
                b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}",
            )
            .unwrap();
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.url_input.update(cx, |input, cx| {
                input.set_content(format!("http://{address}/users"), cx);
            });
            app.send_request(&ClickEvent::default(), window, cx);
        });
    });
    server.join().unwrap();

    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        let response = request.response.as_ref().expect("response should be real");
        assert_eq!(response.status, 201);
        assert_eq!(response.status_text, "Created");
        assert_eq!(response.body, "{\"ok\":true}");
        assert_eq!(request.history.len(), 1);
        assert!(
            response
                .headers
                .iter()
                .any(|header| header.name.eq_ignore_ascii_case("Content-Type")
                    && header.value == "application/json")
        );
    });
}

#[gpui::test]
fn method_select_updates_active_request(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });

    let trigger_bounds = cx
        .debug_bounds("method-select-trigger")
        .expect("method selector trigger should render");
    cx.simulate_click(trigger_bounds.center(), Modifiers::default());
    cx.debug_bounds("method-select-menu")
        .expect("method selector menu should open");

    let options_bounds = cx
        .debug_bounds("method-option-OPTIONS")
        .expect("OPTIONS method item should render");
    cx.simulate_click(options_bounds.center(), Modifiers::default());

    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.method, Method::Options);
        assert!(!app.method_menu_open);
        assert_eq!(app.active_panel, Panel::Params);
    });
}

#[gpui::test]
fn tabs_can_close_inline_and_leave_empty_state(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });

    let close_bounds = cx
        .debug_bounds("request-tab-close-1")
        .expect("active tab close button should render");
    cx.simulate_click(close_bounds.center(), Modifiers::default());

    app.read_with(cx, |app, cx| {
        assert_eq!(app.workspace.request_count(), 1);
        assert!(app.open_tabs.is_empty());
        assert_eq!(app.active_request_id, None);
        assert_eq!(app.url_input.read(cx).value(), "");
        assert!(app.active_request().is_none());
    });
}

#[gpui::test]
fn right_click_tab_menu_closes_target_tab(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });

    let first_tab_bounds = cx
        .debug_bounds("request-tab-1")
        .expect("first request tab should render");
    cx.simulate_mouse_down(
        first_tab_bounds.center(),
        MouseButton::Right,
        Modifiers::default(),
    );
    cx.debug_bounds("tab-context-menu")
        .expect("tab context menu should open on right click");

    let close_bounds = cx
        .debug_bounds("tab-context-close")
        .expect("tab context close item should render");
    cx.simulate_click(close_bounds.center(), Modifiers::default());

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.request_count(), 2);
        assert_eq!(request_ids(&app.workspace), vec![1, 2]);
        assert_eq!(app.open_tabs, vec![2]);
        assert_eq!(app.active_request_id, Some(2));
        assert!(app.request_context_menu.is_none());
    });
}

#[gpui::test]
fn folder_actions_create_request_and_delete_nested_tabs(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_root_folder(&ClickEvent::default(), window, cx);
            app.add_request_to_folder(1, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.request_count(), 1);
        assert_eq!(app.open_tabs, vec![2]);
        assert_eq!(app.active_request_id, Some(2));
        assert!(app.expanded_folders.contains(&1));
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_delete_folder_dialog(1, window, cx);
            app.confirm_delete_folder(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, cx| {
        assert_eq!(app.workspace.request_count(), 0);
        assert!(app.open_tabs.is_empty());
        assert_eq!(app.active_request_id, None);
        assert_eq!(app.url_input.read(cx).value(), "");
    });
}

#[gpui::test]
fn folder_rename_updates_collection_tree(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_root_folder(&ClickEvent::default(), window, cx);
            app.open_rename_folder_dialog(1, window, cx);
            let dialog = app
                .rename_folder_dialog
                .as_ref()
                .expect("rename dialog should open");
            dialog
                .input
                .update(cx, |input, cx| input.set_content("Auth APIs", cx));
            app.confirm_rename_folder(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.folder_name(1).unwrap(), "Auth APIs");
        assert!(app.rename_folder_dialog.is_none());
    });
}

#[gpui::test]
fn folder_expansion_state_persists_in_workspace(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path.clone()));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_root_folder(&ClickEvent::default(), window, cx);
            assert!(app.expanded_folders.contains(&1));
            app.toggle_folder_expanded(1, cx);
            assert!(!app.expanded_folders.contains(&1));
        });
    });

    std::thread::sleep(std::time::Duration::from_millis(150));

    let (reloaded_app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path.clone()));
    reloaded_app.read_with(cx, |app, _| {
        assert!(!app.expanded_folders.contains(&1));
    });

    cx.update(|_, cx| {
        reloaded_app.update(cx, |app, cx| {
            app.toggle_folder_expanded(1, cx);
            assert!(app.expanded_folders.contains(&1));
        });
    });

    std::thread::sleep(std::time::Duration::from_millis(150));

    let (expanded_app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    expanded_app.read_with(cx, |app, _| {
        assert!(app.expanded_folders.contains(&1));
    });
}

#[gpui::test]
fn closing_tab_before_active_keeps_same_logical_request(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
            app.select_request(2, window, cx);
            app.close_request_tab(1, cx);
        });
    });

    app.read_with(cx, |app, cx| {
        assert_eq!(app.workspace.request_count(), 3);
        assert_eq!(app.open_tabs, vec![2, 3]);
        assert_eq!(app.active_request_id, Some(3));
        assert_eq!(app.url_input.read(cx).value(), "");
    });
}

#[gpui::test]
fn duplicate_names_still_select_correct_request(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
            let first = app.workspace.request_mut_by_id(1).unwrap();
            first.name = "Same name".into();
            first.url = "https://example.test/one".into();
            let second = app.workspace.request_mut_by_id(2).unwrap();
            second.name = "Same name".into();
            second.url = "https://example.test/two".into();
            app.select_request_by_id(2, cx);
        });
    });

    let first_tab_bounds = cx
        .debug_bounds("request-tab-1")
        .expect("first request tab should render");
    cx.simulate_click(first_tab_bounds.center(), Modifiers::default());
    app.read_with(cx, |app, cx| {
        assert_eq!(app.active_request_id, Some(1));
        assert_eq!(app.url_input.read(cx).value(), "https://example.test/one");
    });

    let second_tab_bounds = cx
        .debug_bounds("request-tab-2")
        .expect("second request tab should render");
    cx.simulate_click(second_tab_bounds.center(), Modifiers::default());
    app.read_with(cx, |app, cx| {
        assert_eq!(app.active_request_id, Some(2));
        assert_eq!(app.url_input.read(cx).value(), "https://example.test/two");
    });
}

#[gpui::test]
fn tab_reorder_moves_open_tabs_only(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
            app.select_request_by_id(3, cx);
            assert!(app.move_open_tab_to_drop_index(1, 3));
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.open_tabs, vec![2, 3, 1]);
        assert_eq!(request_ids(&app.workspace), vec![1, 2, 3]);
        assert_eq!(app.active_request_id, Some(3));
    });
}

#[gpui::test]
fn tab_reorder_supports_leftward_and_tail_drops(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
            app.add_request(&ClickEvent::default(), window, cx);
            assert!(app.move_open_tab_to_drop_index(3, 0));
            assert_eq!(app.open_tabs, vec![3, 1, 2]);
            assert!(app.move_open_tab_to_drop_index(3, app.open_tabs.len()));
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.open_tabs, vec![1, 2, 3]);
    });
}

#[gpui::test]
fn tab_context_menu_can_rename_request(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });

    let first_tab_bounds = cx
        .debug_bounds("request-tab-1")
        .expect("first request tab should render");
    cx.simulate_mouse_down(
        first_tab_bounds.center(),
        MouseButton::Right,
        Modifiers::default(),
    );
    let rename_bounds = cx
        .debug_bounds("tab-context-rename")
        .expect("rename action should render in context menu");
    cx.simulate_click(rename_bounds.center(), Modifiers::default());

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            let dialog = app
                .rename_request_dialog
                .as_ref()
                .expect("rename dialog should open");
            dialog
                .input
                .update(cx, |input, cx| input.set_content("Renamed request", cx));
            app.confirm_rename_request(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(
            app.workspace.request_by_id(1).unwrap().name,
            "Renamed request"
        );
        assert!(app.rename_request_dialog.is_none());
    });
}

#[gpui::test]
fn enter_confirms_rename_dialogs(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.open_rename_request_dialog(1, window, cx);
            let dialog = app
                .rename_request_dialog
                .as_ref()
                .expect("request rename dialog should open");
            dialog
                .input
                .update(cx, |input, cx| input.set_content("Enter request", cx));
        });
    });
    cx.simulate_keystrokes("enter");
    app.read_with(cx, |app, _| {
        assert_eq!(
            app.workspace.request_by_id(1).unwrap().name,
            "Enter request"
        );
        assert!(app.rename_request_dialog.is_none());
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_root_folder(&ClickEvent::default(), window, cx);
            app.open_rename_folder_dialog(2, window, cx);
            let dialog = app
                .rename_folder_dialog
                .as_ref()
                .expect("folder rename dialog should open");
            dialog
                .input
                .update(cx, |input, cx| input.set_content("Enter folder", cx));
        });
    });
    cx.simulate_keystrokes("enter");
    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.folder_name(2).unwrap(), "Enter folder");
        assert!(app.rename_folder_dialog.is_none());
    });
}

#[gpui::test]
fn enter_confirms_delete_dialogs_and_escape_cancels_dialogs(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.open_delete_request_dialog(1, window, cx);
        });
    });
    cx.simulate_keystrokes("enter");
    app.read_with(cx, |app, _| {
        assert!(app.workspace.request_by_id(1).is_none());
        assert!(app.delete_request_dialog.is_none());
    });
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.open_delete_request_dialog(2, window, cx);
        });
    });
    cx.simulate_keystrokes("escape");
    app.read_with(cx, |app, _| {
        assert!(app.workspace.request_by_id(2).is_some());
        assert!(app.delete_request_dialog.is_none());
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_root_folder(&ClickEvent::default(), window, cx);
            app.open_delete_folder_dialog(3, window, cx);
        });
    });
    cx.simulate_keystrokes("enter");
    app.read_with(cx, |app, _| {
        assert!(app.workspace.folder_name(3).is_none());
        assert!(app.delete_folder_dialog.is_none());
    });
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_root_folder(&ClickEvent::default(), window, cx);
            app.open_delete_folder_dialog(4, window, cx);
        });
    });
    cx.simulate_keystrokes("escape");
    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.folder_name(4).unwrap(), "New folder 4");
        assert!(app.delete_folder_dialog.is_none());
    });
}

#[gpui::test]
fn collection_selection_delete_and_f2_open_dialogs(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });
    let request_bounds = cx
        .debug_bounds("collection-request-row-1")
        .expect("request collection row should render");
    cx.simulate_click(request_bounds.center(), Modifiers::default());
    cx.simulate_keystrokes("f2");
    app.read_with(cx, |app, _| {
        assert!(app.rename_request_dialog.is_some());
    });
    cx.simulate_keystrokes("escape");
    cx.simulate_keystrokes("delete");
    app.read_with(cx, |app, _| {
        assert!(app.delete_request_dialog.is_some());
    });
    cx.simulate_keystrokes("escape");

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_root_folder(&ClickEvent::default(), window, cx);
        });
    });
    let folder_bounds = cx
        .debug_bounds("collection-folder-row-2")
        .expect("folder collection row should render");
    cx.simulate_click(folder_bounds.center(), Modifiers::default());
    cx.simulate_keystrokes("f2");
    app.read_with(cx, |app, _| {
        assert!(app.rename_folder_dialog.is_some());
    });
    cx.simulate_keystrokes("escape");
    cx.simulate_keystrokes("delete");
    app.read_with(cx, |app, _| {
        assert!(app.delete_folder_dialog.is_some());
    });
}

#[gpui::test]
fn clicking_dialog_backdrop_closes_without_confirming(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.open_delete_request_dialog(1, window, cx);
        });
    });
    cx.simulate_click(point(px(12.0), px(52.0)), Modifiers::default());
    app.read_with(cx, |app, _| {
        assert!(app.workspace.request_by_id(1).is_some());
        assert!(app.delete_request_dialog.is_none());
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_settings_dialog(&ClickEvent::default(), window, cx);
        });
    });
    cx.simulate_click(point(px(12.0), px(52.0)), Modifiers::default());
    app.read_with(cx, |app, _| {
        assert!(!app.settings_dialog_open);
    });
}

#[gpui::test]
fn enter_closes_settings_dialog(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.open_settings_dialog(&ClickEvent::default(), window, cx);
        });
    });
    cx.simulate_keystrokes("enter");
    app.read_with(cx, |app, _| {
        assert!(!app.settings_dialog_open);
    });
}

#[gpui::test]
fn url_input_supports_word_shortcuts(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });

    let url_shell_bounds = cx
        .debug_bounds("url-input-shell")
        .expect("URL input shell should render");
    cx.simulate_click(url_shell_bounds.center(), Modifiers::default());

    app.update(cx, |app, cx| {
        app.url_input.update(cx, |input, cx| {
            input.set_content("https://api.example.com/users/123", cx);
        });
    });
    cx.simulate_keystrokes("ctrl-backspace");
    app.read_with(cx, |app, cx| {
        assert_eq!(
            app.url_input.read(cx).value(),
            "https://api.example.com/users/"
        );
    });

    cx.simulate_keystrokes("ctrl-left");
    cx.simulate_input("X");
    app.read_with(cx, |app, cx| {
        assert_eq!(
            app.url_input.read(cx).value(),
            "https://api.example.com/Xusers/"
        );
    });

    app.update(cx, |app, cx| {
        app.url_input.update(cx, |input, cx| {
            input.set_content("alpha beta", cx);
        });
    });
    cx.simulate_keystrokes("home ctrl-right");
    cx.simulate_input("X");
    app.read_with(cx, |app, cx| {
        assert_eq!(app.url_input.read(cx).value(), "alphaX beta");
    });

    app.update(cx, |app, cx| {
        app.url_input.update(cx, |input, cx| {
            input.set_content("select word on double click", cx);
        });
    });
    let url_text_bounds = cx
        .debug_bounds("url-text-input")
        .expect("URL text input should render");
    cx.simulate_event(MouseDownEvent {
        position: url_text_bounds.center(),
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_input("X");
    app.read_with(cx, |app, cx| {
        assert_eq!(app.url_input.read(cx).value(), "select word on double X");
    });

    app.update(cx, |app, cx| {
        app.url_input.update(cx, |input, cx| {
            input.set_content("shell double click keeps text", cx);
        });
    });
    let url_shell_bounds = cx
        .debug_bounds("url-input-shell")
        .expect("URL input shell should render");
    cx.simulate_event(MouseDownEvent {
        position: point(
            url_shell_bounds.left() + px(3.0),
            url_shell_bounds.center().y,
        ),
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_input("Y");
    app.read_with(cx, |app, cx| {
        assert_eq!(
            app.url_input.read(cx).value(),
            "shell double click keeps textY"
        );
    });
}

#[gpui::test]
fn bottom_bar_icon_controls_toggle_panels_and_settings(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    let collection_bounds = cx
        .debug_bounds("bottom-toggle-collections")
        .expect("collection toggle should render");
    cx.simulate_click(collection_bounds.center(), Modifiers::default());
    app.read_with(cx, |app, _| {
        assert!(!app.left_dock_open);
    });

    let environment_bounds = cx
        .debug_bounds("bottom-toggle-environment")
        .expect("environment toggle should render");
    cx.simulate_click(environment_bounds.center(), Modifiers::default());
    app.read_with(cx, |app, _| {
        assert!(!app.right_dock_open);
    });

    let settings_bounds = cx
        .debug_bounds("bottom-open-settings")
        .expect("settings button should render");
    cx.simulate_click(settings_bounds.center(), Modifiers::default());
    app.read_with(cx, |app, _| {
        assert!(app.settings_dialog_open);
    });
}

#[gpui::test]
fn settings_dialog_opens_with_shortcut_and_auto_saves_font_controls(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
        });
    });
    let url_shell_bounds = cx
        .debug_bounds("url-input-shell")
        .expect("URL input shell should render before shortcut");
    cx.simulate_click(url_shell_bounds.center(), Modifiers::default());

    cx.simulate_keystrokes("ctrl-,");
    cx.debug_bounds("settings-dialog")
        .expect("settings dialog should open from Ctrl+,");

    let ui_increase_bounds = cx
        .debug_bounds("settings-ui-font-increase")
        .expect("UI font increase button should render");
    cx.simulate_click(ui_increase_bounds.center(), Modifiers::default());

    let buffer_decrease_bounds = cx
        .debug_bounds("settings-buffer-font-decrease")
        .expect("buffer font decrease button should render");
    cx.simulate_click(buffer_decrease_bounds.center(), Modifiers::default());

    app.read_with(cx, |app, _| {
        assert!(app.settings_dialog_open);
        assert_eq!(app.settings.ui_font_size, 15.0);
        assert_eq!(app.settings.buffer_font_size, 13.0);
        assert_eq!(load_app_settings(&app.settings_path).unwrap(), app.settings);
    });

    let done_bounds = cx
        .debug_bounds("settings-done")
        .expect("settings done button should render");
    cx.simulate_click(done_bounds.center(), Modifiers::default());
    app.read_with(cx, |app, _| {
        assert!(!app.settings_dialog_open);
    });
}

#[gpui::test]
fn enter_sends_focused_url_request(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(request.starts_with("GET /enter HTTP/1.1"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nsent")
            .unwrap();
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.url_input.update(cx, |input, cx| {
                input.set_content(format!("http://{address}/enter"), cx);
            });
        });
    });

    let url_shell_bounds = cx
        .debug_bounds("url-input-shell")
        .expect("URL input shell should render with a debug selector");
    cx.simulate_click(url_shell_bounds.center(), Modifiers::default());
    cx.simulate_keystrokes("enter");
    server.join().unwrap();

    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        let response = request.response.as_ref().expect("response should exist");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "sent");
        assert_eq!(request.history.len(), 1);
    });
}

#[gpui::test]
fn response_headers_are_not_reused_from_request_headers(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(
            request
                .to_ascii_lowercase()
                .contains("x-request-only: sent-value")
        );
        stream
            .write_all(
                b"HTTP/1.1 201 Created\r\nX-Response-Only: received-value\r\nContent-Length: 2\r\n\r\nok",
            )
            .unwrap();
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.url_input.update(cx, |input, cx| {
                input.set_content(format!("http://{address}/headers"), cx);
            });
            let request = app.active_request_mut().expect("request should exist");
            request.headers = vec![Header::new("X-Request-Only", "sent-value")];
        });
    });

    let url_shell_bounds = cx
        .debug_bounds("url-input-shell")
        .expect("URL input shell should render with a debug selector");
    cx.simulate_click(url_shell_bounds.center(), Modifiers::default());
    cx.simulate_keystrokes("enter");
    server.join().unwrap();

    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        let response = request.response.as_ref().expect("response should exist");
        assert!(
            request
                .headers
                .iter()
                .any(|header| header.name == "X-Request-Only")
        );
        assert!(
            response
                .headers
                .iter()
                .any(|header| header.name == "x-response-only" && header.value == "received-value")
        );
        assert!(
            !response
                .headers
                .iter()
                .any(|header| header.name.eq_ignore_ascii_case("x-request-only"))
        );
    });
}

#[gpui::test]
fn response_tabs_do_not_change_request_panel_and_can_switch_back(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            app.active_panel = Panel::Params;
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: 2,
                headers: vec![ResponseHeader {
                    name: "X-Response".into(),
                    value: "yes".into(),
                }],
                cookies: vec![ResponseHeader {
                    name: "session".into(),
                    value: "abc".into(),
                }],
                timing: None,
                body: "ok".into(),
            });
        });
    });

    for (selector, expected) in [
        ("response-tab-headers", ResponsePanel::Headers),
        ("response-tab-cookies", ResponsePanel::Cookies),
        ("response-tab-body", ResponsePanel::Body),
        ("response-tab-headers", ResponsePanel::Headers),
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} should render"));
        cx.simulate_click(bounds.center(), Modifiers::default());
        app.read_with(cx, |app, _| {
            assert_eq!(app.active_response_panel, expected);
            assert_eq!(app.active_panel, Panel::Params);
        });
    }
}

#[gpui::test]
fn response_headers_wrap_and_copy_values(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    let long_value = "a-very-long-response-header-value-that-should-wrap-below-instead-of-forcing-the-row-to-grow-sideways".to_string();
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: 2,
                headers: vec![ResponseHeader {
                    name: "X-Long-Response".into(),
                    value: long_value.clone().into(),
                }],
                cookies: Vec::new(),
                timing: None,
                body: "ok".into(),
            });
            app.active_response_panel = ResponsePanel::Headers;
        });
    });

    let copy_bounds = cx
        .debug_bounds("response-header-copy-0")
        .expect("response header copy button should render");
    cx.simulate_click(copy_bounds.center(), Modifiers::default());

    app.read_with(cx, |app, _| {
        assert_eq!(app.status_line, "Copied response header value.");
    });
}

#[gpui::test]
fn response_pretty_and_raw_use_code_input(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: 11,
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: "{\"ok\":true}".into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Pretty;
        });
    });

    cx.debug_bounds("response-code-input")
        .expect("pretty response should render with the code input");
    app.read_with(cx, |app, cx| {
        let request_id = app.active_request_id.expect("request should be selected");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Pretty))
            .expect("pretty response input should exist");
        assert!(input.read(cx).value().contains("\"ok\": true"));
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.set_body_view_mode(BodyViewMode::Raw, &ClickEvent::default(), window, cx);
        });
    });
    cx.debug_bounds("response-code-input")
        .expect("raw response should render with the code input");
    app.read_with(cx, |app, cx| {
        let request_id = app.active_request_id.expect("request should be selected");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .expect("raw response input should exist");
        assert_eq!(input.read(cx).value(), "{\"ok\":true}");
    });
}

#[gpui::test]
fn large_response_body_only_renders_visible_code_lines(cx: &mut TestAppContext) {
    // See docs/references/large-response-test-endpoints.md for real-world multi-MB / many-line endpoints
    // to manually exercise the Large Response Viewer, warning banner, and CodeInput on live data.
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    let large_body = (0..5_000)
        .map(|index| format!("{{\"index\":{index}}}"))
        .collect::<Vec<_>>()
        .join("\n");
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: large_body.len(),
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: large_body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });

    cx.debug_bounds("response-code-input")
        .expect("large response should render with the code input");

    app.read_with(cx, |app, cx| {
        let request_id = app.active_request_id.expect("request should be selected");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .expect("raw response input should exist");
        let input = input.read(cx);
        assert_eq!(input.cached_total_line_count(), 5_000);
        assert!(
            input.rendered_line_count() < 200,
            "large response should render only the viewport slice"
        );
    });
}

#[gpui::test]
fn response_body_scrollbar_thumb_drags_scroll_position(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    let large_body = (0..5_000)
        .map(|index| format!("{{\"index\":{index}}}"))
        .collect::<Vec<_>>()
        .join("\n");
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: large_body.len(),
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: large_body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });
    cx.update(|_, cx| app.update(cx, |_, cx| cx.notify()));

    let thumb = cx
        .debug_bounds("response-scrollbar-thumb")
        .expect("response scrollbar thumb should render");
    let start = thumb.center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    app.read_with(cx, |app, cx| {
        assert!(app.response_scrollbar.read(cx).is_dragging());
    });
    cx.simulate_mouse_move(
        point(start.x, start.y + px(8.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(start.x, start.y + px(80.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        point(start.x, start.y + px(80.0)),
        MouseButton::Left,
        Modifiers::default(),
    );

    app.read_with(cx, |app, cx| {
        assert!(app.response_scroll_handle.offset().y < px(0.0));
        assert!(!app.response_scrollbar.read(cx).is_dragging());
    });
}

#[gpui::test]
fn response_body_scrollbar_track_click_scrolls_position(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    let large_body = (0..5_000)
        .map(|index| format!("{{\"index\":{index}}}"))
        .collect::<Vec<_>>()
        .join("\n");
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: large_body.len(),
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: large_body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });
    cx.update(|_, cx| app.update(cx, |_, cx| cx.notify()));

    let track = cx
        .debug_bounds("response-scrollbar-track")
        .expect("response scrollbar track should render");
    cx.simulate_mouse_down(
        point(track.center().x, track.bottom() - px(20.0)),
        MouseButton::Left,
        Modifiers::default(),
    );

    app.read_with(cx, |app, _| {
        assert!(app.response_scroll_handle.offset().y < px(0.0));
    });
}

#[gpui::test]
fn response_body_wheel_scroll_uses_native_scroll_handle(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    let large_body = (0..5_000)
        .map(|index| format!("{{\"index\":{index}}}"))
        .collect::<Vec<_>>()
        .join("\n");
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: large_body.len(),
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: large_body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });

    let scroll_bounds = cx
        .debug_bounds("response-body-code-scroll")
        .expect("response body scroll area should render");
    cx.simulate_event(ScrollWheelEvent {
        position: scroll_bounds.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });

    app.read_with(cx, |app, cx| {
        assert!(app.response_scroll_handle.offset().y < px(0.0));
        assert!(!app.response_scrollbar.read(cx).is_dragging());
    });
}

#[gpui::test]
fn response_body_with_long_lines_exposes_horizontal_scroll_range(cx: &mut TestAppContext) {
    // Regression test: when the CodeInput contains lines wider than the
    // viewport, the parent overflow_scroll container must detect real
    // horizontal overflow so wheel/bar scrolling can move the text.
    // The container relies on the CodeInput wrapper being able to grow
    // beyond viewport width (`min_w_full`, not `w_full`).
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    // A single very long line that will visibly exceed any reasonable
    // viewport width.
    let wide_line: String = "x".repeat(5_000);
    let body = format!("{wide_line}\n{wide_line}\n{wide_line}");

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: body.len(),
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });

    cx.debug_bounds("response-code-input")
        .expect("response code input should render");

    // After at least one frame, the parent overflow_scroll container should
    // see that the content is wider than the viewport.
    app.read_with(cx, |app, _cx| {
        let max_offset = app.response_scroll_handle.max_offset();
        assert!(
            max_offset.width > px(0.0),
            "long lines should produce horizontal overflow (max_offset.width={:?})",
            max_offset.width
        );
    });
}

#[gpui::test]
fn response_body_scroll_into_large_body_keeps_final_colored_cache(cx: &mut TestAppContext) {
    // See docs/references/large-response-test-endpoints.md for real-world multi-MB / many-line endpoints
    // to manually exercise the Large Response Viewer, warning banner, and CodeInput on live data.
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    let large_body = (0..5_000)
        .map(|index| format!("{{\"index\":{index},\"ok\":true}}"))
        .collect::<Vec<_>>()
        .join("\n");
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: large_body.len(),
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: large_body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });

    let scroll_bounds = cx
        .debug_bounds("response-body-code-scroll")
        .expect("response body scroll area should render");
    cx.simulate_event(ScrollWheelEvent {
        position: scroll_bounds.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-18_000.0))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    cx.refresh()
        .expect("refresh should render scrolled response body");

    app.read_with(cx, |app, cx| {
        let request_id = app.active_request_id.expect("request should be selected");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .expect("raw response input should exist")
            .read(cx);
        assert!(
            input.rendered_first_line_index() > 100,
            "test should scroll well beyond initially rendered rows"
        );
        assert!(
            input.rendered_line_count() < 200,
            "large response should still render only the viewport slice"
        );
        assert!(
            input.shape_cache_len() <= 1024,
            "shape cache should remain bounded"
        );
        assert!(
            input.rendered_rows_have_cached_shapes(),
            "visible rows should be cached as final syntax-colored shapes"
        );
    });
}

#[gpui::test]
fn response_toolbar_metadata_and_body_view_select_work(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 201,
                status_text: "Created".into(),
                duration_ms: 42,
                size_bytes: 17,
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: "{\"ok\":true}".into(),
            });
            app.body_view_mode = BodyViewMode::Pretty;
        });
    });

    cx.debug_bounds("response-status-meta")
        .expect("response status metadata should render");
    cx.debug_bounds("response-time-meta")
        .expect("response time metadata should render");
    cx.debug_bounds("response-size-meta")
        .expect("response size metadata should render");
    assert!(
        cx.debug_bounds("body-view-option-preview").is_none(),
        "preview should not render as a response body option"
    );

    let trigger = cx
        .debug_bounds("body-view-select-trigger")
        .expect("body view select trigger should render");
    cx.simulate_click(trigger.center(), Modifiers::default());
    cx.debug_bounds("body-view-select-menu")
        .expect("body view select menu should open");
    let raw_option = cx
        .debug_bounds("body-view-option-raw")
        .expect("raw body view option should render");
    cx.simulate_click(raw_option.center(), Modifiers::default());

    app.read_with(cx, |app, _| {
        assert_eq!(app.body_view_mode, BodyViewMode::Raw);
        assert!(!app.body_view_menu_open);
    });
}

#[gpui::test]
fn body_editor_enter_inserts_newline_without_sending(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.method = Method::Post;
            app.active_panel = Panel::Body;
        });
    });

    let body_bounds = cx
        .debug_bounds("body-input-shell")
        .expect("body editor shell should render");
    cx.simulate_click(body_bounds.center(), Modifiers::default());
    cx.simulate_input("abc");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("def");

    app.update(cx, |app, cx| app.sync_active_request_inputs(cx));
    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.body, "abc\ndef");
        assert!(request.response.is_none());
    });
}

#[gpui::test]
fn body_editor_supports_up_down_cursor_movement(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.method = Method::Post;
            app.active_panel = Panel::Body;
        });
    });

    let body_bounds = cx
        .debug_bounds("body-input-shell")
        .expect("body editor shell should render");
    cx.simulate_click(body_bounds.center(), Modifiers::default());
    cx.simulate_input("a");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("bc");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("def");
    cx.simulate_keystrokes("up");
    cx.simulate_input("X");

    app.update(cx, |app, cx| app.sync_active_request_inputs(cx));
    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.body, "a\nbcX\ndef");
    });
}

#[gpui::test]
fn body_editor_auto_pairs_when_not_glued_to_text(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.method = Method::Post;
            app.active_panel = Panel::Body;
        });
    });

    let body_bounds = cx
        .debug_bounds("body-input-shell")
        .expect("body editor shell should render");
    cx.simulate_click(body_bounds.center(), Modifiers::default());
    cx.simulate_input("{");
    cx.simulate_keystrokes("enter");

    app.update(cx, |app, cx| app.sync_active_request_inputs(cx));
    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.body, "{\n  \n}");
    });
}

#[gpui::test]
fn body_editor_does_not_auto_pair_when_glued_to_text(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.method = Method::Post;
            app.active_panel = Panel::Body;
        });
    });

    let body_bounds = cx
        .debug_bounds("body-input-shell")
        .expect("body editor shell should render");
    cx.simulate_click(body_bounds.center(), Modifiers::default());
    cx.simulate_input("abc{");

    app.update(cx, |app, cx| app.sync_active_request_inputs(cx));
    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.body, "abc{");
    });
}

#[gpui::test]
fn body_editor_double_click_selects_word_not_all(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.method = Method::Post;
            app.active_panel = Panel::Body;
        });
    });

    let shell_bounds = cx
        .debug_bounds("body-input-shell")
        .expect("body editor shell should render");
    cx.simulate_click(shell_bounds.center(), Modifiers::default());
    cx.simulate_input("alpha beta gamma");
    let code_bounds = cx
        .debug_bounds("body-code-input")
        .expect("body code input should render");
    cx.simulate_event(MouseDownEvent {
        position: code_bounds.center(),
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_input("X");

    app.update(cx, |app, cx| app.sync_active_request_inputs(cx));
    app.read_with(cx, |app, _| {
        let request = app.active_request().expect("request should exist");
        assert_eq!(request.body, "alpha beta X");
    });
}

#[gpui::test]
fn large_response_body_still_renders_code_input(cx: &mut TestAppContext) {
    // See docs/references/large-response-test-endpoints.md for real-world multi-MB / many-line endpoints
    // to manually exercise the Large Response Viewer, warning banner, and CodeInput on live data.
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    // Body above the new LARGE_RESPONSE_WARNING_BYTES threshold.
    // We expect the CodeInput to still be used (with a warning banner).
    let body: String = std::iter::repeat('a')
        .take(10 * 1024 * 1024 + 100)
        .collect();
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: body.len(),
                headers: vec![ResponseHeader {
                    name: "content-type".into(),
                    value: "application/json".into(),
                }],
                cookies: Vec::new(),
                timing: None,
                body: body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });

    // With the Large Response Viewer feature, we should still get the CodeInput.
    cx.debug_bounds("response-code-input")
        .expect("large body should still render the code input (with warning banner)");
}

#[gpui::test]
fn very_large_response_body_cursor_movement_stays_responsive(cx: &mut TestAppContext) {
    // See docs/references/large-response-test-endpoints.md for real-world multi-MB / many-line endpoints
    // to manually exercise the Large Response Viewer, warning banner, and CodeInput on live data.
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    // 100k lines, well below the 2 MB cap but well above the 5_000-line
    // fold-detection threshold. Confirms that visible_line_count and
    // home/end stay O(1) instead of walking every line.
    let large_body = (0..100_000)
        .map(|index| format!("{{\"index\":{index}}}"))
        .collect::<Vec<_>>()
        .join("\n");
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: large_body.len(),
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: large_body.clone().into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });

    cx.debug_bounds("response-code-input")
        .expect("100k-line response should still render the code input");

    let started = std::time::Instant::now();
    app.update(cx, |app, cx| {
        let request_id = app.active_request_id.expect("request should be selected");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .cloned()
            .expect("raw response input should exist");
        input.update(cx, |input, _cx| {
            // Compute visible line count and source line count many
            // times. With the LineIndex-backed fast path these are O(1)
            // / O(folds), so this loop should finish almost instantly.
            for _ in 0..1_000 {
                let _ = input.visible_line_count();
                let _ = input.source_line_count();
            }
        });
    });
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "1k visible_line_count calls on a 100k-line body took {elapsed:?}"
    );
}

#[gpui::test]
fn collection_crud_and_drag_actions(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    // 1. Create a collection
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_collection(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.collections.len(), 1);
        assert_eq!(app.workspace.collections[0].name, "New Collection 1");
    });

    // 2. Add request to this collection
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.add_request_to_collection(1, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.collections[0].items.len(), 1);
    });

    // 3. Rename collection
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_rename_collection_dialog(1, window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert!(app.rename_collection_dialog.is_some());
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.rename_collection_dialog
                .as_ref()
                .unwrap()
                .input
                .update(cx, |input, cx| {
                    input.set_content("Scalar API", cx);
                });
            app.confirm_rename_collection(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.collections[0].name, "Scalar API");
        assert!(app.rename_collection_dialog.is_none());
    });

    // 4. Create another collection
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_collection(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.collections.len(), 2);
        assert_eq!(app.workspace.collections[1].name, "New Collection 2");
    });

    // 5. Drag/move request from collection 1 to collection 2
    app.read_with(cx, |app, _| {
        let req_id = app.workspace.collections[0].items[0].id();
        assert_eq!(req_id, 1);
    });

    cx.update(|_, cx| {
        app.update(cx, |app, _| {
            assert!(app.workspace.move_item_between_collections(1, 2));
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.collections[0].items.len(), 0);
        assert_eq!(app.workspace.collections[1].items.len(), 1);
    });

    // 6. Delete collection
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_delete_collection_dialog(1, window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert!(app.delete_collection_dialog.is_some());
    });

    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.confirm_delete_collection(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.collections.len(), 1);
        assert_eq!(app.workspace.collections[0].name, "New Collection 2");
        assert!(app.delete_collection_dialog.is_none());
    });
}

#[gpui::test]
fn workspace_creation_and_switching(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");

    // Make sure workspaces subfolder exists
    std::fs::create_dir_all(temp_dir.path().join("workspaces")).unwrap();

    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path.clone()));

    // Persist the default workspace to disk so it is scanned and discovered in workspaces_list
    cx.update(|_, cx| {
        app.update(cx, |app, _| {
            app.persist_workspace();
            app.refresh_workspaces_list();
        });
    });

    // 1. Initial State Checks
    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.name, "Local API Workspace");
        assert_eq!(app.workspaces_list.len(), 1);
        assert!(app.create_workspace_dialog.is_none());
    });

    // 2. Open Create Workspace Dialog
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_create_workspace_dialog(&ClickEvent::default(), window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert!(app.create_workspace_dialog.is_some());
    });

    // Simulate typing workspace name "Production API"
    cx.simulate_keystrokes("shift-p r o d u c t i o n space shift-a p i");
    cx.simulate_keystrokes("enter");

    // 3. Verify Switch to New Workspace
    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.name, "Production Api");
        assert_eq!(app.workspaces_list.len(), 2);
        assert!(app.create_workspace_dialog.is_none());
    });

    // 4. Switch back to the original workspace
    let original_workspace_path = temp_dir.path().join("workspaces/local.json");
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.switch_to_workspace(original_workspace_path, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert_eq!(app.workspace.name, "Local API Workspace");
    });
}

#[gpui::test]
fn find_in_response_body(cx: &mut TestAppContext) {
    cx.update(bind_text_input_keys);
    cx.update(bind_code_input_keys);
    cx.update(bind_app_keys);
    let temp_dir = tempfile::tempdir().unwrap();
    let settings_path = temp_dir.path().join("settings.json");
    let (app, cx) =
        cx.add_window_view(|_, cx| ApiClientApp::new_with_settings_path(cx, settings_path));

    // 1. Mock request and response body
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.add_request(&ClickEvent::default(), window, cx);
            let request = app.active_request_mut().expect("request should exist");
            request.response = Some(ResponseRecord {
                status: 200,
                status_text: "OK".into(),
                duration_ms: 1,
                size_bytes: 33,
                headers: Vec::new(),
                cookies: Vec::new(),
                timing: None,
                body: "Hello World! World is beautiful.".into(),
            });
            app.active_response_panel = ResponsePanel::Body;
            app.body_view_mode = BodyViewMode::Raw;
        });
    });

    // Draw window once to populate response_body_inputs
    cx.debug_bounds("response-body-code-scroll")
        .expect("response body code input should render");

    // Verify initial state
    app.read_with(cx, |app, cx| {
        assert!(app.find_state.is_none());
        let request_id = app.active_request_id.expect("request should be active");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .expect("response input should exist");
        assert!(input.read(cx).search_highlights.is_empty());
    });

    // 2. Open Find Bar via shortcut/action
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.toggle_find(&ToggleFind, window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        assert!(app.find_state.is_some());
    });

    // 3. Simulate typing the search query "World"
    cx.simulate_keystrokes("w o r l d");

    app.read_with(cx, |app, cx| {
        let find_state = app.find_state.as_ref().unwrap();
        assert_eq!(find_state.query_input.read(cx).value(), "world");
        assert_eq!(find_state.matches.len(), 2);
        assert_eq!(find_state.active_match_idx, Some(0));

        let request_id = app.active_request_id.expect("request should be active");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .expect("response input should exist");
        assert_eq!(input.read(cx).search_highlights.len(), 2);
        assert_eq!(
            input
                .read(cx)
                .active_search_highlight
                .as_ref()
                .unwrap()
                .start,
            6 // byte index of the first "World"
        );
    });

    // 4. Navigate to next match
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.find_next_from_action(&FindNext, window, cx);
        });
    });

    app.read_with(cx, |app, cx| {
        let find_state = app.find_state.as_ref().unwrap();
        assert_eq!(find_state.active_match_idx, Some(1));

        let request_id = app.active_request_id.expect("request should be active");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .expect("response input should exist");
        assert_eq!(
            input
                .read(cx)
                .active_search_highlight
                .as_ref()
                .unwrap()
                .start,
            13 // byte index of the second "World"
        );
    });

    // 5. Navigate to previous match
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.find_previous_from_action(&FindPrevious, window, cx);
        });
    });

    app.read_with(cx, |app, _| {
        let find_state = app.find_state.as_ref().unwrap();
        assert_eq!(find_state.active_match_idx, Some(0));
    });

    // 6. Close the search bar
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.close_find_from_action(&CloseFind, window, cx);
        });
    });

    app.read_with(cx, |app, cx| {
        assert!(app.find_state.is_none());

        let request_id = app.active_request_id.expect("request should be active");
        let input = app
            .response_body_inputs
            .get(&(request_id, BodyViewMode::Raw))
            .expect("response input should exist");
        assert!(input.read(cx).search_highlights.is_empty());
    });
}
