use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextUsage {
    pub percent: i64,
    pub tokens: i64,
    pub window: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipLevel {
    Calm,
    Warn,
    Danger,
}

pub(crate) fn dart_round(v: f64) -> i64 {
    v.round() as i64
}

impl ContextUsage {
    pub fn from_status_line(json: &Value) -> Option<Self> {
        let window = json.as_object()?.get("context_window")?.as_object()?;
        let percent = window.get("used_percentage")?.as_f64()?;
        let size = window.get("context_window_size")?.as_f64()?;
        let usage = window.get("current_usage")?.as_object()?;
        if size <= 0.0 {
            return None;
        }
        let count = |key: &str| usage.get(key).and_then(Value::as_f64).unwrap_or(0.0);
        let tokens = count("input_tokens")
            + count("cache_creation_input_tokens")
            + count("cache_read_input_tokens");
        Some(Self {
            percent: dart_round(percent),
            tokens: dart_round(tokens),
            window: dart_round(size),
        })
    }

    pub fn chip_filled(&self) -> usize {
        ((self.percent as f64) / 20.0).ceil().clamp(0.0, 5.0) as usize
    }

    pub fn chip_level(&self) -> ChipLevel {
        match self.percent {
            p if p >= 85 => ChipLevel::Danger,
            p if p >= 60 => ChipLevel::Warn,
            _ => ChipLevel::Calm,
        }
    }

    pub fn chip_text(&self) -> String {
        format!("{}%", self.percent)
    }

    pub fn chip_tooltip(&self) -> String {
        format!(
            "{} de {} tokens",
            format_tokens(self.tokens),
            format_tokens(self.window)
        )
    }
}

pub fn format_tokens(n: i64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let thousands = dart_round(n as f64 / 1000.0);
    if thousands < 1000 {
        return format!("{thousands} mil");
    }
    let tenths = dart_round(n as f64 / 100_000.0);
    if tenths % 10 == 0 {
        format!("{} mi", tenths / 10)
    } else {
        format!("{},{} mi", tenths / 10, tenths % 10)
    }
}
