use std::cell::Cell;
use std::rc::Rc;

use crate::ast::{Entries, Expr, Item, Library, Part, Switch, WidgetDecl};
use crate::content::DynamicContent;
use crate::tree::{Event, LocalLibrary, Node, Widget};
use crate::value::Value;

pub const PLUGIN_LIBRARY: &str = "plugin";
const MAX_DEPTH: usize = 512;

static NO_ARGS: Entries = Vec::new();

pub fn evaluate(library: &Library, root: &str, data: &DynamicContent) -> Widget {
    if library.imports.iter().any(|i| i.dotted() == PLUGIN_LIBRARY) {
        return Widget::Error(format!("Library {PLUGIN_LIBRARY} depends on itself."));
    }
    let evaluator = Evaluator { library, data, depth: Cell::new(0) };
    evaluator.call(root, &NO_ARGS, &Env::default())
}

#[derive(Clone)]
enum Lazy<'a> {
    Ast(&'a Expr, Env<'a>),
    Args(Rc<ArgFrame<'a>>),
    Data(&'a Value),
    Missing,
}

#[derive(Clone, Default)]
struct Env<'a> {
    args: Option<Rc<ArgFrame<'a>>>,
    loops: Option<Rc<LoopFrame<'a>>>,
    chain: Option<Rc<Chain<'a>>>,
}

struct ArgFrame<'a> {
    args: &'a Entries,
    env: Env<'a>,
}

struct LoopFrame<'a> {
    value: Lazy<'a>,
    parent: Option<Rc<LoopFrame<'a>>>,
}

struct Chain<'a> {
    name: &'a str,
    parent: Option<Rc<Chain<'a>>>,
}

impl<'a> Env<'a> {
    fn with_loop(&self, value: Lazy<'a>) -> Self {
        Env {
            args: self.args.clone(),
            loops: Some(Rc::new(LoopFrame { value, parent: self.loops.clone() })),
            chain: self.chain.clone(),
        }
    }

    fn loop_at(&self, depth: usize) -> Lazy<'a> {
        let mut frame = self.loops.as_ref();
        for _ in 0..depth {
            frame = frame.and_then(|f| f.parent.as_ref());
        }
        frame.map_or(Lazy::Missing, |f| f.value.clone())
    }

    fn in_chain(&self, name: &str) -> bool {
        let mut link = self.chain.as_ref();
        while let Some(l) = link {
            if l.name == name {
                return true;
            }
            link = l.parent.as_ref();
        }
        false
    }
}

fn data_lazy(v: &Value) -> Lazy<'_> {
    if v.is_missing() { Lazy::Missing } else { Lazy::Data(v) }
}

fn literal(e: &Expr) -> Option<Value> {
    Some(match e {
        Expr::Int(v) => Value::Int(*v),
        Expr::Double(v) => Value::Double(*v),
        Expr::Bool(v) => Value::Bool(*v),
        Expr::String(v) => Value::String(v.clone()),
        _ => return None,
    })
}

enum Found<'a> {
    Remote(&'a WidgetDecl),
    Local(LocalLibrary),
    Absent(Vec<String>),
}

struct Evaluator<'a> {
    library: &'a Library,
    data: &'a DynamicContent,
    depth: Cell<usize>,
}

impl<'a> Evaluator<'a> {
    fn find(&self, name: &str) -> Found<'a> {
        if let Some(decl) = self.library.widgets.iter().find(|w| w.name == name) {
            return Found::Remote(decl);
        }
        let mut missing = Vec::new();
        for import in &self.library.imports {
            let dotted = import.dotted();
            match LocalLibrary::from_import(&dotted) {
                Some(lib) if lib.has(name) => return Found::Local(lib),
                Some(_) => {}
                None => missing.push(dotted),
            }
        }
        Found::Absent(missing)
    }

    fn call(&self, name: &str, args: &'a Entries, env: &Env<'a>) -> Widget {
        if self.depth.get() >= MAX_DEPTH {
            return Widget::Error(format!("Widget tree deeper than {MAX_DEPTH} levels at {PLUGIN_LIBRARY}:{name}."));
        }
        self.call_inner(name, args, env)
    }

    fn call_inner(&self, name: &str, args: &'a Entries, env: &Env<'a>) -> Widget {
        match self.find(name) {
            Found::Remote(decl) => {
                if env.in_chain(&decl.name) {
                    return Widget::Error(format!(
                        "Widget loop: Tried to call {PLUGIN_LIBRARY}:{name} constructor reentrantly."
                    ));
                }
                let body = Env {
                    args: Some(Rc::new(ArgFrame { args, env: env.clone() })),
                    loops: None,
                    chain: Some(Rc::new(Chain { name: &decl.name, parent: env.chain.clone() })),
                };
                match self.force(Lazy::Ast(&decl.root, body)) {
                    Value::Widget(w) => *w,
                    other => Widget::Error(format!(
                        "Switch in {PLUGIN_LIBRARY}:{name} did not resolve to a widget (got {other})."
                    )),
                }
            }
            Found::Local(library) => Widget::Node(Node {
                library,
                name: name.to_owned(),
                args: self.force_entries(args, env),
            }),
            Found::Absent(missing) if missing.is_empty() => {
                Widget::Error(format!("Could not find remote widget named {name} in {PLUGIN_LIBRARY}."))
            }
            Found::Absent(missing) => Widget::Error(format!(
                "Could not find remote widget named {name} in {PLUGIN_LIBRARY}, possibly because some dependencies were missing: {}",
                missing.join(", ")
            )),
        }
    }

    fn force_entries(&self, entries: &'a Entries, env: &Env<'a>) -> crate::value::Map {
        entries.iter().map(|(k, e)| (k.clone(), self.force(Lazy::Ast(e, env.clone())))).collect()
    }

    fn force(&self, lazy: Lazy<'a>) -> Value {
        self.depth.set(self.depth.get() + 1);
        let value = self.force_inner(lazy);
        self.depth.set(self.depth.get() - 1);
        value
    }

    fn force_inner(&self, lazy: Lazy<'a>) -> Value {
        match self.resolve(lazy, &[]) {
            Lazy::Missing => Value::Missing,
            Lazy::Data(v) => v.clone(),
            Lazy::Args(frame) => Value::Map(self.force_entries(frame.args, &frame.env)),
            Lazy::Ast(expr, env) => match expr {
                Expr::Map(entries) => Value::Map(self.force_entries(entries, &env)),
                Expr::List(items) => {
                    Value::List(self.expand(items, &env).into_iter().map(|l| self.force(l)).collect())
                }
                Expr::Event { name, args } => {
                    Value::Event(Box::new(Event { name: name.clone(), args: self.force_entries(args, &env) }))
                }
                Expr::Call { name, args } => Value::Widget(Box::new(self.call(name, args, &env))),
                other => literal(other).unwrap_or(Value::Missing),
            },
        }
    }

    fn expand(&self, items: &'a [Item], env: &Env<'a>) -> Vec<Lazy<'a>> {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            match item {
                Item::Value(e) => out.push(Lazy::Ast(e, env.clone())),
                Item::Loop { input, template } => {
                    let elements = match self.resolve(Lazy::Ast(input, env.clone()), &[]) {
                        Lazy::Ast(Expr::List(inner), inner_env) => self.expand(inner, &inner_env),
                        Lazy::Data(Value::List(l)) => l.iter().map(data_lazy).collect(),
                        _ => Vec::new(),
                    };
                    out.extend(elements.into_iter().map(|el| Lazy::Ast(template, env.with_loop(el))));
                }
            }
        }
        out
    }

    fn choose(&self, switch: &'a Switch, env: &Env<'a>) -> Option<&'a Expr> {
        let control = match self.resolve(Lazy::Ast(&switch.input, env.clone()), &[]) {
            Lazy::Data(v) => Some(v.clone()),
            Lazy::Ast(e, _) => literal(e),
            _ => None,
        };
        if let Some(control) = control {
            for (key, value) in &switch.cases {
                if literal(key).is_some_and(|k| control.matches_case(&k)) {
                    return Some(value);
                }
            }
        }
        switch.default.as_ref()
    }

    fn resolve(&self, start: Lazy<'a>, parts: &[Part]) -> Lazy<'a> {
        let mut current = start;
        let mut parts: Vec<Part> = parts.to_vec();
        let mut index = 0;
        loop {
            let part = parts.get(index);
            current = match current {
                Lazy::Missing => return Lazy::Missing,
                Lazy::Ast(expr, env) => match expr {
                    Expr::Data(p) => {
                        let full: Vec<Part> = p.iter().chain(&parts[index..]).cloned().collect();
                        index = parts.len();
                        data_lazy(self.data.lookup(&full))
                    }
                    Expr::Args(p) => {
                        let Some(frame) = env.args.clone() else { return Lazy::Missing };
                        parts = p.iter().chain(&parts[index..]).cloned().collect();
                        index = 0;
                        Lazy::Args(frame)
                    }
                    Expr::LoopVar { depth, parts: p } => {
                        parts = p.iter().chain(&parts[index..]).cloned().collect();
                        index = 0;
                        env.loop_at(*depth)
                    }
                    Expr::Switch(sw) => match self.choose(sw, &env) {
                        Some(e) => Lazy::Ast(e, env),
                        None => return Lazy::Missing,
                    },
                    _ if part.is_none() => return Lazy::Ast(expr, env),
                    Expr::Map(entries) => {
                        index += 1;
                        match part {
                            Some(Part::Key(k)) => match entries.iter().find(|(ek, _)| ek == k) {
                                Some((_, e)) => Lazy::Ast(e, env),
                                None => return Lazy::Missing,
                            },
                            _ => return Lazy::Missing,
                        }
                    }
                    Expr::List(items) => {
                        index += 1;
                        match part.and_then(|p| if let Part::Index(i) = p { usize::try_from(*i).ok() } else { None }) {
                            Some(i) => match self.expand(items, &env).into_iter().nth(i) {
                                Some(l) => l,
                                None => return Lazy::Missing,
                            },
                            None => return Lazy::Missing,
                        }
                    }
                    _ => return Lazy::Missing,
                },
                Lazy::Args(frame) => {
                    let Some(part) = part else { return Lazy::Args(frame) };
                    index += 1;
                    match part {
                        Part::Key(k) => match frame.args.iter().find(|(ek, _)| ek == k) {
                            Some((_, e)) => Lazy::Ast(e, frame.env.clone()),
                            None => return Lazy::Missing,
                        },
                        Part::Index(_) => return Lazy::Missing,
                    }
                }
                Lazy::Data(v) => {
                    let Some(part) = part else { return Lazy::Data(v) };
                    index += 1;
                    match (v, part) {
                        (Value::Map(m), Part::Key(k)) => m.get(k).map_or(Lazy::Missing, data_lazy),
                        (Value::List(l), Part::Index(i)) => {
                            usize::try_from(*i).ok().and_then(|i| l.get(i)).map_or(Lazy::Missing, data_lazy)
                        }
                        _ => return Lazy::Missing,
                    }
                }
            };
        }
    }
}
