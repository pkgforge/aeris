use std::ops::Range;

use gpui::*;
use unicode_segmentation::*;

actions!(
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
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        InsertNewline,
    ]
);

pub fn bind_text_input_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("up", Up, Some("TextInput")),
        KeyBinding::new("down", Down, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
        KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
        KeyBinding::new("cmd-c", Copy, Some("TextInput")),
        KeyBinding::new("ctrl-c", Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", Cut, Some("TextInput")),
        KeyBinding::new("ctrl-x", Cut, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("TextInput")),
        KeyBinding::new("enter", InsertNewline, Some("MultilineInput")),
    ]);
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_lines: Vec<ShapedLine>,
    last_line_starts: Vec<usize>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    scroll_offset: Pixels,
    multiline: bool,
    min_lines: usize,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_lines: Vec::new(),
            last_line_starts: Vec::new(),
            last_bounds: None,
            is_selecting: false,
            scroll_offset: px(0.0),
            multiline: false,
            min_lines: 1,
        }
    }

    /// Enable multiline editing. `min_lines` controls the minimum visible height.
    pub fn multiline(mut self, min_lines: usize) -> Self {
        self.multiline = true;
        self.min_lines = min_lines.max(1);
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn set_content(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        let len = self.content.len();
        self.selected_range = len..len;
        self.marked_range = None;
        cx.notify();
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
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let cursor = self.cursor_offset();
        let Some((line_idx, x_in_line)) = self.line_position_for_offset(cursor) else {
            return;
        };
        if line_idx == 0 {
            self.move_to(0, cx);
            return;
        }
        let target_line = line_idx - 1;
        let Some(line) = self.last_lines.get(target_line) else {
            return;
        };
        let new_in_line = line.closest_index_for_x(x_in_line);
        let new_cursor = self.last_line_starts[target_line] + new_in_line;
        self.move_to(new_cursor, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let cursor = self.cursor_offset();
        let Some((line_idx, x_in_line)) = self.line_position_for_offset(cursor) else {
            return;
        };
        if line_idx + 1 >= self.last_lines.len() {
            self.move_to(self.content.len(), cx);
            return;
        }
        let target_line = line_idx + 1;
        let Some(line) = self.last_lines.get(target_line) else {
            return;
        };
        let new_in_line = line.closest_index_for_x(x_in_line);
        let new_cursor = self.last_line_starts[target_line] + new_in_line;
        self.move_to(new_cursor, cx);
    }

    fn insert_newline(&mut self, _: &InsertNewline, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    /// Return the (line_index, x_within_line) for a byte offset in content.
    fn line_position_for_offset(&self, offset: usize) -> Option<(usize, Pixels)> {
        if self.last_lines.is_empty() {
            return None;
        }
        for (i, start) in self.last_line_starts.iter().enumerate() {
            let line = &self.last_lines[i];
            let end = start + line.len;
            if offset <= end {
                let local = offset - start;
                return Some((i, line.x_for_index(local)));
            }
        }
        let last = self.last_lines.len() - 1;
        let local = self.last_lines[last].len;
        Some((last, self.last_lines[last].x_for_index(local)))
    }

    /// Return the start byte offset of the line containing `offset`.
    fn current_line_start(&self, offset: usize) -> usize {
        self.content[..offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    /// Return the end byte offset of the line containing `offset` (before the \n).
    fn current_line_end(&self, offset: usize) -> usize {
        self.content[offset..]
            .find('\n')
            .map(|i| offset + i)
            .unwrap_or(self.content.len())
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
        if self.multiline {
            let target = self.current_line_start(self.cursor_offset());
            self.move_to(target, cx);
        } else {
            self.move_to(0, cx);
        }
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            let target = self.current_line_end(self.cursor_offset());
            self.move_to(target, cx);
        } else {
            self.move_to(self.content.len(), cx);
        }
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
        let lh = window.line_height();
        let idx = self.index_for_mouse_position(event.position, lh);
        if event.modifiers.shift {
            self.select_to(idx, cx);
        } else {
            self.move_to(idx, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let lh = window.line_height();
            let idx = self.index_for_mouse_position(event.position, lh);
            self.select_to(idx, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            if self.multiline {
                self.replace_text_in_range(None, &text, window, cx);
            } else {
                self.replace_text_in_range(None, &text.replace("\n", " "), window, cx);
            }
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
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>, line_height: Pixels) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let Some(bounds) = self.last_bounds.as_ref() else {
            return 0;
        };
        if self.last_lines.is_empty() {
            return 0;
        }
        let relative_y = (position.y - bounds.top()).max(px(0.0));
        let mut idx = (relative_y / line_height) as usize;
        if idx >= self.last_lines.len() {
            idx = self.last_lines.len() - 1;
        }
        let line = &self.last_lines[idx];
        let local = line.closest_index_for_x(position.x - bounds.left() + self.scroll_offset);
        self.last_line_starts[idx] + local
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let owned: String;
        let inserted: &str = if !self.multiline && new_text.contains('\n') {
            owned = new_text.replace('\n', " ");
            owned.as_str()
        } else {
            new_text
        };

        self.content =
            (self.content[0..range.start].to_owned() + inserted + &self.content[range.end..])
                .into();
        self.selected_range = range.start + inserted.len()..range.start + inserted.len();
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
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if self.last_lines.is_empty() {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        let lh = window.line_height();
        let (start_line, start_local) = self.line_and_local(range.start)?;
        let (end_line, end_local) = self.line_and_local(range.end)?;
        let start_x = self.last_lines[start_line].x_for_index(start_local);
        let end_x = self.last_lines[end_line].x_for_index(end_local);
        Some(Bounds::from_corners(
            point(
                bounds.left() + start_x - self.scroll_offset,
                bounds.top() + lh * start_line as f32,
            ),
            point(
                bounds.left() + end_x - self.scroll_offset,
                bounds.top() + lh * (end_line + 1) as f32,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let lh = window.line_height();
        let offset = self.index_for_mouse_position(point, lh);
        Some(self.offset_to_utf16(offset))
    }
}

impl TextInput {
    fn line_and_local(&self, offset: usize) -> Option<(usize, usize)> {
        for (i, start) in self.last_line_starts.iter().enumerate() {
            let line = &self.last_lines[i];
            let end = start + line.len;
            if offset <= end {
                return Some((i, offset - start));
            }
        }
        let last = self.last_lines.len() - 1;
        Some((last, self.last_lines[last].len))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut wrapper = div()
            .flex()
            .key_context(if self.multiline {
                "TextInput MultilineInput"
            } else {
                "TextInput"
            })
            .track_focus(&self.focus_handle(cx))
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
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full();
        if self.multiline {
            wrapper = wrapper.on_action(cx.listener(Self::insert_newline));
        }
        wrapper.child(div().w_full().child(TextElement { input: cx.entity() }))
    }
}

pub struct TextElement {
    input: Entity<TextInput>,
}

pub struct PrepaintState {
    lines: Vec<ShapedLine>,
    line_starts: Vec<usize>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
    scroll_offset: Pixels,
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
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let lines = if input.multiline {
            input
                .content
                .split('\n')
                .count()
                .max(input.min_lines)
        } else {
            1
        };
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * lines as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let placeholder = input.placeholder.clone();
        let selected_range = input.selected_range.clone();
        let marked_range = input.marked_range.clone();
        let cursor = input.cursor_offset();
        let prev_offset = input.scroll_offset;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();

        let is_empty = content.is_empty();
        let display_text: SharedString = if is_empty { placeholder } else { content };
        let text_color = if is_empty {
            hsla(0., 0., 0.5, 0.5)
        } else {
            style.color
        };

        // Shape one ShapedLine per source line (split by \n). Marked range
        // gets an underline only when it falls entirely within a single line.
        let mut lines: Vec<ShapedLine> = Vec::new();
        let mut line_starts: Vec<usize> = Vec::new();
        let mut start = 0usize;
        for line_text in display_text.split('\n') {
            let line_end = start + line_text.len();
            let mut runs: Vec<TextRun> = Vec::new();
            let base = TextRun {
                len: line_text.len(),
                font: style.font(),
                color: text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            if let Some(ref mr) = marked_range {
                if mr.start >= start && mr.end <= line_end && !line_text.is_empty() {
                    let local_start = mr.start - start;
                    let local_end = mr.end - start;
                    if local_start > 0 {
                        runs.push(TextRun {
                            len: local_start,
                            ..base.clone()
                        });
                    }
                    if local_end > local_start {
                        runs.push(TextRun {
                            len: local_end - local_start,
                            underline: Some(UnderlineStyle {
                                color: Some(text_color),
                                thickness: px(1.0),
                                wavy: false,
                            }),
                            ..base.clone()
                        });
                    }
                    if local_end < line_text.len() {
                        runs.push(TextRun {
                            len: line_text.len() - local_end,
                            ..base.clone()
                        });
                    }
                } else if !line_text.is_empty() {
                    runs.push(base);
                }
            } else if !line_text.is_empty() {
                runs.push(base);
            }
            let shaped = window.text_system().shape_line(
                SharedString::from(line_text.to_string()),
                font_size,
                &runs,
                None,
            );
            lines.push(shaped);
            line_starts.push(start);
            start = line_end + 1;
        }

        // Locate the cursor within the shaped lines.
        let (cursor_line, cursor_local) = {
            let mut found = (lines.len() - 1, lines.last().map(|l| l.len).unwrap_or(0));
            for (i, ls) in line_starts.iter().enumerate() {
                let line_end = ls + lines[i].len;
                if cursor <= line_end {
                    found = (i, cursor - ls);
                    break;
                }
            }
            found
        };
        let cursor_x = lines[cursor_line].x_for_index(cursor_local);

        // Horizontal scroll keeps the cursor in view on its line.
        let viewport = bounds.right() - bounds.left();
        let current_line_width = lines[cursor_line].width;
        let padding = px(2.0);
        let scroll_offset = if is_empty || current_line_width <= viewport {
            px(0.0)
        } else {
            let mut so = prev_offset;
            if cursor_x - so < padding {
                so = (cursor_x - padding).max(px(0.0));
            }
            if cursor_x - so > viewport - padding {
                so = cursor_x - (viewport - padding);
            }
            let max_offset = (current_line_width - viewport).max(px(0.0));
            so.clamp(px(0.0), max_offset)
        };
        self.input.update(cx, |input, _| {
            input.scroll_offset = scroll_offset;
        });

        let cursor_y = bounds.top() + line_height * cursor_line as f32;
        let caret = if is_empty || selected_range.is_empty() {
            Some(fill(
                Bounds::new(
                    point(bounds.left() + cursor_x - scroll_offset, cursor_y),
                    size(px(2.), line_height),
                ),
                hsla(0.6, 0.8, 0.5, 1.0),
            ))
        } else {
            None
        };

        let mut selections: Vec<PaintQuad> = Vec::new();
        if !is_empty && !selected_range.is_empty() {
            // Find which lines the selection spans and emit a rect per line.
            let mut sel_start_line = 0usize;
            let mut sel_start_local = 0usize;
            let mut sel_end_line = 0usize;
            let mut sel_end_local = 0usize;
            for (i, ls) in line_starts.iter().enumerate() {
                let line_end = ls + lines[i].len;
                if selected_range.start <= line_end && sel_start_line == 0 && i == 0 {
                    // initialize defaults below
                }
                if selected_range.start >= *ls && selected_range.start <= line_end {
                    sel_start_line = i;
                    sel_start_local = selected_range.start - ls;
                }
                if selected_range.end >= *ls && selected_range.end <= line_end {
                    sel_end_line = i;
                    sel_end_local = selected_range.end - ls;
                }
            }
            for li in sel_start_line..=sel_end_line {
                let line = &lines[li];
                let left_local = if li == sel_start_line { sel_start_local } else { 0 };
                let right_local = if li == sel_end_line { sel_end_local } else { line.len };
                let left_x = line.x_for_index(left_local);
                let right_x = if li == sel_end_line {
                    line.x_for_index(right_local)
                } else {
                    line.width.max(left_x + px(4.0))
                };
                let y_top = bounds.top() + line_height * li as f32;
                selections.push(fill(
                    Bounds::from_corners(
                        point(bounds.left() + left_x - scroll_offset, y_top),
                        point(
                            bounds.left() + right_x - scroll_offset,
                            y_top + line_height,
                        ),
                    ),
                    hsla(0.6, 0.8, 0.5, 0.2),
                ));
            }
        }

        PrepaintState {
            lines,
            line_starts,
            cursor: caret,
            selections,
            scroll_offset,
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
        cx: &mut gpui::App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }
        let line_height = window.line_height();
        let scroll_offset = prepaint.scroll_offset;
        for (i, line) in prepaint.lines.iter().enumerate() {
            let origin = gpui::point(
                bounds.origin.x - scroll_offset,
                bounds.origin.y + line_height * i as f32,
            );
            line.paint(origin, line_height, window, cx).unwrap();
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        let lines = std::mem::take(&mut prepaint.lines);
        let line_starts = std::mem::take(&mut prepaint.line_starts);
        self.input.update(cx, |input, _cx| {
            input.last_lines = lines;
            input.last_line_starts = line_starts;
            input.last_bounds = Some(bounds);
        });
    }
}
