use std::path::Path;

use crate::Platform;
use crate::paths::dirname;
use crate::runner::CommandRunner;
use crate::shell::{q, q_operand};

pub fn encode_full(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &b in text.as_bytes() {
        let keep = b.is_ascii_alphanumeric() || b"!#$&'()*+,-./:;=?@[]_~".contains(&b);
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn show_items_command(path: &str) -> String {
    format!(
        "dbus-send --session --print-reply --dest=org.freedesktop.FileManager1 \
         /org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems \
         array:string:{} string:\"\"",
        q(&format!("file://{}", encode_full(path)))
    )
}

pub fn windows_path(path: &str) -> String {
    path.replace('/', "\\")
}

pub fn windows_select_command(path: &str) -> String {
    format!("MSYS_NO_PATHCONV=1 explorer.exe {}; true", q(&format!("/select,{}", windows_path(path))))
}

pub fn windows_open_command(target: &str) -> String {
    format!("MSYS_NO_PATHCONV=1 rundll32.exe url.dll,FileProtocolHandler {}", q(target))
}

pub async fn reveal(runner: &dyn CommandRunner, path: &str) -> bool {
    reveal_on(runner, Platform::current(), path).await
}

pub async fn reveal_on(runner: &dyn CommandRunner, platform: Platform, path: &str) -> bool {
    match platform {
        Platform::Linux => {
            if runner.run(show_items_command(path), None).await.ok() {
                return true;
            }
            runner.run(format!("xdg-open {}", q_operand(&dirname(path))), None).await.ok()
        }
        Platform::MacOs => runner.run(format!("open -R {}", q_operand(path)), None).await.ok(),
        Platform::Windows => runner.run(windows_select_command(path), None).await.ok(),
    }
}

pub async fn quick_look(runner: &dyn CommandRunner, path: &str) -> bool {
    quick_look_on(runner, Platform::current(), path).await
}

pub async fn quick_look_on(runner: &dyn CommandRunner, platform: Platform, path: &str) -> bool {
    if platform == Platform::Linux {
        if !Path::new(path).is_file() {
            return false;
        }
        if runner.run(format!("xdg-open {}", q_operand(path)), None).await.ok() {
            return true;
        }
    }
    reveal_on(runner, platform, path).await
}

pub async fn open_link(runner: &dyn CommandRunner, url: &str) -> bool {
    open_link_on(runner, Platform::current(), url).await
}

pub async fn open_link_on(runner: &dyn CommandRunner, platform: Platform, url: &str) -> bool {
    let opener = match platform {
        Platform::Linux => "xdg-open",
        Platform::MacOs => "open",
        Platform::Windows => return runner.run(windows_open_command(url), None).await.ok(),
    };
    runner.run(format!("{opener} {}", q_operand(url)), None).await.ok()
}
