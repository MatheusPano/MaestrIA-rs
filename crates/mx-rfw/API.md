# mx-rfw — API

RFW 1.1.4 text format (spec 04 §3.4/§3.5), toolkit-independent: parse → evaluate → typed decoders.
No UI dependency; the renderer (mx-app) matches on `widgets::Local` and draws.

## Parse
- `parse_library_file(&str) -> Result<ast::Library, ParseError>` — `.rfwtxt` library (imports + widget decls).
  Exact rfw tokenizer: only space/LF are whitespace (TAB/CR outside strings fail), `//` glued to a token fails,
  `1.`/`+1`/`e+3` fail, hex is positive i64, `12` is `Int`, `12.0`/`1e3` are `Double`.
- `parse_data_file(&str) -> Result<Map, ParseError>` — literal-only data file.
- `ParseError { kind: Syntax|Unsupported|TooDeep, message, line, column }`; `Display` = `"<msg> at line L column C."`.
  `Unsupported`: stateful widgets, `state.*`, `set state`, widget builders. `TooDeep`: >200 nested values.
- `parse_error_text(&ParseError) -> String` — the host surface text `"a interface do plugin não leu:\n<e>"`.
- `lexer::{Lexer, Tok, Token}` — `Lexer::tokenize(&str)` (exposed for tests/tools).
- `ast::{Library, Import, WidgetDecl, Expr, Item, Switch, Part}` — the parsed tree.

## Data
- `Value` = `Missing | Null | Bool | Int(i64) | Double(f64) | String | List | Map | Widget(Box<Widget>) | Event(Box<Event>)`.
  Getters are strict like rfw `v<T>`: `as_int()` is `None` for a double and vice versa. `get(key)`, `at(i)` never
  panic (yield `Missing`). `Value::from_json` keeps JSON int vs double; `to_json()` drops `Missing`/widgets/events.
- `Map` = `IndexMap<String, Value>` (insertion order).
- `DynamicContent` — the `data` root. `merge_plugin_data(&serde_json::Map)` replaces per top-level key and skips
  `theme`; `set_theme(Value)` injects `data.theme` (ARGB ints + `dark` + `mono`); `update`, `update_all`, `lookup`.

## Evaluate
- `evaluate(&Library, root: &str, &DynamicContent) -> Widget` — resolves `data.*`, `args.*` (lazily, in the caller's
  scope), loop variables (De Bruijn), `...for` (non-lists expand to nothing), `switch` (numbers compare by value,
  `1` matches `1.0`; maps/lists never match; no match → `default` → `Missing`), and events (args deep-resolved).
  Lookup: own decls first, then imports in order (`core.widgets`, `core.material`, `maestria`). The library is
  always called `plugin` in messages (`PLUGIN_LIBRARY`).
- `Widget = Node(Node) | Error(String)`. Errors are rendered in place, with rfw's messages: unknown widget
  (`Could not find remote widget named X in plugin.` + missing deps), `Widget loop: …reentrantly.`,
  `Switch in plugin:X did not resolve to a widget (got …).`, depth cap (512) instead of a stack overflow.
- `Node { library: LocalLibrary, name, args: Map }` — only local widgets survive evaluation; child widgets live
  inside `args` as `Value::Widget`. Accessors (rfw `DataSource` semantics):
  `arg(key)`, `child(key) -> Child` (Err = "Not a widget at [key] (got …) for lib:Name."),
  `optional_child(key)`, `child_list(key)` (missing → empty; non-list → one error), `indexed_children(key)`
  (ListView/GridView), `handlers(key) -> Vec<&Event>` (single event or list of events; empty = disabled),
  `full_name()`. `Child<'a> = Result<&'a Widget, String>`. `Widget::walk` visits the whole tree.
- `LocalLibrary::{CoreWidgets, CoreMaterial, Maestria}` with `name()`, `widgets()`, `has()`.
- `Event { name, args }`; `values(&extra) -> serde_json::Map` = declared args then `extra` (extra wins, keeps
  position), missing dropped — this is `view.action.values`; `name` is `view.action.action`.

## Decoders (`decoders::*`, all take the `&Value` at the key)
- `color -> Option<Color(u32 ARGB)>` (int only, masked to 32 bits); `edge_insets -> EdgeInsets{start,top,end,bottom}`
  (list of 1–4 doubles, `b??a, c??a, d??b??a`; bare number / ints → None); `alignment` (map `{x|start, y}`);
  `radius`, `border_radius` ([topStart, topEnd, bottomStart, bottomEnd]); `border_side` (black, 1.0, solid),
  `border` ([start, top, end, bottom]); `decoration -> BoxDecoration` (type `box` only); `box_shadow`, `offset`,
  `box_constraints`; `grid_delegate`; `font_weight`; `text_decoration`; `duration_ms` (default 200); `curve`
  (default `FastOutSlowIn`).
- `text_style(&Node, key)` — `overflow`/`textBaseline` read from the widget root (rfw bug kept).
- Enums with `decode(&Value)` / `from_name` (case-sensitive short names): `MainAxisAlignment`, `MainAxisSize`,
  `CrossAxisAlignment`, `TextDirection`, `VerticalDirection`, `TextBaseline`, `Clip`, `FlexFit`, `HitTestBehavior`,
  `Axis`, `StackFit`, `TextAlign`, `TextOverflow`, `WrapAlignment`, `WrapCrossAlignment`, `BorderStyle`, `BoxShape`,
  `FontStyle`, `TextDecorationStyle`, `TextLeadingDistribution`, `Curve`.

## Typed widgets (`widgets::*`)
- `decode(&Node) -> Local<'_>`: one variant per catalog widget with rfw defaults applied — core: Align, Center,
  Column/Row (`Flex`), Container, Expanded/Flexible, FractionallySizedBox, GestureDetector, GridView/ListView
  (`ScrollList`), IntrinsicHeight, Opacity (default **0.0**), Padding, Positioned (start/top/end/bottom only;
  `left`/`right` ignored), Rotation, SingleChildScrollView (primary default true), SizedBox, Spacer, Stack, Text
  (list text concatenated, default ""), Wrap; material: CircularProgressIndicator (`value: None` = spinning);
  maestria: Svg, Glyph, IconButton (`enabled()`), Tooltip, Pressable / MenuButton (`menu: Vec<MenuItem>`,
  `MenuItem::is_divider`, `shown_label`, submenu `color` int), Draggable (`draggable()`, `fits(id)`), DropTarget
  (`drop_extra(payload)` → `{payload, to}`), CodeEditor (`code_editor::CodeEditor`: doc, text, revision, language,
  line/column, jump, readOnly, wordWrap, fontSize (int or double, default 13), onChange/onSave/onCursor);
  anything else → `Local::Unsupported(&Node)`.
- `maestria::menu_extra(choice)` → `{action: choice}` for `onMenu`.
