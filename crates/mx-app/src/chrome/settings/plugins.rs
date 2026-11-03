use gpui::{AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px};
use mx_plugins::catalog::PluginKind;

use super::dialog::{SettingsDialog, heading};
use super::model::{PluginMenuAction, PluginRow, PluginState, SettingsAction, checked_label, conflicts_label, update_status};
use crate::chrome::icon::{self, icon};
use crate::chrome::menu::{MenuEntry, MenuItem, Tone};
use crate::chrome::style::{MONO, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, IconButton, MxSwitch, wide_tip};
use crate::plugin_ui::kind_tag::kind_tag;

pub const PLUGINS_INFO: &str = "Temas, comandos e janelas que vêm de fora, instalados em ~/.maestria-rs/plugins. Um plugin com programa roda com as suas permissões — instale só o que você confia. Como escrever um: docs/plugins.md, no repositório da maestria.";
pub const AUTO_UPDATE_TITLE: &str = "Atualizar sozinho";
pub const PLUGINS_EMPTY: &str = "Nenhum ainda. Os exemplos do repositório (examples/plugins) são um bom começo: um tema com atalhos, e um plugin completo em node com comandos, hooks e uma janela.";

pub fn state_tone(s: PluginState) -> Tone {
    match s {
        PluginState::Broken | PluginState::Stopped => Tone::Red,
        PluginState::Disabled => Tone::Faint,
        PluginState::Ready => Tone::Dim,
        PluginState::Starting => Tone::Yellow,
        PluginState::Running => Tone::Green,
    }
}

pub fn plugin_menu(p: &PluginRow, auto_all: bool) -> Vec<MenuEntry<PluginMenuAction>> {
    let mut configure = MenuItem::new("Configurar…", PluginMenuAction::Configure).icon(icon::TUNE, Tone::Dim);
    if p.conflicts > 0 {
        configure = configure.subtitle(conflicts_label(p.conflicts), Tone::Yellow);
    }
    let mut items: Vec<MenuEntry<PluginMenuAction>> = vec![
        MenuItem::new("Reiniciar", PluginMenuAction::Restart).icon(icon::RESTART_ALT, Tone::Dim).subtitle("Relê a pasta e sobe o processo de novo", Tone::Faint).into(),
        configure.into(),
    ];
    if let (Some(u), false) = (&p.update, p.busy) {
        let mut item = MenuItem::new(format!("Atualizar pra {}", u.to), PluginMenuAction::Update).icon(icon::SYSTEM_UPDATE, Tone::Purple);
        if u.asks_more {
            item = item.subtitle(format!("Pede mais do que a {} pedia", u.from), Tone::Yellow);
        }
        items.push(item.into());
    }
    if let (Some(o), false) = (&p.offer, p.busy) {
        let sub = if o.newer { format!("Instala a {} de lá", o.to) } else { format!("A mesma {}, e as próximas de lá", o.to) };
        items.push(MenuItem::new("Receber pelo catálogo", PluginMenuAction::Adopt).icon(icon::STOREFRONT, Tone::Dim).subtitle(sub, Tone::Faint).into());
    }
    if p.has_origin {
        let check = if p.manual { icon::CHECK_BOX_OUTLINE_BLANK } else { icon::CHECK_BOX };
        let mut item = MenuItem::new("Atualizar sozinho", PluginMenuAction::ToggleAutoUpdate).icon(check, Tone::Dim);
        if !auto_all {
            item = item.subtitle("Desligado pra todos, acima", Tone::Faint);
        }
        items.push(item.into());
    }
    items.extend([
        MenuItem::new("Log", PluginMenuAction::Log).icon(icon::RECEIPT_LONG_OUTLINED, Tone::Dim).into(),
        MenuItem::new("Mostrar no Finder", PluginMenuAction::Reveal).icon(icon::FOLDER_OPEN, Tone::Dim).into(),
        MenuEntry::Divider,
        MenuItem::new("Remover…", PluginMenuAction::Remove).icon(icon::DELETE_OUTLINE, Tone::Red).danger().into(),
    ]);
    items
}

pub fn plugins_menu() -> Vec<MenuEntry<SettingsAction>> {
    vec![
        MenuItem::new("De uma URL, .zip ou pasta…", SettingsAction::InstallPlugin).icon(icon::LINK, Tone::Dim).into(),
        MenuItem::new("Abrir a pasta", SettingsAction::OpenPluginsFolder).icon(icon::FOLDER_OPEN, Tone::Dim).into(),
        MenuItem::new("Reler a pasta", SettingsAction::RescanPlugins).icon(icon::REFRESH, Tone::Dim).subtitle("Pra quando um arquivo mudou fora daqui", Tone::Faint).into(),
        MenuEntry::Divider,
        MenuItem::new("Carregar pasta de desenvolvimento…", SettingsAction::LoadDevPlugin).icon(icon::CODE, Tone::Dim).into(),
    ]
}

/// A plugin with no manifest has no kind, and its error is what matters: it shows on both tabs.
pub fn holds(kind: PluginKind, p: &PluginRow) -> bool {
    p.kinds.is_empty() || p.kinds.contains(&kind)
}

fn kind_label(kind: PluginKind) -> &'static str {
    match kind {
        PluginKind::Feature => "Recursos",
        PluginKind::Theme => "Temas",
    }
}

fn kind_empty(kind: PluginKind) -> &'static str {
    match kind {
        PluginKind::Feature => "Nenhum recurso instalado. O catálogo tem alguns.",
        PluginKind::Theme => "Nenhum tema instalado. O catálogo tem alguns.",
    }
}

fn kind_tabs(d: &SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let mut row = div().flex().mb(px(10.)).border_b_1().border_color(hsla(t.border));
    for kind in [PluginKind::Feature, PluginKind::Theme] {
        let on = kind == d.plugin_kind;
        let count = d.data.plugins.iter().filter(|p| holds(kind, p)).count();
        let glyph = match kind {
            PluginKind::Feature => icon::EXTENSION_OUTLINED,
            PluginKind::Theme => icon::PALETTE_OUTLINED,
        };
        let hover = hsla(t.fg);
        row = row.child(
            div()
                .id(SharedString::from(format!("plugin-kind-{}", kind_label(kind))))
                .debug_selector(move || format!("plugin-kind:{}", kind_label(kind)))
                .mr(px(14.))
                .px(px(2.))
                .pt(px(6.))
                .pb(px(8.))
                .mb(px(-1.))
                .border_b_2()
                .border_color(if on { hsla(t.accent) } else { gpui::transparent_black() })
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .text_color(hsla(if on { t.fg } else { t.fg_dim }))
                .hover(move |s| s.text_color(hover))
                .child(icon(glyph, 14., hsla(if on { t.accent } else { t.fg_dim })))
                .child(div().text_size(px(12.5)).font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).child(kind_label(kind)))
                .child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child(count.to_string()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.plugin_kind = kind;
                    cx.notify();
                })),
        );
    }
    row.into_any_element()
}

/// Closed, a card is its name and one line of description; the rest waits behind "Ver mais".
/// Errors are never cut: they are what the person needs to read.
fn card(d: &SettingsDialog, p: &PluginRow, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let tone = state_tone(p.state).color(&t);
    let open = d.expanded.contains(&p.id);
    let mut head = div().flex().flex_wrap().items_center().gap(px(8.))
        .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(p.name.clone()))
        .child(div().font_family(MONO).text_size(px(11.)).text_color(hsla(t.fg_faint)).child(p.version.clone()))
        .children(p.kinds.iter().map(|k| kind_tag(*k, &t)));
    if p.dev {
        head = head.child(div().text_size(px(11.)).text_color(hsla(t.purple)).child("Desenvolvimento"));
    }
    if let Some((text, tone)) = update_status(p) {
        head = head.child(div().text_size(px(11.)).text_color(hsla(tone.color(&t))).child(text));
    }
    let glyph = div()
        .id(SharedString::from(format!("state-{}", p.id)))
        .tooltip(wide_tip(p.state.label(), 240.))
        .child(icon(icon::EXTENSION_OUTLINED, 17., hsla(tone)));
    let hidden = !p.commands.is_empty() || !p.themes.is_empty() || !p.details.is_empty() || p.description.chars().count() > 90;
    let shut = hidden && !open;
    let mut body = div().flex().flex_col().gap(px(1.)).pl(px(28.)).pt(px(3.)).text_size(px(11.5)).line_height(px(16.)).text_color(hsla(t.fg_dim));
    body = body.children(p.errors.iter().map(|e| div().text_color(hsla(t.red)).child(e.clone())));
    if p.errors.is_empty() && !p.description.is_empty() {
        let line = div().child(p.description.clone());
        body = body.child(if shut { line.whitespace_nowrap().overflow_hidden().text_ellipsis() } else { line });
    }
    if !shut {
        if !p.commands.is_empty() {
            body = body.child(format!("Comandos: {}", p.commands.join(" · ")));
        }
        if !p.themes.is_empty() {
            body = body.child(format!("Temas: {} — em aparência", p.themes.join(", ")));
        }
        body = body.children(p.details.iter().map(|l| div().child(l.clone())));
    }
    if hidden {
        let id = p.id.clone();
        body = body.child(div().id(SharedString::from(format!("expand-{}", p.id))).pt(px(2.)).cursor_pointer().text_color(hsla(t.purple)).child(if open { "Ver menos" } else { "Ver mais" })
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.expanded.remove(&id) {
                    this.expanded.insert(id.clone());
                }
                cx.notify();
            })));
    }
    let (more_tip, more_color) = if p.conflicts > 0 { ("Mais — atalho em conflito em configurar", t.yellow) } else { ("Mais", t.fg_dim) };
    let id = p.id.clone();
    let row = p.clone();
    let auto_all = d.data.plugin_auto.on;
    let me = cx.entity().downgrade();
    let more = IconButton::new(SharedString::from(format!("more-{}", p.id)), icon::MORE_VERT).size(28., 16., 6.).tooltip(more_tip).color(more_color)
        .on_click(move |w, cx| {
            let at = w.mouse_position();
            let id = id.clone();
            me.update(cx, |this, cx| this.open_menu(plugin_menu(&row, auto_all), at, move |a| SettingsAction::Plugin(id.clone(), a), cx)).ok();
        });
    let text = div().flex_1().min_w_0().flex().flex_col()
        .child(div().flex().items_center().gap(px(11.)).child(glyph).child(head.flex_1().min_w_0()))
        .child(body);
    div().flex().items_center().mb(px(6.)).px(px(12.)).py(px(10.)).rounded(px(9.)).bg(hsla(t.bg)).border_1().border_color(hsla(t.border))
        .child(text)
        .child(div().w(px(20.)).flex_none())
        .child(more)
        .child(div().ml(px(6.)).child(MxSwitch::new(SharedString::from(format!("enable-{}", p.id)), p.enabled).on_change({
            let (send, id) = (d.on_action.clone(), p.id.clone());
            move |on, w, cx| send(SettingsAction::SetPluginEnabled(id.clone(), on), w, cx)
        })))
        .into_any_element()
}

/// One line: the switch is what matters; when it last looked and "procurar agora" are its fine print.
fn auto_update(d: &SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let a = &d.data.plugin_auto;
    let faint = hsla(t.fg_faint);
    let mut row = div().flex().items_center().py(px(4.)).text_size(px(11.5))
        .child(div().mr(px(10.)).text_size(px(12.5)).text_color(hsla(t.fg)).child(AUTO_UPDATE_TITLE));
    let status = checked_label(a);
    if let Some(text) = &status {
        row = row.child(div().text_color(faint).child(text.clone()));
    }
    if !a.checking {
        if status.is_some() {
            row = row.child(div().text_color(faint).child(" · "));
        }
        row = row.child(div().id("plugin-check").cursor_pointer().text_color(hsla(t.accent)).child("procurar agora").on_click({
            let send = d.send(SettingsAction::CheckPluginUpdates);
            move |_, w, cx| send(w, cx)
        }));
    }
    row = row.child(div().flex_1()).child(MxSwitch::new("plugin-auto-update", a.on).on_change({
        let send = d.on_action.clone();
        move |on, w, cx| send(SettingsAction::SetPluginAutoUpdate(on), w, cx)
    }));
    let mut col = div().flex().flex_col().mb(px(10.)).child(row);
    if let Some(e) = &a.error {
        col = col.child(div().text_size(px(11.5)).text_color(hsla(t.red)).child(e.clone()));
    }
    col.into_any_element()
}

pub fn page(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let me = cx.entity().downgrade();
    let buttons = div().flex().items_center().gap(px(2.))
        .child(Button::new("plugin-catalog", "Catálogo", ButtonKind::Text).leading(icon::ADD).on_click(d.send(SettingsAction::OpenCatalog)))
        .child(IconButton::new("plugins-more", icon::MORE_VERT).size(28., 16., 6.).tooltip("Mais").color(t.fg_dim).on_click(move |w, cx| {
            let at = w.mouse_position();
            me.update(cx, |this, cx| this.open_menu(plugins_menu(), at, |a| a, cx)).ok();
        }))
        .into_any_element();
    let mut col = div().flex().flex_col().child(heading("Plugins", Some(PLUGINS_INFO), true, Some(buttons), cx)).child(auto_update(d, cx));
    if d.data.plugins.is_empty() {
        return col.child(div().p(px(14.)).rounded(px(9.)).border_1().border_color(hsla(t.border)).text_size(px(12.)).line_height(px(17.)).text_color(hsla(t.fg_faint)).child(PLUGINS_EMPTY)).into_any_element();
    }
    col = col.child(kind_tabs(d, cx));
    let kind = d.plugin_kind;
    let shown: Vec<PluginRow> = d.data.plugins.iter().filter(|p| holds(kind, p)).cloned().collect();
    if shown.is_empty() {
        col = col.child(div().px(px(2.)).py(px(8.)).text_size(px(12.)).text_color(hsla(t.fg_faint)).child(kind_empty(kind)));
    }
    for p in &shown {
        col = col.child(card(d, p, cx));
    }
    col.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::settings::model::{PluginAuto, RowUpdate};

    fn row() -> PluginRow {
        PluginRow {
            id: "mx.git".into(),
            name: "Git".into(),
            version: "1.0.0".into(),
            state: PluginState::Ready,
            dev: false,
            description: String::new(),
            errors: vec![],
            details: vec![],
            commands: vec![],
            themes: vec![],
            enabled: true,
            conflicts: 0,
            kinds: vec![],
            busy: false,
            update: None,
            offer: None,
            has_origin: false,
            manual: false,
        }
    }

    fn labels(p: &PluginRow, auto_all: bool) -> Vec<String> {
        plugin_menu(p, auto_all)
            .into_iter()
            .filter_map(|e| match e {
                MenuEntry::Item(i) => Some(i.label.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn update_adopt_and_auto_items_come_before_log() {
        let mut p = row();
        p.update = Some(RowUpdate { from: "1.0.0".into(), to: "1.1.0".into(), asks_more: true, newer: true });
        p.offer = Some(RowUpdate { from: "1.0.0".into(), to: "1.0.0".into(), asks_more: false, newer: false });
        p.has_origin = true;
        let l = labels(&p, true);
        let log = l.iter().position(|x| x == "Log").unwrap();
        assert_eq!(&l[2..log], ["Atualizar pra 1.1.0", "Receber pelo catálogo", "Atualizar sozinho"]);
        p.busy = true;
        assert!(!labels(&p, true).iter().any(|x| x.starts_with("Atualizar pra") || x == "Receber pelo catálogo"));
    }

    #[test]
    fn a_plugin_lands_on_the_tab_of_each_kind_and_one_without_kind_on_both() {
        let mut p = row();
        assert!(holds(PluginKind::Feature, &p) && holds(PluginKind::Theme, &p));
        p.kinds = vec![PluginKind::Theme];
        assert!(!holds(PluginKind::Feature, &p) && holds(PluginKind::Theme, &p));
        p.kinds = vec![PluginKind::Feature, PluginKind::Theme];
        assert!(holds(PluginKind::Feature, &p) && holds(PluginKind::Theme, &p));
    }

    #[test]
    fn the_title_menu_holds_what_is_for_plugin_authors() {
        let l = crate::chrome::menu::model::labels(&plugins_menu());
        assert_eq!(l, ["De uma URL, .zip ou pasta…", "Abrir a pasta", "Reler a pasta", "—", "Carregar pasta de desenvolvimento…"]);
    }

    #[test]
    fn a_plugin_without_origin_has_no_auto_update_item() {
        assert!(!labels(&row(), true).contains(&"Atualizar sozinho".to_owned()));
    }

    #[test]
    fn the_row_status_prefers_busy_then_permission_then_available() {
        let mut p = row();
        assert_eq!(update_status(&p), None);
        p.update = Some(RowUpdate { from: "1.0.0".into(), to: "2.0.0".into(), asks_more: false, newer: true });
        assert_eq!(update_status(&p).unwrap().0, "2.0.0 disponível");
        p.update.as_mut().unwrap().asks_more = true;
        assert_eq!(update_status(&p).unwrap().0, "2.0.0 pede permissão nova");
        p.busy = true;
        assert_eq!(update_status(&p).unwrap().0, "Atualizando…");
    }

    #[test]
    fn the_check_line_reads_searching_or_the_time() {
        let mut a = PluginAuto::default();
        assert_eq!(checked_label(&a), None);
        a.checked_at = Some("09:05".into());
        assert_eq!(checked_label(&a).unwrap(), "procurado às 09:05");
        a.checking = true;
        assert_eq!(checked_label(&a).unwrap(), "procurando…");
    }
}
