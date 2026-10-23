use gpui::{Div, Hsla, IntoElement, ParentElement, Styled, div, px, svg};

pub const ICON_FONT: &[u8] = include_bytes!("../../assets/fonts/MaestriaShellIcons.otf");
const FAMILY: &str = "Maestria Shell Icons";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    SpaceDashboard,
    Settings,
    Search,
    CloseRounded,
    Close,
    FilterList,
    FilterListOff,
    ChevronRight,
    ExpandMore,
    Hexagon,
    Folder,
    FolderOutlined,
    CallSplit,
    Add,
    MoreHoriz,
    TrackChanges,
    Bolt,
    StickyNote,
    Article,
    Tune,
    TaskAlt,
    Bedtime,
    NotificationsNone,
    Notifications,
    NotificationsOff,
    CreateNewFolder,
    History,
    Speed,
    ViewSidebar,
    GridView,
    LinkOff,
    PushPin,
    PushPinOutlined,
    Fingerprint,
    MyLocation,
    CheckCircle,
    CheckCircleOutline,
    Replay,
    PlayArrow,
    Check,
    ChevronRightRounded,
    PriorityHigh,
    QuestionMark,
    Lock,
    InfoOutline,
    Terminal,
}

impl Icon {
    pub fn code(self) -> u32 {
        match self {
            Icon::SpaceDashboard => 0xf3bd,
            Icon::Settings => 0xf36e,
            Icon::Search => 0xe567,
            Icon::CloseRounded => 0xf647,
            Icon::Close => 0xe16a,
            Icon::FilterList => 0xf75b,
            Icon::FilterListOff => 0xf031a,
            Icon::ChevronRight => 0xe15f,
            Icon::ExpandMore => 0xe246,
            Icon::Hexagon => 0xf0610,
            Icon::Folder => 0xf77e,
            Icon::FolderOutlined => 0xf091,
            Icon::CallSplit => 0xe12d,
            Icon::Add => 0xe047,
            Icon::MoreHoriz => 0xe402,
            Icon::TrackChanges => 0xe673,
            Icon::Bolt => 0xe0ee,
            Icon::StickyNote => 0xf3e8,
            Icon::Article => 0xee93,
            Icon::Tune => 0xe683,
            Icon::TaskAlt => 0xe646,
            Icon::Bedtime => 0xeecb,
            Icon::NotificationsNone => 0xf235,
            Icon::Notifications => 0xf237,
            Icon::NotificationsOff => 0xf236,
            Icon::CreateNewFolder => 0xef8c,
            Icon::History => 0xf102,
            Icon::Speed => 0xf3c3,
            Icon::ViewSidebar => 0xf49b,
            Icon::GridView => 0xf7c4,
            Icon::LinkOff => 0xe381,
            Icon::PushPin => 0xe4f4,
            Icon::PushPinOutlined => 0xf2d7,
            Icon::Fingerprint => 0xe287,
            Icon::MyLocation => 0xf1fe,
            Icon::CheckCircle => 0xe159,
            Icon::CheckCircleOutline => 0xe15a,
            Icon::Replay => 0xe523,
            Icon::PlayArrow => 0xf00a0,
            Icon::Check => 0xf636,
            Icon::ChevronRightRounded => 0xf63b,
            Icon::PriorityHigh => 0xf00c1,
            Icon::QuestionMark => 0xf036b,
            Icon::Lock => 0xf889,
            Icon::InfoOutline => 0xe33d,
            Icon::Terminal => 0xf0589,
        }
    }

    pub fn glyph(self) -> String {
        char::from_u32(self.code()).map(String::from).unwrap_or_default()
    }
}

pub fn icon(i: Icon, size: f32, color: Hsla) -> Div {
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .font_family(FAMILY)
        .text_size(px(size))
        .line_height(px(size))
        .text_color(color)
        .child(i.glyph())
}

pub fn claude_mark(size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/claude.svg")
        .flex_none()
        .size(px(size))
        .text_color(color)
}

pub fn terminal_mark(size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path("icons/terminal.svg")
        .flex_none()
        .size(px(size))
        .text_color(color)
}
