use std::time::Duration;

#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process, kill_process_group};

pub const GRACE: Duration = Duration::from_secs(3);

#[cfg(windows)]
fn signal(pid: u32) -> bool {
    if pid <= 4 {
        return false;
    }
    mx_core::proc::kill_group(pid, true);
    true
}

#[cfg(windows)]
pub fn hang_up(pid: u32) -> bool {
    signal(pid)
}

#[cfg(windows)]
pub fn force_kill(pid: u32) -> bool {
    signal(pid)
}

#[cfg(unix)]
fn signal(pid: u32, sig: Signal) -> bool {
    let Some(pid) = i32::try_from(pid).ok().filter(|p| *p > 1).and_then(Pid::from_raw) else {
        return false;
    };
    kill_process_group(pid, sig).is_ok() || kill_process(pid, sig).is_ok()
}

#[cfg(unix)]
pub fn hang_up(pid: u32) -> bool {
    signal(pid, Signal::HUP)
}

#[cfg(unix)]
pub fn force_kill(pid: u32) -> bool {
    signal(pid, Signal::KILL)
}

#[cfg(unix)]
pub fn exit_code(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.code().or_else(|| status.signal().map(|s| -s))
}

#[cfg(unix)]
pub fn idle_shell(pty: impl std::os::fd::AsFd, pid: u32) -> bool {
    let Ok(group) = rustix::termios::tcgetpgrp(pty) else { return false };
    Pid::as_raw(Some(group)) as u32 == pid && shell_without_children(pid)
}

#[cfg(target_os = "linux")]
fn shell_without_children(pid: u32) -> bool {
    const SHELLS: [&str; 6] = ["zsh", "bash", "fish", "sh", "dash", "ksh"];
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
    let children = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap_or_else(|_| "?".into());
    SHELLS.contains(&comm.trim()) && children.trim().is_empty()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn shell_without_children(_: u32) -> bool {
    true
}

#[cfg(unix)]
pub fn redraw(pty: impl std::os::fd::AsFd) {
    if let Ok(group) = rustix::termios::tcgetpgrp(pty) {
        let _ = kill_process_group(group, Signal::WINCH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_signals_init_or_zero() {
        assert!(!hang_up(0));
        assert!(!hang_up(1));
        assert!(!force_kill(1));
    }
}
