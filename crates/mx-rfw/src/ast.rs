use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Library {
    pub imports: Vec<Import>,
    pub widgets: Vec<WidgetDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub name: Vec<String>,
}

impl Import {
    pub fn dotted(&self) -> String {
        self.name.join(".")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDecl {
    pub name: String,
    pub root: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Key(String),
    Index(i64),
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Part::Key(k) => f.write_str(k),
            Part::Index(i) => write!(f, "{i}"),
        }
    }
}

pub type Entries = Vec<(String, Expr)>;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Double(f64),
    Bool(bool),
    String(String),
    Map(Entries),
    List(Vec<Item>),
    Args(Vec<Part>),
    Data(Vec<Part>),
    LoopVar { depth: usize, parts: Vec<Part> },
    Switch(Box<Switch>),
    Event { name: String, args: Entries },
    Call { name: String, args: Entries },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Value(Expr),
    Loop { input: Expr, template: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Switch {
    pub input: Expr,
    pub cases: Vec<(Expr, Expr)>,
    pub default: Option<Expr>,
}
