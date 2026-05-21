use std::{collections::BTreeSet, ops::Range, time::Duration};

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, InteractiveElement,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ShapedLine, SharedString, Style, Task, TextRun, UTF16Selection, Window, actions,
    div, fill, point, prelude::*, px, relative, rgb, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    code_input,
    [
        CodeBackspace,
        CodeDelete,
        CodeLeft,
        CodeRight,
        CodeUp,
        CodeDown,
        CodeWordBackspace,
        CodeWordDelete,
        CodeWordLeft,
        CodeWordRight,
        CodeSelectWordLeft,
        CodeSelectWordRight,
        CodeSelectLeft,
        CodeSelectRight,
        CodeSelectUp,
        CodeSelectDown,
        CodeSelectAll,
        CodeHome,
        CodeEnd,
        CodePaste,
        CodeCut,
        CodeCopy,
    ]
);

pub(crate) struct CodeInput {
    focus_handle: FocusHandle,
    debug_selector: &'static str,
    content: SharedString,
    placeholder: SharedString,
    placeholder_color: gpui::Hsla,
    cursor_color: gpui::Hsla,
    selection_color: gpui::Hsla,
    syntax_colors: SyntaxColors,
    read_only: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    collapsed_folds: BTreeSet<usize>,
    last_layout: Option<CachedCodeLayout>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    cursor_visible: bool,
    cursor_blink_enabled: bool,
    cursor_blink_epoch: usize,
    cursor_blink_task: Option<Task<()>>,
}

#[derive(Clone)]
struct CachedCodeLayout {
    lines: Vec<ShapedLine>,
    gutter_lines: Vec<ShapedLine>,
    line_starts: Vec<usize>,
    line_ends: Vec<usize>,
    fold_starts: Vec<Option<usize>>,
    line_height: Pixels,
    text_left: Pixels,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CodeLine {
    start: usize,
    end: usize,
    text: String,
    source_line: usize,
    fold_start: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FoldRange {
    start_line: usize,
    end_line: usize,
    start_offset: usize,
    end_offset: usize,
    close_char: char,
}

#[derive(Clone, Copy)]
pub(crate) struct SyntaxColors {
    pub(crate) property: gpui::Hsla,
    pub(crate) string: gpui::Hsla,
    pub(crate) number: gpui::Hsla,
    pub(crate) keyword: gpui::Hsla,
    pub(crate) punctuation: gpui::Hsla,
}

impl Default for SyntaxColors {
    fn default() -> Self {
        Self {
            property: rgb(0x7aa2f7).into(),
            string: rgb(0x9ece6a).into(),
            number: rgb(0xff9e64).into(),
            keyword: rgb(0xbb9af7).into(),
            punctuation: rgb(0x8a8a8a).into(),
        }
    }
}

fn fold_ranges_for_text(text: &str) -> Vec<FoldRange> {
    let mut ranges = Vec::new();
    let mut stack: Vec<(char, usize, usize)> = Vec::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut line = 0usize;

    for (offset, ch) in text.char_indices() {
        if ch == '\n' {
            line += 1;
        }

        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' | '[' => stack.push((ch, offset, line)),
            '}' | ']' => {
                let expected = if ch == '}' { '{' } else { '[' };
                if let Some(stack_index) = stack.iter().rposition(|(open, _, _)| *open == expected)
                {
                    let (_, start_offset, start_line) = stack.remove(stack_index);
                    if line > start_line {
                        ranges.push(FoldRange {
                            start_line,
                            end_line: line,
                            start_offset,
                            end_offset: offset + ch.len_utf8(),
                            close_char: ch,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    ranges.sort_by_key(|range| (range.start_line, range.end_line));
    ranges
}

fn is_pair_boundary_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn is_closing_pair_char(ch: char) -> bool {
    matches!(ch, '}' | ']' | '"' | '\'')
}

fn source_lines_for_text(text: &str) -> Vec<CodeLine> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (source_line, line) in text.split('\n').enumerate() {
        let end = start + line.len();
        lines.push(CodeLine {
            start,
            end,
            text: line.to_string(),
            source_line,
            fold_start: None,
        });
        start = start.saturating_add(line.len()).saturating_add(1);
    }
    if lines.is_empty() {
        lines.push(CodeLine {
            start: 0,
            end: 0,
            text: String::new(),
            source_line: 0,
            fold_start: None,
        });
    }
    lines
}

fn visible_lines_for_text(text: &str, collapsed_folds: &BTreeSet<usize>) -> Vec<CodeLine> {
    let source_lines = source_lines_for_text(text);
    let folds = fold_ranges_for_text(text);
    let mut visible = Vec::new();
    let mut index = 0usize;
    while index < source_lines.len() {
        let line = source_lines[index].clone();
        let fold = folds
            .iter()
            .find(|fold| fold.start_line == line.source_line);
        if let Some(fold) = fold {
            if collapsed_folds.contains(&fold.start_line) {
                let mut folded = line.clone();
                folded.text = format!("{} ... {}", folded.text.trim_end(), fold.close_char);
                folded.fold_start = Some(fold.start_line);
                visible.push(folded);
                index = fold.end_line.saturating_add(1);
                continue;
            }
            let mut expanded = line.clone();
            expanded.fold_start = Some(fold.start_line);
            visible.push(expanded);
        } else {
            visible.push(line);
        }
        index += 1;
    }
    visible
}

impl CodeInput {
    pub(crate) fn new(
        cx: &mut Context<Self>,
        content: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
    ) -> Self {
        let content: SharedString = Self::sanitize_content(content.into().as_ref()).into();
        let end = content.len();
        Self {
            focus_handle: cx.focus_handle(),
            debug_selector: "body-code-input",
            content,
            placeholder: placeholder.into(),
            placeholder_color: rgba(0xffffff66).into(),
            cursor_color: rgb(0x4f8cff).into(),
            selection_color: rgba(0x4f8cff30).into(),
            syntax_colors: SyntaxColors::default(),
            read_only: false,
            selected_range: end..end,
            selection_reversed: false,
            marked_range: None,
            collapsed_folds: BTreeSet::new(),
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            cursor_visible: true,
            cursor_blink_enabled: false,
            cursor_blink_epoch: 0,
            cursor_blink_task: None,
        }
    }

    pub(crate) fn new_read_only(
        cx: &mut Context<Self>,
        content: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
    ) -> Self {
        let mut input = Self::new(cx, content, placeholder);
        input.debug_selector = "response-code-input";
        input.read_only = true;
        input
    }

    pub(crate) fn set_content(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = Self::sanitize_content(content.into().as_ref()).into();
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.prune_collapsed_folds();
        self.reset_cursor_blink(cx);
    }

    pub(crate) fn value(&self) -> String {
        self.content.to_string()
    }

    pub(crate) fn set_placeholder(&mut self, placeholder: impl Into<SharedString>) {
        self.placeholder = placeholder.into();
    }

    pub(crate) fn set_placeholder_color(&mut self, color: gpui::Hsla) {
        self.placeholder_color = color;
    }

    pub(crate) fn set_syntax_colors(&mut self, colors: SyntaxColors) {
        self.syntax_colors = colors;
    }

    pub(crate) fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub(crate) fn insert_newline(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let range = self.clamp_range(self.selected_range.clone());
        self.collapsed_folds.clear();
        let (new_text, cursor_delta) = self.newline_text_for_range(&range);
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        let cursor = range.start + cursor_delta;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.prune_collapsed_folds();
        self.reset_cursor_blink(cx);
    }

    fn source_lines(&self) -> Vec<CodeLine> {
        source_lines_for_text(self.content.as_ref())
    }

    fn visible_lines(&self) -> Vec<CodeLine> {
        visible_lines_for_text(self.content.as_ref(), &self.collapsed_folds)
    }

    fn prune_collapsed_folds(&mut self) {
        let valid_folds = fold_ranges_for_text(self.content.as_ref())
            .into_iter()
            .map(|fold| fold.start_line)
            .collect::<BTreeSet<_>>();
        self.collapsed_folds
            .retain(|line| valid_folds.contains(line));
    }

    fn enable_cursor_blink(&mut self, cx: &mut Context<Self>) {
        if !self.cursor_blink_enabled {
            self.cursor_blink_enabled = true;
            self.reset_cursor_blink(cx);
        }
    }

    fn disable_cursor_blink(&mut self) {
        self.cursor_blink_enabled = false;
        self.cursor_visible = true;
        self.cursor_blink_epoch = self.cursor_blink_epoch.wrapping_add(1);
        self.cursor_blink_task = None;
    }

    fn reset_cursor_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self.cursor_blink_epoch = self.cursor_blink_epoch.wrapping_add(1);
        cx.notify();

        if self.cursor_blink_enabled {
            self.schedule_cursor_blink(Duration::from_millis(500), cx);
        }
    }

    fn schedule_cursor_blink(&mut self, interval: Duration, cx: &mut Context<Self>) {
        let epoch = self.cursor_blink_epoch;
        self.cursor_blink_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(interval).await;
            if let Some(this) = this.upgrade() {
                let _ = this.update(cx, |this, cx| this.blink_cursor(epoch, cx));
            }
        }));
    }

    fn blink_cursor(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if !self.cursor_blink_enabled || epoch != self.cursor_blink_epoch {
            return;
        }
        self.cursor_visible = !self.cursor_visible;
        cx.notify();
        self.schedule_cursor_blink(Duration::from_millis(530), cx);
    }

    fn backspace(&mut self, _: &CodeBackspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &CodeDelete, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn left(&mut self, _: &CodeLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &CodeRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_left(&mut self, _: &CodeSelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &CodeSelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn up(&mut self, _: &CodeUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1, false, cx);
    }

    fn down(&mut self, _: &CodeDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1, false, cx);
    }

    fn select_up(&mut self, _: &CodeSelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1, true, cx);
    }

    fn select_down(&mut self, _: &CodeSelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1, true, cx);
    }

    fn word_backspace(
        &mut self,
        _: &CodeWordBackspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.previous_word_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn word_delete(&mut self, _: &CodeWordDelete, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.next_word_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn word_left(&mut self, _: &CodeWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_word_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn word_right(&mut self, _: &CodeWordRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_word_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_word_left(&mut self, _: &CodeSelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word_boundary(self.cursor_offset()), cx);
    }

    fn select_word_right(
        &mut self,
        _: &CodeSelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.next_word_boundary(self.cursor_offset()), cx);
    }

    fn home(&mut self, _: &CodeHome, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let line_start = self
            .source_lines()
            .into_iter()
            .take_while(|line| line.start <= cursor)
            .last()
            .map(|line| line.start)
            .unwrap_or(0);
        self.move_to(line_start, cx);
    }

    fn end(&mut self, _: &CodeEnd, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let line_end = self
            .source_lines()
            .into_iter()
            .find_map(|line| {
                let end = line.start + line.text.len();
                (cursor <= end).then_some(end)
            })
            .unwrap_or(self.content.len());
        self.move_to(line_end, cx);
    }

    fn select_all(&mut self, _: &CodeSelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_content(cx);
    }

    fn paste(&mut self, _: &CodePaste, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &CodeCopy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &CodeCut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            self.copy(&CodeCopy, window, cx);
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.click_count >= 2 {
            self.is_selecting = false;
            self.select_word_at(self.index_for_mouse_position(event.position), cx);
            return;
        }

        if self.toggle_fold_for_position(event.position, cx) {
            self.is_selecting = false;
            return;
        }

        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.clamp_offset(offset);
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.reset_cursor_blink(cx);
    }

    fn move_vertical(&mut self, direction: i32, select: bool, cx: &mut Context<Self>) {
        let lines = self.visible_lines();
        if lines.is_empty() {
            return;
        }
        let cursor = self.cursor_offset();
        let current_index = lines
            .iter()
            .position(|line| cursor >= line.start && cursor <= line.end)
            .or_else(|| {
                lines
                    .iter()
                    .enumerate()
                    .take_while(|(_, line)| line.start <= cursor)
                    .last()
                    .map(|(index, _)| index)
            })
            .unwrap_or(0);
        let target_index = (current_index as i32 + direction)
            .clamp(0, lines.len().saturating_sub(1) as i32) as usize;
        if target_index == current_index {
            return;
        }
        let current_line = &lines[current_index];
        let target_line = &lines[target_index];
        let column = cursor
            .saturating_sub(current_line.start)
            .min(current_line.text.len());
        let target_index =
            self.clamp_offset(target_line.start + column.min(target_line.text.len()));
        if select {
            self.select_to(target_index, cx);
        } else {
            self.move_to(target_index, cx);
        }
    }

    fn toggle_fold_for_position(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(layout) = self.last_layout.as_ref() else {
            return false;
        };
        let Some(bounds) = self.last_bounds.as_ref() else {
            return false;
        };
        if position.x < bounds.left()
            || position.x > bounds.left() + fold_marker_width()
            || position.y < bounds.top()
            || position.y > bounds.bottom()
        {
            return false;
        }
        let mut line_index = None;
        let mut line_origin_y = bounds.top();
        for index in 0..layout.lines.len() {
            if position.y <= line_origin_y + layout.line_height {
                line_index = Some(index);
                break;
            }
            line_origin_y += layout.line_height;
        }
        let Some(line_index) = line_index else {
            return false;
        };
        let Some(Some(fold_start)) = layout.fold_starts.get(line_index).copied() else {
            return false;
        };
        if !self.collapsed_folds.remove(&fold_start) {
            self.collapsed_folds.insert(fold_start);
        }
        self.reset_cursor_blink(cx);
        true
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.clamp_offset(offset);
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.reset_cursor_blink(cx);
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn select_all_content(&mut self, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.reset_cursor_blink(cx);
    }

    fn select_word_at(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.clamp_offset(offset);
        self.selected_range = self.previous_word_boundary(offset)..self.next_word_boundary(offset);
        self.selection_reversed = false;
        self.reset_cursor_blink(cx);
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return self.content.len();
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }

        let mut line_origin_y = bounds.top();
        for ((line, line_start), line_end) in layout
            .lines
            .iter()
            .zip(layout.line_starts.iter().copied())
            .zip(layout.line_ends.iter().copied())
        {
            if position.y <= line_origin_y + layout.line_height {
                return (line_start + line.closest_index_for_x(position.x - layout.text_left))
                    .min(line_end);
            }
            line_origin_y += layout.line_height;
        }
        self.content.len()
    }

    fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        let bounds = self.last_bounds?;
        let layout = self.last_layout.as_ref()?;
        let mut line_origin_y = bounds.top();
        for ((line, line_start), line_end) in layout
            .lines
            .iter()
            .zip(layout.line_starts.iter().copied())
            .zip(layout.line_ends.iter().copied())
        {
            if index <= line_end {
                let local_index = index.saturating_sub(line_start);
                return Some(point(
                    layout.text_left + line.x_for_index(local_index.min(line.len())),
                    line_origin_y,
                ));
            }
            line_origin_y += layout.line_height;
        }
        Some(point(bounds.left(), line_origin_y))
    }

    fn selection_quads(
        &self,
        bounds: Bounds<Pixels>,
        lines: &[ShapedLine],
        line_starts: &[usize],
        line_ends: &[usize],
        line_height: Pixels,
        text_left: Pixels,
    ) -> Vec<PaintQuad> {
        if self.selected_range.is_empty() {
            return Vec::new();
        }
        let mut quads = Vec::new();
        let mut line_origin_y = bounds.top();
        for ((line, line_start), line_end) in lines
            .iter()
            .zip(line_starts.iter().copied())
            .zip(line_ends.iter().copied())
        {
            let segment_start = self.selected_range.start.max(line_start);
            let segment_end = self.selected_range.end.min(line_end);
            if segment_start < segment_end || self.selected_range.contains(&line_end) {
                let local_start = segment_start.saturating_sub(line_start);
                let local_end = segment_end.saturating_sub(line_start);
                let start_x = line.x_for_index(local_start.min(line.len()));
                let end_x = line
                    .x_for_index(local_end.min(line.len()))
                    .max(start_x + px(2.0));
                quads.push(fill(
                    Bounds::from_corners(
                        point(text_left + start_x, line_origin_y),
                        point(text_left + end_x, line_origin_y + line_height),
                    ),
                    self.selection_color,
                ));
            }
            line_origin_y += line_height;
        }
        quads
    }

    fn sanitize_content(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }

    fn newline_text_for_range(&self, range: &Range<usize>) -> (String, usize) {
        let indent = self.current_line_indent(range.start);
        if range.is_empty() && self.is_between_empty_pair(range.start) {
            let inner_indent = format!("{indent}  ");
            let text = format!("\n{inner_indent}\n{indent}");
            let cursor_delta = 1 + inner_indent.len();
            (text, cursor_delta)
        } else {
            let text = format!("\n{indent}");
            let cursor_delta = text.len();
            (text, cursor_delta)
        }
    }

    fn current_line_indent(&self, offset: usize) -> String {
        let text = self.content.as_ref();
        let line_start = text[..self.clamp_offset(offset)]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        text[line_start..]
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect()
    }

    fn is_between_empty_pair(&self, offset: usize) -> bool {
        let Some(previous) = self.char_before(offset) else {
            return false;
        };
        let Some(next) = self.char_at(offset) else {
            return false;
        };
        matches!((previous, next), ('{', '}') | ('[', ']'))
    }

    fn char_before(&self, offset: usize) -> Option<char> {
        self.content.as_ref()[..self.clamp_offset(offset)]
            .chars()
            .next_back()
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        self.content.as_ref()[self.clamp_offset(offset)..]
            .chars()
            .next()
    }

    fn auto_pair_for(&self, ch: char, offset: usize) -> Option<char> {
        let closer = match ch {
            '{' => '}',
            '[' => ']',
            '"' => '"',
            '\'' => '\'',
            _ => return None,
        };
        if self.is_glued_to_text(offset) {
            return None;
        }
        Some(closer)
    }

    fn is_glued_to_text(&self, offset: usize) -> bool {
        self.char_before(offset).is_some_and(is_pair_boundary_char)
            || self.char_at(offset).is_some_and(is_pair_boundary_char)
    }

    fn replace_range_with_text(
        &mut self,
        range: Range<usize>,
        new_text: &str,
        cursor_offset: usize,
        cx: &mut Context<Self>,
    ) {
        self.collapsed_folds.clear();
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        let cursor = range.start + cursor_offset;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range.take();
        self.prune_collapsed_folds();
        self.reset_cursor_blink(cx);
    }

    fn clamp_offset(&self, offset: usize) -> usize {
        let text = self.content.as_ref();
        let mut offset = offset.min(text.len());
        while offset > 0 && !text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn clamp_range(&self, range: Range<usize>) -> Range<usize> {
        let start = self.clamp_offset(range.start);
        let end = self.clamp_offset(range.end);
        start.min(end)..start.max(end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        let text = self.content.as_ref();
        let mut index = offset.min(text.len());

        while index > 0 {
            let previous = self.previous_boundary(index);
            let Some(ch) = text[previous..index].chars().next() else {
                break;
            };
            if Self::is_word_char(ch) {
                break;
            }
            index = previous;
        }

        while index > 0 {
            let previous = self.previous_boundary(index);
            let Some(ch) = text[previous..index].chars().next() else {
                break;
            };
            if !Self::is_word_char(ch) {
                break;
            }
            index = previous;
        }

        index
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        let text = self.content.as_ref();
        let mut index = offset.min(text.len());

        while index < text.len() {
            let next = self.next_boundary(index);
            let Some(ch) = text[index..next].chars().next() else {
                break;
            };
            if Self::is_word_char(ch) {
                break;
            }
            index = next;
        }

        while index < text.len() {
            let next = self.next_boundary(index);
            let Some(ch) = text[index..next].chars().next() else {
                break;
            };
            if !Self::is_word_char(ch) {
                break;
            }
            index = next;
        }

        index
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }
}

impl EntityInputHandler for CodeInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.clamp_range(self.range_from_utf16(&range_utf16));
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.clamp_range(range);
        let new_text = Self::sanitize_content(new_text);
        if range.is_empty()
            && self.marked_range.is_none()
            && range_utf16.is_none()
            && new_text.chars().count() == 1
        {
            let ch = new_text.chars().next().unwrap();
            if is_closing_pair_char(ch) && self.char_at(range.start) == Some(ch) {
                let cursor = self.next_boundary(range.start);
                self.selected_range = cursor..cursor;
                self.selection_reversed = false;
                self.reset_cursor_blink(cx);
                return;
            }
            if let Some(closer) = self.auto_pair_for(ch, range.start) {
                let paired = format!("{ch}{closer}");
                self.replace_range_with_text(range, &paired, ch.len_utf8(), cx);
                return;
            }
        }
        let cursor_offset = new_text.len();
        self.replace_range_with_text(range, &new_text, cursor_offset, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.clamp_range(range);
        let new_text = Self::sanitize_content(new_text);
        self.collapsed_folds.clear();
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.marked_range =
            (!new_text.is_empty()).then_some(range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.prune_collapsed_folds();
        self.reset_cursor_blink(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start = self.position_for_index(range.start)?;
        let end = self.position_for_index(range.end)?;
        Some(Bounds::from_corners(
            point(start.x.min(end.x), start.y.min(end.y)),
            point(end.x.max(start.x), start.y.min(end.y) + bounds.size.height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let utf8_index = self.index_for_mouse_position(point);
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct CodeElement {
    input: Entity<CodeInput>,
}

struct CodePrepaintState {
    layout: CachedCodeLayout,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

fn line_number_digits(max_line_number: usize) -> usize {
    max_line_number.max(1).to_string().len()
}

fn gutter_width(max_line_number: usize) -> Pixels {
    px(22.0 + line_number_digits(max_line_number) as f32 * 8.0 + 10.0)
}

fn fold_marker_width() -> Pixels {
    px(18.0)
}

fn syntax_runs_for_line(
    line: &str,
    base_run: &TextRun,
    base_color: gpui::Hsla,
    colors: SyntaxColors,
) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut index = 0usize;

    while index < line.len() {
        let start = index;
        let Some(ch) = line[index..].chars().next() else {
            break;
        };

        let (end, color) = if ch == '"' || ch == '\'' {
            let quote = ch;
            let mut escaped = false;
            index += quote.len_utf8();
            while index < line.len() {
                let Some(next) = line[index..].chars().next() else {
                    break;
                };
                index += next.len_utf8();
                if escaped {
                    escaped = false;
                    continue;
                }
                if next == '\\' {
                    escaped = true;
                    continue;
                }
                if next == quote {
                    break;
                }
            }
            let mut lookahead = index;
            while lookahead < line.len() {
                let Some(next) = line[lookahead..].chars().next() else {
                    break;
                };
                if !next.is_whitespace() {
                    break;
                }
                lookahead += next.len_utf8();
            }
            let is_property = line[lookahead..].starts_with(':');
            (
                index,
                if is_property {
                    colors.property
                } else {
                    colors.string
                },
            )
        } else if ch.is_ascii_digit() || ch == '-' {
            index += ch.len_utf8();
            while index < line.len() {
                let Some(next) = line[index..].chars().next() else {
                    break;
                };
                if !(next.is_ascii_digit() || matches!(next, '.' | 'e' | 'E' | '+' | '-')) {
                    break;
                }
                index += next.len_utf8();
            }
            (index, colors.number)
        } else if ch.is_ascii_alphabetic() {
            index += ch.len_utf8();
            while index < line.len() {
                let Some(next) = line[index..].chars().next() else {
                    break;
                };
                if !next.is_ascii_alphabetic() {
                    break;
                }
                index += next.len_utf8();
            }
            let token = &line[start..index];
            let color = if matches!(token, "true" | "false" | "null") {
                colors.keyword
            } else {
                base_color
            };
            (index, color)
        } else if matches!(ch, '{' | '}' | '[' | ']' | ':' | ',') {
            index += ch.len_utf8();
            (index, colors.punctuation)
        } else {
            index += ch.len_utf8();
            (index, base_color)
        };

        if end > start {
            runs.push(TextRun {
                len: end - start,
                color,
                ..base_run.clone()
            });
        }
    }

    if runs.is_empty() {
        vec![base_run.clone()]
    } else {
        runs
    }
}

impl IntoElement for CodeElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CodeElement {
    type RequestLayoutState = ();
    type PrepaintState = CodePrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let line_count = input.visible_lines().len().max(1);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * line_count as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let max_line_number = input
            .source_lines()
            .last()
            .map(|line| line.source_line + 1)
            .unwrap_or(1);
        let gutter_width = gutter_width(max_line_number);
        let text_left = bounds.left() + gutter_width;
        let use_placeholder = input.content.is_empty();
        let display_lines = if use_placeholder {
            vec![CodeLine {
                start: 0,
                end: 0,
                text: input.placeholder.to_string(),
                source_line: 0,
                fold_start: None,
            }]
        } else {
            input.visible_lines()
        };
        let text_color = if use_placeholder {
            input.placeholder_color
        } else {
            style.color
        };
        let mut lines = Vec::with_capacity(display_lines.len());
        let mut gutter_lines = Vec::with_capacity(display_lines.len());
        let mut line_starts = Vec::with_capacity(display_lines.len());
        let mut line_ends = Vec::with_capacity(display_lines.len());
        let mut fold_starts = Vec::with_capacity(display_lines.len());
        let gutter_digits = line_number_digits(max_line_number);

        for line in display_lines {
            line_starts.push(line.start);
            line_ends.push(line.end);
            fold_starts.push(line.fold_start);
            let base_run = TextRun {
                len: line.text.len(),
                font: style.font(),
                color: text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let runs = if use_placeholder {
                vec![base_run]
            } else {
                syntax_runs_for_line(&line.text, &base_run, text_color, input.syntax_colors)
            };
            lines.push(window.text_system().shape_line(
                line.text.clone().into(),
                font_size,
                &runs,
                None,
            ));
            let fold_marker = match line.fold_start {
                Some(fold_start) if input.collapsed_folds.contains(&fold_start) => ">",
                Some(_) => "v",
                None => " ",
            };
            let gutter_text = format!(
                "{fold_marker} {:>width$}",
                line.source_line + 1,
                width = gutter_digits
            );
            let gutter_run = TextRun {
                len: gutter_text.len(),
                font: style.font(),
                color: input.placeholder_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            gutter_lines.push(window.text_system().shape_line(
                gutter_text.into(),
                font_size,
                &[gutter_run],
                None,
            ));
        }

        let layout = CachedCodeLayout {
            lines,
            gutter_lines,
            line_starts,
            line_ends,
            fold_starts,
            line_height,
            text_left,
        };
        let selection = if use_placeholder {
            Vec::new()
        } else {
            input.selection_quads(
                bounds,
                &layout.lines,
                &layout.line_starts,
                &layout.line_ends,
                layout.line_height,
                layout.text_left,
            )
        };
        let cursor = if !use_placeholder
            && input.selected_range.is_empty()
            && input.cursor_visible
        {
            input
                .position_for_index(input.cursor_offset())
                .map(|cursor_position| {
                    fill(
                        Bounds::new(cursor_position, size(px(2.0), line_height)),
                        input.cursor_color,
                    )
                })
        } else if !input.read_only && use_placeholder && input.cursor_visible {
            Some(fill(
                Bounds::new(point(text_left, bounds.top()), size(px(2.0), line_height)),
                input.cursor_color,
            ))
        } else {
            None
        };

        CodePrepaintState {
            layout,
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let input = self.input.read(cx);
        let focus_handle = input.focus_handle.clone();
        let debug_bounds = bounds;
        let _ = input;
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        let mut line_origin = bounds.origin;
        for (line, gutter_line) in prepaint
            .layout
            .lines
            .iter()
            .zip(prepaint.layout.gutter_lines.iter())
        {
            let gutter_origin = point(
                prepaint.layout.text_left - px(8.0) - gutter_line.width,
                line_origin.y,
            );
            gutter_line
                .paint(gutter_origin, prepaint.layout.line_height, window, cx)
                .unwrap();
            line.paint(
                point(prepaint.layout.text_left, line_origin.y),
                prepaint.layout.line_height,
                window,
                cx,
            )
            .unwrap();
            line_origin.y += prepaint.layout.line_height;
        }
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        let layout = prepaint.layout.clone();
        self.input.update(cx, move |input, _cx| {
            input.last_layout = Some(layout);
            input.last_bounds = Some(debug_bounds);
        });
    }
}

impl Render for CodeInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_handle.is_focused(window) {
            self.enable_cursor_blink(cx);
        } else if self.cursor_blink_enabled {
            self.disable_cursor_blink();
        }
        let debug_selector = self.debug_selector;

        div()
            .id(debug_selector)
            .debug_selector(move || debug_selector.into())
            .key_context("CodeInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::word_backspace))
            .on_action(cx.listener(Self::word_delete))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .child(CodeElement { input: cx.entity() })
    }
}

impl Focusable for CodeInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub(crate) fn bind_code_input_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", CodeBackspace, Some("CodeInput")),
        KeyBinding::new("delete", CodeDelete, Some("CodeInput")),
        KeyBinding::new("left", CodeLeft, Some("CodeInput")),
        KeyBinding::new("right", CodeRight, Some("CodeInput")),
        KeyBinding::new("up", CodeUp, Some("CodeInput")),
        KeyBinding::new("down", CodeDown, Some("CodeInput")),
        KeyBinding::new("ctrl-backspace", CodeWordBackspace, Some("CodeInput")),
        KeyBinding::new("ctrl-delete", CodeWordDelete, Some("CodeInput")),
        KeyBinding::new("ctrl-left", CodeWordLeft, Some("CodeInput")),
        KeyBinding::new("ctrl-right", CodeWordRight, Some("CodeInput")),
        KeyBinding::new("ctrl-shift-left", CodeSelectWordLeft, Some("CodeInput")),
        KeyBinding::new("ctrl-shift-right", CodeSelectWordRight, Some("CodeInput")),
        KeyBinding::new("shift-left", CodeSelectLeft, Some("CodeInput")),
        KeyBinding::new("shift-right", CodeSelectRight, Some("CodeInput")),
        KeyBinding::new("shift-up", CodeSelectUp, Some("CodeInput")),
        KeyBinding::new("shift-down", CodeSelectDown, Some("CodeInput")),
        KeyBinding::new("ctrl-a", CodeSelectAll, Some("CodeInput")),
        KeyBinding::new("ctrl-v", CodePaste, Some("CodeInput")),
        KeyBinding::new("ctrl-c", CodeCopy, Some("CodeInput")),
        KeyBinding::new("ctrl-x", CodeCut, Some("CodeInput")),
        KeyBinding::new("cmd-a", CodeSelectAll, Some("CodeInput")),
        KeyBinding::new("cmd-v", CodePaste, Some("CodeInput")),
        KeyBinding::new("cmd-c", CodeCopy, Some("CodeInput")),
        KeyBinding::new("cmd-x", CodeCut, Some("CodeInput")),
        KeyBinding::new("home", CodeHome, Some("CodeInput")),
        KeyBinding::new("end", CodeEnd, Some("CodeInput")),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_ranges_ignore_braces_inside_strings() {
        let folds = fold_ranges_for_text(
            "{\n  \"literal\": \"{ not a block }\",\n  \"items\": [\n    1\n  ]\n}",
        );

        assert!(
            folds.contains(&FoldRange {
                start_line: 0,
                end_line: 5,
                start_offset: 0,
                end_offset: folds
                    .iter()
                    .find(|fold| fold.start_line == 0)
                    .unwrap()
                    .end_offset,
                close_char: '}',
            })
        );
        assert!(
            folds
                .iter()
                .any(|fold| fold.start_line == 2 && fold.end_line == 4)
        );
        assert!(!folds.iter().any(|fold| fold.start_line == 1));
    }

    #[test]
    fn visible_lines_collapse_and_expand_json_blocks() {
        let text = "{\n  \"user\": {\n    \"name\": \"Ada\"\n  }\n}";
        let mut collapsed = BTreeSet::new();
        collapsed.insert(1);

        let visible = visible_lines_for_text(text, &collapsed);
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[1].source_line, 1);
        assert_eq!(visible[1].text, "  \"user\": { ... }");
        assert_eq!(visible[1].end, visible[1].start + "  \"user\": {".len());

        collapsed.clear();
        let expanded = visible_lines_for_text(text, &collapsed);
        assert_eq!(expanded.len(), 5);
        assert_eq!(expanded[1].text, "  \"user\": {");
    }

    #[test]
    fn gutter_width_grows_with_line_number_digits() {
        assert!(gutter_width(100) > gutter_width(9));
        assert_eq!(line_number_digits(100), 3);
    }

    #[test]
    fn syntax_runs_color_json_tokens() {
        let base = TextRun {
            len: 0,
            font: gpui::Font {
                family: "JetBrains Mono".into(),
                features: Default::default(),
                fallbacks: None,
                weight: gpui::FontWeight::NORMAL,
                style: gpui::FontStyle::Normal,
            },
            color: rgb(0xffffff).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let colors = SyntaxColors::default();
        let runs = syntax_runs_for_line("  \"ok\": true, \"count\": 12", &base, base.color, colors);

        assert!(runs.iter().any(|run| run.color == colors.property));
        assert!(runs.iter().any(|run| run.color == colors.keyword));
        assert!(runs.iter().any(|run| run.color == colors.number));
        assert!(runs.iter().any(|run| run.color == colors.punctuation));
        assert_eq!(
            runs.iter().map(|run| run.len).sum::<usize>(),
            "  \"ok\": true, \"count\": 12".len()
        );
    }
}
