use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    AnyElement, AnyView, App, AppContext, Context, Entity, Hsla, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, prelude::*, px,
};
use mx_core::json::Obj;
use mx_plugins::ui::blocks::Block;
use mx_plugins::ui::calendar::Calendar;
use mx_plugins::ui::console::PluginConsole;
use mx_plugins::ui::fields::FieldState;
use mx_theme::{Theme, Typography};

use super::menu::{self, HasMenu, OpenMenu};
use super::style::hsla;
use super::text_input::{InputColors, TextInput};
use super::view::text_root;
use super::{ActionSink, ViewAction, blocks_fields, blocks_list, blocks_misc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlocksMode {
    Framed,
    Bare,
    Modal,
}

pub type TerminalSlot = Box<dyn Fn(&str, &mut App) -> Option<AnyView>>;

pub struct BlocksView {
    raw: Vec<Obj>,
    pub(super) blocks: Vec<Block>,
    pub(super) fields: FieldState,
    pub(super) inputs: HashMap<String, Entity<TextInput>>,
    pub(super) calendars: HashMap<String, Calendar>,
    pub(super) consoles: BTreeMap<String, PluginConsole>,
    pub(super) console_scroll: HashMap<String, (ScrollHandle, u64)>,
    pub(super) focused: HashSet<String>,
    pub(super) input_texts: HashMap<String, String>,
    calendar_to: HashMap<String, Option<chrono::NaiveDate>>,
    pub(super) theme: Arc<Theme>,
    pub(super) dir: Option<PathBuf>,
    pub(super) mode: BlocksMode,
    pub(super) zoom: f32,
    pub(super) terminal: Option<TerminalSlot>,
    sink: ActionSink,
    scroll: ScrollHandle,
    pub(super) menu: Option<OpenMenu>,
    hoisted_menu: bool,
}

impl HasMenu for BlocksView {
    fn menu_slot(&mut self) -> &mut Option<OpenMenu> {
        &mut self.menu
    }
}

pub struct Paint<'a> {
    pub view: &'a BlocksView,
    pub ids: u64,
    pub surface: Hsla,
}

impl Paint<'_> {
    pub fn id(&mut self, name: &'static str) -> gpui::ElementId {
        self.ids += 1;
        gpui::ElementId::NamedInteger(name.into(), self.ids)
    }

    pub fn size(&self) -> f32 {
        13.5 + self.view.zoom - if self.view.mode == BlocksMode::Bare { 1.0 } else { 0.0 }
    }

    pub fn bare(&self) -> bool {
        self.view.mode == BlocksMode::Bare
    }
}

impl BlocksView {
    pub fn new(theme: Arc<Theme>, _: &Typography, dir: Option<PathBuf>, mode: BlocksMode, sink: ActionSink) -> Self {
        Self {
            raw: Vec::new(),
            blocks: Vec::new(),
            fields: FieldState::default(),
            inputs: HashMap::new(),
            calendars: HashMap::new(),
            consoles: BTreeMap::new(),
            console_scroll: HashMap::new(),
            focused: HashSet::new(),
            input_texts: HashMap::new(),
            calendar_to: HashMap::new(),
            theme,
            dir,
            mode,
            zoom: 0.0,
            terminal: None,
            sink,
            scroll: ScrollHandle::new(),
            menu: None,
            hoisted_menu: false,
        }
    }

    pub fn set_theme(&mut self, theme: Arc<Theme>) {
        self.theme = theme;
    }

    pub fn hoist_menu(&mut self) {
        self.hoisted_menu = true;
    }

    pub fn menu_overlay(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = self.theme.clone();
        self.menu.as_ref().map(|m| menu::floating(&theme, cx, m).into_any_element())
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
    }

    pub fn set_consoles(&mut self, consoles: BTreeMap<String, PluginConsole>) {
        self.consoles = consoles;
    }

    pub fn set_terminal_slot(&mut self, slot: Option<TerminalSlot>) {
        self.terminal = slot;
    }

    pub fn set_blocks(&mut self, raw: Vec<Obj>, cx: &mut Context<Self>) {
        self.pull_texts(cx);
        self.fields.sync(&raw);
        self.blocks = Block::parse_all(&raw);
        self.raw = raw;
        self.adopt_inputs(cx);
        cx.notify();
    }

    fn pull_texts(&mut self, cx: &App) {
        for (id, input) in &self.inputs {
            let text = input.read(cx).text.clone();
            self.fields.set_text(id, &text);
        }
    }

    fn adopt_inputs(&mut self, cx: &mut Context<Self>) {
        let mut seen = Vec::new();
        let colors = self.input_colors();
        visit(&self.blocks, &mut |b| {
            if let Block::Input { id, .. } = b {
                seen.push(id.clone());
            }
        });
        self.inputs.retain(|id, _| seen.contains(id));
        for id in seen {
            let text = self.fields.text(&id).unwrap_or_default().to_owned();
            match self.inputs.get(&id) {
                Some(e) => e.update(cx, |i, cx| {
                    if i.text != text {
                        i.text = text;
                        cx.notify();
                    }
                }),
                None => {
                    let e = cx.new(|cx| TextInput::new(text, colors, cx));
                    self.inputs.insert(id, e);
                }
            }
        }
    }

    pub(super) fn input_colors(&self) -> InputColors {
        InputColors { fg: hsla(self.theme.fg), faint: hsla(self.theme.fg_faint), caret: hsla(self.theme.accent) }
    }

    pub fn act(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_texts(cx);
        let values = self.fields.values();
        (self.sink)(ViewAction { action: action.to_owned(), values }, window, cx);
    }

    fn prepare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let size = 13.5 + self.zoom - if self.mode == BlocksMode::Bare { 1.0 } else { 0.0 } - 0.5;
        let weak = cx.weak_entity();
        let mut inputs = Vec::new();
        let mut calendars = Vec::new();
        let mut consoles = Vec::new();
        visit(&self.blocks, &mut |b| match b {
            Block::Input { id, label, placeholder, multiline, submit, disabled, .. } => {
                let hint = (placeholder.clone(), label.is_some());
                inputs.push((id.clone(), hint, *multiline, submit.clone(), *disabled))
            }
            Block::Calendar(c) => calendars.push(c.clone()),
            Block::Console { id, follow, .. } => consoles.push((id.clone(), *follow)),
            _ => {}
        });
        self.focused.clear();
        self.input_texts.clear();
        for (id, (placeholder, labelled), multiline, submit, disabled) in inputs {
            let Some(e) = self.inputs.get(&id) else { continue };
            let focused = e.read(cx).focused(window);
            let placeholder = placeholder.filter(|_| focused || !labelled);
            let submit = submit.filter(|_| !multiline).map(|a| {
                let weak = weak.clone();
                Rc::new(move |window: &mut Window, app: &mut App| {
                    weak.update(app, |v, cx| v.act(&a, window, cx)).ok();
                }) as super::text_input::Submit
            });
            e.update(cx, |i, _| {
                i.placeholder = placeholder;
                i.multiline = multiline;
                i.disabled = disabled;
                i.size = size;
                i.submit = submit;
            });
            if focused {
                self.focused.insert(id.clone());
            }
            self.input_texts.insert(id, e.read(cx).text.clone());
        }
        let today = chrono::Local::now().date_naive();
        self.calendars.retain(|id, _| calendars.iter().any(|c| &c.id == id));
        for c in calendars {
            let (_, to) = mx_plugins::ui::calendar::selection(self.fields.pick(&c.id));
            let cal = self.calendars.entry(c.id.clone()).or_insert_with(|| {
                Calendar::new(c.range, to, c.min_day(today), c.max_day(today), today)
            });
            if self.calendar_to.get(&c.id) != Some(&to) {
                cal.follow(to);
                self.calendar_to.insert(c.id.clone(), to);
            }
        }
        self.console_scroll.retain(|id, _| consoles.iter().any(|(c, _)| c == id));
        for (id, follow) in consoles {
            let revision = self.consoles.get(&id).map_or(0, |c| c.revision);
            let (handle, seen) = self.console_scroll.entry(id).or_insert_with(|| (ScrollHandle::new(), 0));
            if *seen != revision {
                let at_bottom = handle.max_offset().height + handle.offset().y <= px(24.0);
                if follow && (*seen == 0 || at_bottom) {
                    handle.scroll_to_bottom();
                }
                *seen = revision;
            }
        }
    }

    pub(super) fn surface(&self) -> Hsla {
        hsla(if self.mode == BlocksMode::Bare { self.theme.bg_sidebar } else { self.theme.bg })
    }
}

fn visit(blocks: &[Block], f: &mut impl FnMut(&Block)) {
    for b in blocks {
        f(b);
        match b {
            Block::Row { children, .. } => children.iter().for_each(|(c, _)| visit(std::slice::from_ref(c), f)),
            Block::Card { children, .. } | Block::Columns { children } => visit(children, f),
            _ => {}
        }
    }
}

pub fn block(p: &mut Paint, b: &Block, last: bool, cx: &mut Context<BlocksView>) -> AnyElement {
    let el = match b {
        Block::List { .. } => blocks_list::list(p, b, cx),
        Block::Input { .. } | Block::Checkbox { .. } | Block::Select { .. } | Block::Calendar(_) => {
            blocks_fields::field(p, b, cx)
        }
        _ => blocks_misc::render(p, b, cx),
    };
    if last {
        return el;
    }
    div().pb(px(if p.bare() { 10.0 } else { 12.0 })).child(el).into_any_element()
}

fn expanding(blocks: &[Block]) -> Option<usize> {
    blocks.iter().position(|b| {
        matches!(b, Block::Console { expand: true, .. } | Block::Terminal { expand: true, .. })
    })
}

fn column(p: &mut Paint, blocks: &[Block], cx: &mut Context<BlocksView>) -> gpui::Div {
    let mut col = div().flex().flex_col();
    for b in blocks {
        col = col.child(block(p, b, false, cx));
    }
    col
}

impl Render for BlocksView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare(window, cx);
        let theme = self.theme.clone();
        let mut p = Paint { view: self, ids: 0, surface: self.surface() };
        let root = text_root(div(), &theme).size_full().flex().flex_col().bg(self.surface());
        let body = if let Some(at) = expanding(&self.blocks) {
            let fill = blocks_misc::expanded(&mut p, &self.blocks[at], cx);
            let mut col = div().size_full().flex().flex_col().px(px(14.0)).py(px(12.0));
            col = col.child(column(&mut p, &self.blocks[..at], cx)).child(div().flex_1().min_h(px(0.0)).flex().flex_col().child(fill));
            if at + 1 < self.blocks.len() {
                col = col.child(column(&mut p, &self.blocks[at + 1..], cx).pt(px(12.0)));
            }
            col.into_any_element()
        } else if self.blocks.is_empty() && self.mode == BlocksMode::Framed {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(hsla(theme.fg_faint))
                .child(mx_plugins::ui::view::EMPTY_VIEW)
                .into_any_element()
        } else {
            let content = column(&mut p, &self.blocks, cx);
            let scroller = div().id("blocks-scroll").size_full().overflow_y_scroll().track_scroll(&self.scroll);
            match self.mode {
                BlocksMode::Bare => scroller.child(content.pt(px(12.0)).px(px(14.0)).pb(px(20.0))).into_any_element(),
                BlocksMode::Framed | BlocksMode::Modal => scroller
                    .child(
                        div()
                            .flex()
                            .justify_center()
                            .pt(px(18.0))
                            .px(px(22.0))
                            .pb(px(32.0))
                            .child(content.w_full().min_w(px(0.0)).max_w(px(760.0))),
                    )
                    .into_any_element(),
            }
        };
        let overlay = self.menu.as_ref().filter(|_| !self.hoisted_menu).map(|m| menu::render(&theme, cx, m));
        root.child(body).children(overlay)
    }
}

pub fn open_demo(skip: usize, cx: &mut App) {
    let raw: serde_json::Value = serde_json::from_str(DEMO_BLOCKS).unwrap_or_default();
    let blocks: Vec<Obj> =
        raw.as_array().into_iter().flatten().skip(skip).filter_map(|b| b.as_object().cloned()).collect();
    let options = super::demo::window_options("blocks", 760.0, 900.0, cx);
    let theme = mx_theme::default_theme().clone();
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|cx| {
            let mut v = BlocksView::new(theme, &Typography::standard(), None, BlocksMode::Framed, super::demo::logging_sink());
            let mut console = PluginConsole::default();
            console.append(&serde_json::json!(["$ flutter run", {"text": "Launching lib/main.dart", "tone": "dim"},
                {"spans": [{"text": "✓ ", "tone": "green", "bold": true}, "Built build/app.apk"]}, {"text": "erro: x", "tone": "red"}]));
            v.consoles.insert("log".into(), console);
            v.set_blocks(blocks, cx);
            v
        })
    });
    if let Err(err) = opened {
        log::error!("failed to open blocks demo: {err:#}");
        cx.quit();
    }
}

const DEMO_BLOCKS: &str = include_str!("demo_blocks.json");
