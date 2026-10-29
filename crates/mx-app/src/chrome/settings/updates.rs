use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};

use super::dialog::{SettingsDialog, heading};
use super::model::{AppUpdates, SettingsAction};
use crate::chrome::widgets::Toggle;

pub const TITLE: &str = "Atualizar sozinho";

pub fn hint(u: &AppUpdates) -> String {
    if u.current.is_empty() {
        "Build de desenvolvimento: sem número de versão, não há com o que comparar a release publicada.".into()
    } else {
        format!("Esta é a {}.", u.current)
    }
}

pub fn subtitle(u: &AppUpdates) -> String {
    if !u.enabled {
        return "Só vale pro app instalado pelo pacote da release.".into();
    }
    let when = if u.installs_on_quit { "Entra quando você fecha o app." } else { "Instala quando você clica nele (o sistema pede a senha)." };
    format!("Quando sai versão nova, ela é baixada em segundo plano e o número aparece na barra de status. {when} Nada reinicia sem você pedir.")
}

pub fn page(d: &mut SettingsDialog, cx: &mut Context<SettingsDialog>) -> AnyElement {
    let u = &d.data.app_updates;
    div()
        .flex()
        .flex_col()
        .child(heading("Atualizações", Some(&hint(u)), false, None, cx))
        .child(
            Toggle::new("app-auto-update", TITLE, u.active, d.send(SettingsAction::SetAutoUpdate(!u.active)))
                .subtitle(subtitle(u))
                .disabled(!u.enabled),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn updates(current: &str, enabled: bool, installs_on_quit: bool) -> AppUpdates {
        AppUpdates { current: current.into(), enabled, active: false, installs_on_quit }
    }

    #[test]
    fn a_dev_build_says_it_has_no_version_to_compare() {
        assert!(hint(&updates("", false, false)).starts_with("Build de desenvolvimento"));
        assert_eq!(hint(&updates("3.0.0", true, false)), "Esta é a 3.0.0.");
    }

    #[test]
    fn the_subtitle_says_when_the_update_lands() {
        assert_eq!(subtitle(&updates("3.0.0", false, false)), "Só vale pro app instalado pelo pacote da release.");
        assert!(subtitle(&updates("3.0.0", true, false)).contains("Instala quando você clica nele (o sistema pede a senha)."));
        assert!(subtitle(&updates("3.0.0", true, true)).contains("Entra quando você fecha o app."));
    }
}
