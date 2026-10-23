use mx_core::model::{FeatureOrHotfixKind, LauncherIcon};

use super::form::{ActionSpec, FieldSpec, FormSpec};
use crate::chrome::icon;
use crate::chrome::menu::Tone;
use crate::chrome::menu::actions::launcher_icon;
use crate::chrome::widgets::ButtonKind;

fn actions(primary: &str) -> Vec<ActionSpec> {
    vec![
        ActionSpec::new("Cancelar", ButtonKind::Text),
        ActionSpec::new(primary.to_owned(), ButtonKind::Primary),
    ]
}

pub fn prompt_text(title: &str, label: &str, initial: &str) -> FormSpec {
    FormSpec {
        title: title.to_owned().into(),
        fields: vec![FieldSpec::new(label.to_owned(), "").initial(initial)],
        actions: actions("Ok"),
        submit: 1,
        ..Default::default()
    }
}

pub const ADD_FOLDER_NOTE: &str = "Um .code-workspace adiciona todas as pastas dele de uma vez.";
pub const ADD_FOLDER_TROUBLE: &str = "Não consegui abrir o seletor — cole aí em cima o caminho do .code-workspace";

pub fn add_folder() -> FormSpec {
    FormSpec {
        title: "Adicionar pasta".into(),
        width: Some(460.),
        fields: vec![FieldSpec::new("Caminho do repo", "~/repos/...")],
        extras: vec![
            (icon::FOLDER_OPEN, "Escolher pasta…".into()),
            (icon::HEXAGON_OUTLINED, "Escolher workspace…".into()),
        ],
        note: Some((ADD_FOLDER_NOTE.into(), Tone::Faint)),
        actions: actions("Adicionar"),
        submit: 1,
        ..Default::default()
    }
}

pub fn loose_in(loose_root: &str) -> FormSpec {
    FormSpec {
        title: "Abrir sem pasta".into(),
        width: Some(460.),
        fields: vec![FieldSpec::new("Pasta", "Onde o painel vai rodar").initial(loose_root)],
        extras: vec![(icon::FOLDER_OPEN, "Escolher pasta…".into())],
        note: Some(("O painel fica em \"avulsos\" — nenhuma pasta é adicionada.".into(), Tone::Faint)),
        actions: vec![
            ActionSpec::new("Cancelar", ButtonKind::Text),
            ActionSpec::new("Terminal", ButtonKind::Text),
            ActionSpec::new("Sessão do Claude", ButtonKind::Primary),
        ],
        submit: 2,
        ..Default::default()
    }
}

pub fn new_task(place: &str) -> FormSpec {
    FormSpec {
        title: format!("Nova task em {place}").into(),
        width: Some(520.),
        fields: vec![
            FieldSpec::new("id da task", "47730"),
            FieldSpec::new("Branch", "").initial("feature/TASK#{id}").row(1),
            FieldSpec::new("Pasta da worktree", "").initial("TASK-{id}").row(1),
            FieldSpec::new("Base", "Vazio = origin/HEAD"),
            FieldSpec::new("Comando pós-criação", "Ex: flutter pub get"),
        ],
        note: Some(("{id} é substituído. O \"#\" sai do nome da pasta e fica na branch.".into(), Tone::Faint)),
        actions: actions("Criar e abrir"),
        submit: 1,
        ..Default::default()
    }
}

pub fn new_workspace(folders: &[(String, bool)]) -> FormSpec {
    FormSpec {
        title: "Novo workspace".into(),
        width: Some(480.),
        fields: vec![FieldSpec::new("Nome", "ATRIUM")],
        checks: folders.iter().map(|(name, on)| (name.clone().into(), *on)).collect(),
        checks_max_height: 320.,
        actions: actions("Criar"),
        submit: 1,
        required: vec![0],
        ..Default::default()
    }
}

pub fn new_feature(kind: FeatureOrHotfixKind, folder: &str) -> FormSpec {
    FormSpec {
        title: format!("{} em {folder}", kind.new_label()).into(),
        width: Some(560.),
        fields: vec![
            FieldSpec::new("Nome", "Permissão do google"),
            FieldSpec::new(
                "Briefing (opcional)",
                format!("O que todo agente {} precisa saber antes de começar", kind.of_this()),
            )
            .lines(3, 8),
        ],
        note: Some((
            "O briefing entra como --append-system-prompt em toda sessão aberta aqui dentro — vale a conversa inteira, não só a primeira mensagem.".into(),
            Tone::Faint,
        )),
        actions: actions("Criar"),
        submit: 1,
        required: vec![0],
        ..Default::default()
    }
}

pub fn brief(name: &str, current: &str) -> FormSpec {
    FormSpec {
        title: format!("Briefing de {name}").into(),
        width: Some(560.),
        fields: vec![FieldSpec::new("Contexto", "O que já foi decidido, o que não encostar").initial(current).lines(6, 14)],
        note: Some((
            "As sessões que já estão abertas não mudam — o briefing entra na linha de comando, então vale a partir da próxima que você abrir aqui.".into(),
            Tone::Faint,
        )),
        actions: actions("Salvar"),
        submit: 1,
        ..Default::default()
    }
}

pub const LAUNCHER_ICONS: [LauncherIcon; 10] = [
    LauncherIcon::Terminal,
    LauncherIcon::Monitor,
    LauncherIcon::Chart,
    LauncherIcon::Git,
    LauncherIcon::Server,
    LauncherIcon::Database,
    LauncherIcon::Rocket,
    LauncherIcon::Bug,
    LauncherIcon::Edit,
    LauncherIcon::Box,
];

pub fn launcher_form(editing: Option<(&str, &str, LauncherIcon)>) -> FormSpec {
    let (name, command, glyph) = editing.unwrap_or(("", "", LauncherIcon::Terminal));
    let selected = LAUNCHER_ICONS.iter().position(|i| *i == glyph).unwrap_or(0);
    FormSpec {
        title: if editing.is_some() { "Editar programa" } else { "Novo programa" }.into(),
        width: Some(460.),
        fields: vec![
            FieldSpec::new("Comando", "Btop, lazygit, npm run dev…")
                .initial(command)
                .mono()
                .note("Roda no painel como você digitaria no terminal — e o painel abre já dentro dele, sem prompt no caminho."),
            FieldSpec::new("Nome", "Como ele aparece no menu e na lateral").initial(name),
        ],
        chips: Some((
            "Desenho".into(),
            LAUNCHER_ICONS.iter().map(|i| (launcher_icon(*i), i.label().into())).collect(),
            selected,
        )),
        actions: actions("Salvar"),
        submit: 1,
        required: vec![0],
        ..Default::default()
    }
}

pub fn launcher_name(command: &str, typed: &str) -> String {
    let typed = typed.trim();
    if !typed.is_empty() {
        return typed.to_owned();
    }
    command.split_whitespace().next().unwrap_or_default().to_owned()
}
