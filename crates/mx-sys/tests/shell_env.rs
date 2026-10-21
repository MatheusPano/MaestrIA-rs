use std::path::Path;

use mx_sys::Platform;
use mx_sys::shell::{
    bin_dirs, early_exit_message, env, path_with_fallbacks, profile_file, q, resolve_shell, run,
};

const BARE: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

#[test]
fn an_existing_dir_missing_from_path_is_appended_at_the_end() {
    let path = path_with_fallbacks(BARE, &["/opt/claude/bin"], |_| true);
    assert_eq!(path, format!("{BARE}:/opt/claude/bin"));
}

#[test]
fn a_dir_that_does_not_exist_stays_out() {
    let path = path_with_fallbacks(BARE, &["/opt/claude/bin", "/opt/nao-instalado/bin"], |d| {
        d == "/opt/claude/bin"
    });
    assert_eq!(path, format!("{BARE}:/opt/claude/bin"));
}

#[test]
fn a_dir_already_in_path_is_not_added_again() {
    let inherited = format!("{BARE}:/opt/claude/bin");
    let path = path_with_fallbacks(&inherited, &["/opt/claude/bin", "/opt/claude/bin"], |_| true);
    assert_eq!(path, format!("{BARE}:/opt/claude/bin"));
}

#[test]
fn empty_path_still_yields_a_usable_path() {
    assert_eq!(path_with_fallbacks("", &["/opt/claude/bin"], |_| true), "/opt/claude/bin");
}

#[test]
fn sh_env_path_carries_every_existing_bin_dir_of_this_machine() {
    let home = std::env::var("HOME").unwrap_or_default();
    let path: Vec<&str> = env()["PATH"].split(':').collect();
    for dir in bin_dirs(&home, Platform::current()) {
        if Path::new(&dir).is_dir() {
            assert!(path.contains(&dir.as_str()), "{dir} missing from PATH");
        }
    }
}

#[test]
fn bin_dirs_are_home_based_first_and_linuxbrew_only_on_linux() {
    let linux = bin_dirs("/h", Platform::Linux);
    assert_eq!(linux[0], "/h/.local/bin");
    assert_eq!(linux[6], "/h/.cargo/bin");
    assert_eq!(linux.last().unwrap(), "/home/linuxbrew/.linuxbrew/bin");
    let mac = bin_dirs("", Platform::MacOs);
    assert_eq!(mac, ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "/usr/local/sbin"]);
}

#[test]
fn exit_127_is_claude_missing_from_path_and_says_where_to_put_it() {
    let message = early_exit_message("maestria_v2", Some(127), Platform::Linux);
    assert!(message.starts_with("maestria_v2: "));
    assert!(message.contains("PATH"));
    assert!(message.contains(profile_file(Platform::Linux)));
    assert!(message.contains("`claude`"));
}

#[test]
fn any_other_exit_still_asks_to_read_the_panel() {
    let one = early_exit_message("maestria_v2", Some(1), Platform::Linux);
    assert!(one.contains("código 1"));
    assert!(one.contains("abra o painel"));
    assert!(early_exit_message("maestria_v2", None, Platform::Linux).contains("código ?"));
}

#[test]
fn profile_file_is_zprofile_on_mac_and_profile_elsewhere() {
    assert_eq!(profile_file(Platform::MacOs), "~/.zprofile");
    assert_eq!(profile_file(Platform::Linux), "~/.profile");
}

#[test]
fn q_single_quotes_and_escapes_embedded_quotes() {
    assert_eq!(q("a b"), "'a b'");
    assert_eq!(q("it's"), r"'it'\''s'");
    assert_eq!(q(""), "''");
}

#[test]
fn mac_always_uses_zsh() {
    assert_eq!(resolve_shell(Platform::MacOs, Some("/bin/fish"), |_| true), "/bin/zsh");
}

#[test]
fn linux_prefers_an_existing_shell_env() {
    assert_eq!(resolve_shell(Platform::Linux, Some("/usr/bin/fish"), |_| true), "/usr/bin/fish");
}

#[test]
fn linux_falls_back_through_bash_and_zsh_then_sh() {
    let only_zsh = |p: &str| p == "/usr/bin/zsh";
    assert_eq!(resolve_shell(Platform::Linux, Some("/nope"), only_zsh), "/usr/bin/zsh");
    assert_eq!(resolve_shell(Platform::Linux, Some(""), |p| p == "/bin/bash"), "/bin/bash");
    assert_eq!(resolve_shell(Platform::Linux, None, |_| false), "/bin/sh");
}

#[tokio::test]
async fn run_trims_output_and_reports_the_exit_code() {
    let r = run("printf '  out \\n'; printf ' err \\n' >&2; exit 3", None).await;
    assert_eq!(r.code, 3);
    assert_eq!(r.stdout, "out");
    assert_eq!(r.stderr, "err");
    assert!(!r.ok());
}

#[tokio::test]
async fn run_uses_the_given_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let r = run("pwd -P", Some(dir.path().to_str().unwrap())).await;
    assert!(r.ok());
    assert_eq!(Path::new(&r.stdout), dir.path().canonicalize().unwrap());
}

#[tokio::test]
async fn spawn_failure_is_code_minus_one_with_the_message() {
    let r = run("true", Some("/nao/existe/mesmo")).await;
    assert_eq!(r.code, -1);
    assert_eq!(r.stdout, "");
    assert!(!r.stderr.is_empty());
}
