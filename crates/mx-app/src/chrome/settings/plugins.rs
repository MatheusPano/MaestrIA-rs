use gpui::{AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px};

use super::dialog::{SettingsDialog, heading};
use super::model::{PluginMenuAction, PluginRow, PluginState, SettingsAction, conflicts_label};
use crate::chrome::icon::{self, icon};
use crate::chrome::menu::{MenuEntry, MenuItem, Tone};
use crate::chrome::style::{MONO, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, IconButton};

pub const PLUGINS_INFO: &str = "Temas, comandos e janelas que vêm de fora, instalados em ~/.maestria/plugins. Um plugin com programa roda com as suas permissões — instale só o que você confia. Como escrever um: docs/plugins.md, no repositório da maestria.";
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

pub fn plugin_menu(conflicts: usize) -> Vec<MenuEntry<PluginMenuAction>> {
    let mut configure = MenuItem::new("Configurar…", PluginMenuAction::Configure).icon(icon::TUNE, Tone::Dim);
    if conflicts > 0 {
        configure = configure.subtitle(conflicts_label(conflicts), Tone::Yellow);
    }
    vec![
        MenuItem::new("Reiniciar", PluginMenuAction::Restart).icon(icon::RESTART_ALT, Tone::Dim).subtitle("Relê a pasta e sobe o processo de novo", Tone::Faint).into(),
        configure.into(),
        MenuItem::new("Log", PluginMenuAction::Log).icon(icon::RECEIPT_LONG_OUTLINED, Tone::Dim).into(),
        MenuItem::new("Mostrar no Finder", PluginMenuAction::Reveal).icon(icon::FOLDER_OPEN, Tone::Dim).into(),
        MenuEntry::Divider,
        MenuItem::new("Remover…", PluginMenuAction::Remove).icon(icon::DELETE_OUTLINE, Tone::Red).danger().into(),
    ]
}

fn card(d: &SettingsDialog, p: &PluginRow, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let tone = state_tone(p.state).color(&t);
    let open = d.expanded.contains(&p.id);
    let mut head = div().flex().items_center().gap(px(8.))
        .child(icon(icon::EXTENSION_OUTLINED, 16., hsla(tone)))
        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(p.name.clone()))
        .child(div().font_family(MONO).text_size(px(11.)).text_color(hsla(t.fg_faint)).child(p.version.clone()))
        .child(div().text_size(px(11.)).text_color(hsla(tone)).child(p.state.label()));
    if p.dev {
        head = head.child(div().flex().items_center().gap(px(3.)).child(icon(icon::LINK, 12., hsla(t.purple))).child(div().text_size(px(11.)).text_color(hsla(t.purple)).child("Desenvolvimento")));
    }
    let (more_tip, more_color) = if p.conflicts > 0 { ("Mais — atalho em conflito em configurar", t.yellow) } else { ("Mais", t.fg_dim) };
    let id = p.id.clone();
    let conflicts = p.conflicts;
    let me = cx.entity().downgrade();
    head = head.child(div().flex_1())
        .child(IconButton::new(SharedString::from(format!("more-{}", p.id)), icon::MORE_VERT).size(26., 16., 6.).tooltip(more_tip).color(more_color)
            .on_click(move |w, cx| {
                let at = w.mouse_position();
                me.update(cx, |this, cx| this.open_menu(plugin_menu(conflicts), at, id.clone(), cx)).ok();
            }))
        .child(crate::chrome::widgets::Switch::new(SharedString::from(format!("enable-{}", p.id)), "", p.enabled, d.send(SettingsAction::SetPluginEnabled(p.id.clone(), !p.enabled))));
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

pub fn page(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let t = theme(cx);
    let buttons = div().flex().gap(px(4.))
        .child(Button::new("plugin-install", "Instalar…", ButtonKind::Text).on_click(d.send(SettingsAction::InstallPlugin)))
        .child(Button::new("plugin-folder", "Abrir a pasta", ButtonKind::Text).on_click(d.send(SettingsAction::OpenPluginsFolder)))
        .child(Button::new("plugin-rescan", "Reler", ButtonKind::Text).on_click(d.send(SettingsAction::RescanPlugins)))
        .into_any_element();
    let mut col = div().flex().flex_col().child(heading("Plugins", Some(PLUGINS_INFO), true, Some(buttons), cx));
    if d.data.plugins.is_empty() {
        col = col.child(div().p(px(14.)).rounded(px(9.)).border_1().border_color(hsla(t.border)).text_size(px(12.)).line_height(px(17.)).text_color(hsla(t.fg_faint)).child(PLUGINS_EMPTY));
    }
    for p in d.data.plugins.clone() {
        col = col.child(card(d, &p, cx));
    }
    col.child(div().mt(px(10.)).flex().child(Button::new("plugin-dev", "Escrevendo um plugin? Carregar pasta de desenvolvimento…", ButtonKind::Text).on_click(d.send(SettingsAction::LoadDevPlugin))))
        .into_any_element()
}
