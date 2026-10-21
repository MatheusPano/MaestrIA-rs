# mx-theme — API pública

Dados puros (sem gpui). Deps: `serde_json`, `thiserror`.

## Cores
- `Color { a, r, g, b }` — ARGB 8 bits. `from_argb(u32)`, `opaque(rgb)`, `to_argb()`,
  `rgb() -> Rgb`, `with_alpha(f64)`, `red()/green()/blue()/alpha()` em 0..1,
  `luminance()` (fórmula do Flutter), `parse_hex(&str)` (`#RRGGBB`/`#AARRGGBB`, `#`
  opcional, trim), `to_hex()`. `Color::WHITE`. `From<Rgb>`/`Into<Rgb>`.
- `CLAUDE` — `#D97757`, nunca tematizada.
- `Oklch { l, c, hue }` — `from_color`, `to_color`, `to_srgb_unit` (matrizes Ottosson).

## Paleta e tema
- `Palette` — campos públicos `id, label, dark, canvas, bg, bg_sidebar, bg_hover,
  bg_active, border, fg, fg_dim, fg_faint, accent, green, yellow, red, purple, ansi`.
  Derivados: `shadow()`, `on_accent()`, `group_tints() -> [Color; 4]`,
  `group_tint(id)` (soma UTF-16 % 4), `terminal() -> TerminalPalette`.
  JSON: `Palette::from_json(&Value)` / `from_json_str(&str)` → `Result<_, ThemeError>`.
- `Ansi` — 8 normais + `bright: [Option<Color>; 8]`; `bright_black()…bright_white()`
  caem na normal; `normal()`, `bright(i)`, `all16()`.
- `Theme` — `Palette` imutável + as 13 tints calculadas uma vez. `Deref<Target = Palette>`,
  `Theme::new(palette)`, `palette()`, `tint(Tint) -> Color`, `tints()`.
- `ThemeError` — mensagens em português, verbatim do Dart (`Display`):
  `Syntax`, `NotAnObject` (`não é um objeto`), `MissingId`/`MissingLabel`/`MissingAnsi`
  (`"id" é obrigatório`…), `BadColor(key)` (`"ansi.cyan" precisa ser uma cor #RRGGBB`),
  `BuiltInId(id)` (`"x" é o id de um tema embutido`), `DuplicateId(id)`
  (`outro plugin já tem um tema "x"`).

## Embutidos e resolução
- `built_ins() -> &'static [Arc<Theme>]` — os 19, na ordem do picker.
- `built_in(id)`, `default_theme()` (`maestria`), `DEFAULT_THEME_ID`.
- `RETIRED_ALIASES`, `retired_alias(id)`, `is_built_in(id)` (embutido ou aposentado).
- `Themes` — registro com os temas de plugin (`extra`):
  `new()`, `built_ins()`, `extra()`, `by_id(Option<&str>) -> Arc<Theme>` (embutido ▸ plugin
  ▸ alias aposentado ▸ padrão; nunca falha), `register(Palette) -> Result<Arc<Theme>,
  ThemeError>` (recusa id embutido/aposentado e id repetido), `clear_extra()` (chamar antes
  de remontar a cada carga de plugins). Persistir só o id.

## Tints (cores de etiqueta)
- `Tint` — `Red, Orange, Yellow, Lime, Green, Aqua, Cyan, Petrol, Violet, Magenta, Pink,
  Brown, Grey`; `ALL`, `COUNT`, `name()` (o que vai pro config), `label()` (PT),
  `hue()`, `by_name(Option<&str>) -> Option<Tint>` (nome desconhecido → `None`).
  Cor: `theme.tint(t)` — fórmula OKLCH do Dart, idêntica à tabela da spec.

## Tipografia
- `Typography` — `mono()`, `size()`, `line()`; `standard()`/`Default` = Hack 13.0 / 1.2.
  `new(mono, size, line, installed)` normaliza: clamp 9–24 / 1.0–1.8, arredonda a 2
  casas, face não oferecida ou não instalada → Hack. `with(...)` = copyWith.
  `clamp_zoom(i64)`, `size_at(zoom)`, `to_json()` (só o que difere do padrão),
  `from_json(Option<&Value>, installed)` (campo de tipo errado é ignorado),
  `Display` = `"Hack 13.0px/1.2"`, `style_key(zoom) -> StyleKey`.
  Constantes `MIN_SIZE, MAX_SIZE, MIN_LINE, MAX_LINE, SIZE_STEP, LINE_STEP, STANDARD_*`.
- `installed: &dyn Fn(&str) -> bool` — quem chama faz a descoberta (fontconfig/CoreText).
- `Face { family, note, bundled }`, `HACK`, `FACES` (31, ordem da tela),
  `installed_faces(installed)`, `face_by_family(Option<&str>, installed)`,
  `PTY_FALLBACK` (cascata de glifos; a face escolhida vai antes).
- `StyleKey { mono, size(), line() }` e `StyleCache<T>` — `get_or_insert_with(key, make)
  -> Arc<T>`: mesma (face, corpo, entrelinha) devolve a mesma instância.

## Terminal
- `Rgb { r, g, b }` — `hex(u32)`, `to_u32()`, `scaled(f32)`.
- `TerminalPalette { foreground, background, cursor, ansi: [Rgb; 16], selection: Color,
  search_hit_background, search_hit_background_current, search_hit_foreground }` —
  `indexed(u8)` (16 ansi + cubo 6×6×6 + rampa de cinza), `maestria_dark()` = `Default`
  = `default_theme().terminal()`.
