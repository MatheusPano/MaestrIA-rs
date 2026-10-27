use gpui::{ClipboardItem, Context, KeyDownEvent, Window};

use super::FieldView;
use crate::rfw_view::layout::owned;

impl FieldView {
    fn list_key(&mut self, key: &str, shift: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let matches = self.matches();
        if matches.is_empty() {
            return false;
        }
        match key {
            "up" | "down" => {
                self.step_pick(matches.len(), key == "up");
                cx.notify();
            }
            "tab" => {
                let chosen = matches[self.picked(matches.len())].clone();
                self.complete(&chosen, cx);
            }
            "enter" if !shift => {
                let chosen = matches[self.picked(matches.len())].clone();
                let text = format!("/{}", chosen.name);
                let end = text.len();
                self.buffer.reset(text);
                self.buffer.set_cursor(end, false);
                self.edited(cx);
                self.submit(window, cx);
            }
            "escape" => self.dismiss(cx),
            _ => return false,
        }
        true
    }

    fn arrow_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let t = self.input();
        let Some(extra) = t.arrow(key, self.buffer.text()) else { return false };
        let events = owned(&t.on_arrow);
        self.fire_with(events, &extra, window, cx);
        true
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let t = self.input();
        let text = cx.read_from_clipboard().and_then(|c| c.text()).filter(|t| !t.is_empty());
        let wants_image = !t.on_paste_image.is_empty();
        match text {
            Some(text) => {
                let text = text.replace("\r\n", "\n");
                let text = if self.spec.multiline { text } else { text.replace('\n', " ") };
                self.buffer.insert(&text);
                self.edited(cx);
            }
            None if wants_image => {
                let events = owned(&t.on_paste_image);
                let extra = t.image_extra();
                self.fire_with(events, &extra, window, cx);
            }
            None => {}
        }
    }

    fn move_to(&mut self, to: usize, extend: bool, cx: &mut Context<Self>) {
        self.buffer.set_cursor(to, extend);
        self.moved(cx);
    }

    fn vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let head = self.buffer.head();
        let target = match (&self.layout, self.spec.multiline) {
            (Some(l), true) => {
                let p = l.position(head);
                let y = if down { p.y + l.line_height * 1.5 } else { p.y - l.line_height * 0.5 };
                if y < gpui::px(0.) { 0 } else { l.offset_at(gpui::point(p.x, y)) }
            }
            _ if down => self.buffer.text().len(),
            _ => 0,
        };
        self.move_to(target, extend, cx);
    }

    pub(super) fn key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        let cmd = m.control || m.platform;
        let (shift, word) = (m.shift, m.control || m.alt);
        if self.spec.disabled {
            return;
        }
        let key = k.key.as_str();
        if !cmd && (self.list_key(key, shift, window, cx) || self.arrow_key(key, window, cx)) {
            cx.stop_propagation();
            return;
        }
        let b = &self.buffer;
        let (len, head) = (b.text().len(), b.head());
        match key {
            "enter" if shift && self.spec.multiline && !cmd => {
                self.buffer.insert("\n");
                self.edited(cx);
            }
            "enter" if !cmd => self.submit(window, cx),
            "left" | "right" => {
                let right = key == "right";
                let target = if b.has_selection() && !shift && !word {
                    if right { b.selection().end } else { b.selection().start }
                } else {
                    match (right, word) {
                        (true, true) => b.word_after(head),
                        (true, false) => b.next(head),
                        (false, true) => b.word_before(head),
                        (false, false) => b.prev(head),
                    }
                };
                self.move_to(target, shift, cx);
            }
            "up" | "down" => self.vertical(key == "down", shift, cx),
            "home" if cmd => self.move_to(0, shift, cx),
            "end" if cmd => self.move_to(len, shift, cx),
            "home" => self.move_to(b.line_start(head), shift, cx),
            "end" => self.move_to(b.line_end(head), shift, cx),
            "backspace" => {
                self.buffer.backspace(word);
                self.edited(cx);
            }
            "delete" => {
                self.buffer.delete(word);
                self.edited(cx);
            }
            "a" if cmd => {
                self.buffer.select_all();
                self.moved(cx);
            }
            "c" if cmd => {
                if b.has_selection() {
                    cx.write_to_clipboard(ClipboardItem::new_string(b.selected_text().to_owned()));
                }
            }
            "x" if cmd => {
                if let Some(text) = self.buffer.cut() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.edited(cx);
                }
            }
            "v" if cmd => self.paste(window, cx),
            "z" if cmd && shift => {
                if self.buffer.redo() {
                    self.edited(cx);
                }
            }
            "z" if cmd => {
                if self.buffer.undo() {
                    self.edited(cx);
                }
            }
            "y" if cmd => {
                if self.buffer.redo() {
                    self.edited(cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}
