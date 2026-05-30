use crate::app::*;

impl ApiClientApp {
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

    fn render_tab_button(
        &mut self,
        key: &'static str,
        label: impl Into<SharedString>,
        active: bool,
        style: ButtonStyle,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let handle = self.tab_handle(format!("button:{key}"), cx);
        let id = key.bytes().fold(0usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as usize)
        });
        ui::button_base(("button", id), label, active, style, theme)
            .track_focus(&handle)
            .tab_stop(true)
            .on_click(cx.listener(on_click))
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
        &mut self,
        label: impl Into<SharedString>,
        active: bool,
        style: ButtonStyle,
        theme: AppTheme,
        on_click: impl Fn(&mut Self, &gpui::ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let handle = self.tab_handle(format!("toolbar-btn:{}", label.as_ref()), cx);
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
        .track_focus(&handle)
        .tab_stop(true)
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
            ui::ButtonSize::Compact,
            theme,
        )
        .child(ui::icon(icon, ui::IconSize::XSmall, icon_color))
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
                        Some(CollectionSelection::Request { item_id, .. }) if item_id == request_id
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
                                            .flex_1()
                                            .min_w_0()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .truncate()
                                            .child(request.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_ui_xs(typography)
                                            .text_color(request.method.color())
                                            .child(request.method.as_str()),
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
                        Some(CollectionSelection::Folder { item_id, .. }) if item_id == folder_id
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
            .drag_over::<DraggedCollection>(move |target, _, _, _| {
                target.bg(theme.element_selected).border_1()
            })
            .on_drop(
                cx.listener(move |this, dragged_col: &DraggedCollection, window, cx| {
                    this.drop_collection_at_end(dragged_col, window, cx)
                }),
            )
            .into_any_element()
    }

    fn render_sidebar(&self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();

        let mut request_rows = Vec::new();
        for collection in &self.workspace.collections {
            let col_id = collection.id;
            let expanded = self.expanded_collections.contains(&col_id);
            let dragged_col = DraggedCollection {
                collection_id: col_id,
                label: collection.name.clone(),
                theme,
                typography,
            };
            request_rows.push(
                ui::list_item(("collection-header", col_id), false, theme)
                    .pr_0()
                    .relative()
                    .on_drag(dragged_col, |dragged_col, _, _, cx| {
                        cx.new(|_| dragged_col.clone())
                    })
                    .drag_over::<DraggedCollection>(move |row, dragged, _, _| {
                        if dragged.collection_id == col_id {
                            row
                        } else {
                            row.bg(theme.element_selected)
                                .border_color(theme.border_focused)
                        }
                    })
                    .on_drop(
                        cx.listener(move |this, dragged: &DraggedCollection, window, cx| {
                            this.drop_collection_before_collection(col_id, dragged, window, cx)
                        }),
                    )
                    .drag_over::<DraggedCollectionItem>(move |row, _, _, _| {
                        row.bg(theme.element_selected)
                            .border_color(theme.border_focused)
                    })
                    .on_drop(cx.listener(
                        move |this, dragged: &DraggedCollectionItem, window, cx| {
                            this.drop_item_into_collection(col_id, dragged, window, cx)
                        },
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_collection_expanded(col_id, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event, window, cx| {
                            this.open_collection_context_menu(col_id, event, window, cx);
                        }),
                    )
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
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.text)
                                    .truncate()
                                    .child(collection.name.clone()),
                            ),
                    )
                    .into_any_element(),
            );

            if expanded {
                if collection.items.is_empty() {
                    request_rows.push(
                        div()
                            .id(("collection-empty", collection.id))
                            .pl(px(18.0))
                            .py(spacing.base04())
                            .text_ui_xs(typography)
                            .text_color(theme.text_muted)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(div().child("Empty collection"))
                                    .child(
                                        ui::icon_button_base(
                                            ("add-req-to-empty-col", col_id),
                                            IconName::Plus,
                                            false,
                                            theme,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.add_request_to_collection(col_id, cx);
                                            }),
                                        ),
                                    ),
                            )
                            .into_any_element(),
                    );
                } else {
                    request_rows.extend(self.render_collection_items(
                        &collection.items,
                        1,
                        theme,
                        typography,
                        cx,
                    ));
                }
            }
        }

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
                    .p(spacing.base08())
                    .child(
                        div()
                            .id("workspace-switcher-header")
                            .flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .p(spacing.base04())
                            .rounded_sm()
                            .hover(|style| style.bg(theme.element_hover))
                            .active(|style| style.bg(theme.element_selected))
                            .on_click(cx.listener(Self::toggle_workspace_menu))
                            .child(
                                ui::label(self.workspace.name.clone(), theme)
                                    .text_ui_lg(typography)
                                    .font_weight(gpui::FontWeight::BOLD),
                            )
                            .child(
                                div()
                                    .text_ui_xs(typography)
                                    .text_color(theme.text_muted)
                                    .child(if self.workspace_menu_open {
                                        "▾"
                                    } else {
                                        "▸"
                                    }),
                            ),
                    ),
            )
            .child(
                ui::toolbar("collection-toolbar", theme)
                    .h(px(34.0))
                    .p(spacing.base08())
                    .child(Self::render_button(
                        "+ Req",
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
                    ))
                    .child(Self::render_button(
                        "+ Col",
                        false,
                        ButtonStyle::Subtle,
                        theme,
                        Self::add_collection,
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
            .tab_stop(true)
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
            .tab_stop(true)
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
        &mut self,
        request: &Request,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current_method = request.method;
        let spacing = Spacing::app();
        let method_handle = self.tab_handle("method-select", cx);

        div()
            .relative()
            .flex_none()
            .child(
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
                .track_focus(&method_handle)
                .tab_stop(true)
                .on_click(cx.listener(Self::toggle_method_menu)),
            )
            .when(self.method_menu_open, |this| {
                let current_method = request.method;
                let items = Method::all()
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, method)| {
                        let debug_selector = format!("method-option-{}", method.as_str());
                        let item_handle = self.tab_handle(format!("method-opt-{index}"), cx);
                        ui::select_menu_item(
                            ("method-option", index),
                            method.as_str(),
                            method.color(),
                            method == current_method,
                            theme,
                            typography,
                        )
                        .debug_selector(move || debug_selector.clone())
                        .track_focus(&item_handle)
                        .tab_stop(true)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event, window, cx| {
                                this.set_active_request_method_from_mouse_down(
                                    method, event, window, cx,
                                )
                            }),
                        )
                        .on_click(cx.listener(move |this, event, window, cx| {
                            this.set_active_request_method(method, event, window, cx)
                        }))
                        .into_any_element()
                    })
                    .collect::<Vec<_>>();

                this.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(26.0))
                            .left_0()
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
                            .on_mouse_move(|_, _, _| {})
                            .children(items),
                    )
                    .with_priority(3),
                )
            })
    }

    fn render_method_menu_overlay(
        &self,
        theme: AppTheme,
        _typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.method_menu_open {
            return div().into_any_element();
        }
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
                    .on_mouse_move(|_, _, _| {})
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_method_menu))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::dismiss_method_menu)),
            )
            .into_any_element()
    }

    fn render_body_view_select(
        &self,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let spacing = Spacing::app();
        div()
            .relative()
            .flex_none()
            .child(
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
            .when(self.body_view_menu_open, |this| {
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

                this.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(26.0))
                            .left_0()
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
                            .on_mouse_move(|_, _, _| {})
                            .children(items),
                    )
                    .with_priority(3),
                )
            })
    }

    fn render_body_view_menu_overlay(
        &self,
        theme: AppTheme,
        _typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.body_view_menu_open {
            return div().into_any_element();
        }
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
                    .on_mouse_move(|_, _, _| {})
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_body_view_menu))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(Self::dismiss_body_view_menu),
                    ),
            )
            .into_any_element()
    }

    fn render_response_meta_popover(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.response_meta_popover {
            return div().into_any_element();
        }
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
                    .on_mouse_move(|_, _, _| {})
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(Self::dismiss_response_meta_popover),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(Self::dismiss_response_meta_popover),
                    ),
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
        self.sync_url_input_to_request(cx);
        self.reconcile_path_params_from_url();
        // Update name field inputs when path params are re-keyed by URL changes
        // (e.g. "{a}" deleted and "{abc}" typed — the old field input at index 0
        // still shows "a", so we push the reconciled name into the TextInput).
        if let Some(request) = self.active_request() {
            let request_id = request.id;
            for (index, param) in request.path_params.iter().enumerate() {
                let name_key = format!("req:{request_id}:path:{index}:name");
                if let Some(input) = self.field_inputs.get(&name_key) {
                    let current = input.read(cx).value();
                    if current != param.name.as_ref() {
                        input.update(cx, |input, cx| {
                            input.set_content(param.name.to_string(), cx);
                        });
                    }
                }
            }
        }
        self.sync_path_param_values_from_inputs(cx);
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
                Panel::Path.label(),
                self.active_panel == Panel::Path,
                ButtonStyle::Transparent,
                theme,
                |this, _, window, cx| this.set_panel(Panel::Path, window, cx),
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
                    .h(px(36.0))
                    .bg(theme.surface_background)
                    .px(spacing.base12())
                    .gap(spacing.component_gap())
                    .child(self.render_method_select(&request, theme, typography, cx))
                    .child(self.render_url_input_field(theme, typography, window, cx))
                    .child({
                        let in_flight = self.is_active_request_in_flight();
                        if in_flight {
                            let id = "Send".bytes().fold(0usize, |hash, byte| {
                                hash.wrapping_mul(31).wrapping_add(byte as usize)
                            });
                            ui::button_base_with_size(
                                ("toolbar-button", id),
                                "Send",
                                false,
                                ButtonStyle::Subtle,
                                ui::ButtonSize::Medium,
                                theme,
                            )
                            .into_any_element()
                        } else {
                            self.render_toolbar_button(
                                "Send",
                                true,
                                ButtonStyle::Filled,
                                theme,
                                Self::send_request,
                                cx,
                            )
                            .into_any_element()
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
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
                            .children(panels),
                    ),
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
            Panel::Path => {
                let request = self.active_request().cloned().unwrap();
                let request_id = request.id;

                // Path parameters are auto-synced from {name} patterns in the URL.
                let path_rows: Vec<AnyElement> = request
                    .path_params
                    .iter()
                    .enumerate()
                    .map(|(index, param)| {
                        self.render_editable_pair_row_with_controls(
                            format!("req:{request_id}:path:{index}"),
                            param,
                            index,
                            PairRowKind::Path,
                            true,
                            "Path parameter",
                            "Value",
                            theme,
                            typography,
                            window,
                            cx,
                        )
                    })
                    .collect();

                ui::flat_section(theme)
                    .flex_1()
                    .min_h_0()
                    .children(path_rows)
                    .into_any_element()
            }
            Panel::Params => {
                let request = self.active_request().cloned().unwrap();
                let request_id = request.id;

                self.ensure_param_row();
                let param_count = request.query.len();
                let can_remove_param = param_count > 1;
                let param_rows: Vec<AnyElement> = request
                    .query
                    .iter()
                    .enumerate()
                    .map(|(index, param)| {
                        self.render_editable_pair_row_with_controls(
                            format!("req:{request_id}:param:{index}"),
                            param,
                            index,
                            PairRowKind::Param,
                            can_remove_param,
                            "Query parameter",
                            "Value",
                            theme,
                            typography,
                            window,
                            cx,
                        )
                    })
                    .collect();

                ui::flat_section(theme)
                    .flex_1()
                    .min_h_0()
                    .children(param_rows)
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
                    .children(auth_inputs)
                    .child(
                        div()
                            .px(spacing.base12())
                            .py(spacing.base08())
                            .flex()
                            .flex_col()
                            .gap(spacing.base06())
                            .bg(theme.surface_background)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(spacing.cluster_gap())
                                    .child(
                                        div()
                                            .text_ui_sm(typography)
                                            .text_color(theme.text_muted)
                                            .child("Auth Type"),
                                    )
                                    .child(
                                        self.render_auth_select(&request, theme, typography, cx),
                                    ),
                            )
                            .when(matches!(request.auth, Auth::ApiKey { .. }), |this| {
                                this.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(spacing.cluster_gap())
                                        .child(
                                            div()
                                                .text_ui_sm(typography)
                                                .text_color(theme.text_muted)
                                                .child("Location"),
                                        )
                                        .child(self.render_auth_location_select(
                                            &request, theme, typography, cx,
                                        )),
                                )
                            }),
                    )
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
                    input.set_background_color(theme.editor_background);
                    input.set_gutter_border_color(theme.border);
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
                                            .items_start()
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
                            ))
                            .child(Self::response_tab_button(
                                "Stream",
                                self.active_response_panel == ResponsePanel::Stream,
                                ResponsePanel::Stream,
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
                                    let has_timing = response
                                        .as_ref()
                                        .and_then(|r| r.timing)
                                        .is_some();
                                    this.child(
                                        div()
                                            .id("response-meta-click-target")
                                            .debug_selector(|| "response-meta-group".into())
                                            .relative()
                                            .flex()
                                            .items_center()
                                            .gap(spacing.cluster_gap())
                                            .when(has_timing, |this| this.cursor(CursorStyle::PointingHand))
                                            .when(has_timing, |this| {
                                                this.on_click(cx.listener(|this, _event: &gpui::ClickEvent, _window, cx| {
                                                    this.response_meta_popover = !this.response_meta_popover;
                                                    cx.notify();
                                                }))
                                            })
                                            .child(
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
                                                    .child(duration.clone()),
                                            )
                                            .child(
                                                div()
                                                    .debug_selector(|| "response-size-meta".into())
                                                    .text_color(theme.text_muted)
                                                    .child(size),
                                            )
                                            .when(self.response_meta_popover, |this| {
                                                let timing = response.as_ref().and_then(|r| r.timing).unwrap();
                                                let phases: [(usize, u64, &str); 5] = [
                                                    (0, timing.dns_lookup_ms, "DNS Lookup"),
                                                    (1, timing.connect_ms, "Connect"),
                                                    (2, timing.tls_handshake_ms, "TLS Handshake"),
                                                    (3, timing.time_to_first_byte_ms, "TTFB"),
                                                    (4, timing.transfer_ms, "Transfer"),
                                                ];
                                                let max_val = phases
                                                    .iter()
                                                    .map(|(_, val, _)| *val)
                                                    .max()
                                                    .unwrap_or(1)
                                                    .max(1);
                                                let bar_width = px(80.0);

                                                let rows = phases
                                                    .iter()
                                                    .map(|(index, val, name)| {
                                                        let label_width = px(90.0);
                                                        let pct = if *val > 0 {
                                                            (*val as f32 / max_val as f32).min(1.0)
                                                        } else {
                                                            0.0
                                                        };
                                                        let bar_color = theme.timing_phase_color(*index);
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap(spacing.cluster_gap())
                                                            .child(
                                                                div()
                                                                    .w(label_width)
                                                                    .text_ui_sm(typography)
                                                                    .text_color(theme.text)
                                                                    .child(SharedString::from(*name)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .w(px(48.0))
                                                                    .flex()
                                                                    .justify_end()
                                                                    .text_ui_sm(typography)
                                                                    .text_color(theme.text_muted)
                                                                    .child(SharedString::from(format!("{val} ms"))),
                                                            )
                                                            .child(
                                                                div()
                                                                    .w(bar_width)
                                                                    .h(px(8.0))
                                                                    .rounded(px(3.0))
                                                                    .bg(theme.element_background)
                                                                    .overflow_hidden()
                                                                    .child(
                                                                        div()
                                                                            .h_full()
                                                                            .w(bar_width * pct)
                                                                            .rounded(px(3.0))
                                                                            .bg(bar_color),
                                                                    ),
                                                            )
                                                            .into_any_element()
                                                    })
                                                    .collect::<Vec<_>>();

                                                this.child(
                                                    deferred(
                                                        div()
                                                            .absolute()
                                                            .top(px(20.0))
                                                            .right_0()
                                                            .id("timing-popover")
                                                            .debug_selector(|| "timing-popover".into())
                                                            .w(px(260.0))
                                                            .p(spacing.base12())
                                                            .rounded_sm()
                                                            .border_1()
                                                            .border_color(theme.panel_focused_border)
                                                            .bg(theme.panel_overlay_background)
                                                            .shadow_lg()
                                                            .flex()
                                                            .flex_col()
                                                            .gap(spacing.base08())
                                                            .on_mouse_move(|_, _, _| {})
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_between()
                                                                    .child(
                                                                        ui::label("Timing Breakdown", theme)
                                                                            .font_weight(gpui::FontWeight::BOLD)
                                                                            .text_ui_sm(typography),
                                                                    )
                                                                    .child(
                                                                        ui::muted_label(
                                                                            format!("{} ms total", timing.total_ms()),
                                                                            theme,
                                                                        )
                                                                        .text_ui_xs(typography),
                                                                    ),
                                                            )
                                                            .child(
                                                                div()
                                                                    .h(px(1.0))
                                                                    .w_full()
                                                                    .bg(theme.border_variant),
                                                            )
                                                            .children(rows),
                                                    )
                                                    .with_priority(3),
                                                )
                                            })
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
                (ResponsePanel::Stream, _) => {
                    self.render_response_stream(active_request_id.unwrap_or_default(), theme, cx)
                }
                (_, None) if self.is_active_request_in_flight() => {
                    self.render_response_loading(theme, cx)
                }
                (_, None) => div()
                    .flex_1()
                    .min_h_0()
                    .p(spacing.base12())
                    .text_color(theme.text_muted)
                    .child("Send the request to populate status, headers, body, timing, and size.")
                    .into_any_element(),
            })
    }

    fn render_response_loading(&self, theme: AppTheme, cx: &mut Context<Self>) -> AnyElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let elapsed = self
            .request_started_at
            .map(|start| start.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        let label = if elapsed < 1.0 {
            "Sending request…".into()
        } else {
            format!("Sending request… ({elapsed:.1}s)")
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(spacing.component_gap())
            .p(spacing.base12())
            .bg(theme.surface_background)
            .child(
                div()
                    .text_color(theme.text_muted)
                    .text_ui(typography)
                    .child(label),
            )
            .child(Self::render_button(
                "Cancel",
                true,
                ButtonStyle::Tinted(ui::TintColor::Error),
                theme,
                Self::cancel_active_request,
                cx,
            ))
            .into_any_element()
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

    pub(crate) fn formatted_body_for(
        &mut self,
        request_id: usize,
        response: &ResponseRecord,
    ) -> SharedString {
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

        let is_large = response.body.len() > super::LARGE_RESPONSE_WARNING_BYTES;

        // Binary content (e.g. octet-stream) contains U+FFFD replacement chars
        // that can crash cosmic-text's bidi parser. Show a placeholder instead.
        if looks_like_binary(&response.body) {
            return self.render_binary_response_placeholder(response, theme, cx);
        }

        // For very large bodies we still render via CodeInput.
        // If the user has Pretty selected but the body is large, the formatting
        // guard will have returned raw content. We pass the user's mode anyway
        // (the cache key uses it), but content will be raw.
        let body = self.formatted_body_for(request_id, response);
        let effective_mode = if is_large && self.body_view_mode == BodyViewMode::Pretty {
            BodyViewMode::Raw // display as Raw even if selector says Pretty
        } else {
            self.body_view_mode
        };

        let response_code_input =
            self.response_body_input(request_id, effective_mode, body.clone(), cx);
        let scroll_handle = self.response_scroll_handle.clone();

        response_code_input.update(cx, |input, _cx| {
            input.set_placeholder_color(theme.text_placeholder);
            input.set_syntax_colors(Self::syntax_colors_for_theme(theme));
            input.set_background_color(theme.editor_background);
            input.set_gutter_border_color(theme.border);
        });
        self.response_scrollbar.update(cx, |scrollbar, _cx| {
            scrollbar.set_colors(theme.border_variant, theme.text_muted);
            scrollbar.set_code_input(response_code_input.downgrade());
        });
        let scrollbar = ui::vertical_scrollbar(&self.response_scrollbar);

        let response_content_width = response_code_input.read(cx).last_content_width();

        self.response_horizontal_scrollbar
            .update(cx, |scrollbar, _cx| {
                scrollbar.set_colors(theme.border_variant, theme.text_muted);
                scrollbar.set_code_input(response_code_input.downgrade());
                // Push the CodeInput's measured (or estimated) content width so the
                // horizontal bar can show a thumb even on the first frame.
                scrollbar.set_content_width(response_content_width);
            });
        let horizontal_scrollbar = ui::horizontal_scrollbar(&self.response_horizontal_scrollbar);

        let mut body_area = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.surface_background);

        if is_large {
            let size_label = format_byte_count(response.body.len());
            let body_for_save = response.body.clone();
            let suggested_name = suggest_response_filename(response);

            body_area = body_area.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(spacing.base08())
                    .py(spacing.base04())
                    .bg(theme.element_background)
                    .text_color(theme.text_muted)
                    .text_ui_sm(typography)
                    .child(SharedString::from(format!(
                        "Large response ({size_label}). Pretty mode may be slow."
                    )))
                    .child(Self::render_button(
                        "Save to file",
                        false,
                        ButtonStyle::Transparent,
                        theme,
                        move |this, _event, _window, cx| {
                            this.save_response_body_to_file(
                                body_for_save.clone(),
                                suggested_name.clone(),
                                cx,
                            );
                        },
                        cx,
                    )),
            );
        }

        body_area
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
                            .items_start() // allow the CodeInput to be wider than the container (for horizontal scroll)
                            .overflow_scroll()
                            .track_scroll(&scroll_handle)
                            .p(spacing.base08())
                            .font_family(ui::JETBRAINS_FONT_FAMILY)
                            .text_buffer(typography)
                            .text_color(theme.editor_text)
                            .child(response_code_input),
                    )
                    .child(scrollbar)
                    .child(horizontal_scrollbar)
                    .children(if self.find_state.is_some() {
                        Some(self.render_find_bar(theme, cx))
                    } else {
                        None
                    }),
            )
            .into_any_element()
    }

    fn render_find_bar(&mut self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();

        let find_state = self.find_state.as_ref().unwrap();
        let query_input = find_state.query_input.clone();

        let status_label = if find_state.matches.is_empty() {
            if query_input.read(cx).value().is_empty() {
                "".to_string()
            } else {
                "No matches".to_string()
            }
        } else {
            let current = find_state.active_match_idx.unwrap_or(0) + 1;
            format!("{} of {}", current, find_state.matches.len())
        };

        div()
            .absolute()
            .top(spacing.base08())
            .right(spacing.base24()) // padding from the right vertical scrollbar
            .flex()
            .items_center()
            .gap(spacing.base06())
            .px(spacing.base08())
            .h(px(28.0))
            .bg(theme.panel_overlay_background)
            .border_1()
            .border_color(theme.border)
            .rounded(px(4.0))
            .child(ui::icon(
                IconName::Search,
                ui::IconSize::Small,
                theme.text_muted,
            ))
            .child(
                div()
                    .w(px(140.0))
                    .h(px(20.0))
                    .key_context("FindQueryInput")
                    .child(query_input),
            )
            .child(
                div()
                    .text_color(theme.text_muted)
                    .text_ui_xs(typography)
                    .child(SharedString::from(status_label)),
            )
            .child(div().w(px(1.0)).h(px(16.0)).bg(theme.border))
            .child(
                div()
                    .id("find-prev-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(16.0))
                    .h(px(16.0))
                    .rounded(px(2.0))
                    .hover(|style| style.bg(theme.element_hover))
                    .active(|style| style.bg(theme.element_active))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.find_previous_from_action(&FindPrevious, window, cx);
                    }))
                    .text_color(theme.icon)
                    .text_ui_sm(typography)
                    .child("▲"),
            )
            .child(
                div()
                    .id("find-next-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(16.0))
                    .h(px(16.0))
                    .rounded(px(2.0))
                    .hover(|style| style.bg(theme.element_hover))
                    .active(|style| style.bg(theme.element_active))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.find_next_from_action(&FindNext, window, cx);
                    }))
                    .text_color(theme.icon)
                    .text_ui_sm(typography)
                    .child("▼"),
            )
            .child(div().w(px(1.0)).h(px(16.0)).bg(theme.border))
            .child(
                div()
                    .id("find-close-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(16.0))
                    .h(px(16.0))
                    .rounded(px(2.0))
                    .hover(|style| style.bg(theme.element_hover))
                    .active(|style| style.bg(theme.element_active))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_find_from_action(&CloseFind, window, cx);
                    }))
                    .child(ui::icon(IconName::Close, ui::IconSize::Small, theme.icon)),
            )
    }

    fn render_binary_response_placeholder(
        &self,
        response: &ResponseRecord,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let body = response.body.clone();
        let suggested_name = suggest_response_filename(response);
        div()
            .debug_selector(|| "response-body-binary-placeholder".into())
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
                    .child(SharedString::from(
                        "Binary response body — not displayed inline.",
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
            .flex()
            .flex_col()
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
        let value_for_text = header.value.clone();
        let value_for_icon = value_for_text.clone();
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event, window, cx| {
                            this.copy_response_header_value_on_mouse_down(
                                value_for_text.clone(),
                                event,
                                window,
                                cx,
                            )
                        }),
                    )
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
                                this.copy_response_header_value(
                                    value_for_icon.clone(),
                                    event,
                                    window,
                                    cx,
                                )
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
                            PairRowKind::Path => this.toggle_path_enabled(index, cx),
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
                                    PairRowKind::Path => this.remove_path_row(index, cx),
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

    fn render_auth_select(
        &mut self,
        request: &Request,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let spacing = Spacing::app();
        let auth_handle = self.tab_handle("auth-select", cx);
        div()
            .relative()
            .flex_none()
            .child(
                ui::select_trigger_with_size(
                    "auth-select-trigger",
                    request.auth.label(),
                    theme.accent,
                    self.auth_menu_open,
                    ui::ButtonSize::Default,
                    theme,
                    typography,
                )
                .debug_selector(|| "auth-select-trigger".into())
                .track_focus(&auth_handle)
                .tab_stop(true)
                .on_click(cx.listener(Self::toggle_auth_menu)),
            )
            .when(self.auth_menu_open, |this| {
                let items: Vec<AnyElement> = [
                    Auth::None,
                    Auth::Bearer {
                        label: "token".into(),
                        secret_ref: "{{token}}".into(),
                    },
                    Auth::Basic {
                        username: "user".into(),
                        password: "{{password}}".into(),
                    },
                    Auth::ApiKey {
                        name: "x-api-key".into(),
                        secret_ref: "{{token}}".into(),
                        location: AuthLocation::Header,
                    },
                ]
                .into_iter()
                .enumerate()
                .map(|(index, mode)| {
                    let label = mode.label();
                    let is_current =
                        std::mem::discriminant(&request.auth) == std::mem::discriminant(&mode);
                    let debug_selector = format!(
                        "auth-option-{}",
                        label.to_ascii_lowercase().replace(' ', "-")
                    );
                    let mode_slot = std::sync::Arc::new(mode);
                    let item_handle = cx.focus_handle().tab_stop(true);
                    ui::select_menu_item(
                        ("auth-option", index),
                        label,
                        theme.accent,
                        is_current,
                        theme,
                        typography,
                    )
                    .debug_selector(move || debug_selector.clone())
                    .track_focus(&item_handle)
                    .tab_stop(true)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let mode_slot = mode_slot.clone();
                            move |this, event, window, cx| {
                                this.set_auth_mode_from_mouse_down(
                                    (*mode_slot).clone(),
                                    event,
                                    window,
                                    cx,
                                )
                            }
                        }),
                    )
                    .on_click(cx.listener(move |this, event, window, cx| {
                        this.set_auth_mode((*mode_slot).clone(), event, window, cx)
                    }))
                    .into_any_element()
                })
                .collect();

                this.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(26.0))
                            .left_0()
                            .id("auth-select-menu")
                            .debug_selector(|| "auth-select-menu".into())
                            .w(px(130.0))
                            .p(spacing.base04())
                            .rounded_sm()
                            .border_1()
                            .border_color(theme.panel_focused_border)
                            .bg(theme.panel_overlay_background)
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .gap(spacing.base04())
                            .on_mouse_move(|_, _, _| {})
                            .children(items),
                    )
                    .with_priority(3),
                )
            })
    }

    fn render_auth_location_select(
        &self,
        request: &Request,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let spacing = Spacing::app();
        div()
            .relative()
            .flex_none()
            .child(
                ui::select_trigger_with_size(
                    "auth-location-select-trigger",
                    match &request.auth {
                        Auth::ApiKey { location, .. } => location.label(),
                        _ => "header",
                    },
                    theme.accent,
                    self.auth_location_menu_open,
                    ui::ButtonSize::Default,
                    theme,
                    typography,
                )
                .debug_selector(|| "auth-location-select-trigger".into())
                .on_click(cx.listener(Self::toggle_auth_location_menu)),
            )
            .when(self.auth_location_menu_open, |this| {
                let items: Vec<AnyElement> = AuthLocation::all()
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, location)| {
                        let is_current = matches!(&request.auth, Auth::ApiKey { location: loc, .. } if *loc == location);
                        let debug_selector =
                            format!("auth-location-option-{}", location.label());
                        ui::select_menu_item(
                            ("auth-location-option", index),
                            location.label(),
                            theme.accent,
                            is_current,
                            theme,
                            typography,
                        )
                        .debug_selector(move || debug_selector.clone())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event, window, cx| {
                                this.set_api_key_location_from_mouse_down(location, event, window, cx)
                            }),
                        )
                        .on_click(cx.listener(move |this, event, window, cx| {
                            this.set_api_key_location(location, event, window, cx)
                        }))
                        .into_any_element()
                    })
                    .collect();

                this.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(26.0))
                            .left_0()
                            .id("auth-location-select-menu")
                            .debug_selector(|| "auth-location-select-menu".into())
                            .w(px(120.0))
                            .p(spacing.base04())
                            .rounded_sm()
                            .border_1()
                            .border_color(theme.panel_focused_border)
                            .bg(theme.panel_overlay_background)
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .gap(spacing.base04())
                            .on_mouse_move(|_, _, _| {})
                            .children(items),
                    )
                    .with_priority(3),
                )
            })
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

    fn render_rename_collection_dialog(
        &self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.rename_collection_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "rename-collection-dialog".into())
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
                    .child(ui::panel_header("Rename Collection", "", theme, typography))
                    .child(Self::render_field_input(
                        "rename-collection-input-shell",
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
                                Self::close_rename_collection_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Rename",
                                false,
                                ButtonStyle::Filled,
                                theme,
                                Self::confirm_rename_collection,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_delete_collection_dialog(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.delete_collection_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "delete-collection-dialog".into())
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
                    .child(ui::panel_header("Delete Collection", "", theme, typography))
                    .child(
                        ui::muted_label(
                            format!("Delete '{}' permanently? All nested requests and folders will be removed.", dialog.collection_name),
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
                                Self::close_delete_collection_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Delete",
                                false,
                                ButtonStyle::Tinted(ui::TintColor::Error),
                                theme,
                                Self::confirm_delete_collection,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_import_openapi_dialog(
        &self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.import_openapi_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "import-openapi-dialog".into())
                    .absolute()
                    .top(px(110.0))
                    .left(px(300.0))
                    .right(px(300.0))
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
                        "Import OpenAPI Spec",
                        "",
                        theme,
                        typography,
                    ))
                    .child(
                        ui::muted_label(
                            "Enter the path to an OpenAPI 3.x JSON or YAML file.",
                            theme,
                        )
                        .text_ui_sm(typography),
                    )
                    .child(Self::render_field_input(
                        "import-openapi-input-shell",
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
                                Self::close_import_openapi_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Import",
                                false,
                                ButtonStyle::Filled,
                                theme,
                                Self::confirm_import_openapi,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_export_openapi_dialog(
        &self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.export_openapi_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "export-openapi-dialog".into())
                    .absolute()
                    .top(px(110.0))
                    .left(px(300.0))
                    .right(px(300.0))
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
                        "Export OpenAPI Spec",
                        "",
                        theme,
                        typography,
                    ))
                    .child(
                        ui::muted_label(
                            "Enter the output path for the OpenAPI 3.x JSON file.",
                            theme,
                        )
                        .text_ui_sm(typography),
                    )
                    .child(Self::render_field_input(
                        "export-openapi-input-shell",
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
                                Self::close_export_openapi_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Export",
                                false,
                                ButtonStyle::Filled,
                                theme,
                                Self::confirm_export_openapi,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_collection_context_menu(
        &self,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(menu) = self.collection_context_menu else {
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
            .child(deferred(
                anchored()
                    .anchor(Corner::TopLeft)
                    .position(menu.position)
                    .child(
                        ui::context_menu_panel("collection-context-menu", theme)
                            .debug_selector(|| "collection-context-menu".into())
                            .child(
                                ui::context_menu_item(
                                    "collection-context-new-request",
                                    "New Request",
                                    theme,
                                    typography,
                                )
                                .debug_selector(|| "collection-context-new-request".into())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(Self::add_request_to_context_collection_mouse_down),
                                )
                                .on_click(cx.listener(Self::add_request_to_context_collection)),
                            )
                            .child(
                                ui::context_menu_item(
                                    "collection-context-new-folder",
                                    "New Folder",
                                    theme,
                                    typography,
                                )
                                .debug_selector(|| "collection-context-new-folder".into())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(Self::add_folder_to_context_collection_mouse_down),
                                )
                                .on_click(cx.listener(Self::add_folder_to_context_collection)),
                            )
                            .child(
                                ui::context_menu_item(
                                    "collection-context-rename",
                                    "Rename...",
                                    theme,
                                    typography,
                                )
                                .debug_selector(|| "collection-context-rename".into())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        Self::start_rename_collection_from_context_menu_mouse_down,
                                    ),
                                )
                                .on_click(
                                    cx.listener(Self::start_rename_collection_from_context_menu),
                                ),
                            )
                            .child(
                                ui::context_menu_item(
                                    "collection-context-delete",
                                    "Delete",
                                    theme,
                                    typography,
                                )
                                .debug_selector(|| "collection-context-delete".into())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        Self::start_delete_collection_from_context_menu_mouse_down,
                                    ),
                                )
                                .on_click(
                                    cx.listener(Self::start_delete_collection_from_context_menu),
                                ),
                            )
                            .child(
                                ui::context_menu_item(
                                    "collection-context-import-openapi",
                                    "Import OpenAPI Spec...",
                                    theme,
                                    typography,
                                )
                                .debug_selector(|| "collection-context-import-openapi".into())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        Self::start_import_openapi_from_context_menu_mouse_down,
                                    ),
                                )
                                .on_click(
                                    cx.listener(Self::start_import_openapi_from_context_menu),
                                ),
                            )
                            .child(
                                ui::context_menu_item(
                                    "collection-context-export-openapi",
                                    "Export as OpenAPI Spec...",
                                    theme,
                                    typography,
                                )
                                .debug_selector(|| "collection-context-export-openapi".into())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        Self::start_export_openapi_from_context_menu_mouse_down,
                                    ),
                                )
                                .on_click(
                                    cx.listener(Self::start_export_openapi_from_context_menu),
                                ),
                            ),
                    ),
            ))
    }

    fn render_workspace_menu(&self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.workspace_menu_open {
            return div().into_any_element();
        }
        let typography = self.typography();
        let spacing = Spacing::app();

        ui::modal_overlay(theme)
            .child(
                div()
                    .id("workspace-menu-dismiss-overlay")
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .occlude()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dismiss_workspace_menu(cx);
                    })),
            )
            .child(
                div()
                    .id("workspace-switcher-menu")
                    .absolute()
                    .top(px(40.0))
                    .left(px(8.0))
                    .w(px(240.0))
                    .bg(theme.panel_overlay_background)
                    .border_1()
                    .border_color(theme.panel_focused_border)
                    .rounded_sm()
                    .shadow_lg()
                    .occlude()
                    .p(spacing.base04())
                    .flex()
                    .flex_col()
                    .gap(spacing.base04())
                    .children(
                        self.workspaces_list
                            .iter()
                            .enumerate()
                            .map(|(idx, (name, path))| {
                                let path_clone = path.clone();
                                let active = path == &self.workspace_path;
                                ui::list_item(("workspace-menu-item", idx), active, theme)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.switch_to_workspace(path_clone.clone(), cx);
                                        this.workspace_menu_open = false;
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .w_full()
                                            .child(
                                                ui::label(name.clone(), theme)
                                                    .text_ui_xs(typography),
                                            )
                                            .child(if active {
                                                div()
                                                    .text_ui_xs(typography)
                                                    .text_color(theme.text)
                                                    .child("✓")
                                            } else {
                                                div()
                                            }),
                                    )
                            }),
                    )
                    .child(div().h_px().bg(theme.border_variant))
                    .child(
                        ui::list_item("workspace-menu-create-btn", false, theme)
                            .on_click(cx.listener(Self::open_create_workspace_dialog))
                            .child(
                                ui::label("+ New Workspace", theme)
                                    .text_ui_xs(typography)
                                    .text_color(theme.text),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_create_workspace_dialog(
        &self,
        theme: AppTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let Some(dialog) = self.create_workspace_dialog.as_ref() else {
            return div().into_any_element();
        };
        let typography = self.typography();
        let spacing = Spacing::app();
        ui::modal_overlay(theme)
            .child(Self::render_dialog_backdrop(cx))
            .child(
                div()
                    .debug_selector(|| "create-workspace-dialog".into())
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
                    .child(ui::panel_header(
                        "Create New Workspace",
                        "",
                        theme,
                        typography,
                    ))
                    .child(Self::render_field_input(
                        "create-workspace-input-shell",
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
                                Self::close_create_workspace_dialog,
                                cx,
                            ))
                            .child(Self::render_button(
                                "Create",
                                false,
                                ButtonStyle::Filled,
                                theme,
                                Self::confirm_create_workspace,
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

    fn render_response_stream(
        &mut self,
        request_id: usize,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let spacing = Spacing::app();
        let typography = self.typography();

        let stream_state = self
            .stream_state
            .get(&request_id)
            .cloned()
            .unwrap_or_else(StreamState::new);

        let status_text = match stream_state.status {
            StreamStatus::Disconnected => "Disconnected",
            StreamStatus::Connecting => "Connecting…",
            StreamStatus::Connected => "Connected",
        };

        let status_color = match stream_state.status {
            StreamStatus::Disconnected => theme.text_muted,
            StreamStatus::Connecting => theme.warning,
            StreamStatus::Connected => theme.success,
        };

        let list_state = self.stream_list_state.clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.surface_background)
            // Status header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(spacing.base12())
                    .py(spacing.base08())
                    .border_b_1()
                    .border_color(theme.border_variant)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(spacing.base08())
                            .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                            .child(
                                div()
                                    .text_color(status_color)
                                    .text_ui_sm(typography)
                                    .child(status_text),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(spacing.base08())
                            .when(stream_state.status == StreamStatus::Disconnected, |this| {
                                this.child(
                                    ui::button_base(
                                        "stream-connect",
                                        "Connect",
                                        false,
                                        ButtonStyle::Tinted(ui::TintColor::Accent),
                                        theme,
                                    )
                                    .on_click(cx.listener(
                                        |this, event, window, cx| {
                                            this.start_stream(event, window, cx);
                                        },
                                    )),
                                )
                            })
                            .when(
                                stream_state.status == StreamStatus::Connected
                                    || stream_state.status == StreamStatus::Connecting,
                                |this| {
                                    this.child(
                                        ui::button_base(
                                            "stream-disconnect",
                                            "Disconnect",
                                            false,
                                            ButtonStyle::Tinted(ui::TintColor::Error),
                                            theme,
                                        )
                                        .on_click(
                                            cx.listener(|this, event, window, cx| {
                                                this.stop_stream(event, window, cx);
                                            }),
                                        ),
                                    )
                                },
                            ),
                    ),
            )
            // Error message
            .when_some(stream_state.error.as_ref(), |this, error| {
                this.child(
                    div()
                        .px(spacing.base12())
                        .py(spacing.base08())
                        .text_color(theme.error)
                        .text_ui_sm(typography)
                        .child(format!("Error: {}", error)),
                )
            })
            // Message list — only visible items are rendered by GPUI
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        gpui_list(list_state, cx.processor(Self::render_stream_message))
                            .with_sizing_behavior(gpui::ListSizingBehavior::Auto)
                            .size_full(),
                    )
                    .child({
                        self.stream_list_scrollbar.update(cx, |scrollbar, _cx| {
                            scrollbar.set_colors(theme.border_variant, theme.text_muted);
                        });
                        self.stream_list_scrollbar.clone()
                    }),
            )
            .into_any_element()
    }

    fn render_stream_message(
        &mut self,
        msg_idx: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = AppTheme::for_mode(self.theme_mode);
        let spacing = Spacing::app();
        let typography = self.typography();
        let request_id = self.active_request_id.unwrap_or(0);

        let Some(state) = self.stream_state.get(&request_id) else {
            return div().into_any_element();
        };
        let Some(msg) = state.messages.get(msg_idx) else {
            return div().into_any_element();
        };

        let is_expanded = self
            .stream_expanded_messages
            .contains(&(request_id, msg_idx));
        let lines_count = msg.data.split('\n').count();
        let is_large = lines_count > 1 || msg.data.len() > 120;

        fn format_wall_clock_time(timestamp_ms: u64) -> Option<SharedString> {
            let secs = timestamp_ms / 1000;
            if secs > 0 {
                let hours = (secs / 3600) % 24;
                let minutes = (secs / 60) % 60;
                let seconds = secs % 60;
                Some(format!("{:02}:{:02}:{:02}", hours, minutes, seconds).into())
            } else {
                None
            }
        }

        let time_str = format_wall_clock_time(msg.received_at);
        let dir_arrow = match msg.direction {
            domain::StreamDirection::Sent => "↑",
            domain::StreamDirection::Received => "↓",
        };
        let header_chevron = if is_large {
            if is_expanded { "▼ " } else { "▶ " }
        } else {
            ""
        };
        let timestamp_label = match &time_str {
            Some(t) => format!("{}[{}] {}", header_chevron, t, dir_arrow),
            None => format!("{}[00:00:00] {}", header_chevron, dir_arrow),
        };

        let formatted_data: SharedString = if msg.is_json {
            serde_json::to_string_pretty(&msg.data)
                .unwrap_or(msg.data.to_string())
                .into()
        } else {
            msg.data.clone()
        };

        let display_data: SharedString = if is_large && !is_expanded {
            let first_line = msg.data.lines().next().unwrap_or("").trim_end();
            let truncated = if first_line.len() > 120 {
                format!("{}...", &first_line[..120])
            } else if lines_count > 1 {
                format!("{}...", first_line)
            } else {
                first_line.to_string()
            };
            truncated.into()
        } else {
            formatted_data.clone()
        };

        let msg_data_for_copy = formatted_data;
        let toggle_key = (request_id, msg_idx);

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .id(("stream-header", msg_idx))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(spacing.base12())
                    .py(spacing.base04())
                    .border_b_1()
                    .border_color(theme.border_variant)
                    .text_color(theme.text_muted)
                    .text_ui_sm(typography)
                    .when(is_large, |this| {
                        this.cursor_pointer()
                            .hover(|style| style.bg(theme.element_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.stream_expanded_messages.contains(&toggle_key) {
                                    this.stream_expanded_messages.remove(&toggle_key);
                                } else {
                                    this.stream_expanded_messages.insert(toggle_key);
                                }
                                // Force remeasurement of this item's height.
                                this.stream_list_state.splice(msg_idx..msg_idx + 1, 1);
                                cx.notify();
                            }))
                    })
                    .child(timestamp_label)
                    .child(
                        ui::icon_button_base(
                            ("stream-msg-copy", msg_idx),
                            IconName::Copy,
                            false,
                            theme,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                msg_data_for_copy.to_string(),
                            ));
                            this.status_line = "Copied stream message.".into();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .w_full()
                    .bg(theme.editor_background)
                    .p(spacing.base08())
                    .font_family(ui::JETBRAINS_FONT_FAMILY)
                    .text_buffer(typography)
                    .text_color(theme.editor_text)
                    .whitespace_normal()
                    .child(display_data),
            )
            .into_any_element()
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
            .on_action(cx.listener(Self::new_tab_from_action))
            .on_action(cx.listener(Self::close_tab_from_action))
            .on_action(cx.listener(Self::toggle_find))
            .on_action(cx.listener(Self::find_next_from_action))
            .on_action(cx.listener(Self::find_previous_from_action))
            .on_action(cx.listener(Self::close_find_from_action))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_prev))
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
            .child(self.render_workspace_menu(theme, cx))
            .child(self.render_create_workspace_dialog(theme, window, cx))
            .child(self.render_rename_request_dialog(theme, window, cx))
            .child(self.render_delete_request_dialog(theme, cx))
            .child(self.render_rename_folder_dialog(theme, window, cx))
            .child(self.render_delete_folder_dialog(theme, cx))
            .child(self.render_rename_collection_dialog(theme, window, cx))
            .child(self.render_delete_collection_dialog(theme, cx))
            .child(self.render_import_openapi_dialog(theme, window, cx))
            .child(self.render_export_openapi_dialog(theme, window, cx))
            .child(self.render_request_context_menu(theme, cx))
            .child(self.render_folder_context_menu(theme, cx))
            .child(self.render_collection_context_menu(theme, cx))
            .child(self.render_method_menu_overlay(theme, typography, cx))
            .child(self.render_body_view_menu_overlay(theme, typography, cx))
            .child(self.render_response_meta_popover(theme, cx))
            .when(!window.is_maximized() && !window.is_fullscreen(), |this| {
                this.children(Self::render_resize_hitboxes())
            })
    }
}
