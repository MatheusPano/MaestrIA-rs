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
    }
}
