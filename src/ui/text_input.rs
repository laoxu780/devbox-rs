//! A small text editor used by every tool page.
//!
//! GPUI ships no ready-made text field, so this follows the upstream input
//! example: an `EntityInputHandler` for IME/clipboard support plus a custom
//! element that shapes and paints the text.

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, InspectorElementId, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, ShapedLine, SharedString, Style, TextRun, UTF16Selection,
    Window, div, fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

gpui::actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
    ]
);

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Vec<ShapedLine>,
    last_line_starts: Vec<usize>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    multiline: bool,
    scroll_line: usize,
    font_size: f32,
    line_height: f32,
}

impl TextInput {
    pub fn single(cx: &mut Context<Self>, placeholder: impl Into<SharedString>) -> Self {
        Self::new(cx, placeholder, false, "")
    }

    pub fn multi(cx: &mut Context<Self>, placeholder: impl Into<SharedString>) -> Self {
        Self::new(cx, placeholder, true, "")
    }

    pub fn single_with(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self::new(cx, placeholder, false, value)
    }

    pub fn multi_with(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self::new(cx, placeholder, true, value)
    }

    fn new(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        multiline: bool,
        value: impl Into<SharedString>,
    ) -> Self {
        let content: SharedString = value.into();
        let len = content.len();
        Self {
            focus_handle: cx.focus_handle(),
            content,
            placeholder: placeholder.into(),
            selected_range: len..len,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_line_starts: vec![0],
            last_bounds: None,
            is_selecting: false,
            multiline,
            scroll_line: 0,
            font_size: 13.0,
            line_height: 20.0,
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_text(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = value.into();
        let len = self.content.len();
        self.selected_range = len..len;
        self.selection_reversed = false;
        self.marked_range = None;
        self.scroll_line = 0;
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text("", cx);
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn line_starts(&self) -> Vec<usize> {
        let mut starts = vec![0];
        for (index, byte) in self.content.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(index + 1);
            }
        }
        starts
    }

    fn line_bounds(&self) -> Vec<Range<usize>> {
        let starts = self.line_starts();
        let mut bounds = Vec::with_capacity(starts.len());
        for (index, start) in starts.iter().enumerate() {
            let end = starts
                .get(index + 1)
                .map(|next| next - 1)
                .unwrap_or(self.content.len());
            bounds.push(*start..end);
        }
        bounds
    }

    fn row_col(&self, offset: usize) -> (usize, usize) {
        let starts = self.line_starts();
        let mut row = 0;
        for (index, start) in starts.iter().enumerate() {
            if *start <= offset {
                row = index;
            } else {
                break;
            }
        }
        (row, offset.saturating_sub(starts[row]))
    }

    fn offset_for(&self, row: usize, col: usize) -> usize {
        let bounds = self.line_bounds();
        let Some(line) = bounds.get(row.min(bounds.len().saturating_sub(1))) else {
            return self.content.len();
        };
        line.start + col.min(line.end - line.start)
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let (row, col) = self.row_col(self.cursor_offset());
        if row == 0 {
            self.move_to(0, cx);
        } else {
            let target = self.offset_for(row - 1, col);
            self.move_to(target, cx);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let (row, col) = self.row_col(self.cursor_offset());
        let last = self.line_starts().len().saturating_sub(1);
        if row >= last {
            self.move_to(self.content.len(), cx);
        } else {
            let target = self.offset_for(row + 1, col);
            self.move_to(target, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let (row, _) = self.row_col(self.cursor_offset());
        let target = self.offset_for(row, 0);
        self.move_to(target, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let (row, _) = self.row_col(self.cursor_offset());
        let target = self.offset_for(row, usize::MAX);
        self.move_to(target, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        window.focus(&self.focus_handle);
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
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

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let lines = match event.delta {
            ScrollDelta::Lines(delta) => -delta.y,
            ScrollDelta::Pixels(delta) => -f32::from(delta.y) / self.line_height,
        };
        let total = self.line_starts().len();
        let max = total.saturating_sub(self.visible_lines());
        let next = (self.scroll_line as f32 + lines).round().max(0.0) as usize;
        let next = next.min(max);
        if next != self.scroll_line {
            self.scroll_line = next;
            cx.notify();
        }
    }

    fn visible_lines(&self) -> usize {
        let Some(bounds) = self.last_bounds else {
            return 12;
        };
        ((f32::from(bounds.size.height) - self.pad_y() * 2.0) / self.line_height)
            .floor()
            .max(1.0) as usize
    }

    fn pad_y(&self) -> f32 {
        if self.multiline { 8.0 } else { 0.0 }
    }

    /// Vertical offset used to paint (and hit-test) text. Single-line fields
    /// centre their line in the available height so the text lines up with
    /// sibling labels such as the `/` delimiters in the regex editor.
    fn pad_y_for(&self, height: f32) -> f32 {
        if self.multiline {
            self.pad_y()
        } else {
            ((height - self.line_height) / 2.0).max(0.0)
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = if self.multiline {
                text
            } else {
                text.replace(['\n', '\r'], " ")
            };
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), true) = (self.last_bounds.as_ref(), !self.last_layout.is_empty()) else {
            return self.content.len();
        };
        let local_y = f32::from(position.y - bounds.top()) - self.pad_y();
        let row = (local_y / self.line_height).floor().max(0.0) as usize + self.scroll_line;
        let row = row.min(self.last_layout.len().saturating_sub(1));
        let local_x = f32::from(position.x - bounds.left()) - 10.0;
        let col = self.last_layout[row].closest_index_for_x(px(local_x.max(0.0)));
        self.offset_for(row, col)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
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
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
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
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let new_text = if self.multiline {
            new_text.to_string()
        } else {
            new_text.replace(['\n', '\r'], " ")
        };

        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let (row, col) = self.row_col(range.start);
        let line = self.last_layout.get(row)?;
        let x = line.x_for_index(col);
        let pad_y = self.pad_y_for(f32::from(bounds.size.height));
        Some(Bounds::from_corners(
            point(bounds.left() + px(10.0) + x, bounds.top() + px(pad_y)),
            point(
                bounds.left() + px(10.0) + x + px(2.0),
                bounds.top() + px(pad_y) + px(self.line_height),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let local = bounds.localize(&point)?;
        let pad_y = self.pad_y_for(f32::from(bounds.size.height));
        let row = ((f32::from(local.y) - pad_y) / self.line_height)
            .floor()
            .max(0.0) as usize
            + self.scroll_line;
        let line = self.last_layout.get(row)?;
        let utf8_index = line.index_for_x(local.x - px(10.0))?;
        Some(self.offset_to_utf16(self.offset_for(row, utf8_index)))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Vec<ShapedLine>,
    line_starts: Vec<usize>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    scroll: usize,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor_offset = input.cursor_offset();
        let style = window.text_style();
        let line_height = input.line_height;
        let pad_y = input.pad_y_for(f32::from(bounds.size.height));

        let line_bounds = input.line_bounds();
        let line_starts = input.line_starts();
        let total = line_bounds.len();
        let visible = ((f32::from(bounds.size.height) - input.pad_y() * 2.0) / line_height)
            .floor()
            .max(1.0) as usize;

        let (cursor_row, _) = input.row_col(cursor_offset);
        let mut scroll = input.scroll_line.min(total.saturating_sub(visible));
        if cursor_row < scroll {
            scroll = cursor_row;
        } else if cursor_row >= scroll + visible {
            scroll = cursor_row + 1 - visible;
        }

        let text_color: gpui::Hsla = if input.content.is_empty() {
            gpui::rgba(0x9a9ca655).into()
        } else {
            style.color
        };
        let placeholder = input.placeholder.clone();
        let mut lines = Vec::new();
        for index in scroll..(scroll + visible).min(total) {
            let line = if content.is_empty() {
                placeholder.clone()
            } else {
                let range = line_bounds[index].clone();
                content[range].to_string().into()
            };
            let run = TextRun {
                len: line.len(),
                font: style.font(),
                color: text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let font_size = px(input.font_size);
            lines.push(
                window
                    .text_system()
                    .shape_line(line, font_size, &[run], None),
            );
        }

        let mut selection = Vec::new();
        if !selected_range.is_empty() {
            let (start_row, start_col) = input.row_col(selected_range.start);
            let (end_row, end_col) = input.row_col(selected_range.end);
            for row in start_row..=end_row {
                let Some(line) = lines.get(row.wrapping_sub(scroll)) else {
                    continue;
                };
                let line_len = line_bounds
                    .get(row)
                    .map(|range| range.end - range.start)
                    .unwrap_or(0);
                let from = if row == start_row { start_col } else { 0 };
                let to = if row == end_row { end_col } else { line_len };
                if from == to {
                    continue;
                }
                let y = bounds.top()
                    + px(pad_y + (row - scroll) as f32 * line_height);
                selection.push(fill(
                    Bounds::from_corners(
                        point(bounds.left() + px(10.0) + line.x_for_index(from), y),
                        point(bounds.left() + px(10.0) + line.x_for_index(to), y + px(line_height)),
                    ),
                    gpui::rgba(0x6d5bd040),
                ));
            }
        }

        let cursor = if selected_range.is_empty() {
            let row = cursor_row;
            lines.get(row.wrapping_sub(scroll)).map(|line| {
                let (_, col) = input.row_col(cursor_offset);
                let x = line.x_for_index(col);
                let y = bounds.top() + px(pad_y + (row - scroll) as f32 * line_height);
                fill(
                    Bounds::new(
                        point(bounds.left() + px(10.0) + x, y),
                        size(px(1.5), px(line_height)),
                    ),
                    style.color,
                )
            })
        } else {
            None
        };

        PrepaintState {
            lines,
            line_starts,
            cursor,
            selection,
            scroll,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let focused = focus_handle.is_focused(window);
        let line_height = self.input.read(cx).line_height;
        let pad_y = self.input.read(cx).pad_y_for(f32::from(bounds.size.height));
        for (index, line) in prepaint.lines.iter().enumerate() {
            let origin = point(
                bounds.left() + px(10.0),
                bounds.top() + px(pad_y + index as f32 * line_height),
            );
            let _ = line.paint(origin, px(line_height), window, cx);
        }
        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }
        if focused && let Some(cursor) = prepaint.cursor.take() {
            window.paint_quad(cursor);
        }

        let lines = std::mem::take(&mut prepaint.lines);
        let line_starts = prepaint.line_starts.clone();
        let scroll = prepaint.scroll;
        self.input.update(cx, |input, _cx| {
            input.last_layout = lines;
            input.last_line_starts = line_starts;
            input.last_bounds = Some(bounds);
            input.scroll_line = scroll;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input_id = gpui::ElementId::from(&self.focus_handle);
        div()
            .size_full()
            .key_context("TextInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .id(input_id)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn single_line_text_is_vertically_centered(cx: &mut TestAppContext) {
        let input = cx.new(|cx| TextInput::single(cx, ""));
        assert_eq!(cx.read_entity(&input, |input, _| input.pad_y_for(36.0)), 8.0);
        assert_eq!(cx.read_entity(&input, |input, _| input.pad_y_for(34.0)), 7.0);
        assert_eq!(cx.read_entity(&input, |input, _| input.pad_y_for(16.0)), 0.0);

        let multi = cx.new(|cx| TextInput::multi(cx, ""));
        assert_eq!(cx.read_entity(&multi, |input, _| input.pad_y_for(36.0)), 8.0);
    }
}
