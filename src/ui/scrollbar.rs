use std::time::{Duration, Instant};

use super::*;
use gpui::{
    Bounds, Context, CursorStyle, DragMoveEvent, Entity, InteractiveElement, ListState,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Point, Render, ScrollHandle,
    StatefulInteractiveElement, WeakEntity, Window, div, point, px,
};

const SCROLLBAR_WIDTH: Pixels = px(8.0);
const THUMB_WIDTH: Pixels = px(6.0);
const MIN_THUMB_HEIGHT: Pixels = px(20.0);
const NOTIFY_THROTTLE: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug)]
struct ScrollbarGeometry {
    track_bounds: Bounds<Pixels>,
    thumb_bounds: Bounds<Pixels>,
    max_scroll: Pixels,
}

#[derive(Clone, Copy, Debug)]
struct ScrollbarDrag {
    cursor_thumb_offset: Pixels,
}

#[derive(Clone)]
struct ScrollbarDragToken;

struct ScrollbarDragGhost;

pub(crate) struct VerticalScrollbar {
    scroll_handle: ScrollHandle,
    thumb_color: Hsla,
    thumb_hover_color: Hsla,
    track_bounds: Option<Bounds<Pixels>>,
    thumb_bounds: Option<Bounds<Pixels>>,
    active_drag: Option<ScrollbarDrag>,
    code_input_target: Option<WeakEntity<CodeInput>>,
    last_notify: Option<Instant>,
}

impl VerticalScrollbar {
    pub(crate) fn new(scroll_handle: ScrollHandle) -> Self {
        Self {
            scroll_handle,
            thumb_color: rgba(0x00000033).into(),
            thumb_hover_color: rgba(0x00000066).into(),
            track_bounds: None,
            thumb_bounds: None,
            active_drag: None,
            code_input_target: None,
            last_notify: None,
        }
    }

    pub(crate) fn set_code_input(&mut self, target: WeakEntity<CodeInput>) {
        self.code_input_target = Some(target);
    }

    fn notify_code_input(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if self
            .last_notify
            .is_some_and(|t| now.duration_since(t) < NOTIFY_THROTTLE)
        {
            return;
        }
        self.last_notify = Some(now);
        if let Some(target) = &self.code_input_target {
            if let Some(input) = target.upgrade() {
                let _ = cx.update_entity(&input, |_, cx| cx.notify());
            }
        }
    }

    fn notify_code_input_drag(&mut self, dragging: bool, cx: &mut Context<Self>) {
        if let Some(target) = &self.code_input_target {
            if let Some(input) = target.upgrade() {
                let _ = cx.update_entity(&input, |code_input, cx| {
                    code_input.set_drag_in_progress(dragging);
                    cx.notify();
                });
            }
        }
    }

    pub(crate) fn set_colors(&mut self, thumb_color: Hsla, thumb_hover_color: Hsla) {
        self.thumb_color = thumb_color;
        self.thumb_hover_color = thumb_hover_color;
    }

    #[cfg(test)]
    pub(crate) fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    fn max_scroll(&self) -> Pixels {
        self.scroll_handle.max_offset().height.max(px(0.0))
    }

    fn geometry(&self) -> Option<ScrollbarGeometry> {
        let track_bounds = self.track_bounds?;
        let max_scroll = self.max_scroll();
        let viewport_height = track_bounds.size.height;
        if max_scroll <= px(0.0) || viewport_height <= px(0.0) {
            return None;
        }

        let content_height = viewport_height + max_scroll;
        let thumb_height = ((viewport_height / content_height) * viewport_height)
            .max(MIN_THUMB_HEIGHT)
            .min(viewport_height);
        let thumb_travel = (viewport_height - thumb_height).max(px(1.0));
        let scroll_fraction = ((-self.scroll_handle.offset().y) / max_scroll).clamp(0.0, 1.0);
        let thumb_top = track_bounds.top() + thumb_travel * scroll_fraction;
        Some(ScrollbarGeometry {
            track_bounds,
            thumb_bounds: Bounds::new(
                point(track_bounds.right() - THUMB_WIDTH - px(1.0), thumb_top),
                gpui::size(THUMB_WIDTH, thumb_height),
            ),
            max_scroll,
        })
    }

    fn set_thumb_top(&mut self, thumb_top: Pixels, geometry: ScrollbarGeometry) {
        let current = self.scroll_handle.offset();
        let thumb_travel =
            (geometry.track_bounds.size.height - geometry.thumb_bounds.size.height).max(px(1.0));
        let relative_thumb_top =
            (thumb_top - geometry.track_bounds.top()).clamp(px(0.0), thumb_travel);
        let scroll_fraction = relative_thumb_top / thumb_travel;
        self.scroll_handle
            .set_offset(point(current.x, -geometry.max_scroll * scroll_fraction));
    }

    fn sync_bounds(&mut self, bounds: Vec<Bounds<Pixels>>, cx: &mut Context<Self>) {
        let next_track = bounds.first().copied();
        let changed = self.track_bounds != next_track;
        self.track_bounds = next_track;
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        if changed {
            cx.notify();
        }
    }

    fn start_thumb_drag(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(geometry) = self.geometry() else {
            return;
        };
        let cursor_thumb_offset = (event.position.y - geometry.thumb_bounds.top())
            .clamp(px(0.0), geometry.thumb_bounds.size.height);
        self.active_drag = Some(ScrollbarDrag {
            cursor_thumb_offset,
        });
        self.thumb_bounds = Some(geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
        self.notify_code_input_drag(true, cx);
    }

    fn jump_to_track_position(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(geometry) = self.geometry() else {
            return;
        };
        let target_thumb_top = event.position.y - geometry.thumb_bounds.size.height / 2.0;
        self.set_thumb_top(target_thumb_top, geometry);
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
        self.notify_code_input(cx);
    }

    fn update_drag(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_drag(event.position, event.dragging(), window, cx);
    }

    fn apply_drag(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.active_drag else {
            return;
        };
        if !dragging {
            self.active_drag = None;
            cx.notify();
            return;
        }
        let Some(geometry) = self.geometry() else {
            return;
        };
        self.set_thumb_top(position.y - drag.cursor_thumb_offset, geometry);
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
        self.notify_code_input(cx);
    }

    fn update_drag_move(
        &mut self,
        event: &DragMoveEvent<ScrollbarDragToken>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_drag(event.event.position, event.event.dragging(), window, cx);
    }

    fn finish_drag(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_drag.is_some() {
            self.active_drag = None;
            cx.stop_propagation();
            cx.notify();
            self.notify_code_input_drag(false, cx);
        }
    }
}

impl Render for ScrollbarDragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_0()
    }
}

impl Render for VerticalScrollbar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = self.geometry();
        let thumb_bounds = geometry.map(|geometry| geometry.thumb_bounds);
        self.thumb_bounds = thumb_bounds;
        let scrollbar = cx.weak_entity();

        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(SCROLLBAR_WIDTH)
            .block_mouse_except_scroll()
            .on_children_prepainted(move |bounds, _, cx| {
                let _ = scrollbar.update(cx, |scrollbar, cx| {
                    scrollbar.sync_bounds(bounds, cx);
                });
            })
            .on_mouse_move(cx.listener(Self::update_drag))
            .on_drag_move::<ScrollbarDragToken>(cx.listener(Self::update_drag_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_drag))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_drag))
            .child(
                div()
                    .id("response-scrollbar-track")
                    .debug_selector(|| "response-scrollbar-track".into())
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(SCROLLBAR_WIDTH)
                    .block_mouse_except_scroll()
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::jump_to_track_position)),
            )
            .when_some(thumb_bounds, |this, thumb_bounds| {
                this.child(
                    div()
                        .id("response-scrollbar-thumb")
                        .debug_selector(|| "response-scrollbar-thumb".into())
                        .absolute()
                        .top(thumb_bounds.top() - self.track_bounds.unwrap().top())
                        .right(px(1.0))
                        .w(THUMB_WIDTH)
                        .h(thumb_bounds.size.height)
                        .rounded(px(3.0))
                        .bg(self.thumb_color)
                        .cursor(CursorStyle::PointingHand)
                        .hover({
                            let thumb_hover_color = self.thumb_hover_color;
                            move |this| this.bg(thumb_hover_color)
                        })
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::start_thumb_drag))
                        .on_drag(ScrollbarDragToken, |_, _, _, cx| {
                            cx.new(|_| ScrollbarDragGhost)
                        }),
                )
            })
    }
}

pub(crate) fn vertical_scrollbar(
    scrollbar: &Entity<VerticalScrollbar>,
) -> Entity<VerticalScrollbar> {
    scrollbar.clone()
}

// -----------------------------------------------------------------------------
// HorizontalScrollbar (modeled directly on VerticalScrollbar for consistency)
// -----------------------------------------------------------------------------

pub(crate) struct HorizontalScrollbar {
    scroll_handle: ScrollHandle,
    thumb_color: Hsla,
    thumb_hover_color: Hsla,
    track_bounds: Option<Bounds<Pixels>>,
    thumb_bounds: Option<Bounds<Pixels>>,
    active_drag: Option<ScrollbarDrag>,
    code_input_target: Option<WeakEntity<CodeInput>>,
    last_notify: Option<Instant>,
    /// Extra content width pushed from the owner. This parameter's usage is
    /// deprecated — horizontal scroll behavior now relies entirely on the GPUI
    /// overflow_scroll system, which provides accurate scrollable ranges directly
    /// via `scroll_handle.max_offset()`. The `forced_content_width` is retained
    /// for backward compatibility with old code that may still call it.
    forced_content_width: Option<Pixels>,
}

impl HorizontalScrollbar {
    pub(crate) fn new(scroll_handle: ScrollHandle) -> Self {
        Self {
            scroll_handle,
            thumb_color: rgba(0x00000033).into(),
            thumb_hover_color: rgba(0x00000066).into(),
            track_bounds: None,
            thumb_bounds: None,
            active_drag: None,
            code_input_target: None,
            last_notify: None,
            forced_content_width: None,
        }
    }

    pub(crate) fn set_code_input(&mut self, target: WeakEntity<CodeInput>) {
        self.code_input_target = Some(target);
    }

    fn notify_code_input(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if self
            .last_notify
            .is_some_and(|t| now.duration_since(t) < NOTIFY_THROTTLE)
        {
            return;
        }
        self.last_notify = Some(now);
        if let Some(target) = &self.code_input_target {
            if let Some(input) = target.upgrade() {
                let _ = cx.update_entity(&input, |_, cx| cx.notify());
            }
        }
    }

    fn notify_code_input_drag(&mut self, dragging: bool, cx: &mut Context<Self>) {
        if let Some(target) = &self.code_input_target {
            if let Some(input) = target.upgrade() {
                let _ = cx.update_entity(&input, |code_input, cx| {
                    code_input.set_drag_in_progress(dragging);
                    cx.notify();
                });
            }
        }
    }

    pub(crate) fn set_colors(&mut self, thumb_color: Hsla, thumb_hover_color: Hsla) {
        self.thumb_color = thumb_color;
        self.thumb_hover_color = thumb_hover_color;
    }

    pub(crate) fn set_content_width(&mut self, width: Pixels) {
        self.forced_content_width = Some(width.max(px(0.)));
    }

    #[cfg(test)]
    pub(crate) fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    fn max_scroll(&self) -> Pixels {
        let handle_max = self.scroll_handle.max_offset().width.max(px(0.0));
        // The scroll handle's max_offset comes from GPUI's clamp_scroll_position()
        // which accurately reports the actual scrollable range (content - viewport).
        // We ignore forced_content_width for this calculation because it represents
        // total content width, not the scrollable range. Using it here would add
        // false scrollable space beyond actual content.
        handle_max.max(px(0.))
    }

    fn geometry(&self) -> Option<ScrollbarGeometry> {
        let track_bounds = self.track_bounds?;
        let max_scroll = self.max_scroll();
        let viewport_width = track_bounds.size.width;
        if max_scroll <= px(0.0) || viewport_width <= px(0.0) {
            return None;
        }

        let content_width = viewport_width + max_scroll;
        let thumb_width = ((viewport_width / content_width) * viewport_width)
            .max(MIN_THUMB_HEIGHT)
            .min(viewport_width)
            .min(max_scroll);
        let thumb_travel = (viewport_width - thumb_width).max(px(1.0));
        let scroll_fraction = ((-self.scroll_handle.offset().x) / max_scroll).clamp(0.0, 1.0);
        let thumb_left = track_bounds.left() + thumb_travel * scroll_fraction;
        Some(ScrollbarGeometry {
            track_bounds,
            thumb_bounds: Bounds::new(
                point(thumb_left, track_bounds.bottom() - THUMB_WIDTH - px(1.0)),
                gpui::size(thumb_width, THUMB_WIDTH),
            ),
            max_scroll,
        })
    }

    fn set_thumb_left(&mut self, thumb_left: Pixels, geometry: ScrollbarGeometry) {
        let current = self.scroll_handle.offset();
        let thumb_travel =
            (geometry.track_bounds.size.width - geometry.thumb_bounds.size.width).max(px(1.0));
        let relative_thumb_left =
            (thumb_left - geometry.track_bounds.left()).clamp(px(0.0), thumb_travel);
        let scroll_fraction = relative_thumb_left / thumb_travel;
        self.scroll_handle
            .set_offset(point(-geometry.max_scroll * scroll_fraction, current.y));
    }

    fn sync_bounds(&mut self, bounds: Vec<Bounds<Pixels>>, cx: &mut Context<Self>) {
        let next_track = bounds.first().copied();
        let changed = self.track_bounds != next_track;
        self.track_bounds = next_track;
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        if changed {
            cx.notify();
        }
    }

    fn start_thumb_drag(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(geometry) = self.geometry() else {
            return;
        };
        let cursor_thumb_offset = (event.position.x - geometry.thumb_bounds.left())
            .clamp(px(0.0), geometry.thumb_bounds.size.width);
        self.active_drag = Some(ScrollbarDrag {
            cursor_thumb_offset,
        });
        self.thumb_bounds = Some(geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
        self.notify_code_input_drag(true, cx);
    }

    fn jump_to_track_position(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(geometry) = self.geometry() else {
            return;
        };
        let target_thumb_left = event.position.x - geometry.thumb_bounds.size.width / 2.0;
        self.set_thumb_left(target_thumb_left, geometry);
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
        self.notify_code_input(cx);
    }

    fn update_drag(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_drag(event.position, event.dragging(), window, cx);
    }

    fn apply_drag(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.active_drag else {
            return;
        };
        if !dragging {
            self.active_drag = None;
            cx.notify();
            return;
        }
        let Some(geometry) = self.geometry() else {
            return;
        };
        self.set_thumb_left(position.x - drag.cursor_thumb_offset, geometry);
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
        self.notify_code_input(cx);
    }

    fn finish_drag(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.active_drag = None;
        cx.notify();
        self.notify_code_input_drag(false, cx);
    }

    fn update_drag_move(
        &mut self,
        event: &DragMoveEvent<ScrollbarDragToken>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_drag(event.event.position, event.event.dragging(), window, cx);
    }
}

impl Render for HorizontalScrollbar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = self.geometry();
        let thumb_bounds = geometry.map(|geometry| geometry.thumb_bounds);
        self.thumb_bounds = thumb_bounds;
        let scrollbar = cx.weak_entity();

        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(SCROLLBAR_WIDTH)
            .block_mouse_except_scroll()
            .on_children_prepainted(move |bounds, _, cx| {
                let _ = scrollbar.update(cx, |scrollbar, cx| {
                    scrollbar.sync_bounds(bounds, cx);
                });
            })
            .on_mouse_move(cx.listener(Self::update_drag))
            .on_drag_move::<ScrollbarDragToken>(cx.listener(Self::update_drag_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_drag))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_drag))
            .child(
                div()
                    .id("response-horizontal-scrollbar-track")
                    .debug_selector(|| "response-horizontal-scrollbar-track".into())
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(SCROLLBAR_WIDTH)
                    .block_mouse_except_scroll()
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::jump_to_track_position)),
            )
            .when_some(thumb_bounds, |this, thumb_bounds| {
                this.child(
                    div()
                        .id("response-horizontal-scrollbar-thumb")
                        .debug_selector(|| "response-horizontal-scrollbar-thumb".into())
                        .absolute()
                        .left(thumb_bounds.left() - self.track_bounds.unwrap().left())
                        .bottom(px(1.0))
                        .w(thumb_bounds.size.width)
                        .h(THUMB_WIDTH)
                        .rounded(px(3.0))
                        .bg(self.thumb_color)
                        .cursor(CursorStyle::PointingHand)
                        .hover({
                            let thumb_hover_color = self.thumb_hover_color;
                            move |this| this.bg(thumb_hover_color)
                        })
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::start_thumb_drag))
                        .on_drag(ScrollbarDragToken, |_, _, _, cx| {
                            cx.new(|_| ScrollbarDragGhost)
                        }),
                )
            })
    }
}

pub(crate) fn horizontal_scrollbar(
    scrollbar: &Entity<HorizontalScrollbar>,
) -> Entity<HorizontalScrollbar> {
    scrollbar.clone()
}

// -----------------------------------------------------------------------------
// ListScrollbar — scrollbar driven by a GPUI ListState
// -----------------------------------------------------------------------------

pub(crate) struct ListScrollbar {
    list_state: ListState,
    thumb_color: Hsla,
    thumb_hover_color: Hsla,
    track_bounds: Option<Bounds<Pixels>>,
    thumb_bounds: Option<Bounds<Pixels>>,
    active_drag: Option<ScrollbarDrag>,
}

impl ListScrollbar {
    pub(crate) fn new(list_state: ListState) -> Self {
        Self {
            list_state,
            thumb_color: rgba(0x00000033).into(),
            thumb_hover_color: rgba(0x00000066).into(),
            track_bounds: None,
            thumb_bounds: None,
            active_drag: None,
        }
    }

    pub(crate) fn set_colors(&mut self, thumb_color: Hsla, thumb_hover_color: Hsla) {
        self.thumb_color = thumb_color;
        self.thumb_hover_color = thumb_hover_color;
    }

    fn max_scroll(&self) -> Pixels {
        self.list_state
            .max_offset_for_scrollbar()
            .height
            .max(px(0.0))
    }

    fn scroll_offset(&self) -> Point<Pixels> {
        self.list_state.scroll_px_offset_for_scrollbar()
    }

    fn geometry(&self) -> Option<ScrollbarGeometry> {
        let track_bounds = self.track_bounds?;
        let max_scroll = self.max_scroll();
        let viewport_height = track_bounds.size.height;
        if max_scroll <= px(0.0) || viewport_height <= px(0.0) {
            return None;
        }

        let content_height = viewport_height + max_scroll;
        let thumb_height = ((viewport_height / content_height) * viewport_height)
            .max(MIN_THUMB_HEIGHT)
            .min(viewport_height);
        let thumb_travel = (viewport_height - thumb_height).max(px(1.0));
        let scroll_fraction = (-self.scroll_offset().y / max_scroll).clamp(0.0, 1.0);
        let thumb_top = track_bounds.top() + thumb_travel * scroll_fraction;
        Some(ScrollbarGeometry {
            track_bounds,
            thumb_bounds: Bounds::new(
                point(track_bounds.right() - THUMB_WIDTH - px(1.0), thumb_top),
                gpui::size(THUMB_WIDTH, thumb_height),
            ),
            max_scroll,
        })
    }

    fn set_thumb_top(&mut self, thumb_top: Pixels, geometry: ScrollbarGeometry) {
        let thumb_travel =
            (geometry.track_bounds.size.height - geometry.thumb_bounds.size.height).max(px(1.0));
        let relative_thumb_top =
            (thumb_top - geometry.track_bounds.top()).clamp(px(0.0), thumb_travel);
        let scroll_fraction = relative_thumb_top / thumb_travel;
        self.list_state
            .set_offset_from_scrollbar(point(px(0.0), -geometry.max_scroll * scroll_fraction));
    }

    fn sync_bounds(&mut self, bounds: Vec<Bounds<Pixels>>, cx: &mut Context<Self>) {
        let next_track = bounds.first().copied();
        let changed = self.track_bounds != next_track;
        self.track_bounds = next_track;
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        if changed {
            cx.notify();
        }
    }

    fn start_thumb_drag(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(geometry) = self.geometry() else {
            return;
        };
        let cursor_thumb_offset = (event.position.y - geometry.thumb_bounds.top())
            .clamp(px(0.0), geometry.thumb_bounds.size.height);
        self.active_drag = Some(ScrollbarDrag {
            cursor_thumb_offset,
        });
        self.thumb_bounds = Some(geometry.thumb_bounds);
        self.list_state.scrollbar_drag_started();
        cx.stop_propagation();
        cx.notify();
    }

    fn jump_to_track_position(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(geometry) = self.geometry() else {
            return;
        };
        let target_thumb_top = event.position.y - geometry.thumb_bounds.size.height / 2.0;
        self.set_thumb_top(target_thumb_top, geometry);
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
    }

    fn update_drag(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_drag(event.position, event.dragging(), cx);
    }

    fn apply_drag(&mut self, position: Point<Pixels>, dragging: bool, cx: &mut Context<Self>) {
        let Some(drag) = self.active_drag else {
            return;
        };
        if !dragging {
            self.active_drag = None;
            self.list_state.scrollbar_drag_ended();
            cx.notify();
            return;
        }
        let Some(geometry) = self.geometry() else {
            return;
        };
        self.set_thumb_top(position.y - drag.cursor_thumb_offset, geometry);
        self.thumb_bounds = self.geometry().map(|geometry| geometry.thumb_bounds);
        cx.stop_propagation();
        cx.notify();
    }

    fn update_drag_move(
        &mut self,
        event: &DragMoveEvent<ScrollbarDragToken>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_drag(event.event.position, event.event.dragging(), cx);
    }

    fn finish_drag(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_drag.is_some() {
            self.active_drag = None;
            self.list_state.scrollbar_drag_ended();
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl Render for ListScrollbar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = self.geometry();
        let thumb_bounds = geometry.map(|geometry| geometry.thumb_bounds);
        self.thumb_bounds = thumb_bounds;
        let scrollbar = cx.weak_entity();

        div()
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(SCROLLBAR_WIDTH)
            .block_mouse_except_scroll()
            .on_children_prepainted(move |bounds, _, cx| {
                let _ = scrollbar.update(cx, |scrollbar, cx| {
                    scrollbar.sync_bounds(bounds, cx);
                });
            })
            .on_mouse_move(cx.listener(Self::update_drag))
            .on_drag_move::<ScrollbarDragToken>(cx.listener(Self::update_drag_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_drag))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_drag))
            .child(
                div()
                    .id("list-scrollbar-track")
                    .debug_selector(|| "list-scrollbar-track".into())
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(SCROLLBAR_WIDTH)
                    .block_mouse_except_scroll()
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::jump_to_track_position)),
            )
            .when_some(thumb_bounds, |this, thumb_bounds| {
                this.child(
                    div()
                        .id("list-scrollbar-thumb")
                        .debug_selector(|| "list-scrollbar-thumb".into())
                        .absolute()
                        .top(thumb_bounds.top() - self.track_bounds.unwrap().top())
                        .right(px(1.0))
                        .w(THUMB_WIDTH)
                        .h(thumb_bounds.size.height)
                        .rounded(px(3.0))
                        .bg(self.thumb_color)
                        .cursor(CursorStyle::PointingHand)
                        .hover({
                            let thumb_hover_color = self.thumb_hover_color;
                            move |this| this.bg(thumb_hover_color)
                        })
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::start_thumb_drag))
                        .on_drag(ScrollbarDragToken, |_, _, _, cx| {
                            cx.new(|_| ScrollbarDragGhost)
                        }),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    fn test_geometry() -> ScrollbarGeometry {
        ScrollbarGeometry {
            track_bounds: Bounds::new(point(px(100.0), px(20.0)), size(px(8.0), px(200.0))),
            thumb_bounds: Bounds::new(point(px(101.0), px(60.0)), size(px(6.0), px(40.0))),
            max_scroll: px(1_000.0),
        }
    }

    #[test]
    fn set_thumb_top_maps_and_clamps_to_scroll_offset() {
        let mut scrollbar = VerticalScrollbar::new(ScrollHandle::new());
        let geometry = test_geometry();

        scrollbar.set_thumb_top(geometry.track_bounds.top(), geometry);
        assert_eq!(scrollbar.scroll_handle.offset().y, px(0.0));

        scrollbar.set_thumb_top(geometry.track_bounds.bottom(), geometry);
        assert_eq!(scrollbar.scroll_handle.offset().y, px(-1_000.0));
    }

    #[test]
    fn set_thumb_top_preserves_fractional_scroll_position() {
        let mut scrollbar = VerticalScrollbar::new(ScrollHandle::new());
        let geometry = test_geometry();
        let midpoint = geometry.track_bounds.top()
            + (geometry.track_bounds.size.height - geometry.thumb_bounds.size.height) / 2.0;

        scrollbar.set_thumb_top(midpoint, geometry);

        assert_eq!(scrollbar.scroll_handle.offset().y, px(-500.0));
    }
}
