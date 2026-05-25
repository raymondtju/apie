use std::{
    collections::{BTreeSet, HashMap},
    ops::Range,
    time::Duration,
};

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, InteractiveElement,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ShapedLine, SharedString, Style, Task, TextRun, UTF16Selection, Window, actions,
    div, fill, point, prelude::*, px, rgb, rgba, size,
};
use unicode_segmentation::GraphemeCursor;

/// Above this source-line count, fold detection is skipped entirely.
/// Folds are a quality-of-life feature; rescanning a multi-megabyte body
/// every edit is more expensive than the feature is worth.
const FOLD_DETECTION_LINE_LIMIT: usize = 5_000;

/// Maximum bytes of a single visible line that we feed to the OS text
/// shaper. Anything beyond this is truncated visually with a marker; the
/// underlying buffer keeps the full content so copy/paste/value() are
/// unaffected.
const SHAPE_LINE_BYTE_CAP: usize = 4_000;

/// Shape and gutter cache capacity.  8192 entries covers a full 5000-line
/// document with room to spare — no cache eviction during long drags
/// through the whole body.  Memory cost at ~2 KB per shaped line ≈ 16 MB.
const LINE_SHAPE_CACHE_LIMIT: usize = 8192;

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
    drag_in_progress: bool,
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
    line_index: LineIndex,
    fold_cache: Option<FoldCache>,
    /// Bumped on every content change. Used as the cache invalidation
    /// key so shaped lines from the previous revision are dropped on
    /// the next render.
    content_revision: u64,
    /// Identity of the syntax color set in use; bumped when colors
    /// change to invalidate cached runs/shapes that depend on them.
    syntax_revision: u64,
    /// Cache of shaped content lines keyed by source row, display form,
    /// and revision.
    /// Bound at LINE_SHAPE_CACHE_LIMIT entries via deterministic LRU eviction.
    shape_cache: HashMap<ShapeCacheKey, ShapedLineEntry>,
    /// Monotonic counter used for deterministic least-recently-used
    /// cache eviction.
    shape_cache_clock: u64,
    /// Cache of shaped gutter line numbers keyed by source row.
    gutter_cache: HashMap<(usize, GutterMarker), ShapedLine>,
    /// Last computed max content width (gutter + widest line) from the previous
    /// frame's prepaint. Used in request_layout to declare a scrollable width
    /// for horizontal overflow in the ancestor `overflow_scroll` container.
    last_content_width: Pixels,
    pub(crate) background_color: gpui::Hsla,
    pub(crate) gutter_border_color: Option<gpui::Hsla>,
}

#[derive(Clone)]
struct ShapedLineEntry {
    revision: u64,
    syntax_revision: u64,
    shaped: ShapedLine,
    last_used: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct ShapeCacheKey {
    source_row: usize,
    display: ShapeDisplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
enum ShapeDisplay {
    Normal,
    Plain,
    Folded(char),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
enum GutterMarker {
    None,
    Expanded,
    Collapsed,
}

/// Sum-tree-like line offset index. Stores the byte offset of each line
/// start in the buffer. Line `i` spans `starts[i] .. starts[i+1]-1`
/// (the trailing newline is excluded), or `starts[i] .. content.len()`
/// for the final line. This is the lightweight stand-in for Zed's rope
/// crate: O(1) line count, O(log n) row<->offset conversion.
#[derive(Clone, Default)]
struct LineIndex {
    /// Byte offsets of each line's first character. `starts[0]` is always 0.
    starts: Vec<usize>,
    total_len: usize,
}

impl LineIndex {
    fn build(text: &str) -> Self {
        let mut starts = Vec::with_capacity(text.len() / 64 + 1);
        starts.push(0);
        for (idx, byte) in text.as_bytes().iter().enumerate() {
            if *byte == b'\n' {
                starts.push(idx + 1);
            }
        }
        Self {
            starts,
            total_len: text.len(),
        }
    }

    fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Returns `(line_start, line_end)` byte offsets. `line_end` is the
    /// position of the trailing newline (or content end for the last
    /// line) and is exclusive of the newline byte.
    fn line_range(&self, row: usize) -> (usize, usize) {
        let start = self.starts[row];
        let end = if row + 1 < self.starts.len() {
            self.starts[row + 1].saturating_sub(1)
        } else {
            self.total_len
        };
        (start, end)
    }

    /// Returns the row containing byte offset `offset`.
    fn row_for_offset(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(row) => row,
            Err(row) => row.saturating_sub(1),
        }
    }
}

#[derive(Clone)]
struct FoldCache {
    ranges: Vec<FoldRange>,
}

#[derive(Clone)]
struct CachedCodeLayout {
    lines: Vec<ShapedLine>,
    gutter_lines: Vec<ShapedLine>,
    line_starts: Vec<usize>,
    line_ends: Vec<usize>,
    fold_starts: Vec<Option<usize>>,
    first_line_index: usize,
    #[cfg(test)]
    total_line_count: usize,
    line_height: Pixels,
    text_left: Pixels,
    /// Maximum advance width of the visible content (gutter + widest shaped line).
    /// Used to drive horizontal scroll range in the parent overflow container.
    content_width: Pixels,
}

#[cfg(test)]
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

/// Lightweight descriptor for one visible row inside the current
/// viewport.  Contains only byte offsets and fold metadata — no owned
/// text.  We allocate the actual display string only on a shape-cache
/// miss.  This is the core of the "Zed-style" viewport-only shaping.
#[derive(Clone, Copy)]
struct VisibleRow {
    source_row: usize,
    byte_start: usize,
    byte_end: usize,
    fold: Option<FoldRange>,
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

impl SyntaxColors {
    fn eq_to(&self, other: &SyntaxColors) -> bool {
        self.property == other.property
            && self.string == other.string
            && self.number == other.number
            && self.keyword == other.keyword
            && self.punctuation == other.punctuation
    }
}

fn fold_ranges_for_text(text: &str) -> Vec<FoldRange> {
    if text.len() > 4 * 1024 * 1024 {
        // Don't bother scanning multi-megabyte payloads; the line-count
        // gate above already disables fold detection for tall buffers,
        // but very wide single-line payloads also need a guard.
        return Vec::new();
    }
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

#[cfg(test)]
fn visible_lines_for_text(text: &str, collapsed_folds: &BTreeSet<usize>) -> Vec<CodeLine> {
    let index = LineIndex::build(text);
    let folds = fold_ranges_for_text(text);
    let mut visible = Vec::new();
    let mut row = 0usize;
    while row < index.line_count() {
        let (start, end) = index.line_range(row);
        let line_text = &text[start..end];
        let fold = folds.iter().find(|fold| fold.start_line == row);
        if let Some(fold) = fold {
            if collapsed_folds.contains(&fold.start_line) {
                let folded_text = format!("{} ... {}", line_text.trim_end(), fold.close_char);
                visible.push(CodeLine {
                    start,
                    end,
                    text: folded_text,
                    source_line: row,
                    fold_start: Some(fold.start_line),
                });
                row = fold.end_line.saturating_add(1);
                continue;
            }
            visible.push(CodeLine {
                start,
                end,
                text: line_text.to_string(),
                source_line: row,
                fold_start: Some(fold.start_line),
            });
        } else {
            visible.push(CodeLine {
                start,
                end,
                text: line_text.to_string(),
                source_line: row,
                fold_start: None,
            });
        }
        row += 1;
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
        let line_index = LineIndex::build(content.as_ref());
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
            drag_in_progress: false,
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
            line_index,
            fold_cache: None,
            content_revision: 0,
            syntax_revision: 0,
            shape_cache: HashMap::new(),
            shape_cache_clock: 0,
            gutter_cache: HashMap::new(),
            last_content_width: px(0.),
            background_color: rgba(0x00000000).into(),
            gutter_border_color: None,
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
        // Full clear — the entire content is replaced (e.g. switching requests).
        self.rebuild_line_index(None);
        self.prune_collapsed_folds();
        self.reset_cursor_blink(cx);
    }

    pub(crate) fn value(&self) -> String {
        self.content.to_string()
    }

    /// Returns the last computed content width (for use by the horizontal
    /// scrollbar to decide whether to show a thumb).
    pub(crate) fn last_content_width(&self) -> Pixels {
        self.last_content_width
    }

    /// Cheap identity comparison against a `SharedString` without
    /// cloning the underlying buffer. Same `Arc` storage hits the fast
    /// path. Falls back to byte comparison when the storage differs.
    pub(crate) fn content_eq(&self, other: &SharedString) -> bool {
        if std::ptr::eq(self.content.as_ref().as_ptr(), other.as_ref().as_ptr())
            && self.content.len() == other.len()
        {
            return true;
        }
        self.content.as_ref() == other.as_ref()
    }

    pub(crate) fn set_placeholder(&mut self, placeholder: impl Into<SharedString>) {
        self.placeholder = placeholder.into();
    }

    pub(crate) fn set_placeholder_color(&mut self, color: gpui::Hsla) {
        if self.placeholder_color != color {
            self.placeholder_color = color;
            self.gutter_cache.clear();
        }
    }

    pub(crate) fn set_background_color(&mut self, color: gpui::Hsla) {
        if self.background_color != color {
            self.background_color = color;
        }
    }

    pub(crate) fn set_gutter_border_color(&mut self, color: gpui::Hsla) {
        if self.gutter_border_color != Some(color) {
            self.gutter_border_color = Some(color);
        }
    }

    pub(crate) fn set_syntax_colors(&mut self, colors: SyntaxColors) {
        if !self.syntax_colors.eq_to(&colors) {
            self.syntax_colors = colors;
            self.syntax_revision = self.syntax_revision.wrapping_add(1);
            self.shape_cache.clear();
        }
    }

    pub(crate) fn set_drag_in_progress(&mut self, dragging: bool) {
        self.drag_in_progress = dragging;
    }

    pub(crate) fn drag_in_progress(&self) -> bool {
        self.drag_in_progress
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
        let affected_row = self.line_index.row_for_offset(range.start);
        let (new_text, cursor_delta) = self.newline_text_for_range(&range);
        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        let cursor = range.start + cursor_delta;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.rebuild_line_index(Some(affected_row));
        self.reset_cursor_blink(cx);
    }

    /// Rebuilds the line-offset index and selectively invalidates
    /// shape/gutter caches.
    ///
    /// `invalidate_from` — if `Some(row)`, only cache entries whose
    /// source row >= `row` are dropped (rows before the edit point have
    /// unchanged content and keep their cached shapes).  Pass `None` to
    /// clear everything (e.g. when the entire content is replaced).
    ///
    /// Fold detection is a full-document scan, so we only invalidate
    /// the fold cache when the line count actually changes (newlines
    /// inserted or removed).  Otherwise the cached fold ranges are
    /// still accurate and we skip the O(n) re-scan during typing.
    fn rebuild_line_index(&mut self, invalidate_from: Option<usize>) {
        let old_line_count = self.line_index.line_count();
        self.line_index = LineIndex::build(self.content.as_ref());

        if old_line_count != self.line_index.line_count() {
            // Line structure changed — fold ranges now point at wrong
            // source rows.  Recompute lazily on next access.
            self.fold_cache = None;
        }
        // Line count unchanged → fold ranges are still valid; keep cache.

        if let Some(from_row) = invalidate_from {
            self.shape_cache.retain(|key, _| key.source_row < from_row);
            self.gutter_cache.retain(|&(row, _), _| row < from_row);
        } else {
            self.shape_cache.clear();
            self.gutter_cache.clear();
        }

        self.content_revision = self.content_revision.wrapping_add(1);
    }

    fn folds(&mut self) -> &[FoldRange] {
        if self.fold_cache.is_none() {
            let ranges = if self.line_index.line_count() > FOLD_DETECTION_LINE_LIMIT {
                Vec::new()
            } else {
                fold_ranges_for_text(self.content.as_ref())
            };
            self.fold_cache = Some(FoldCache { ranges });
        }
        &self.fold_cache.as_ref().unwrap().ranges
    }

    /// Returns the currently collapsed folds as a sorted list.
    /// This is the small set we use for all visible-row <-> source-row
    /// mapping.  Because the number of user-collapsed blocks is almost
    /// always tiny, a linear Vec is perfectly fine (Zed uses sum trees
    /// for the general case; we only need the active ones here).
    fn active_collapsed(&mut self) -> Vec<FoldRange> {
        if self.collapsed_folds.is_empty() {
            return Vec::new();
        }
        // Avoid self-borrow conflict by materialising the folds first.
        let all = self.folds().to_vec();
        all.into_iter()
            .filter(|f| self.collapsed_folds.contains(&f.start_line))
            .collect()
    }

    /// Given a 0-based visible row index, return the corresponding
    /// source row and, if this visible row is the header of a collapsed
    /// fold, the FoldRange describing it.
    ///
    /// This replaces the old "walk the entire document from row 0 and
    /// allocate strings for everything" approach.  We only pay O(#folds)
    /// once per frame for the first visible row in the viewport, then
    /// advance sequentially for the next ~50 rows.  This is the Zed
    /// pattern (only materialize what you are about to draw).
    fn source_row_for_visible_row(&mut self, target_vis: usize) -> (usize, Option<FoldRange>) {
        let collapsed = self.active_collapsed();
        let mut vis = 0usize;
        let mut src = 0usize;
        let total_src = self.line_index.line_count();
        while src < total_src && vis <= target_vis {
            if let Some(f) = collapsed.iter().find(|f| f.start_line == src).copied() {
                if vis == target_vis {
                    return (src, Some(f));
                }
                // The current visible row shows the folded header.
                // Skip the entire folded region for subsequent rows.
                src = f.end_line + 1;
                vis += 1;
                continue;
            }
            if vis == target_vis {
                return (src, None);
            }
            src += 1;
            vis += 1;
        }
        (src.min(total_src.saturating_sub(1)), None)
    }

    pub(crate) fn visible_line_count(&mut self) -> usize {
        let source = self.line_index.line_count().max(1);
        if self.collapsed_folds.is_empty() {
            return source;
        }
        let mut hidden = 0usize;
        let folds: Vec<FoldRange> = self.folds().to_vec();
        for fold in folds {
            if self.collapsed_folds.contains(&fold.start_line) {
                hidden += fold.end_line.saturating_sub(fold.start_line);
            }
        }
        source.saturating_sub(hidden).max(1)
    }

    #[cfg(test)]
    pub(crate) fn rendered_line_count(&self) -> usize {
        self.last_layout
            .as_ref()
            .map(|layout| layout.lines.len())
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn cached_total_line_count(&self) -> usize {
        self.last_layout
            .as_ref()
            .map(|layout| layout.total_line_count)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn rendered_first_line_index(&self) -> usize {
        self.last_layout
            .as_ref()
            .map(|layout| layout.first_line_index)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn shape_cache_len(&self) -> usize {
        self.shape_cache.len()
    }

    #[cfg(test)]
    pub(crate) fn rendered_rows_have_cached_shapes(&self) -> bool {
        let Some(layout) = self.last_layout.as_ref() else {
            return false;
        };
        layout
            .line_starts
            .iter()
            .zip(layout.fold_starts.iter())
            .all(|(start, fold_start)| {
                let source_row = self.line_index.row_for_offset(*start);
                let display = if fold_start.is_some() {
                    self.shape_cache
                        .keys()
                        .find(|key| key.source_row == source_row)
                        .map(|key| key.display)
                        .unwrap_or(ShapeDisplay::Normal)
                } else {
                    ShapeDisplay::Normal
                };
                self.shape_cache
                    .get(&ShapeCacheKey {
                        source_row,
                        display,
                    })
                    .is_some_and(|entry| {
                        entry.revision == self.content_revision
                            && entry.syntax_revision == self.syntax_revision
                    })
            })
    }

    pub(crate) fn source_line_count(&self) -> usize {
        self.line_index.line_count()
    }

    /// Returns the minimal information needed to render the visible rows
    /// in `range`.  This is O(1) per returned row after we locate the
    /// starting source row (O(#collapsed folds) for the jump).
    fn visible_rows(&mut self, range: Range<usize>) -> Vec<VisibleRow> {
        let total = self.visible_line_count();
        if total == 0 || range.start >= total {
            return Vec::new();
        }
        let end = range.end.min(total);
        let start = range.start.min(end);

        let mut rows = Vec::with_capacity(end - start);

        let (mut source_row, mut fold) = self.source_row_for_visible_row(start);
        let mut vis = start;

        while vis < end {
            let (bstart, bend) = self.line_index.line_range(source_row);
            rows.push(VisibleRow {
                source_row,
                byte_start: bstart,
                byte_end: bend,
                fold,
            });

            vis += 1;
            if let Some(f) = fold {
                source_row = f.end_line + 1;
                fold = None;
            } else {
                source_row += 1;
            }

            if fold.is_none() {
                fold = self
                    .active_collapsed()
                    .into_iter()
                    .find(|f| f.start_line == source_row);
            }
        }
        rows
    }

    fn prune_collapsed_folds(&mut self) {
        let valid_folds: BTreeSet<usize> =
            self.folds().iter().map(|fold| fold.start_line).collect();
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
        let row = self.line_index.row_for_offset(cursor);
        let (line_start, _) = self.line_index.line_range(row);
        self.move_to(line_start, cx);
    }

    fn end(&mut self, _: &CodeEnd, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let row = self.line_index.row_for_offset(cursor);
        let (_, line_end) = self.line_index.line_range(row);
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
        // Build only the lines we actually need: current visible row
        // plus the neighbour in the requested direction. This keeps
        // arrow-key navigation O(folds) instead of O(document).
        let total = self.visible_line_count();
        if total == 0 {
            return;
        }
        let cursor = self.cursor_offset();
        let cursor_source_row = self.line_index.row_for_offset(cursor);
        let folds: Vec<FoldRange> = self.folds().to_vec();
        let collapsed: Vec<FoldRange> = folds
            .into_iter()
            .filter(|fold| self.collapsed_folds.contains(&fold.start_line))
            .collect();

        // Map source row to visible row (and find the visible row's
        // source row, which may differ when the cursor is inside a
        // collapsed range).
        let mut visible_row_of_cursor = 0usize;
        let mut current_visible_source = 0usize;
        let mut source_row = 0usize;
        let total_source = self.line_index.line_count();
        let mut visible_row = 0usize;
        while source_row < total_source {
            let collapse = collapsed
                .iter()
                .find(|fold| fold.start_line == source_row)
                .copied();
            let span_end = collapse.map(|fold| fold.end_line).unwrap_or(source_row);
            if cursor_source_row <= span_end && source_row <= cursor_source_row {
                visible_row_of_cursor = visible_row;
                current_visible_source = source_row;
                break;
            }
            visible_row += 1;
            source_row = if let Some(fold) = collapse {
                fold.end_line.saturating_add(1)
            } else {
                source_row + 1
            };
        }

        let target_visible_row = (visible_row_of_cursor as i32 + direction)
            .clamp(0, total.saturating_sub(1) as i32) as usize;
        if target_visible_row == visible_row_of_cursor {
            return;
        }

        // Walk to the target visible row in source-row space.
        let (current_start, _) = self.line_index.line_range(current_visible_source);
        let column = cursor.saturating_sub(current_start);

        let mut walked_visible = visible_row_of_cursor;
        let mut walked_source = current_visible_source;
        while walked_visible != target_visible_row {
            let collapse = collapsed
                .iter()
                .find(|fold| fold.start_line == walked_source)
                .copied();
            walked_source = if let Some(fold) = collapse {
                fold.end_line.saturating_add(1)
            } else {
                walked_source + 1
            };
            if direction > 0 {
                walked_visible += 1;
            } else if walked_visible == 0 {
                break;
            }
            if direction < 0 {
                // Going backward: we walk forward to find the row at
                // `target_visible_row`. This branch should not be hit;
                // direction-aware traversal happens below.
                break;
            }
        }

        // Backward direction needs a forward walk from row 0 to the
        // target visible row, which is still O(target_row + folds) and
        // bounded by the viewport in practice.
        if direction < 0 {
            walked_visible = 0;
            walked_source = 0;
            while walked_visible < target_visible_row && walked_source < total_source {
                let collapse = collapsed
                    .iter()
                    .find(|fold| fold.start_line == walked_source)
                    .copied();
                walked_visible += 1;
                walked_source = if let Some(fold) = collapse {
                    fold.end_line.saturating_add(1)
                } else {
                    walked_source + 1
                };
            }
        }

        let (target_start, target_end) = self.line_index.line_range(walked_source);
        let target_offset = (target_start + column).min(target_end);
        let target_offset = self.clamp_offset(target_offset);
        if select {
            self.select_to(target_offset, cx);
        } else {
            self.move_to(target_offset, cx);
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
        let mut line_origin_y = bounds.top() + layout.line_height * layout.first_line_index as f32;
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
        self.fold_cache = None;
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

        let absolute_line_index = ((position.y - bounds.top()) / layout.line_height)
            .floor()
            .max(0.0) as usize;
        let Some(relative_line_index) = absolute_line_index.checked_sub(layout.first_line_index)
        else {
            return first_offset_for_layout(layout).unwrap_or_else(|| self.content.len());
        };
        if relative_line_index >= layout.lines.len() {
            return last_offset_for_layout(layout).unwrap_or_else(|| self.content.len());
        }

        let mut line_origin_y = bounds.top() + layout.line_height * absolute_line_index as f32;
        for ((line, line_start), line_end) in layout
            .lines
            .iter()
            .zip(layout.line_starts.iter().copied())
            .zip(layout.line_ends.iter().copied())
            .skip(relative_line_index)
        {
            if position.y <= line_origin_y + layout.line_height {
                // The parent overflow_scroll container already translates our
                // bounds by its scroll offset (via `with_element_offset`), so
                // `layout.text_left` is already in scroll-corrected coords.
                // No manual subtraction needed here.
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
        position_for_index_in_layout(index, bounds, layout)
    }

    fn selection_quads(
        &self,
        bounds: Bounds<Pixels>,
        lines: &[ShapedLine],
        line_starts: &[usize],
        line_ends: &[usize],
        first_line_index: usize,
        line_height: Pixels,
        text_left: Pixels,
    ) -> Vec<PaintQuad> {
        if self.selected_range.is_empty() {
            return Vec::new();
        }
        let mut quads = Vec::new();
        let mut line_origin_y = bounds.top() + line_height * first_line_index as f32;
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
                // The parent overflow_scroll container already translates our
                // bounds by its scroll offset, so `text_left` is already in
                // scroll-corrected window coords.
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
        // Capture the affected source row BEFORE mutating content.
        let affected_row = self.line_index.row_for_offset(range.start);
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        let cursor = range.start + cursor_offset;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range.take();
        self.rebuild_line_index(Some(affected_row));
        // Fold detection is deferred to render; do not call
        // prune_collapsed_folds here — it triggers a full-document scan.
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
        let text = self.content.as_ref();
        let offset = offset.min(text.len());
        let mut cursor = GraphemeCursor::new(offset, text.len(), true);
        match cursor.prev_boundary(text, 0) {
            Ok(Some(prev)) => prev,
            _ => 0,
        }
    }

    fn next_boundary(&self, offset: usize) -> usize {
        let text = self.content.as_ref();
        let offset = offset.min(text.len());
        let mut cursor = GraphemeCursor::new(offset, text.len(), true);
        match cursor.next_boundary(text, 0) {
            Ok(Some(next)) => next,
            _ => text.len(),
        }
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
        let affected_row = self.line_index.row_for_offset(range.start);
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
        self.rebuild_line_index(Some(affected_row));
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VisibleLineRange {
    start: usize,
    end: usize,
}

fn visible_line_range(
    bounds: Bounds<Pixels>,
    visible_bounds: Bounds<Pixels>,
    line_height: Pixels,
    total_line_count: usize,
) -> VisibleLineRange {
    // Moderate overscan: pre-shapes a few rows above/below the viewport
    // to absorb rapid wheel scrolling without cache misses on every
    // frame.  5-line overscan adds ~10 shaped lines per frame (negligible
    // cost) while preventing the stutter that 1-line overscan caused on
    // fast scrolls.  The 1024-entry shape cache absorbs the overhead.
    const OVERSCAN_LINES: usize = 5;

    if total_line_count == 0 || line_height <= px(0.0) {
        return VisibleLineRange { start: 0, end: 0 };
    }

    let clipped_top = (visible_bounds.top() - bounds.top()).max(px(0.0));
    let clipped_bottom = (visible_bounds.bottom() - bounds.top()).max(px(0.0));
    let first_visible = ((clipped_top / line_height).floor() as usize)
        .saturating_sub(OVERSCAN_LINES)
        .min(total_line_count);
    let last_visible = ((clipped_bottom / line_height).ceil() as usize)
        .saturating_add(OVERSCAN_LINES)
        .min(total_line_count);
    let end = last_visible.max(first_visible.saturating_add(1).min(total_line_count));

    VisibleLineRange {
        start: first_visible,
        end,
    }
}

fn position_for_index_in_layout(
    index: usize,
    bounds: Bounds<Pixels>,
    layout: &CachedCodeLayout,
) -> Option<Point<Pixels>> {
    let mut line_origin_y = bounds.top() + layout.line_height * layout.first_line_index as f32;
    for ((line, line_start), line_end) in layout
        .lines
        .iter()
        .zip(layout.line_starts.iter().copied())
        .zip(layout.line_ends.iter().copied())
    {
        if index >= line_start && index <= line_end {
            let local_index = index.saturating_sub(line_start);
            // The parent overflow_scroll container already translates our
            // bounds by its scroll offset, so `layout.text_left` is already in
            // scroll-corrected window coords.
            return Some(point(
                layout.text_left + line.x_for_index(local_index.min(line.len())),
                line_origin_y,
            ));
        }
        line_origin_y += layout.line_height;
    }
    None
}

fn first_offset_for_layout(layout: &CachedCodeLayout) -> Option<usize> {
    layout.line_starts.first().copied()
}

fn last_offset_for_layout(layout: &CachedCodeLayout) -> Option<usize> {
    layout.line_ends.last().copied()
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

fn display_for_row(row: VisibleRow, drag_in_progress: bool) -> ShapeDisplay {
    row.fold
        .map(|fold| ShapeDisplay::Folded(fold.close_char))
        .unwrap_or(if drag_in_progress {
            ShapeDisplay::Plain
        } else {
            ShapeDisplay::Normal
        })
}

fn display_text_for_row(content: &str, row: VisibleRow) -> String {
    let raw = &content[row.byte_start..row.byte_end];
    if let Some(fold) = row.fold {
        format!("{} ... {}", raw.trim_end(), fold.close_char)
    } else {
        raw.to_string()
    }
}

fn truncate_shape_text(text: String) -> String {
    if text.len() <= SHAPE_LINE_BYTE_CAP {
        return text;
    }

    let mut cap = SHAPE_LINE_BYTE_CAP;
    while cap > 0 && !text.is_char_boundary(cap) {
        cap -= 1;
    }
    let total = text.len();
    let mut truncated = text[..cap].to_string();
    truncated.push_str(&format!(" ... [line truncated, {} chars total]", total));
    truncated
}

fn prune_shape_cache(
    cache: &mut HashMap<ShapeCacheKey, ShapedLineEntry>,
    protected: &BTreeSet<ShapeCacheKey>,
) {
    if cache.len() <= LINE_SHAPE_CACHE_LIMIT {
        return;
    }

    let mut candidates = cache
        .iter()
        .filter_map(|(key, entry)| (!protected.contains(key)).then_some((*key, entry.last_used)))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(key, last_used)| (*last_used, *key));

    for (key, _) in candidates {
        if cache.len() <= LINE_SHAPE_CACHE_LIMIT {
            break;
        }
        cache.remove(&key);
    }
}

fn syntax_runs_for_line(
    line: &str,
    base_run: &TextRun,
    base_color: gpui::Hsla,
    colors: SyntaxColors,
) -> Vec<TextRun> {
    let bytes = line.as_bytes();
    let mut runs = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let start = i;
        let b = bytes[i];

        if b == b'"' || b == b'\'' {
            let quote = b;
            i += 1;
            while i < bytes.len() && bytes[i] != quote {
                if bytes[i] == b'\\' {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < bytes.len() {
                i += 1;
            }
            let end = i;
            let is_property = {
                let mut look = i;
                while look < bytes.len() && matches!(bytes[look], b' ' | b'\t') {
                    look += 1;
                }
                look < bytes.len() && bytes[look] == b':'
            };
            runs.push(TextRun {
                len: end - start,
                color: if is_property {
                    colors.property
                } else {
                    colors.string
                },
                ..base_run.clone()
            });
        } else if b.is_ascii_digit() || b == b'-' {
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_digit()
                    || matches!(bytes[i], b'.' | b'e' | b'E' | b'+' | b'-'))
            {
                i += 1;
            }
            runs.push(TextRun {
                len: i - start,
                color: colors.number,
                ..base_run.clone()
            });
        } else if b.is_ascii_alphabetic() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let color = if &bytes[start..i] == b"true"
                || &bytes[start..i] == b"false"
                || &bytes[start..i] == b"null"
            {
                colors.keyword
            } else {
                base_color
            };
            runs.push(TextRun {
                len: i - start,
                color,
                ..base_run.clone()
            });
        } else if matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',') {
            i += 1;
            runs.push(TextRun {
                len: 1,
                color: colors.punctuation,
                ..base_run.clone()
            });
        } else if b.is_ascii() {
            // Batch consecutive plain-ASCII characters (whitespace, etc.)
            // into a single run to avoid O(N) TextRun allocations.
            i += 1;
            while i < bytes.len()
                && bytes[i].is_ascii()
                && !matches!(
                    bytes[i],
                    b'"' | b'\'' | b'{' | b'}' | b'[' | b']' | b':' | b','
                )
                && !bytes[i].is_ascii_alphanumeric()
                && bytes[i] != b'-'
            {
                i += 1;
            }
            runs.push(TextRun {
                len: i - start,
                color: base_color,
                ..base_run.clone()
            });
        } else {
            let ch_len = line[start..].chars().next().map_or(1, |c| c.len_utf8());
            i += ch_len;
            runs.push(TextRun {
                len: ch_len,
                color: base_color,
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
        let line_count = self.input.update(cx, |input, _| input.visible_line_count());
        let last_content_width = self.input.update(cx, |input, _| input.last_content_width);

        let mut style = Style::default();

        // Prefer the accurately measured width from the previous frame's shaping.
        // If we don't have one yet (first render of a document), compute a cheap
        // estimate from the raw text so the scroll container immediately sees
        // horizontal range for wide content. This solves the "horizontal scroll
        // not available on first load" problem.
        let desired_width = if last_content_width > px(0.) {
            last_content_width
        } else {
            self.input.update(cx, |input, _| {
                let longest = input
                    .content
                    .lines()
                    .map(|l| l.chars().count())
                    .max()
                    .unwrap_or(0);

                // Conservative first-frame estimate (only used until the first real
                // shaping pass stores an accurate `content_width`).
                // ~7.5px per char is reasonable for typical monospace at default sizes.
                // Use a more generous estimate (9px per char + gutter) so that
                // even moderately wide JSON triggers horizontal scroll range on the
                // very first frame. The real shaped value will refine it immediately.
                // Cap at 20k px to avoid cosmic-text assertion failures on multi-MB
                // single-line responses (e.g. httpbin.org/bytes/1500000).
                let estimated_text = px((longest as f32 * 9.0).min(20_000.0));

                let approx_gutter = gutter_width(input.source_line_count().max(1));
                estimated_text + approx_gutter + px(24.)
            })
        };

        style.size.width = desired_width.into();
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
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();

        let (use_placeholder, total_line_count, max_line_number, drag_in_progress) =
            self.input.update(cx, |input, _| {
                let use_placeholder = input.content.is_empty();
                let visible = if use_placeholder {
                    1
                } else {
                    input.visible_line_count()
                };
                (
                    use_placeholder,
                    visible,
                    input.source_line_count().max(1),
                    input.drag_in_progress(),
                )
            });
        let gutter_width_px = gutter_width(max_line_number);
        let text_left = bounds.left() + gutter_width_px;

        // Note: GPUI's parent `overflow_scroll` container automatically
        // translates our `bounds.origin` by its scroll offset (via
        // `with_element_offset` during prepaint), so `text_left` above is
        // already in scroll-corrected window coords. The sticky gutter
        // re-anchors itself to `visible_bounds.left()` during paint.

        let render_range = visible_line_range(
            bounds,
            window.content_mask().bounds,
            line_height,
            total_line_count,
        );

        // The best (Zed-recommended) path: obtain only the cheap per-row
        // descriptors for the exact visible slice.  We never allocate the
        // display text (or run the syntax highlighter) for rows that are
        // already in the shape cache.  This is what lets 1000-line (and
        // much larger) responses stay smooth.
        let visible_rows: Vec<VisibleRow> = if use_placeholder {
            // placeholder is a single synthetic row
            vec![VisibleRow {
                source_row: 0,
                byte_start: 0,
                byte_end: 0,
                fold: None,
            }]
        } else {
            self.input.update(cx, |input, _| {
                input.visible_rows(render_range.start..render_range.end)
            })
        };

        let (text_color, syntax_colors, placeholder_color, content_revision, syntax_revision) = {
            let input = self.input.read(cx);
            (
                if use_placeholder {
                    input.placeholder_color
                } else {
                    style.color
                },
                input.syntax_colors,
                input.placeholder_color,
                input.content_revision,
                input.syntax_revision,
            )
        };

        // Read entity state once before the hot loop. SharedString::clone
        // is arc-refcount; the shaped-line cache is probed per visible row
        // so we do not clone the full cache on every scroll frame.
        let (content_buf, placeholder_buf, collapsed_folds) = {
            let input = self.input.read(cx);
            (
                if use_placeholder {
                    SharedString::default()
                } else {
                    input.content.clone()
                },
                if use_placeholder {
                    input.placeholder.clone()
                } else {
                    SharedString::default()
                },
                if use_placeholder {
                    BTreeSet::new()
                } else {
                    input.collapsed_folds.clone()
                },
            )
        };

        let mut cache_clock = self.input.read(cx).shape_cache_clock;
        // Pending cache inserts/touches are applied after the hot loop in
        // one update call.
        let mut pending_shape_inserts: Vec<(ShapeCacheKey, ShapedLineEntry)> = Vec::new();
        let mut pending_shape_touches: Vec<(ShapeCacheKey, u64)> = Vec::new();
        let mut pending_gutter_inserts: Vec<((usize, GutterMarker), ShapedLine)> = Vec::new();
        let mut protected_shape_keys = BTreeSet::new();

        let mut lines = Vec::with_capacity(visible_rows.len());
        let mut gutter_lines = Vec::with_capacity(visible_rows.len());
        let mut line_starts = Vec::with_capacity(visible_rows.len());
        let mut line_ends = Vec::with_capacity(visible_rows.len());
        let mut fold_starts = Vec::with_capacity(visible_rows.len());
        let gutter_digits = line_number_digits(max_line_number);

        for vrow in visible_rows {
            line_starts.push(vrow.byte_start);
            line_ends.push(vrow.byte_end);
            fold_starts.push(vrow.fold.map(|f| f.start_line));

            let source_row = vrow.source_row;
            let shape_key = ShapeCacheKey {
                source_row,
                display: display_for_row(vrow, drag_in_progress),
            };
            protected_shape_keys.insert(shape_key);
            if drag_in_progress && shape_key.display != ShapeDisplay::Normal {
                protected_shape_keys.insert(ShapeCacheKey {
                    source_row,
                    display: ShapeDisplay::Normal,
                });
            }
            cache_clock = cache_clock.wrapping_add(1);
            let collapsed_marker = if vrow.fold.is_some() {
                if collapsed_folds.contains(&source_row) {
                    GutterMarker::Collapsed
                } else {
                    GutterMarker::Expanded
                }
            } else {
                GutterMarker::None
            };
            let gutter_key = (source_row, collapsed_marker);

            // Single entity read for both caches: avoids a second
            // GPUI entity borrow per iteration.
            let (cached_shape, cached_gutter) = if !use_placeholder {
                let input = self.input.read(cx);
                let shape = input
                    .shape_cache
                    .get(&shape_key)
                    .filter(|entry| {
                        entry.revision == content_revision
                            && entry.syntax_revision == syntax_revision
                    })
                    .map(|entry| entry.shaped.clone())
                    .or_else(|| {
                        // During drag, also check for Normal entries so
                        // previously-shaped coloured lines stay cached.
                        if drag_in_progress && shape_key.display != ShapeDisplay::Normal {
                            input
                                .shape_cache
                                .get(&ShapeCacheKey {
                                    source_row,
                                    display: ShapeDisplay::Normal,
                                })
                                .filter(|entry| {
                                    entry.revision == content_revision
                                        && entry.syntax_revision == syntax_revision
                                })
                                .map(|entry| entry.shaped.clone())
                        } else {
                            None
                        }
                    });
                let gutter = input.gutter_cache.get(&gutter_key).cloned();
                (shape, gutter)
            } else {
                (None, None)
            };

            let shaped_line = if let Some(shaped) = cached_shape {
                pending_shape_touches.push((shape_key, cache_clock));
                shaped
            } else {
                // Cache miss: shape the visible row with final syntax runs
                // immediately. This avoids the uncoloured first frame that
                // was visible when scrolling into uncached response lines.
                let shape_text = if use_placeholder {
                    placeholder_buf.to_string()
                } else {
                    display_text_for_row(content_buf.as_ref(), vrow)
                };
                let shape_text = truncate_shape_text(shape_text);
                let base_run = TextRun {
                    len: shape_text.len(),
                    font: style.font(),
                    color: text_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let runs = if use_placeholder || drag_in_progress {
                    vec![base_run]
                } else {
                    syntax_runs_for_line(&shape_text, &base_run, text_color, syntax_colors)
                };
                let shaped =
                    window
                        .text_system()
                        .shape_line(shape_text.into(), font_size, &runs, None);

                if !use_placeholder {
                    pending_shape_inserts.push((
                        shape_key,
                        ShapedLineEntry {
                            revision: content_revision,
                            syntax_revision,
                            shaped: shaped.clone(),
                            last_used: cache_clock,
                        },
                    ));
                }
                shaped
            };
            lines.push(shaped_line);

            let gutter_shape = if let Some(shaped) = cached_gutter {
                shaped
            } else {
                let fold_marker = match collapsed_marker {
                    GutterMarker::Collapsed => ">",
                    GutterMarker::Expanded => "v",
                    GutterMarker::None => " ",
                };
                let gutter_text = format!(
                    "{fold_marker} {:>width$}",
                    source_row + 1,
                    width = gutter_digits
                );
                let gutter_run = TextRun {
                    len: gutter_text.len(),
                    font: style.font(),
                    color: placeholder_color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let shaped = window.text_system().shape_line(
                    gutter_text.into(),
                    font_size,
                    &[gutter_run],
                    None,
                );
                pending_gutter_inserts.push((gutter_key, shaped.clone()));
                shaped
            };
            gutter_lines.push(gutter_shape);
        }

        // --- Batch cache updates ---
        // All shape/gutter misses and LRU touches were collected during
        // the loop; apply them atomically now in a single entity update.
        if !use_placeholder
            && (!pending_shape_inserts.is_empty()
                || !pending_shape_touches.is_empty()
                || !pending_gutter_inserts.is_empty())
        {
            self.input.update(cx, |input, _| {
                input.shape_cache_clock = cache_clock;
                for (key, last_used) in pending_shape_touches.drain(..) {
                    if let Some(entry) = input.shape_cache.get_mut(&key) {
                        entry.last_used = last_used;
                    }
                }
                for (key, entry) in pending_shape_inserts.drain(..) {
                    input.shape_cache.insert(key, entry);
                }
                prune_shape_cache(&mut input.shape_cache, &protected_shape_keys);

                for (key, shaped) in pending_gutter_inserts.drain(..) {
                    input.gutter_cache.insert(key, shaped);
                    if input.gutter_cache.len() > LINE_SHAPE_CACHE_LIMIT {
                        let mut keys = input.gutter_cache.keys().copied().collect::<Vec<_>>();
                        keys.sort_by_key(|(row, marker)| (*row, *marker));
                        if let Some(k) = keys.into_iter().next() {
                            input.gutter_cache.remove(&k);
                        }
                    }
                }
            });
        }

        // Compute horizontal content width from the just-shaped visible lines.
        // This (plus gutter) determines the horizontal scroll range reported to
        // the parent overflow_scroll container. We use only the currently visible
        // (plus overscan) lines for cost reasons — sufficient for the vast majority
        // of real-world JSON, including deeply nested or minified cases.
        //
        // Note: We call the gutter_width *function* here (before/after the local
        // `let gutter_width = ...` binding in this scope) and access the public
        // `.width` field on ShapedLine (not a method).
        //
        // Cap at 20k px to avoid cosmic-text assertion failures on multi-MB
        // single-line responses (e.g. httpbin.org/bytes/1500000).
        let gutter_width_px_for_content = gutter_width(max_line_number);
        let max_text_advance = lines
            .iter()
            .map(|line| line.width)
            .fold(px(0.0), |acc, w| acc.max(w));
        let content_width = (gutter_width_px_for_content + max_text_advance).min(px(20_000.0));

        let layout = CachedCodeLayout {
            lines,
            gutter_lines,
            line_starts,
            line_ends,
            fold_starts,
            first_line_index: render_range.start,
            #[cfg(test)]
            total_line_count,
            line_height,
            text_left,
            content_width,
        };
        let input = self.input.read(cx);
        let selection = if use_placeholder {
            Vec::new()
        } else {
            input.selection_quads(
                bounds,
                &layout.lines,
                &layout.line_starts,
                &layout.line_ends,
                layout.first_line_index,
                layout.line_height,
                layout.text_left,
            )
        };
        let cursor = if !use_placeholder && input.selected_range.is_empty() && input.cursor_visible
        {
            position_for_index_in_layout(input.cursor_offset(), bounds, &layout).map(
                |cursor_position| {
                    fill(
                        Bounds::new(cursor_position, size(px(2.0), line_height)),
                        input.cursor_color,
                    )
                },
            )
        } else if !input.read_only && use_placeholder && input.cursor_visible {
            // Bounds are already scroll-shifted by the parent overflow_scroll
            // container; paint the placeholder caret in the same coord space.
            Some(fill(
                Bounds::new(point(text_left, bounds.top()), size(px(2.0), line_height)),
                input.cursor_color,
            ))
        } else {
            None
        };

        // Store layout for position_for_index + hit testing, and the computed
        // content width so the *next* request_layout can declare a proper
        // horizontal scrollable size.
        let stored_layout = layout.clone();
        let new_content_width = layout.content_width;
        self.input.update(cx, move |input, _cx| {
            input.last_layout = Some(stored_layout);
            input.last_bounds = Some(bounds);
            input.last_content_width = new_content_width;
        });

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
        let (focus_handle, background_color, gutter_border_color) = {
            let input = self.input.read(cx);
            (
                input.focus_handle.clone(),
                input.background_color,
                input.gutter_border_color,
            )
        };

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }

        // 1. Paint text lines. The parent overflow_scroll container has
        // already translated our `bounds` by its scroll offset, so painting
        // at `text_left` directly draws in scroll-shifted window coords.
        let mut line_origin = bounds.origin;
        line_origin.y += prepaint.layout.line_height * prepaint.layout.first_line_index as f32;
        for line in &prepaint.layout.lines {
            line.paint(
                point(prepaint.layout.text_left, line_origin.y),
                prepaint.layout.line_height,
                window,
                cx,
            )
            .unwrap();
            line_origin.y += prepaint.layout.line_height;
        }

        // 2. Paint solid gutter background and border line
        let gutter_width_px = prepaint.layout.text_left - bounds.left();
        let visible_bounds = window.content_mask().bounds;

        if visible_bounds.left() < bounds.right() && gutter_width_px > px(0.0) {
            let gutter_rect = Bounds::new(
                point(visible_bounds.left(), bounds.top()),
                size(gutter_width_px, bounds.size.height),
            );
            window.paint_quad(fill(gutter_rect, background_color));

            // Paint subtle vertical border on the right of the gutter
            if let Some(border_color) = gutter_border_color {
                let border_rect = Bounds::new(
                    point(
                        visible_bounds.left() + gutter_width_px - px(1.0),
                        bounds.top(),
                    ),
                    size(px(1.0), bounds.size.height),
                );
                window.paint_quad(fill(border_rect, border_color));
            }
        }

        // 3. Paint gutter line numbers (sticky to the viewport left).
        // `bounds.left()` is in scroll-shifted window coords; pinning the
        // gutter to `visible_bounds.left()` keeps it in place even when
        // the parent scroll container has translated everything else.
        let mut line_origin = bounds.origin;
        line_origin.y += prepaint.layout.line_height * prepaint.layout.first_line_index as f32;
        let sticky_text_left = visible_bounds.left() + gutter_width_px;
        for gutter_line in &prepaint.layout.gutter_lines {
            let gutter_origin = point(
                sticky_text_left - px(8.0) - gutter_line.width,
                line_origin.y,
            );

            // Only paint the gutter line number if it's within the viewport bounds
            if gutter_origin.x >= visible_bounds.left() && gutter_origin.x < visible_bounds.right()
            {
                gutter_line
                    .paint(gutter_origin, prepaint.layout.line_height, window, cx)
                    .unwrap();
            }
            line_origin.y += prepaint.layout.line_height;
        }

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
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
            // `min_w_full` (not `w_full`) lets the wrapper grow with the
            // CodeElement's intrinsic width when content is wider than the
            // viewport, which is what an ancestor `overflow_scroll` needs to
            // detect horizontal overflow and enable wheel/bar scrolling.
            // Narrow content still fills the viewport via the 100% min-width.
            .min_w_full()
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
    fn visible_line_range_clamps_to_viewport_with_overscan() {
        let bounds = Bounds::new(point(px(0.0), px(-400.0)), size(px(500.0), px(2000.0)));
        let visible_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(500.0), px(240.0)));

        let range = visible_line_range(bounds, visible_bounds, px(20.0), 200);

        // With OVERSCAN_LINES=5: row 20 visible → 20-5=15 start, 32+5=37 end
        assert_eq!(range.start, 15);
        assert_eq!(range.end, 37);
    }

    #[test]
    fn visible_line_range_clamps_at_document_edges() {
        let line_height = px(20.0);
        let visible_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(500.0), px(240.0)));

        let top = visible_line_range(
            Bounds::new(point(px(0.0), px(0.0)), size(px(500.0), px(2000.0))),
            visible_bounds,
            line_height,
            100,
        );
        // With OVERSCAN_LINES=5: rows 0..12 visible → end = 12+5 = 17
        assert_eq!(top, VisibleLineRange { start: 0, end: 17 });

        let bottom = visible_line_range(
            Bounds::new(point(px(0.0), px(-1880.0)), size(px(500.0), px(2000.0))),
            visible_bounds,
            line_height,
            100,
        );
        // With OVERSCAN_LINES=5: row 94 is first → 94-5 = 89 start
        assert_eq!(
            bottom,
            VisibleLineRange {
                start: 89,
                end: 100
            }
        );
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

    #[test]
    fn shape_cache_key_distinguishes_folded_display_text() {
        let normal = ShapeCacheKey {
            source_row: 2,
            display: ShapeDisplay::Normal,
        };
        let folded = ShapeCacheKey {
            source_row: 2,
            display: ShapeDisplay::Folded('}'),
        };

        assert_ne!(normal, folded);
    }
}
