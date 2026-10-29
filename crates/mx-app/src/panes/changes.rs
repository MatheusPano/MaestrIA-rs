use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::time::UNIX_EPOCH;

use gpui::{
    AnyElement, App, Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ListAlignment, ListState,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div,
    prelude::FluentBuilder, px, relative,
};
use mx_store::{DocPlace, Store};
use mx_sys::diff::{DiffKind, DiffLine, FileDiff};

use crate::app::host::{StoreHost, update_store};
use crate::app::theme::{Mx, RADIUS};
use crate::app::tooltip::tip;
use crate::chrome::icon::{self as glyphs, Icon, icon};
use crate::chrome::style::MONO;
use crate::panes::PaneArea;
use crate::panes::header::files_label;

pub const SHARE: f32 = 0.4;
pub const MIN_WIDTH: f32 = 300.;
pub const MAX_WIDTH: f32 = 600.;
const ROW: f32 = 28.;
const SHOWN_ROWS: f32 = 6.;

#[derive(Clone, PartialEq, Default)]
pub struct Shown {
    pub files: Vec<String>,
    pub cwd: String,
    stamps: Vec<u128>,
}

impl Shown {
    pub fn of(store: &Store, tab: &str) -> Shown {
        let Some(t) = store.tab(tab) else { return Shown::default() };
        let files: Vec<String> = t.hooks.touched.iter().rev().cloned().collect();
        let stamps = files.iter().map(|f| stamp(f)).collect();
        Shown { files, cwd: t.cwd.clone(), stamps }
    }
}

#[derive(Clone)]
enum Slot {
    Pending,
    Done(Option<Rc<FileDiff>>),
}

pub struct ChangesView {
    host: Entity<StoreHost>,
    area: WeakEntity<PaneArea>,
    tab: String,
    shown: Shown,
    picked: Option<String>,
    diffs: HashMap<(String, u128), Slot>,
    list: ListState,
    listed: Option<(String, u128)>,
    lines: Rc<Vec<DiffLine>>,
}

impl ChangesView {
    pub fn new(host: Entity<StoreHost>, area: WeakEntity<PaneArea>, tab: &str) -> ChangesView {
        ChangesView {
            host,
            area,
            tab: tab.to_owned(),
            shown: Shown::default(),
            picked: None,
            diffs: HashMap::new(),
            list: ListState::new(0, ListAlignment::Top, px(400.)),
            listed: None,
            lines: Rc::new(Vec::new()),
        }
    }

    pub fn set(&mut self, shown: Shown) -> bool {
        if self.shown == shown {
            return false;
        }
        self.diffs.retain(|(path, _), _| shown.files.contains(path));
        self.shown = shown;
        true
    }

    fn slot(&mut self, path: &str, cx: &mut Context<Self>) -> Slot {
        let key = (path.to_owned(), stamp(path));
        if let Some(slot) = self.diffs.get(&key) {
            return slot.clone();
        }
        self.diffs.retain(|(p, _), _| p != path);
        self.diffs.insert(key.clone(), Slot::Pending);
        let rt = self.host.read(cx).rt();
        let file = path.to_owned();
        let task = rt.spawn(async move { mx_sys::diff::diff_of(&mx_sys::LoginShell, &file).await });
        cx.spawn(async move |this, cx| {
            let diff = task.await.ok().flatten();
            this.update(cx, |v, cx| {
                if let Some(slot) = v.diffs.get_mut(&key) {
                    *slot = Slot::Done(diff.map(Rc::new));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        Slot::Pending
    }

    fn show(&mut self, path: &str, diff: &FileDiff) {
        let key = (path.to_owned(), stamp(path));
        if self.listed.as_ref() == Some(&key) {
            return;
        }
        let same_file = self.listed.as_ref().is_some_and(|(p, _)| p == path);
        let lines: Vec<DiffLine> = diff.shown().cloned().collect();
        if same_file {
            self.list.splice(0..self.list.item_count(), lines.len());
        } else {
            self.list.reset(lines.len());
        }
        self.lines = Rc::new(lines);
        self.listed = Some(key);
    }

    fn header(&self, mx: &Mx) -> impl IntoElement {
        let (area, tab) = (self.area.clone(), self.tab.clone());
        let (host, clear_area, clear_tab) = (self.host.clone(), self.area.clone(), self.tab.clone());
        div()
            .flex_none()
            .flex()
            .items_center()
            .pl(px(14.))
            .pr(px(6.))
            .pt(px(10.))
            .pb(px(6.))
            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(mx.fg).child("Mudanças"))
            .child(div().w(px(8.)))
            .child(div().text_size(px(11.)).text_color(mx.fg_muted).child(files_label(self.shown.files.len())))
            .child(div().flex_1())
            .child(
                div()
                    .id("changes-clear")
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|s| s.bg(mx.bg_hover))
                    .text_size(px(11.))
                    .text_color(mx.fg_dim)
                    .tooltip(tip("Esquecer esta lista — os arquivos ficam onde estão"))
                    .on_click(move |_, _, cx: &mut App| {
                        update_store(&host, cx, |s| s.clear_touched(&clear_tab));
                        clear_area.update(cx, |a, cx| a.close_changes(&clear_tab, cx)).ok();
                    })
                    .child("Limpar"),
            )
            .child(
                div()
                    .id("changes-close")
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(mx.bg_hover))
                    .tooltip(tip("Fechar"))
                    .on_click(move |_, _, cx: &mut App| {
                        area.update(cx, |a, cx| a.close_changes(&tab, cx)).ok();
                    })
                    .child(icon(glyphs::CLOSE, 15., mx.fg_dim)),
            )
    }

    fn row(&self, mx: &Mx, ix: usize, path: &str, selected: bool, slot: &Slot, cx: &mut Context<Self>) -> AnyElement {
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        let place = produced_where(path, &self.shown.cwd);
        let gone = !Path::new(path).exists();
        let stat = match slot {
            Slot::Done(Some(d)) if !d.is_empty() => Some(stat(mx, d.added(), d.removed())),
            _ => None,
        };
        let picked = path.to_owned();
        let (open_host, open_tab, open_path) = (self.host.clone(), self.tab.clone(), path.to_owned());
        let (reveal_host, reveal_path) = (self.host.clone(), path.to_owned());
        let actions = div()
            .absolute()
            .top_0()
            .right_0()
            .h_full()
            .flex()
            .items_center()
            .invisible()
            .group_hover("changes-row", |s| s.visible())
            .child(row_icon(mx, ("changes-open", ix), glyphs::OPEN_IN_NEW, "Abrir", move |cx| {
                open_produced(&open_host, &open_tab, &open_path, cx)
            }))
            .child(row_icon(mx, ("changes-reveal", ix), glyphs::FOLDER_OPEN, "Mostrar no Finder", move |cx| {
                update_store(&reveal_host, cx, |s| s.reveal_file(&reveal_path))
            }));
        div()
            .id(("changes-file", ix))
            .group("changes-row")
            .flex_none()
            .h(px(ROW))
            .mx(px(6.))
            .pl(px(8.))
            .pr(px(4.))
            .flex()
            .items_center()
            .rounded(px(6.))
            .cursor_pointer()
            .when(selected, |d| d.bg(mx.bg_selected))
            .when(!selected, |d| d.hover(|s| s.bg(mx.bg_hover)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.picked = Some(picked.clone());
                cx.notify();
            }))
            .child(icon(if gone { glyphs::HELP_OUTLINE } else { file_glyph(&name) }, 13., if gone { mx.fg_faint } else { mx.fg_dim }))
            .child(div().flex_none().w(px(8.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(if gone { mx.fg_faint } else { mx.fg })
                            .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
                            .when(gone, |d| d.line_through())
                            .child(name),
                    )
                    .when(!place.is_empty(), |d| {
                        d.child(
                            div()
                                .ml(px(8.))
                                .min_w_0()
                                .flex_shrink()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(mx.fg_muted)
                                .child(place),
                        )
                    }),
            )
            .child(div().flex_none().w(px(8.)))
            .child(
                div()
                    .relative()
                    .flex_none()
                    .h_full()
                    .min_w(px(44.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .children(stat.map(|s| if gone { s } else { s.group_hover("changes-row", |s| s.invisible()) }))
                    .when(!gone, |d| d.child(actions)),
            )
            .into_any_element()
    }

    fn body(&mut self, mx: &Mx, picked: &str, slot: Slot) -> AnyElement {
        let diff = match slot {
            Slot::Pending => return div().into_any_element(),
            Slot::Done(None) if Path::new(picked).exists() => {
                return message(mx, "Sem diff pra mostrar: o arquivo está fora de um repositório, é ignorado pelo git ou é binário.");
            }
            Slot::Done(None) => return message(mx, "Esse arquivo não está mais lá."),
            Slot::Done(Some(d)) if d.is_empty() => {
                return message(mx, "Nada diferente do último commit — a mudança já foi commitada ou desfeita.");
            }
            Slot::Done(Some(d)) => d,
        };
        self.show(picked, &diff);
        let (lines, mx) = (self.lines.clone(), mx.clone());
        div()
            .size_full()
            .py(px(6.))
            .child(gpui::list(self.list.clone(), move |ix, _, _| match lines.get(ix) {
                Some(line) => diff_row(&mx, line),
                None => div().into_any_element(),
            })
            .size_full())
            .into_any_element()
    }
}

impl Render for ChangesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx).clone();
        let files = self.shown.files.clone();
        let Some(first) = files.first() else {
            return div().into_any_element();
        };
        let picked = self.picked.clone().filter(|p| files.contains(p)).unwrap_or_else(|| first.clone());
        let mut rows = Vec::with_capacity(files.len());
        let mut chosen = Slot::Pending;
        for (ix, f) in files.iter().enumerate() {
            let slot = self.slot(f, cx);
            rows.push(self.row(&mx, ix, f, *f == picked, &slot, cx));
            if *f == picked {
                chosen = slot;
            }
        }
        let body = self.body(&mx, &picked, chosen);
        div()
            .size_full()
            .flex()
            .flex_col()
            .rounded_br(px(RADIUS))
            .bg(mx.bg_sidebar)
            .border_l_1()
            .border_color(mx.border)
            .child(self.header(&mx))
            .child(
                div()
                    .id("changes-files")
                    .flex_none()
                    .max_h(px(SHOWN_ROWS * ROW + 8.))
                    .overflow_y_scroll()
                    .pb(px(8.))
                    .flex()
                    .flex_col()
                    .children(rows),
            )
            .child(div().flex_none().h(px(1.)).bg(mx.border))
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

fn stamp(path: &str) -> u128 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

fn open_produced(host: &Entity<StoreHost>, tab: &str, path: &str, cx: &mut App) {
    update_store(host, cx, |s| {
        if Store::readable(path) {
            s.show_file(path, DocPlace::from_tab(tab));
        } else {
            s.open_in_editor(path);
        }
    })
}

pub fn produced_where(path: &str, cwd: &str) -> String {
    let dir = path.rfind('/').map_or("", |i| &path[..i]);
    if dir == cwd {
        return String::new();
    }
    match dir.strip_prefix(cwd).and_then(|rest| rest.strip_prefix('/')) {
        Some(rest) if !cwd.is_empty() => rest.to_owned(),
        _ => dir.to_owned(),
    }
}

fn file_glyph(name: &str) -> Icon {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "csv" | "xlsx" | "xls" | "numbers" => glyphs::BAR_CHART,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => glyphs::PALETTE_OUTLINED,
        "" | "md" | "txt" | "rtf" | "doc" | "docx" | "pages" | "pdf" => glyphs::DESCRIPTION_OUTLINED,
        _ => glyphs::CODE,
    }
}

fn row_icon(
    mx: &Mx,
    id: (&'static str, usize),
    glyph: Icon,
    tooltip: &'static str,
    act: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .cursor_pointer()
        .hover(|s| s.bg(mx.bg_hover))
        .tooltip(tip(tooltip))
        .on_click(move |_, _, cx: &mut App| {
            cx.stop_propagation();
            act(cx)
        })
        .child(icon(glyph, 13., mx.fg_dim))
}

fn stat(mx: &Mx, added: usize, removed: usize) -> gpui::Div {
    div()
        .flex()
        .gap(px(4.))
        .font_family(MONO)
        .text_size(px(10.5))
        .when(added > 0, |d| d.child(div().text_color(mx.green).child(format!("+{added}"))))
        .when(removed > 0, |d| d.child(div().text_color(mx.red).child(format!("−{removed}"))))
}

fn message(mx: &Mx, text: &'static str) -> AnyElement {
    div()
        .p(px(16.))
        .text_size(px(12.))
        .line_height(relative(1.5))
        .text_color(mx.fg_muted)
        .child(text)
        .into_any_element()
}

fn diff_row(mx: &Mx, line: &DiffLine) -> AnyElement {
    if line.kind == DiffKind::Hunk {
        let context = line.hunk_context();
        let text = if context.is_empty() { "···".to_owned() } else { format!("···  {context}") };
        return div()
            .pt(px(10.))
            .pb(px(4.))
            .px(px(12.))
            .truncate()
            .font_family(MONO)
            .text_size(px(10.5))
            .text_color(mx.fg_faint)
            .child(text)
            .into_any_element();
    }
    let (wash, mark): (Option<Hsla>, Hsla) = match line.kind {
        DiffKind::Add => (Some(mx.green.opacity(0.09)), mx.green),
        DiffKind::Del => (Some(mx.red.opacity(0.09)), mx.red),
        _ => (None, gpui::transparent_black()),
    };
    let number = if line.kind == DiffKind::Del { line.old_line } else { line.new_line };
    let text = if line.text.is_empty() { " ".to_owned() } else { line.text.replace('\t', "    ") };
    div()
        .flex()
        .items_start()
        .when_some(wash, |d, c| d.bg(c))
        .border_l(px(2.))
        .border_color(mark)
        .font_family(MONO)
        .line_height(px(11.5 * 1.45))
        .child(
            div()
                .flex_none()
                .w(px(40.))
                .flex()
                .justify_end()
                .text_size(px(10.5))
                .text_color(mx.fg_faint)
                .child(SharedString::from(number.map(|n| n.to_string()).unwrap_or_default())),
        )
        .child(div().flex_none().w(px(12.)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(11.5))
                .text_color(if line.kind == DiffKind::Context { mx.fg_dim } else { mx.fg })
                .child(text),
        )
        .child(div().flex_none().w(px(10.)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_says_where_it_lives_the_shortest_way_that_is_not_ambiguous() {
        assert_eq!(produced_where("/repo/a.rs", "/repo"), "");
        assert_eq!(produced_where("/repo/src/ui/a.rs", "/repo"), "src/ui");
        assert_eq!(produced_where("/repository/a.rs", "/repo"), "/repository");
        assert_eq!(produced_where("/tmp/plan.md", "/repo"), "/tmp");
    }

    #[test]
    fn the_glyph_hints_at_the_kind_of_file() {
        assert_eq!(file_glyph("plan.md"), glyphs::DESCRIPTION_OUTLINED);
        assert_eq!(file_glyph("Makefile"), glyphs::DESCRIPTION_OUTLINED);
        assert_eq!(file_glyph("data.CSV"), glyphs::BAR_CHART);
        assert_eq!(file_glyph("main.rs"), glyphs::CODE);
    }

    #[test]
    fn the_count_reads_as_portuguese() {
        assert_eq!(files_label(1), "1 arquivo");
        assert_eq!(files_label(3), "3 arquivos");
    }
}
