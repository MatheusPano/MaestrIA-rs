use crate::Platform;
use crate::runner::CommandRunner;
use crate::shell::{q, q_operand};

pub const MAC_APP: &str = "Visual Studio Code";

pub async fn open(runner: &dyn CommandRunner, path: &str, line: Option<u32>) -> bool {
    open_on(runner, Platform::current(), path, line).await
}

pub async fn open_on(
    runner: &dyn CommandRunner,
    platform: Platform,
    path: &str,
    line: Option<u32>,
) -> bool {
    let first = match line {
        None => format!("code -- {}", q(path)),
        Some(n) => format!("code -g -- {}", q(&format!("{path}:{n}"))),
    };
    if runner.run(first, None).await.ok() {
        return true;
    }
    match platform {
        Platform::Linux => {
            if runner.run(format!("code-insiders -- {}", q(path)), None).await.ok() {
                return true;
            }
            runner.run(format!("xdg-open {}", q_operand(path)), None).await.ok()
        }
        Platform::MacOs => {
            let cmd = format!("open -a {} {}", q(MAC_APP), q_operand(path));
            runner.run(cmd, None).await.ok()
        }
        Platform::Windows => {
            let Ok(meta) = tokio::fs::metadata(path).await else { return false };
            if meta.is_dir() || is_document(path) {
                let target = crate::reveal::windows_path(path);
                return runner.run(crate::reveal::windows_open_command(&target), None).await.ok();
            }
            runner.run(crate::reveal::windows_select_command(path), None).await;
            false
        }
    }
}

const DOCUMENTS: &[&str] = &[
    "bmp", "cfg", "code-workspace", "conf", "css", "csv", "diff", "docx", "gif", "go", "htm", "html", "ico", "ini",
    "jpeg", "jpg", "json", "jsonc", "log", "markdown", "md", "mp3", "mp4", "odt", "ods", "patch", "pdf", "png", "pptx",
    "rs", "rtf", "sql", "svg", "toml", "ts", "tsv", "txt", "wav", "webp", "xlsx", "xml", "yaml", "yml",
];

// Windows' default verb runs scripts and programs, so without VS Code only known documents are opened with it.
pub fn is_document(path: &str) -> bool {
    let name = mx_core::text::last_segment(path);
    if name.contains(':') || name.ends_with(['.', ' ']) {
        return false;
    }
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    DOCUMENTS.contains(&ext.to_ascii_lowercase().as_str())
}
