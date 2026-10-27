use std::sync::{Arc, Mutex};

use crate::Platform;
use crate::runner::CommandRunner;
use crate::shell::q;

pub fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn alert_command(platform: Platform, title: &str, body: &str) -> String {
    alert_command_with_image(platform, title, body, None)
}

pub fn alert_command_with_image(platform: Platform, title: &str, body: &str, image: Option<&str>) -> String {
    match platform {
        Platform::Linux => {
            let icon = image.map(|i| format!("-i {} ", q(i))).unwrap_or_default();
            format!("notify-send -a maestria {icon}-- {} {}", q(title), q(body))
        }
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

    pub async fn alert_with_image(&self, title: &str, body: &str, image: &str) {
        self.runner.run(alert_command_with_image(self.platform, title, body, Some(image)), None).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_puts_the_portrait_before_the_end_of_options() {
        assert_eq!(
            alert_command_with_image(Platform::Linux, "login", "pergunta: qual?", Some("/p/rosto.png")),
            "notify-send -a maestria -i '/p/rosto.png' -- 'login' 'pergunta: qual?'"
        );
    }

    #[test]
    fn macos_falls_back_to_osascript_and_drops_the_portrait() {
        assert_eq!(
            alert_command_with_image(Platform::MacOs, "login", "x", Some("/p/rosto.png")),
            alert_command(Platform::MacOs, "login", "x")
        );
    }
}
