use std::sync::{Arc, Mutex};

use crate::Platform;
use crate::runner::CommandRunner;
use crate::shell::q;

pub fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn alert_command(platform: Platform, title: &str, body: &str) -> String {
    match platform {
        Platform::Linux => format!("notify-send -a maestria {} {}", q(title), q(body)),
        Platform::MacOs => {
            let script = format!(
                "display notification {} with title {}",
                applescript_string(body),
                applescript_string(title)
            );
            format!("osascript -e {}", q(&script))
        }
    }
}

pub struct Notifier {
    runner: Arc<dyn CommandRunner>,
    platform: Platform,
    last_badge: Mutex<Option<String>>,
}

impl Notifier {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self::on(runner, Platform::current())
    }

    pub fn on(runner: Arc<dyn CommandRunner>, platform: Platform) -> Self {
        Self { runner, platform, last_badge: Mutex::new(None) }
    }

    pub async fn alert(&self, title: &str, body: &str) {
        self.runner.run(alert_command(self.platform, title, body), None).await;
    }

    pub fn badge(&self, label: Option<&str>) -> Option<String> {
        let mut last = self.last_badge.lock().unwrap();
        if last.as_deref() == label {
            return None;
        }
        *last = label.map(str::to_owned);
        match self.platform {
            Platform::Linux => None,
            Platform::MacOs => Some(label.unwrap_or("").to_owned()),
        }
    }
}
