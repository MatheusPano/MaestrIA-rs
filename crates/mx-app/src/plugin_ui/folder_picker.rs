use std::rc::Rc;

use gpui::{
    App, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, px, rgba,
};

use super::widgets::text_button;
use crate::chrome::icon::{self, icon};
use crate::chrome::style::{MONO, hsla, theme};
use crate::chrome::widgets::{TextField, TextFieldEvent};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub root: String,
    pub name: String,
}

pub type Done = Rc<dyn Fn(Option<Vec<String>>, &mut Window, &mut App)>;

pub fn summary(names: &[String]) -> String {
    match names.len() {
        0 => "Nenhuma".into(),
        1 | 2 => names.join(", "),
        n => format!("{} e mais {}", names[..2].join(", "), n - 2),
    }
}

pub fn shown_names(folders: &[Pick], picked: &[String]) -> Vec<String> {
    picked.iter().filter_map(|r| folders.iter().find(|f| &f.root == r).map(|f| f.name.clone())).collect()
}

pub fn button_text(folders: &[Pick], picked: &[String]) -> (String, bool) {
    if folders.is_empty() {
        return ("Nenhuma pasta na lateral".into(), true);
    }
    let names = shown_names(folders, picked);
    (summary(&names), names.is_empty())
}

pub fn filter<'a>(query: &str, folders: &'a [Pick]) -> Vec<&'a Pick> {
    let q = query.to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().collect();
    folders
        .iter()
        .filter(|f| {
            let hay = format!("{} {}", f.name, f.root).to_lowercase();
            terms.iter().all(|t| hay.contains(t))
        })
        .collect()
}

pub fn toggle_all(shown: &[String], picked: &[String]) -> Vec<String> {
    let all = !shown.is_empty() && shown.iter().all(|r| picked.contains(r));
    if all {
        picked.iter().filter(|r| !shown.contains(r)).cloned().collect()
    } else {
        let mut out = picked.to_vec();
        out.extend(shown.iter().filter(|r| !picked.contains(r)).cloned());
        out
    }
}

pub fn ordered_result(folders: &[Pick], picked: &[String]) -> Vec<String> {
    let mut out: Vec<String> = folders.iter().filter(|f| picked.contains(&f.root)).map(|f| f.root.clone()).collect();
    out.extend(picked.iter().filter(|r| !folders.iter().any(|f| &f.root == *r)).cloned());
    out
}

pub struct FolderPicker {
    title: String,
    folders: Vec<Pick>,
    picked: Vec<String>,
    query: Entity<TextField>,
    done: Done,
    _sub: Subscription,
}

impl FolderPicker {
    pub fn new(title: String, folders: Vec<Pick>, picked: Vec<String>, done: Done, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| TextField::new(cx).hint("Filtrar pastas").filled().selector("folders-filter"));
        let sub = cx.subscribe(&query, |_, _, ev: &TextFieldEvent, cx| {
            if matches!(ev, TextFieldEvent::Changed(_)) {
                cx.notify();
            }
        });
        query.read(cx).focus(window);
        FolderPicker { title, folders, picked, query, done, _sub: sub }
    }

    fn shown_roots(&self, cx: &App) -> Vec<String> {
        filter(self.query.read(cx).text(), &self.folders).into_iter().map(|f| f.root.clone()).collect()
    }

    fn toggle(&mut self, root: &str, cx: &mut Context<Self>) {
        match self.picked.iter().position(|r| r == root) {
            Some(i) => {
                self.picked.remove(i);
            }
            None => self.picked.push(root.to_owned()),
        }
        cx.notify();
    }

    fn finish(&self, result: Option<Vec<String>>, window: &mut Window, cx: &mut App) {
        (self.done)(result, window, cx)
    }

    fn row(&self, f: &Pick, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = theme(cx);
        let on = self.picked.contains(&f.root);
        let (glyph, color) = if on { (icon::CHECK_BOX, hsla(t.accent)) } else { (icon::CHECK_BOX_OUTLINE_BLANK, hsla(t.fg_dim)) };
        let root = f.root.clone();
        let name = f.name.clone();
        div()
            .id(SharedString::from(format!("folder-row:{}", f.root)))
            .debug_selector(move || format!("folder:{name}"))
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(hsla(t.bg_hover)))
            .on_click(cx.listener(move |p, _, _, cx| p.toggle(&root, cx)))
            .child(icon(glyph, 18., color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(12.5)).text_color(hsla(t.fg)).child(f.name.clone()))
                    .child(div().truncate().font_family(MONO).text_size(px(10.5)).text_color(hsla(t.fg_faint)).child(f.root.clone())),
            )
            .into_any_element()
    }
}

impl Render for FolderPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let query = self.query.read(cx).text().to_owned();
        let shown: Vec<Pick> = filter(&query, &self.folders).into_iter().cloned().collect();
        let roots: Vec<String> = shown.iter().map(|f| f.root.clone()).collect();
        let all = !roots.is_empty() && roots.iter().all(|r| self.picked.contains(r));
        let count = self.folders.iter().filter(|f| self.picked.contains(&f.root)).count();
        let me = cx.weak_entity();
        let toggle_label = if all { "Nenhuma" } else { "Todas" };
        let mut toggle = div()
            .id("folders-all")
            .debug_selector(move || format!("folders-all:{toggle_label}"))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .text_size(px(12.))
            .text_color(if roots.is_empty() { hsla(t.fg_faint) } else { hsla(t.accent) })
            .child(toggle_label);
        if !roots.is_empty() {
            let me = me.clone();
            toggle = toggle.cursor_pointer().hover(|s| s.bg(hsla(t.bg_hover))).on_click(move |_, _, cx| {
                me.update(cx, |p, cx| {
                    let shown = p.shown_roots(cx);
                    p.picked = toggle_all(&shown, &p.picked);
                    cx.notify();
                })
                .ok();
            });
        }
        let header = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .pt(px(14.))
            .pb(px(4.))
            .pl(px(16.))
            .pr(px(10.))
            .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(self.title.clone()))
            .child(
                div()
                    .id("folders-count")
                    .debug_selector(move || format!("folders-count:{count}"))
                    .text_size(px(11.5))
                    .text_color(hsla(t.fg_faint))
                    .child(format!("{count} de {}", self.folders.len())),
            )
            .child(toggle);
        let filter_row = div()
            .px(px(12.))
            .pt(px(4.))
            .pb(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(crate::app::icons::icon(crate::app::icons::Icon::Search, 16., hsla(t.fg_faint)))
            .child(div().flex_1().child(self.query.clone()));
        let list = if shown.is_empty() {
            div()
                .id("folders-list")
                .px(px(16.))
                .pt(px(8.))
                .pb(px(16.))
                .text_size(px(12.))
                .text_color(hsla(t.fg_faint))
                .child("Nenhuma pasta com esse nome")
        } else {
            let rows: Vec<_> = shown.iter().map(|f| self.row(f, cx)).collect();
            div().id("folders-list").max_h(px(380.)).overflow_y_scroll().px(px(6.)).flex().flex_col().children(rows)
        };
        let (cancel_me, done_me) = (me.clone(), me);
        let footer = div()
            .flex()
            .justify_end()
            .gap(px(4.))
            .px(px(12.))
            .pt(px(8.))
            .pb(px(12.))
            .child(div().debug_selector(|| "folders-cancel".to_owned()).child(text_button("folders-cancel", "Cancelar", hsla(t.accent), None, move |w, cx| {
                if let Some(p) = cancel_me.upgrade() {
                    let done = p.read(cx).done.clone();
                    done(None, w, cx);
                }
            })))
            .child(div().debug_selector(|| "folders-done".to_owned()).child(text_button("folders-done", "Pronto", hsla(t.on_accent()), Some(hsla(t.accent)), move |w, cx| {
                if let Some(p) = done_me.upgrade() {
                    let (done, result) = {
                        let p = p.read(cx);
                        (p.done.clone(), ordered_result(&p.folders, &p.picked))
                    };
                    done(Some(result), w, cx);
                }
            })));
        let cancel = cx.weak_entity();
        div()
            .id("folders-barrier")
            .absolute()
            .inset_0()
            .bg(rgba(super::widgets::PICK_BARRIER))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_click(move |_, w, cx| {
                if let Some(p) = cancel.upgrade() {
                    p.update(cx, |p, cx| p.finish(None, w, cx));
                }
            })
            .child(
                div()
                    .id("folder-picker")
                    .w(px(520.))
                    .flex()
                    .flex_col()
                    .bg(hsla(t.bg_sidebar))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(hsla(t.border))
                    .shadow(crate::chrome::style::shadow(&t, 24., 11.))
                    .overflow_hidden()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(header)
                    .child(filter_row)
                    .child(list)
                    .child(footer),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sidebar() -> Vec<Pick> {
        vec![Pick { root: "/r/app".into(), name: "app".into() }, Pick { root: "/r/api".into(), name: "api".into() }]
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn the_summary_names_up_to_two_folders_and_counts_the_rest() {
        assert_eq!(summary(&[]), "Nenhuma");
        assert_eq!(summary(&s(&["app"])), "app");
        assert_eq!(summary(&s(&["app", "api"])), "app, api");
        assert_eq!(summary(&s(&["a", "b", "c", "d"])), "a, b e mais 2");
    }

    #[test]
    fn the_button_shows_only_picked_folders_still_in_the_sidebar() {
        assert_eq!(button_text(&sidebar(), &s(&["/r/api", "/gone"])), ("api".into(), false));
        assert_eq!(button_text(&sidebar(), &s(&["/gone"])), ("Nenhuma".into(), true));
        assert_eq!(button_text(&[], &[]), ("Nenhuma pasta na lateral".into(), true));
    }

    #[test]
    fn the_filter_needs_every_term_in_the_name_or_root_ignoring_case() {
        let f = sidebar();
        assert_eq!(filter("api", &f).len(), 1);
        assert_eq!(filter("R/ AP", &f).len(), 2);
        assert_eq!(filter("api app", &f).len(), 0);
        assert_eq!(filter("  ", &f).len(), 2);
    }

    #[test]
    fn todas_and_nenhuma_act_only_on_the_filtered_rows() {
        assert_eq!(toggle_all(&s(&["/r/api"]), &[]), s(&["/r/api"]));
        assert_eq!(toggle_all(&s(&["/r/api"]), &s(&["/r/app", "/r/api"])), s(&["/r/app"]));
        assert_eq!(toggle_all(&s(&["/r/app", "/r/api"]), &s(&["/r/api"])), s(&["/r/api", "/r/app"]));
    }

    #[test]
    fn pronto_returns_sidebar_order_then_roots_outside_it() {
        assert_eq!(ordered_result(&sidebar(), &s(&["/gone", "/r/api", "/r/app"])), s(&["/r/app", "/r/api", "/gone"]));
        assert_eq!(ordered_result(&sidebar(), &[]), Vec::<String>::new());
    }
}

#[cfg(test)]
mod view_tests {
    use std::cell::RefCell;

    use gpui::{AppContext, Context, Entity, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div};

    use super::*;

    struct Host(Option<Entity<FolderPicker>>);

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().relative().children(self.0.clone())
        }
    }

    type Answers = Rc<RefCell<Vec<Option<Vec<String>>>>>;

    fn open<'a>(cx: &'a mut TestAppContext, picked: &[&str]) -> (Answers, &'a mut VisualTestContext) {
        let answers: Answers = Rc::default();
        let a = answers.clone();
        let picked: Vec<String> = picked.iter().map(|s| (*s).to_owned()).collect();
        let (_, vcx) = cx.add_window_view(move |window, cx| {
            let done: Done = Rc::new(move |r, _, _| a.borrow_mut().push(r));
            let folders = vec![Pick { root: "/r/app".into(), name: "app".into() }, Pick { root: "/r/api".into(), name: "api".into() }];
            Host(Some(cx.new(|cx| FolderPicker::new("Pastas".into(), folders, picked, done, window, cx))))
        });
        vcx.run_until_parked();
        (answers, vcx)
    }

    fn tap(vcx: &mut VisualTestContext, selector: &str) {
        let b = vcx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str())).unwrap_or_else(|| panic!("no {selector}"));
        vcx.simulate_click(b.center(), Modifiers::none());
        vcx.run_until_parked();
    }

    #[gpui::test]
    fn picking_api_and_pronto_returns_it(cx: &mut TestAppContext) {
        let (answers, vcx) = open(cx, &[]);
        tap(vcx, "folder:api");
        tap(vcx, "folders-done");
        assert_eq!(answers.borrow().as_slice(), &[Some(vec!["/r/api".to_owned()])]);
    }

    #[gpui::test]
    fn cancelar_changes_nothing(cx: &mut TestAppContext) {
        let (answers, vcx) = open(cx, &["/r/api"]);
        tap(vcx, "folder:app");
        tap(vcx, "folders-cancel");
        assert_eq!(answers.borrow().as_slice(), &[None]);
    }

    #[gpui::test]
    fn todas_then_pronto_returns_sidebar_order(cx: &mut TestAppContext) {
        let (answers, vcx) = open(cx, &["/r/api"]);
        tap(vcx, "folders-all:Todas");
        assert!(vcx.debug_bounds("folders-count:2").is_some());
        tap(vcx, "folders-all:Nenhuma");
        assert!(vcx.debug_bounds("folders-count:0").is_some());
        tap(vcx, "folders-all:Todas");
        tap(vcx, "folders-done");
        assert_eq!(answers.borrow().as_slice(), &[Some(vec!["/r/app".to_owned(), "/r/api".to_owned()])]);
    }
}
