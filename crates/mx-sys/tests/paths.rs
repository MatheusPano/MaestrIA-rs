use mx_core::paths::expand_home_with;

const HOME: Option<&str> = Some("/home/eu");

#[test]
fn tilde_slash_becomes_the_home_of_whoever_runs_the_app() {
    assert_eq!(expand_home_with("~/repos/foo", HOME), "/home/eu/repos/foo");
    assert_eq!(expand_home_with("~/", HOME), "/home/eu/");
}

#[test]
fn bare_tilde_is_the_plain_home() {
    assert_eq!(expand_home_with("~", HOME), "/home/eu");
}

#[test]
fn tilde_other_user_stays_literal() {
    assert_eq!(expand_home_with("~outro/repos", HOME), "~outro/repos");
    assert_eq!(expand_home_with("~outro", HOME), "~outro");
}

#[test]
fn absolute_relative_and_empty_paths_pass_untouched() {
    for p in ["/Volumes/Externo/repos/foo", "repos/foo", "./foo", "", "/tmp/notas.md~"] {
        assert_eq!(expand_home_with(p, HOME), p);
    }
}

#[test]
fn unset_or_empty_home_leaves_the_path_alone() {
    assert_eq!(expand_home_with("~/x", None), "~/x");
    assert_eq!(expand_home_with("~/x", Some("")), "~/x");
    assert_eq!(expand_home_with("~/x", Some("/h/")), "/h/x");
}
