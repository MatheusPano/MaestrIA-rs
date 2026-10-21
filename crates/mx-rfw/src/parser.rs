use crate::ast::{Entries, Expr, Import, Item, Library, Part, Switch, WidgetDecl};
use crate::error::{ParseError, ParseErrorKind, dart_double};
use crate::lexer::{Lexer, Tok, Token};
use crate::value::{Map, Value};

const MAX_DEPTH: usize = 200;
const RESERVED: [&str; 7] = ["args", "data", "event", "false", "set", "state", "true"];

pub fn parse_library_file(source: &str) -> Result<Library, ParseError> {
    let mut p = Parser::new(source)?;
    let mut imports = Vec::new();
    while p.found_ident("import") {
        imports.push(p.read_import()?);
    }
    let mut widgets = Vec::new();
    while p.found_ident("widget") {
        widgets.push(p.read_widget_decl()?);
    }
    if widgets.is_empty() {
        p.expect_eof("keywords \"import\" or \"widget\", or end of file")?;
    } else {
        p.expect_eof("keyword \"widget\" or end of file")?;
    }
    Ok(Library { imports, widgets })
}

pub fn parse_data_file(source: &str) -> Result<Map, ParseError> {
    let mut p = Parser::new(source)?;
    let entries = p.read_map(false)?;
    p.expect_eof("end of file")?;
    Ok(entries.into_iter().map(|(k, v)| (k, literal(v))).collect())
}

fn literal(e: Expr) -> Value {
    match e {
        Expr::Int(v) => Value::Int(v),
        Expr::Double(v) => Value::Double(v),
        Expr::Bool(v) => Value::Bool(v),
        Expr::String(v) => Value::String(v),
        Expr::Map(entries) => Value::Map(entries.into_iter().map(|(k, v)| (k, literal(v))).collect()),
        Expr::List(items) => Value::List(
            items
                .into_iter()
                .map(|i| match i {
                    Item::Value(v) => literal(v),
                    Item::Loop { .. } => Value::Missing,
                })
                .collect(),
        ),
        _ => Value::Missing,
    }
}

fn same_case_key(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Int(x), Expr::Int(y)) => x == y,
        (Expr::Double(x), Expr::Double(y)) => x == y,
        (Expr::Int(i), Expr::Double(d)) | (Expr::Double(d), Expr::Int(i)) => (*i as f64) == *d,
        (Expr::String(x), Expr::String(y)) => x == y,
        (Expr::Bool(x), Expr::Bool(y)) => x == y,
        _ => false,
    }
}

fn case_key_text(e: &Expr) -> String {
    match e {
        Expr::Int(v) => v.to_string(),
        Expr::Double(v) => dart_double(*v),
        Expr::Bool(v) => v.to_string(),
        Expr::String(v) => v.clone(),
        _ => String::new(),
    }
}

struct Parser {
    lexer: Lexer,
    current: Token,
    loops: Vec<String>,
    depth: usize,
}

impl Parser {
    fn new(source: &str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;
        Ok(Self { lexer, current, loops: Vec::new(), depth: 0 })
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError::syntax(message, self.current.line, self.current.column)
    }

    fn unsupported(&self, what: &str) -> ParseError {
        ParseError {
            kind: ParseErrorKind::Unsupported,
            message: format!("{what} are not supported by this host"),
            line: self.current.line,
            column: self.current.column,
        }
    }

    fn expected(&self, what: &str) -> ParseError {
        self.error(format!("Expected {what} but found {}", self.current.tok))
    }

    fn unexpected(&self) -> ParseError {
        self.error(format!("Unexpected {}", self.current.tok))
    }

    fn found_ident(&self, name: &str) -> bool {
        matches!(&self.current.tok, Tok::Ident(s) if s == name)
    }

    fn found_symbol(&self, c: char) -> bool {
        self.current.tok == Tok::Symbol(c)
    }

    fn at_symbol(&self) -> bool {
        matches!(self.current.tok, Tok::Symbol(_) | Tok::TripleDot)
    }

    fn expect_symbol(&mut self, c: char) -> Result<(), ParseError> {
        if !self.found_symbol(c) {
            return Err(self.expected(&format!("symbol \"{c}\"")));
        }
        self.advance()
    }

    fn expect_ident(&mut self, name: &str) -> Result<(), ParseError> {
        match &self.current.tok {
            Tok::Ident(s) if s == name => self.advance(),
            Tok::Ident(_) => Err(self.expected(name)),
            _ => Err(self.expected("identifier")),
        }
    }

    fn read_ident(&mut self) -> Result<String, ParseError> {
        let Tok::Ident(s) = &self.current.tok else { return Err(self.expected("identifier")) };
        let s = s.clone();
        self.advance()?;
        Ok(s)
    }

    fn read_string(&mut self) -> Result<String, ParseError> {
        let Tok::Str(s) = &self.current.tok else { return Err(self.expected("string")) };
        let s = s.clone();
        self.advance()?;
        Ok(s)
    }

    fn read_key(&mut self) -> Result<String, ParseError> {
        if matches!(self.current.tok, Tok::Ident(_)) { self.read_ident() } else { self.read_string() }
    }

    fn expect_eof(&mut self, what: &str) -> Result<(), ParseError> {
        if self.current.tok != Tok::Eof {
            return Err(self.expected(what));
        }
        Ok(())
    }

    fn read_import(&mut self) -> Result<Import, ParseError> {
        self.expect_ident("import")?;
        let mut name = vec![self.read_key()?];
        while self.found_symbol('.') {
            self.advance()?;
            name.push(self.read_key()?);
        }
        self.expect_symbol(';')?;
        Ok(Import { name })
    }

    fn read_widget_decl(&mut self) -> Result<WidgetDecl, ParseError> {
        self.expect_ident("widget")?;
        let name = self.read_ident()?;
        if self.found_symbol('{') {
            return Err(self.unsupported("Stateful widgets"));
        }
        self.expect_symbol('=')?;
        let root = if self.found_ident("switch") {
            self.advance()?;
            Expr::Switch(Box::new(self.read_switch()?))
        } else {
            self.read_call()?
        };
        self.expect_symbol(';')?;
        Ok(WidgetDecl { name, root })
    }

    fn read_call(&mut self) -> Result<Expr, ParseError> {
        let name = self.read_ident()?;
        self.expect_symbol('(')?;
        let args = self.read_map_body(true)?;
        self.expect_symbol(')')?;
        Ok(Expr::Call { name, args })
    }

    fn read_map(&mut self, extended: bool) -> Result<Entries, ParseError> {
        self.expect_symbol('{')?;
        let entries = self.read_map_body(extended)?;
        self.expect_symbol('}')?;
        Ok(entries)
    }

    fn read_map_body(&mut self, extended: bool) -> Result<Entries, ParseError> {
        let mut entries: Entries = Vec::new();
        while !self.at_symbol() {
            let key = self.read_key()?;
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(self.error(format!("Duplicate key \"{key}\" in map")));
            }
            self.expect_symbol(':')?;
            if let Some(value) = self.read_value(extended, true)? {
                entries.push((key, value));
            }
            if self.found_symbol(',') {
                self.advance()?;
            } else {
                break;
            }
        }
        Ok(entries)
    }

    fn read_list(&mut self, extended: bool) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();
        self.expect_symbol('[')?;
        while !self.found_symbol(']') {
            if extended && self.current.tok == Tok::TripleDot {
                self.advance()?;
                self.expect_ident("for")?;
                let name = self.read_ident_at_loop()?;
                self.expect_ident("in")?;
                let input = self.read_expr(true)?;
                self.expect_symbol(':')?;
                self.loops.push(name);
                let template = self.read_expr(extended);
                self.loops.pop();
                items.push(Item::Loop { input, template: template? });
            } else {
                items.push(Item::Value(self.read_expr(extended)?));
            }
            if self.found_symbol(',') {
                self.advance()?;
            } else if !self.found_symbol(']') {
                return Err(self.expected("comma"));
            }
        }
        self.expect_symbol(']')?;
        Ok(items)
    }

    fn read_ident_at_loop(&mut self) -> Result<String, ParseError> {
        let at = self.current.clone();
        let name = self.read_ident()?;
        if RESERVED.contains(&name.as_str()) {
            return Err(ParseError::syntax(format!("{name} is a reserved word"), at.line, at.column));
        }
        Ok(name)
    }

    fn read_switch(&mut self) -> Result<Switch, ParseError> {
        let input = self.read_expr(true)?;
        let mut cases: Vec<(Expr, Expr)> = Vec::new();
        let mut default = None;
        self.expect_symbol('{')?;
        while !self.at_symbol() {
            let key = if self.found_ident("default") {
                if default.is_some() {
                    return Err(self.error("Switch has multiple default cases"));
                }
                self.advance()?;
                None
            } else {
                let key = self.read_expr(true)?;
                if cases.iter().any(|(k, _)| same_case_key(k, &key)) {
                    return Err(self.error(format!("Switch has duplicate cases for key {}", case_key_text(&key))));
                }
                Some(key)
            };
            self.expect_symbol(':')?;
            let value = self.read_expr(true)?;
            match key {
                Some(key) => cases.push((key, value)),
                None => default = Some(value),
            }
            if self.found_symbol(',') {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_symbol('}')?;
        Ok(Switch { input, cases, default })
    }

    fn read_parts(&mut self, optional: bool) -> Result<Vec<Part>, ParseError> {
        let mut parts = Vec::new();
        if optional && !self.found_symbol('.') {
            return Ok(parts);
        }
        loop {
            self.expect_symbol('.')?;
            let part = match &self.current.tok {
                Tok::Int(i) => Part::Index(*i),
                Tok::Str(s) | Tok::Ident(s) => Part::Key(s.clone()),
                _ => return Err(self.unexpected()),
            };
            parts.push(part);
            self.advance()?;
            if !self.found_symbol('.') {
                return Ok(parts);
            }
        }
    }

    fn read_expr(&mut self, extended: bool) -> Result<Expr, ParseError> {
        match self.read_value(extended, false)? {
            Some(e) => Ok(e),
            None => Err(self.unexpected()),
        }
    }

    fn read_value(&mut self, extended: bool, null_ok: bool) -> Result<Option<Expr>, ParseError> {
        if self.depth >= MAX_DEPTH {
            return Err(ParseError {
                kind: ParseErrorKind::TooDeep,
                message: format!("Nesting deeper than {MAX_DEPTH} levels"),
                line: self.current.line,
                column: self.current.column,
            });
        }
        self.depth += 1;
        let result = self.read_value_inner(extended, null_ok);
        self.depth -= 1;
        result
    }

    fn read_value_inner(&mut self, extended: bool, null_ok: bool) -> Result<Option<Expr>, ParseError> {
        let simple = match &self.current.tok {
            Tok::Symbol('[') => return Ok(Some(Expr::List(self.read_list(extended)?))),
            Tok::Symbol('{') => return Ok(Some(Expr::Map(self.read_map(extended)?))),
            Tok::Symbol('(') => return Err(self.unsupported("Widget builders")),
            Tok::Int(v) => Expr::Int(*v),
            Tok::Double(v) => Expr::Double(*v),
            Tok::Str(s) => Expr::String(s.clone()),
            Tok::Ident(id) if id == "true" || id == "false" => Expr::Bool(id == "true"),
            Tok::Ident(id) if id == "null" && null_ok => {
                self.advance()?;
                return Ok(None);
            }
            Tok::Ident(_) if !extended => return Err(self.unexpected()),
            Tok::Ident(id) => {
                let id = id.clone();
                return self.read_identifier_value(&id).map(Some);
            }
            _ => return Err(self.unexpected()),
        };
        self.advance()?;
        Ok(Some(simple))
    }

    fn read_identifier_value(&mut self, id: &str) -> Result<Expr, ParseError> {
        match id {
            "event" => {
                self.advance()?;
                let name = self.read_string()?;
                let args = self.read_map(true)?;
                Ok(Expr::Event { name, args })
            }
            "args" => {
                self.advance()?;
                Ok(Expr::Args(self.read_parts(false)?))
            }
            "data" => {
                self.advance()?;
                Ok(Expr::Data(self.read_parts(false)?))
            }
            "state" => Err(self.unsupported("State references")),
            "switch" => {
                self.advance()?;
                Ok(Expr::Switch(Box::new(self.read_switch()?)))
            }
            "set" => Err(self.unsupported("set state handlers")),
            _ => match self.loops.iter().rposition(|l| l == id) {
                Some(pos) => {
                    self.advance()?;
                    let depth = self.loops.len() - 1 - pos;
                    Ok(Expr::LoopVar { depth, parts: self.read_parts(true)? })
                }
                None => self.read_call(),
            },
        }
    }
}
