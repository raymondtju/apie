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
        ui::button_base_with_size(
            ("toolbar-button", id),
            label,
            active,
            style,
            ui::ButtonSize::Medium,
            theme,
        )
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
            ui::ButtonSize::Default,
            theme,
        )
        .w(ui::ButtonSize::Default.height())
        .child(ui::icon(icon, ui::IconSize::Small, icon_color))
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
                        Some(CollectionSelection::Request(id)) if id == request_id
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
                                            .w(px(42.0))
                                            .text_ui_xs(typography)
                                            .text_color(request.method.color())
                                            .child(request.method.as_str()),
                                    )
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.text)
                                            .truncate()
                                            .child(request.name.clone()),
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
                        Some(CollectionSelection::Folder(id)) if id == folder_id
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
            .into_any_element()
    }

    fn render_sidebar(&self, theme: AppTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let request_rows =
            self.render_collection_items(&self.workspace.items, 0, theme, typography, cx);

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
                    .p(spacing.base12())
                    .child(
                        ui::label(self.workspace.name.clone(), theme)
                            .text_ui_lg(typography)
                            .font_weight(gpui::FontWeight::BOLD),
                    ),
            )
            .child(
                ui::toolbar("collection-toolbar", theme)
                    .h(px(34.0))
                    .p(spacing.base08())
                    .child(Self::render_button(
                        "+ Request",
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
        &self,
        request: &Request,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current_method = request.method;

        div().relative().flex_none().child(
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
            .on_click(cx.listener(Self::toggle_method_menu)),
        )
    }

    fn render_method_menu_overlay(
        &self,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.method_menu_open {
            return div().into_any_element();
        }
        let Some(request) = self.active_request() else {
            return div().into_any_element();
        };
        let spacing = Spacing::app();
        let current_method = request.method;
        let position = self
            .method_menu_position
            .unwrap_or_else(|| point(px(0.0), px(0.0)));
        let items = Method::all()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, method)| {
                let debug_selector = format!("method-option-{}", method.as_str());
                ui::select_menu_item(
                    ("method-option", index),
                    method.as_str(),
                    method.color(),
                    method == current_method,
                    theme,
                    typography,
                )
                .debug_selector(move || debug_selector.clone())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event, window, cx| {
                        this.set_active_request_method_from_mouse_down(method, event, window, cx)
                    }),
                )
                .on_click(cx.listener(move |this, event, window, cx| {
                    this.set_active_request_method(method, event, window, cx)
                }))
                .into_any_element()
            })
            .collect::<Vec<_>>();

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
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_method_menu))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::dismiss_method_menu)),
            )
            .child(
                deferred(
                    anchored().anchor(Corner::TopLeft).position(position).child(
                        div()
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
                            .children(items),
                    ),
                )
                .with_priority(3),
            )
            .into_any_element()
    }

    fn render_body_view_select(
        &self,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().relative().flex_none().child(
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
    }

    fn render_body_view_menu_overlay(
        &self,
        theme: AppTheme,
        typography: Typography,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.body_view_menu_open {
            return div().into_any_element();
        }
        let spacing = Spacing::app();
        let position = self
            .body_view_menu_position
            .unwrap_or_else(|| point(px(0.0), px(0.0)));
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
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_body_view_menu))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(Self::dismiss_body_view_menu),
                    ),
            )
            .child(
                deferred(
                    anchored().anchor(Corner::TopLeft).position(position).child(
                        div()
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
                            .children(items),
                    ),
                )
                .with_priority(3),
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
                    .h(px(40.0))
                    .border_0()
                    .bg(theme.surface_background)
                    .px(spacing.base12())
                    .gap(spacing.component_gap())
                    .child(self.render_method_select(&request, theme, typography, cx))
                    .child(self.render_url_input_field(theme, typography, window, cx))
                    .child(if self.is_active_request_in_flight() {
                        Self::render_toolbar_button(
                            "Cancel",
                            true,
                            ButtonStyle::Tinted(ui::TintColor::Error),
                            theme,
                            Self::cancel_active_request,
                            cx,
                        )
                        .into_any_element()
                    } else {
                        Self::render_toolbar_button(
                            "Send",
                            true,
                            ButtonStyle::Filled,
                            theme,
                            Self::send_request,
                            cx,
                        )
                        .into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(spacing.cluster_gap())
                    .px(spacing.base12())
                    .py(spacing.base04())
                    .border_b_1()
                    .border_color(theme.border_variant)
                    .bg(theme.toolbar_background)
                    .children(panels),
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
            Panel::Params => {
                self.ensure_param_row();
                let request = self.active_request().cloned().unwrap();
                let request_id = request.id;
                let row_count = request.query.len();
                let can_remove = row_count > 1;
                let rows = request
                    .query
                    .iter()
                    .enumerate()
                    .map(|(index, param)| {
                        self.render_editable_pair_row_with_controls(
                            format!("req:{request_id}:param:{index}"),
                            param,
                            index,
                            PairRowKind::Param,
                            can_remove,
                            "Query parameter",
                            "Value",
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
                    .child(
                        ui::panel_header(
                            "Authentication",
                            request.auth.summary(),
                            theme,
                            typography,
                        )
                        .bg(theme.surface_background),
                    )
                    .children(auth_inputs)
                    .child(
                        div()
                            .px(spacing.base12())
                            .py(spacing.base08())
                            .flex()
                            .items_center()
                            .gap(spacing.cluster_gap())
                            .bg(theme.surface_background)
                            .child(Self::render_button(
                                "Cycle Auth Mode",
                                false,
                                ButtonStyle::Subtle,
                                theme,
                                Self::cycle_auth,
                                cx,
                            )),
                    )
                    .when(matches!(request.auth, Auth::ApiKey { .. }), |this| {
                        this.child(
                            div()
                                .px(spacing.base12())
                                .pb(spacing.base08())
                                .bg(theme.surface_background)
                                .child(Self::render_button(
                                    "Cycle API Key Location",
                                    false,
                                    ButtonStyle::Transparent,
                                    theme,
                                    Self::cycle_api_key_location,
                                    cx,
                                )),
                        )
                    })
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
                                    this.child(
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
                                        .child(duration),
                                )
                                .child(
                                    div()
                                        .debug_selector(|| "response-size-meta".into())
                                        .text_color(theme.text_muted)
                                        .child(size),
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
                (_, None) => div()
                    .flex_1()
                    .min_h_0()
                    .p(spacing.base12())
                    .text_color(theme.text_muted)
                    .child("Send the request to populate status, headers, body, timing, and size.")
                    .into_any_element(),
            })
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

    fn formatted_body_for(&mut self, request_id: usize, response: &ResponseRecord) -> SharedString {
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
            body_len: response.body.len(),
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

        if response.body.len() > MAX_INLINE_RESPONSE_BODY_BYTES {
            return self.render_oversize_response_placeholder(response, theme, cx);
        }

        let body = self.formatted_body_for(request_id, response);
        let response_code_input =
            self.response_body_input(request_id, self.body_view_mode, body.clone(), cx);
        response_code_input.update(cx, |input, _cx| {
            input.set_placeholder_color(theme.text_placeholder);
            input.set_syntax_colors(Self::syntax_colors_for_theme(theme));
        });
        let scroll_handle = self.response_scroll_handle.clone();
        self.response_scrollbar.update(cx, |scrollbar, _cx| {
            scrollbar.set_colors(theme.border_variant, theme.text_muted);
            scrollbar.set_code_input(response_code_input.downgrade());
        });
        let scrollbar = ui::vertical_scrollbar(&self.response_scrollbar);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.surface_background)
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
                            .overflow_scroll()
                            .track_scroll(&scroll_handle)
                            .p(spacing.base08())
                            .font_family(ui::JETBRAINS_FONT_FAMILY)
                            .text_buffer(typography)
                            .text_color(theme.editor_text)
                            .child(response_code_input),
                    )
                    .child(scrollbar),
            )
            .into_any_element()
    }

    fn render_oversize_response_placeholder(
        &self,
        response: &ResponseRecord,
        theme: AppTheme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let typography = self.typography();
        let spacing = Spacing::app();
        let size_label = format_byte_count(response.body.len());
        let body = response.body.clone();
        let suggested_name = suggest_response_filename(response);
        div()
            .debug_selector(|| "response-body-oversize-placeholder".into())
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
                    .child(SharedString::from(format!(
                        "Body too large to render inline ({size_label})."
                    ))),
            )
            .child(
                div()
                    .text_color(theme.text_muted)
                    .text_ui_sm(typography)
                    .child(SharedString::from(
                        "Save the body to a file to inspect it with another tool.",
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
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event, window, cx| {
                        this.copy_response_header_value_on_mouse_down(value_for_text.clone(), event, window, cx)
                    }))
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
                                this.copy_response_header_value(value_for_icon.clone(), event, window, cx)
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
            .border_b_1()
            .border_color(theme.border_variant)
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
            .child(self.render_rename_request_dialog(theme, window, cx))
            .child(self.render_delete_request_dialog(theme, cx))
            .child(self.render_rename_folder_dialog(theme, window, cx))
            .child(self.render_delete_folder_dialog(theme, cx))
            .child(self.render_request_context_menu(theme, cx))
            .child(self.render_folder_context_menu(theme, cx))
            .child(self.render_method_menu_overlay(theme, typography, cx))
            .child(self.render_body_view_menu_overlay(theme, typography, cx))
            .when(!window.is_maximized() && !window.is_fullscreen(), |this| {
                this.children(Self::render_resize_hitboxes())
            })
    }
}
