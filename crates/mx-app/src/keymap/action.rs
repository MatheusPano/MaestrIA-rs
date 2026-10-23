use mx_plugins::chord::{Chord, Key};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Claude,
    Shell,
    Task,
    Rename,
    Pin,
    Close,
    CloseSettled,
    NextPane,
    PrevPane,
    NextSession,
    PrevSession,
    Search,
    Markdown,
    Plan,
    Refresh,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Sidebar,
    Settings,
}

struct Info {
    id: &'static str,
    label: &'static str,
    hint: Option<&'static str>,
    group: &'static str,
}

const fn info(id: &'static str, label: &'static str, hint: Option<&'static str>, group: &'static str) -> Info {
    Info { id, label, hint, group }
}

const S: &str = "Sessões";
const N: &str = "Navegação";
const E: &str = "Exibição";
const J: &str = "Janela";

const INFO: [Info; 20] = [
    info("claude", "Nova sessão Claude", Some("Na pasta e na feature/hotfix do painel em foco"), S),
    info("shell", "Novo shell", Some("Um terminal comum, no mesmo lugar"), S),
    info("task", "Nova task", Some("Abre a worktree e sobe uma sessão nela"), S),
    info(
        "rename",
        "Renomear o painel em foco",
        Some("O título dele; em branco, volta a ser o da branch ou da pasta"),
        S,
    ),
    info("pin", "Prender o painel em foco", Some("A lateral para de trocá-lo; de novo, solta"), S),
    info("close", "Fechar o painel em foco", None, S),
    info("close-settled", "Fechar as sessões resolvidas", Some("As que já foram marcadas como prontas"), S),
    info("next-pane", "Próximo painel", Some("Na ordem em que estão na tela"), N),
    info("prev-pane", "Painel anterior", None, N),
    info("next-session", "Próxima sessão", Some("Percorre a lateral inteira, painel aberto ou não"), N),
    info("prev-session", "Sessão anterior", None, N),
    info("search", "Buscar na lateral", Some("Foca o campo de busca; Esc limpa e devolve o teclado"), N),
    info("markdown", "Abrir um markdown…", Some("Escolhe um arquivo e o desenha num painel de leitura"), S),
    info("plan", "Ver o plano da sessão em foco", Some("Abre o último plano num painel de leitura, ao lado dela"), S),
    info("refresh", "Reler o git", Some("Worktrees e branches, sem esperar os dez segundos"), J),
    info("zoom-in", "Aumentar o corpo do painel", Some("Só o painel em foco; a base de todos está em aparência"), E),
    info("zoom-out", "Diminuir o corpo do painel", None, E),
    info(
        "zoom-reset",
        "Voltar o painel ao corpo base",
        Some("O 0 fica livre: do 1 ao 9 são as nove sessões da lateral"),
        E,
    ),
    info(
        "sidebar",
        "Esconder ou mostrar a lateral",
        Some("A aba na borda esquerda a traz de volta, e esta tecla também"),
        E,
    ),
    info("settings", "Configurações", Some("Esta tela"), J),
];

impl Action {
    pub const ALL: [Action; 20] = [
        Action::Claude,
        Action::Shell,
        Action::Task,
        Action::Rename,
        Action::Pin,
        Action::Close,
        Action::CloseSettled,
        Action::NextPane,
        Action::PrevPane,
        Action::NextSession,
        Action::PrevSession,
        Action::Search,
        Action::Markdown,
        Action::Plan,
        Action::Refresh,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
        Action::Sidebar,
        Action::Settings,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    fn info(self) -> &'static Info {
        &INFO[self.index()]
    }

    pub fn id(self) -> &'static str {
        self.info().id
    }

    pub fn label(self) -> &'static str {
        self.info().label
    }

    pub fn hint(self) -> Option<&'static str> {
        self.info().hint
    }

    pub fn group(self) -> &'static str {
        self.info().group
    }

    pub fn by_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    pub fn defaults(self, linux: bool) -> Vec<Chord> {
        if linux { linux_defaults(self) } else { mac_defaults(self) }
    }
}

fn c(ch: char) -> Key {
    Key::Char(ch)
}

fn ch(key: Key, ctrl: bool, shift: bool, alt: bool, meta: bool) -> Chord {
    Chord { key, meta, control: ctrl, alt, shift }
}

fn cmd(key: Key) -> Chord {
    ch(key, false, false, false, true)
}

fn shift_cmd(key: Key) -> Chord {
    ch(key, false, true, false, true)
}

fn alt_cmd(key: Key) -> Chord {
    ch(key, false, false, true, true)
}

fn ctrl(key: Key) -> Chord {
    ch(key, true, false, false, false)
}

fn ctrl_shift(key: Key) -> Chord {
    ch(key, true, true, false, false)
}

fn mac_defaults(a: Action) -> Vec<Chord> {
    match a {
        Action::Claude => vec![cmd(c('t'))],
        Action::Shell => vec![shift_cmd(c('t'))],
        Action::Task => vec![cmd(c('n'))],
        Action::Rename => vec![cmd(c('e'))],
        Action::Pin => vec![alt_cmd(c('p'))],
        Action::Close => vec![cmd(Key::Backspace)],
        Action::CloseSettled => vec![shift_cmd(c('k'))],
        Action::NextPane => vec![ctrl(Key::Tab), alt_cmd(Key::Down)],
        Action::PrevPane => vec![ctrl_shift(Key::Tab), alt_cmd(Key::Up)],
        Action::NextSession => vec![alt_cmd(Key::Right)],
        Action::PrevSession => vec![alt_cmd(Key::Left)],
        Action::Search => vec![cmd(c('f'))],
        Action::Markdown => vec![cmd(c('o'))],
        Action::Plan => vec![shift_cmd(c('l'))],
        Action::Refresh => vec![cmd(c('r'))],
        Action::ZoomIn => vec![cmd(c('=')), shift_cmd(c('='))],
        Action::ZoomOut => vec![cmd(c('-'))],
        Action::ZoomReset => vec![cmd(c('0'))],
        Action::Sidebar => vec![alt_cmd(c('s'))],
        Action::Settings => vec![cmd(c(',')), shift_cmd(c('p'))],
    }
}

fn linux_defaults(a: Action) -> Vec<Chord> {
    match a {
        Action::Claude => vec![ctrl_shift(c('t'))],
        Action::Shell => vec![ctrl_shift(Key::Enter)],
        Action::Task => vec![ctrl_shift(c('n'))],
        Action::Rename => vec![Chord::new(Key::F(2))],
        Action::Pin => vec![ch(c('p'), true, false, true, false)],
        Action::Close => vec![ctrl_shift(c('w'))],
        Action::CloseSettled => vec![ctrl_shift(c('k'))],
        Action::NextPane => vec![ctrl(Key::Tab), ctrl(Key::PageDown)],
        Action::PrevPane => vec![ctrl_shift(Key::Tab), ctrl(Key::PageUp)],
        Action::NextSession => vec![ctrl_shift(Key::Right)],
        Action::PrevSession => vec![ctrl_shift(Key::Left)],
        Action::Search => vec![ctrl_shift(c('f'))],
        Action::Markdown => vec![ctrl_shift(c('o'))],
        Action::Plan => vec![ctrl_shift(c('l'))],
        Action::Refresh => vec![ctrl_shift(c('r'))],
        Action::ZoomIn => vec![ctrl(c('=')), ctrl_shift(c('=')), ctrl_shift(c('+'))],
        Action::ZoomOut => vec![ctrl(c('-'))],
        Action::ZoomReset => vec![ctrl(c('0'))],
        Action::Sidebar => vec![ctrl_shift(c('b'))],
        Action::Settings => vec![ctrl(c(',')), ctrl_shift(c('p'))],
    }
}
