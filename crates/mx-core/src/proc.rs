use std::process::{Command, ExitStatus};

pub const PATH_LIST_SEP: char = if cfg!(windows) { ';' } else { ':' };

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

pub fn isolate(cmd: &mut Command) -> &mut Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
    }
}

pub fn hide_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(unix)]
pub fn kill_group(pid: u32, hard: bool) {
    use nix::sys::signal::{Signal, kill, killpg};
    use nix::unistd::Pid;
    let Ok(pid) = i32::try_from(pid) else { return };
    let pid = Pid::from_raw(pid);
    let sig = if hard { Signal::SIGKILL } else { Signal::SIGTERM };
    if killpg(pid, sig).is_err() {
        let _ = kill(pid, sig);
    }
}

// Console programs ignore a soft taskkill, so on Windows every kill is the hard one.
#[cfg(windows)]
pub fn kill_group(pid: u32, _hard: bool) {
    let mut cmd = Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
    hide_window(&mut cmd);
    let spawned = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if let Ok(mut child) = spawned {
        std::thread::spawn(move || child.wait());
    }
}

pub fn exit_code(status: &ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.code().or_else(|| status.signal().map(|s| -s)).unwrap_or(-1)
    }
    #[cfg(not(unix))]
    {
        status.code().unwrap_or(-1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_reads_a_plain_exit() {
        let mut cmd = if cfg!(windows) { Command::new("cmd") } else { Command::new("sh") };
        if cfg!(windows) {
            cmd.args(["/C", "exit 3"]);
        } else {
            cmd.args(["-c", "exit 3"]);
        }
        assert_eq!(exit_code(&cmd.status().unwrap()), 3);
    }

    #[cfg(unix)]
    #[test]
    fn exit_code_is_the_negated_signal_when_killed() {
        let mut child = isolate(&mut Command::new("sleep")).arg("30").spawn().unwrap();
        kill_group(child.id(), true);
        assert_eq!(exit_code(&child.wait().unwrap()), -9);
    }
}
