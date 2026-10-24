use gpui::{ClipboardItem, Context, KeyDownEvent, Window};

use super::{EditorEvent, TextEditor};

impl TextEditor {
    fn newline(&mut self, cx: &mut Context<Self>) {
        if !self.style.code {
            return self.insert("\n", cx);
        }
        let head = self.buffer.head();
        let start = self.buffer.line_start(head);
        let line = &self.buffer.text()[start..head];
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        self.insert(&format!("\n{indent}"), cx);
    }

    fn indent_unit(&self) -> &'static str {
        let text = self.buffer.text();
        let line = &text[self.buffer.line_start(self.buffer.head())..];
        if line.starts_with('\t') { "\t" } else { "  " }
    }

    pub(super) fn key_down(&mut self, ev: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        let cmd = m.control || m.platform;
        let shift = m.shift;
        let word = m.control || m.alt;
        let len = self.buffer.text().len();
        let head = self.buffer.head();
        let editable = !self.read_only;
        match k.key.as_str() {
            "left" => self.horizontal(false, shift, word, cx),
            "right" => self.horizontal(true, shift, word, cx),
            "up" => self.vertical(-1., shift, cx),
            "down" => self.vertical(1., shift, cx),
            "pageup" => self.vertical(-self.page_rows(), shift, cx),
            "pagedown" => self.vertical(self.page_rows(), shift, cx),
            "home" if cmd => self.jump(0, shift, cx),
            "end" if cmd => self.jump(len, shift, cx),
            "home" => self.jump(self.buffer.smart_home(head), shift, cx),
            "end" => self.jump(self.buffer.line_end(head), shift, cx),
            "backspace" if editable => {
                self.buffer.backspace(word);
                self.edited(cx);
            }
            "delete" if editable => {
                self.buffer.delete(word);
                self.edited(cx);
            }
            "enter" if editable && !cmd => self.newline(cx),
            "tab" if editable && !cmd && !shift => self.insert(self.indent_unit(), cx),
            "a" if cmd => {
                self.buffer.select_all();
                self.moved(cx);
            }
            "c" if cmd => self.copy(cx),
            "x" if cmd && editable => {
                if let Some(text) = self.buffer.cut() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.edited(cx);
                }
            }
            "v" if cmd && editable => self.paste(cx),
            "z" if cmd && shift && editable => {
                if self.buffer.redo() {
                    self.edited(cx);
                }
            }
            "z" if cmd && editable => {
                if self.buffer.undo() {
                    self.edited(cx);
                }
            }
            "y" if cmd && editable => {
                if self.buffer.redo() {
                    self.edited(cx);
                }
            }
            "s" if cmd => cx.emit(EditorEvent::Save),
            _ => return,
        }
        cx.stop_propagation();
    }
}
