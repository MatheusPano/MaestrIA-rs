use serde_json::{Map, Value};

pub type Obj = Map<String, Value>;

pub enum Got<T> {
    Absent,
    Value(T),
    WrongType,
}

impl<T> Got<T> {
    pub fn typed(self) -> Option<Option<T>> {
        match self {
            Got::Absent => Some(None),
            Got::Value(v) => Some(Some(v)),
            Got::WrongType => None,
        }
    }

    pub fn lenient(self) -> Option<T> {
        match self {
            Got::Value(v) => Some(v),
            _ => None,
        }
    }
}

pub fn get_str<'a>(m: &'a Obj, key: &str) -> Got<&'a str> {
    match m.get(key) {
        None | Some(Value::Null) => Got::Absent,
        Some(Value::String(s)) => Got::Value(s),
        Some(_) => Got::WrongType,
    }
}

pub fn get_bool(m: &Obj, key: &str) -> Got<bool> {
    match m.get(key) {
        None | Some(Value::Null) => Got::Absent,
        Some(Value::Bool(b)) => Got::Value(*b),
        Some(_) => Got::WrongType,
    }
}

pub fn get_num(m: &Obj, key: &str) -> Got<f64> {
    match m.get(key) {
        None | Some(Value::Null) => Got::Absent,
        Some(Value::Number(n)) => n.as_f64().map_or(Got::WrongType, Got::Value),
        Some(_) => Got::WrongType,
    }
}

pub fn str_of<'a>(m: &'a Obj, key: &str) -> Option<&'a str> {
    get_str(m, key).lenient()
}

pub fn is_true(m: &Obj, key: &str) -> bool {
    matches!(m.get(key), Some(Value::Bool(true)))
}

pub fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

pub fn objects(v: Option<&Value>) -> Vec<&Obj> {
    match v {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_object).collect(),
        _ => Vec::new(),
    }
}

pub fn num_to_int(n: f64) -> i64 {
    n.trunc() as i64
}

pub fn f64_value(n: f64) -> Value {
    serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number)
}
