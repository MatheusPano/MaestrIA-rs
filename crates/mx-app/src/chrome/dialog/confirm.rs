use std::rc::Rc;

use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div};
use mx_core::model::FeatureOrHotfixKind;

use super::frame::{alert, check_line, fact, path_line};
use crate::chrome::icon::{self, Icon};
use crate::chrome::menu::Tone;
use crate::chrome::style::{Handler, cb, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind};

#[derive(Clone, Debug, PartialEq)]
pub struct Confirm {
    pub title: String,
    pub width: f32,
    pub path: Option<String>,
    pub facts: Vec<(Icon, Tone, String)>,
    pub check: Option<String>,
    pub primary: String,
    pub kind: ButtonKind,
}

fn plural(n: usize, one: &str, many: String) -> String {
    if n == 1 { one.to_owned() } else { many }
}

pub struct WorktreeSafety {
    pub dirty: Option<usize>,
    pub base: Option<String>,
    pub unmerged: usize,
}

pub fn remove_worktree(label: &str, path: &str, ghost: bool, safety: &WorktreeSafety, branch: Option<&str>) -> Confirm {
    let mut facts = Vec::new();
    if ghost {
        facts.push((icon::LINK_OFF, Tone::Dim, "A pasta já não existe — isso só limpa o registro que sobrou no git".into()));
    } else {
        let (g, tone) = if safety.dirty == Some(0) { (icon::CHECK_ROUNDED, Tone::Green) } else { (icon::WARNING_AMBER_ROUNDED, Tone::Yellow) };
        let text = match safety.dirty {
            None => "Não deu pra ler o git status — pode haver trabalho não commitado".to_owned(),
            Some(0) => "Nada não commitado".to_owned(),
            Some(n) => plural(n, "1 arquivo não commitado — isso se perde", format!("{n} arquivos não commitados — isso se perde")),
        };
        facts.push((g, tone, text));
        facts.push(match &safety.base {
            None => (icon::HELP_OUTLINE, Tone::Dim, "Sem base pra comparar os commits".into()),
            Some(base) if safety.unmerged == 0 => (icon::CHECK_ROUNDED, Tone::Green, format!("Todo commit daqui já está em {base}")),
            Some(base) => (
                icon::WARNING_AMBER_ROUNDED,
                Tone::Red,
                plural(safety.unmerged, &format!("1 commit que não está em {base}"), format!("{} commits que não estão em {base}", safety.unmerged)),
            ),
        });
    }
    let risky = !ghost && (safety.dirty != Some(0) || (safety.base.is_some() && safety.unmerged > 0));
    Confirm {
        title: format!("{} {label}?", if ghost { "Limpar" } else { "Excluir" }),
        width: 470.,
        path: Some(path.to_owned()),
        facts,
        check: branch.map(|b| format!("Apagar também a branch {b}")),
        primary: if risky { "Excluir de qualquer forma" } else { "Excluir" }.into(),
        kind: ButtonKind::Danger,
    }
}

pub fn close_workspace(name: &str, path: Option<&str>, folders: usize, sessions: usize) -> Confirm {
    let mut facts = vec![
        (icon::FOLDER_OFF_OUTLINED, Tone::Dim, plural(folders, "1 pasta sai da lateral", format!("{folders} pastas saem da lateral"))),
        (icon::CHECK_ROUNDED, Tone::Green, "Nada é apagado do disco — nem os repos, nem o .code-workspace".into()),
    ];
    if sessions > 0 {
        facts.push((
            icon::WARNING_AMBER_ROUNDED,
            Tone::Yellow,
            plural(sessions, "1 sessão aberta é encerrada", format!("{sessions} sessões abertas são encerradas")),
        ));
    }
    Confirm {
        title: format!("Fechar {name}?"),
        width: 470.,
        path: path.map(str::to_owned),
        facts,
        check: None,
        primary: "Fechar workspace".into(),
        kind: ButtonKind::Danger,
    }
}

pub fn clear_groups(total: usize) -> Confirm {
    Confirm {
        title: format!("Esquecer os {total} grupos?"),
        width: 440.,
        path: None,
        facts: vec![
            (icon::CHECK_ROUNDED, Tone::Green, "Nenhum painel fecha — a tela de agora fica exatamente como está".into()),
            (icon::GRID_VIEW_ROUNDED, Tone::Dim, format!("O que se perde são os {total} arranjos guardados, com os nomes deles")),
            (icon::REPLAY, Tone::Faint, "Pra ter um de volta: arrume a grade e agrupe os painéis outra vez".into()),
        ],
        check: None,
        primary: "Esquecer".into(),
        kind: ButtonKind::Danger,
    }
}

pub fn complete_feature(name: &str, kind: FeatureOrHotfixKind, tabs: usize, busy: usize, waiting: usize, has_brief: bool) -> Confirm {
    let mut facts = vec![if tabs == 0 {
        (icon::CHECK_ROUNDED, Tone::Green, format!("Não tem painel aberto {}", kind.in_this()))
    } else {
        (icon::CLOSE_ROUNDED, Tone::Dim, format!("Fecha {tabs} painel(is) — as sessões terminam aqui"))
    }];
    if busy > 0 {
        facts.push((icon::WARNING_AMBER_ROUNDED, Tone::Yellow, format!("{busy} painel(is) ainda no meio de um turno")));
    }
    if waiting > 0 {
        facts.push((icon::WARNING_AMBER_ROUNDED, Tone::Red, format!("{waiting} painel(is) esperando você responder")));
    }
    if has_brief {
        facts.push((icon::STICKY_NOTE_2_OUTLINED, Tone::Dim, "O briefing vai junto — copie o texto antes se for reaproveitar".into()));
    }
    facts.push((icon::FOLDER_OUTLINED, Tone::Faint, format!("Nada é mexido no repo: {} só existia aqui", kind.the())));
    Confirm {
        title: format!("Concluir \"{name}\"?"),
        width: 460.,
        path: None,
        facts,
        check: None,
        primary: if busy + waiting > 0 { "Concluir de qualquer forma" } else { "Concluir" }.into(),
        kind: ButtonKind::Success,
    }
}

pub struct ConfirmDialog {
    confirm: Confirm,
    checked: bool,
    on_done: Handler<Option<bool>>,
}

impl ConfirmDialog {
    pub fn new(confirm: Confirm, on_done: impl Fn(Option<bool>, &mut Window, &mut App) + 'static) -> Self {
        Self { confirm, checked: false, on_done: Rc::new(on_done) }
    }

    pub fn finish(&mut self, go: bool, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_done)(go.then_some(self.checked), window, cx);
    }
}

impl Render for ConfirmDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let me = cx.entity().downgrade();
        let c = &self.confirm;
        let mut body = div().flex().flex_col();
        if let Some(p) = &c.path {
            body = body.child(path_line(p.clone(), &t));
        }
        body = body.children(c.facts.iter().map(|(g, tone, text)| fact(*g, hsla(tone.color(&t)), text.clone())));
        if let Some(label) = &c.check {
            body = body.child(check_line("confirm-check", label.clone(), self.checked, cb(me.clone(), |this, _, cx| {
                this.checked = !this.checked;
                cx.notify();
            }), &t));
        }
        let actions: Vec<AnyElement> = vec![
            Button::new("cancel", "Cancelar", ButtonKind::Text)
                .on_click(cb(me.clone(), |this, w, cx| this.finish(false, w, cx)))
                .into_any_element(),
            Button::new("confirm", SharedString::from(c.primary.clone()), c.kind)
                .on_click(cb(me.clone(), |this, w, cx| this.finish(true, w, cx)))
                .into_any_element(),
        ];
        alert(c.title.clone(), Some(c.width), body, actions, Rc::new(cb(me, |this, w, cx| this.finish(false, w, cx))), cx)
    }
}
