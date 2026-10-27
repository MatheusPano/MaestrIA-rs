use gpui::{AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px};

use super::dialog::{SettingsDialog, heading};
use super::model::{PluginMenuAction, PluginRow, PluginState, SettingsAction, checked_label, conflicts_label, update_status};
use crate::chrome::icon::{self, icon};
use crate::chrome::menu::{MenuEntry, MenuItem, Tone};
use crate::chrome::style::{MONO, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, IconButton, MxSwitch, Toggle};
use crate::plugin_ui::kind_tag::kind_tag;

pub const PLUGINS_INFO: &str = "Temas, comandos e janelas que vêm de fora, instalados em ~/.maestria-rs/plugins. Um plugin com programa roda com as suas permissões — instale só o que você confia. Como escrever um: docs/plugins.md, no repositório da maestria.";
pub const AUTO_UPDATE_TITLE: &str = "Atualizar os plugins sozinho";
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

fn card(d: &SettingsDialog, p: &PluginRow, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let tone = state_tone(p.state).color(&t);
    let open = d.expanded.contains(&p.id);
    let mut head = div().flex().items_center().gap(px(8.))
        .child(icon(icon::EXTENSION_OUTLINED, 16., hsla(tone)))
        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(p.name.clone()))
        .child(div().font_family(MONO).text_size(px(11.)).text_color(hsla(t.fg_faint)).child(p.version.clone()))
        .children(p.kinds.iter().map(|k| kind_tag(*k, &t)))
        .child(div().text_size(px(11.)).text_color(hsla(tone)).child(p.state.label()));
    if p.dev {
        head = head.child(div().flex().items_center().gap(px(3.)).child(icon(icon::LINK, 12., hsla(t.purple))).child(div().text_size(px(11.)).text_color(hsla(t.purple)).child("Desenvolvimento")));
    }
    if let Some((text, tone)) = update_status(p) {
        head = head.child(div().text_size(px(11.)).text_color(hsla(tone.color(&t))).child(text));
    }
    let (more_tip, more_color) = if p.conflicts > 0 { ("Mais — atalho em conflito em configurar", t.yellow) } else { ("Mais", t.fg_dim) };
    let id = p.id.clone();
    let row = p.clone();
    let auto_all = d.data.plugin_auto.on;
    let me = cx.entity().downgrade();
    head = head.child(div().flex_1())
        .child(IconButton::new(SharedString::from(format!("more-{}", p.id)), icon::MORE_VERT).size(26., 16., 6.).tooltip(more_tip).color(more_color)
            .on_click(move |w, cx| {
                let at = w.mouse_position();
                me.update(cx, |this, cx| this.open_menu(plugin_menu(&row, auto_all), at, id.clone(), cx)).ok();
            }))
        .child(MxSwitch::new(SharedString::from(format!("enable-{}", p.id)), p.enabled).on_change({
            let (send, id) = (d.on_action.clone(), p.id.clone());
            move |on, w, cx| send(SettingsAction::SetPluginEnabled(id.clone(), on), w, cx)
        }));
    let mut body = div().flex().flex_col().gap(px(3.)).pl(px(24.)).text_size(px(11.5)).text_color(hsla(t.fg_dim));
    if !p.description.is_empty() {
        body = body.child(div().whitespace_nowrap().overflow_hidden().text_ellipsis().child(p.description.clone()));
    }
    if !p.commands.is_empty() {
        body = body.child(format!("Comandos: {}", p.commands.join(", ")));
    }
    if !p.themes.is_empty() {
        body = body.child(format!("Temas: {} — em aparência", p.themes.join(", ")));
    }
    if !p.details.is_empty() {
        if open {
            body = body.children(p.details.iter().map(|l| div().text_color(hsla(t.fg_faint)).child(l.clone())));
        }
        let id = p.id.clone();
        body = body.child(div().id(SharedString::from(format!("expand-{}", p.id))).cursor_pointer().text_color(hsla(t.purple)).child(if open { "Ver menos" } else { "Ver mais" })
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.expanded.remove(&id) {
                    this.expanded.insert(id.clone());
                }
                cx.notify();
            })));
    }
    div().flex().flex_col().gap(px(6.)).p(px(12.)).mb(px(8.)).rounded(px(9.)).border_1().border_color(hsla(t.border)).child(head).child(body).into_any_element()
}

fn auto_update(d: &SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let a = &d.data.plugin_auto;
    let mut status = div().flex().items_center().gap(px(10.));
    if let Some(text) = checked_label(a) {
        status = status.child(div().text_size(px(11.5)).text_color(hsla(t.fg_faint)).child(text));
    }
    status = status.child(Button::new("plugin-check", "Procurar agora", ButtonKind::Text).disabled(a.checking).on_click(d.send(SettingsAction::CheckPluginUpdates)));
    let mut col = div().flex().flex_col().mb(px(10.))
        .child(Toggle::new("plugin-auto-update", AUTO_UPDATE_TITLE, a.on, d.send(SettingsAction::SetPluginAutoUpdate(!a.on))))
        .child(status);
    if let Some(e) = &a.error {
        col = col.child(div().text_size(px(11.5)).text_color(hsla(t.red)).child(e.clone()));
    }
    col.into_any_element()
}

pub fn page(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let buttons = div().flex().gap(px(4.))
        .child(Button::new("plugin-catalog", "Catálogo", ButtonKind::Text).leading(icon::STOREFRONT).on_click(d.send(SettingsAction::OpenCatalog)))
        .child(Button::new("plugin-install", "Instalar…", ButtonKind::Text).on_click(d.send(SettingsAction::InstallPlugin)))
        .child(Button::new("plugin-folder", "Abrir a pasta", ButtonKind::Text).on_click(d.send(SettingsAction::OpenPluginsFolder)))
        .child(Button::new("plugin-rescan", "Reler", ButtonKind::Text).on_click(d.send(SettingsAction::RescanPlugins)))
        .into_any_element();
    let mut col = div().flex().flex_col().child(heading("Plugins", Some(PLUGINS_INFO), true, Some(buttons), cx)).child(auto_update(d, cx));
    if d.data.plugins.is_empty() {
        col = col.child(div().p(px(14.)).rounded(px(9.)).border_1().border_color(hsla(t.border)).text_size(px(12.)).line_height(px(17.)).text_color(hsla(t.fg_faint)).child(PLUGINS_EMPTY));
    }
    for p in d.data.plugins.clone() {
        col = col.child(card(d, &p, cx));
    }
    col.child(div().mt(px(10.)).flex().child(Button::new("plugin-dev", "Escrevendo um plugin? Carregar pasta de desenvolvimento…", ButtonKind::Text).on_click(d.send(SettingsAction::LoadDevPlugin))))
        .into_any_element()
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
        assert_eq!(checked_label(&a).unwrap(), "Procurado às 09:05");
        a.checking = true;
        assert_eq!(checked_label(&a).unwrap(), "Procurando…");
    }
}
