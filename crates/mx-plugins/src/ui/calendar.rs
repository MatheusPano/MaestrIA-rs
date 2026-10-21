use chrono::{Datelike, NaiveDate};
use mx_core::json::Obj;
use serde_json::{Map, Value};

use super::blocks::s;

pub const MONTHS: [&str; 12] = [
    "janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro",
    "dezembro",
];
pub const WEEKDAYS: [&str; 7] = ["Segunda", "Terça", "Quarta", "Quinta", "Sexta", "Sábado", "Domingo"];
pub const INITIALS: [&str; 7] = ["D", "S", "T", "Q", "Q", "S", "S"];

pub fn day(v: Option<&Value>) -> Option<NaiveDate> {
    let raw = v?.as_str()?;
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .or_else(|| mx_core::text::parse_iso8601(raw).map(|t| t.date_naive()))
}

pub fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn short(d: NaiveDate) -> String {
    d.format("%d/%m").to_string()
}

fn month_of(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub label: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarBlock {
    pub id: String,
    pub range: bool,
    pub min: Option<String>,
    pub max: Option<String>,
    pub action: Option<String>,
    pub presets: Vec<Preset>,
}

impl CalendarBlock {
    pub fn parse(id: String, b: &Obj) -> Self {
        let presets = b
            .get("presets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .filter_map(|p| {
                let from = day(p.get("from"))?;
                let to = day(p.get("to").filter(|v| !v.is_null()).or(p.get("from")))?;
                Some(Preset { label: s(p, "label").unwrap_or_else(|| ymd(from)), from, to })
            })
            .collect();
        CalendarBlock {
            id,
            range: b.get("mode").and_then(Value::as_str) == Some("range"),
            min: s(b, "min"),
            max: s(b, "max"),
            action: s(b, "action"),
            presets,
        }
    }

    fn bound(raw: &Option<String>, today: NaiveDate) -> Option<NaiveDate> {
        match raw.as_deref() {
            Some("today") => Some(today),
            Some(other) => day(Some(&Value::from(other))),
            None => None,
        }
    }

    pub fn min_day(&self, today: NaiveDate) -> Option<NaiveDate> {
        Self::bound(&self.min, today)
    }

    pub fn max_day(&self, today: NaiveDate) -> Option<NaiveDate> {
        Self::bound(&self.max, today)
    }
}

pub fn selection(value: Option<&Value>) -> (Option<NaiveDate>, Option<NaiveDate>) {
    match value {
        Some(Value::Object(m)) if m.contains_key("from") && m.contains_key("to") => {
            (day(m.get("from")), day(m.get("to")))
        }
        Some(v @ Value::String(_)) => (day(Some(v)), day(Some(v))),
        _ => (None, None),
    }
}

pub fn value_of(range: bool, from: NaiveDate, to: NaiveDate) -> Value {
    if !range {
        return Value::from(ymd(from));
    }
    let mut m = Map::new();
    m.insert("from".into(), Value::from(ymd(from)));
    m.insert("to".into(), Value::from(ymd(to)));
    Value::Object(m)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calendar {
    pub range: bool,
    pub min: Option<NaiveDate>,
    pub max: Option<NaiveDate>,
    pub month: NaiveDate,
    pub anchor: Option<NaiveDate>,
}

impl Calendar {
    pub fn new(range: bool, to: Option<NaiveDate>, min: Option<NaiveDate>, max: Option<NaiveDate>, today: NaiveDate) -> Self {
        Calendar { range, min, max, month: month_of(to.or(max).unwrap_or(today)), anchor: None }
    }

    pub fn off(&self, d: NaiveDate) -> bool {
        self.max.is_some_and(|m| d > m) || self.min.is_some_and(|m| d < m)
    }

    pub fn tap(&mut self, d: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
        if self.off(d) {
            return None;
        }
        if !self.range {
            return Some((d, d));
        }
        match self.anchor.take() {
            None => {
                self.anchor = Some(d);
                Some((d, d))
            }
            Some(a) if d < a => Some((d, a)),
            Some(a) => Some((a, d)),
        }
    }

    pub fn preset(&mut self, p: &Preset) -> (NaiveDate, NaiveDate) {
        self.anchor = None;
        self.month = month_of(p.to);
        (p.from, p.to)
    }

    pub fn can_next(&self) -> bool {
        self.max.is_none_or(|m| month_of(m) > self.month)
    }

    pub fn can_prev(&self) -> bool {
        self.min.is_none_or(|m| month_of(m) < self.month)
    }

    pub fn next(&mut self) {
        if self.can_next() {
            self.month = self.month.checked_add_months(chrono::Months::new(1)).unwrap_or(self.month);
        }
    }

    pub fn prev(&mut self) {
        if self.can_prev() {
            self.month = self.month.checked_sub_months(chrono::Months::new(1)).unwrap_or(self.month);
        }
    }

    pub fn header(&self) -> String {
        let name = MONTHS[self.month.month0() as usize];
        let mut chars = name.chars();
        let cap: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
        format!("{cap} de {}", self.month.year())
    }

    pub fn follow(&mut self, to: Option<NaiveDate>) {
        if let Some(t) = to
            && self.anchor.is_none()
        {
            self.month = month_of(t);
        }
    }

    pub fn summary(&self, from: Option<NaiveDate>, to: Option<NaiveDate>) -> String {
        let (Some(f), Some(t)) = (from, to) else {
            return if self.range { "Escolha o primeiro dia".into() } else { "Escolha um dia".into() };
        };
        if self.anchor.is_some() {
            return format!("Começa em {} — clique no último dia", short(f));
        }
        if f == t {
            let wd = WEEKDAYS[f.weekday().num_days_from_monday() as usize];
            return format!("{wd}, {} de {}", f.day(), MONTHS[f.month0() as usize]);
        }
        let days = (t - f).num_days() + 1;
        format!("{} a {} · {days} dias", short(f), short(t))
    }
}
