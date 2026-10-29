use std::path::PathBuf;
use std::time::Duration;

use mx_sys::updater::{Release, Stage, Updater, expected_sum};
use serde_json::json;

fn release_json(with_sums: bool) -> serde_json::Value {
    let mut assets = vec![
        json!({"name": "maestria_2.5.0-1_amd64.deb", "browser_download_url": "https://x/deb"}),
        json!({"name": "maestria_2.5.0-1_macos.dmg", "browser_download_url": "https://x/dmg"}),
    ];
    if with_sums {
        assets.push(json!({"name": "SHA256SUMS", "browser_download_url": "https://x/sums"}));
    }
    json!({"tag_name": "v2.5.0", "assets": assets})
}

fn release(version: &str) -> Release {
    Release {
        version: version.into(),
        asset: format!("maestria-rs_{version}-1_amd64.deb"),
        asset_url: "https://x/deb".into(),
        sums_url: Some("https://x/sums".into()),
    }
}

fn updater(dir: &tempfile::TempDir, enabled: bool) -> Updater {
    Updater::with_enabled("3.0.0", dir.path(), enabled)
}

fn ready(u: &mut Updater, dir: &tempfile::TempDir) -> PathBuf {
    std::fs::create_dir_all(u.dir()).unwrap();
    let staged = u.dir().join("maestria-rs_3.1.0-1_amd64.deb");
    std::fs::write(&staged, b"deb").unwrap();
    u.set_etag(Some("\"abc\"".into()));
    assert!(u.begin_fetch(Some(&release("3.1.0"))));
    u.fetched(release("3.1.0"), Ok(staged.clone())).unwrap();
    let _ = dir;
    staged
}

#[test]
fn sha256sums_lookup_finds_text_and_binary_mode_lines() {
    let sums = format!("{}  maestria_2.5.0-1_amd64.deb\n{} *maestria_2.5.0-1_macos.dmg\n", "a".repeat(64), "B".repeat(64));
    assert_eq!(expected_sum(&sums, "maestria_2.5.0-1_amd64.deb"), Some("a".repeat(64)));
    assert_eq!(expected_sum(&sums, "maestria_2.5.0-1_macos.dmg"), Some("b".repeat(64)));
}

#[test]
fn sha256sums_lookup_misses_other_files_and_empty_sums() {
    let sums = format!("{}  maestria_2.5.0-1_amd64.deb\n", "a".repeat(64));
    assert_eq!(expected_sum(&sums, "maestria_2.5.0-1_amd64.deb.zip"), None);
    assert_eq!(expected_sum("", "x"), None);
}

#[test]
fn release_json_picks_the_asset_by_suffix_and_the_sums() {
    let r = Release::from_json(&release_json(true), "maestria_", "_macos.dmg").unwrap();
    assert_eq!(r.version, "2.5.0");
    assert_eq!(r.asset, "maestria_2.5.0-1_macos.dmg");
    assert_eq!(r.asset_url, "https://x/dmg");
    assert_eq!(r.sums_url.as_deref(), Some("https://x/sums"));
}

#[test]
fn release_json_without_a_matching_asset_is_none() {
    assert_eq!(Release::from_json(&release_json(true), "maestria_", "_arm64.deb"), None);
}

#[test]
fn release_json_without_sha256sums_still_parses_with_no_sums_url() {
    let r = Release::from_json(&release_json(false), "maestria_", "_amd64.deb").unwrap();
    assert_eq!(r.sums_url, None);
}

#[test]
fn the_flutter_deb_never_matches_the_maestria_rs_package() {
    assert_eq!(Release::from_json(&release_json(true), "maestria-rs_", "_amd64.deb"), None);
}

#[test]
fn an_updater_without_a_version_is_disabled() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!Updater::with_enabled("", dir.path(), true).enabled());
}

#[test]
fn a_dev_binary_outside_the_install_dir_is_disabled() {
    let dir = tempfile::tempdir().unwrap();
    if std::env::var_os(mx_sys::updater::ENABLED_ENV).is_none() {
        assert!(!Updater::new("3.0.0", dir.path()).enabled());
    }
}

#[test]
fn the_mac_bundle_is_the_app_folder_around_the_executable() {
    use mx_sys::updater::bundle_of;
    let exe = std::path::Path::new("/Users/a/Apps/maestria-rs.app/Contents/MacOS/maestria");
    assert_eq!(bundle_of(exe), Some(PathBuf::from("/Users/a/Apps/maestria-rs.app")));
    assert_eq!(bundle_of(std::path::Path::new("/opt/maestria-rs/maestria-rs")), None);
}

#[test]
fn the_asset_is_the_maestria_rs_package_for_this_platform() {
    let want = if cfg!(target_os = "macos") { "_macos.dmg" } else { "_amd64.deb" };
    assert_eq!(mx_sys::updater::ASSET_SUFFIX, want);
    assert_eq!(mx_sys::updater::PACKAGE_PREFIX, "maestria-rs_");
}

#[test]
fn linux_quits_into_a_relaunch_only_after_installing_on_request() {
    use mx_sys::updater::QuitPlan;
    if mx_sys::updater::INSTALLS_ON_QUIT {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    u.load_active(true);
    ready(&mut u, &dir);
    assert_eq!(u.quit_plan(), None);
    u.begin_install();
    u.installed(Ok(())).unwrap();
    assert_eq!(u.quit_plan(), None);
    u.relaunch_after_quit();
    assert_eq!(u.quit_plan(), Some(QuitPlan::Relaunch));
}

#[test]
fn only_the_maestria_rs_install_dir_counts_as_installed() {
    let here = mx_sys::updater::installed_here(std::path::Path::new("/opt/maestria-rs/maestria-rs"));
    assert_eq!(here, cfg!(all(target_os = "linux", target_arch = "x86_64")));
    assert!(!mx_sys::updater::installed_here(std::path::Path::new("/opt/maestria/maestria")));
}

#[test]
fn starting_wipes_leftovers_and_schedules_the_first_check_after_30_s_when_active() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    std::fs::create_dir_all(u.dir().join("old")).unwrap();
    assert_eq!(u.start(), None);
    assert!(!u.dir().exists());
    u.load_active(true);
    assert_eq!(u.start(), Some(Duration::from_secs(30)));
}

#[test]
fn given_stage_ready_when_switched_off_then_everything_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    u.load_active(true);
    ready(&mut u, &dir);
    assert_eq!(u.stage(), Stage::Ready);
    assert!(u.shows_chip());
    assert_eq!(u.set_active(false), None);
    assert_eq!(u.stage(), Stage::Idle);
    assert!(u.release().is_none() && u.staged().is_none() && u.etag().is_none());
    assert!(!u.dir().exists());
}

#[test]
fn given_enabled_and_off_when_switched_on_then_a_check_is_scheduled_now() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    assert_eq!(u.set_active(true), Some(Duration::ZERO));
    assert_eq!(u.set_active(true), None);
}

#[test]
fn given_not_enabled_when_switched_on_then_the_value_sticks_but_nothing_is_scheduled() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, false);
    assert_eq!(u.set_active(true), None);
    assert!(u.active());
    assert!(!u.can_check());
}

#[test]
fn given_switched_off_during_fetching_then_the_result_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    u.load_active(true);
    assert!(u.begin_fetch(Some(&release("3.1.0"))));
    u.set_active(false);
    u.fetched(release("3.1.0"), Ok(dir.path().join("x.deb"))).unwrap();
    assert_eq!(u.stage(), Stage::Idle);
    assert!(u.release().is_none());
}

#[test]
fn only_a_newer_release_starts_a_fetch() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    u.load_active(true);
    assert!(!u.begin_fetch(None));
    assert!(!u.begin_fetch(Some(&release("3.0.0"))));
    assert!(!u.begin_fetch(Some(&release("2.7.0"))));
    assert!(u.begin_fetch(Some(&release("3.0.1"))));
    assert!(!u.begin_fetch(Some(&release("3.0.2"))));
}

#[test]
fn a_failed_fetch_drops_back_to_idle() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    u.load_active(true);
    u.begin_fetch(Some(&release("3.1.0")));
    assert!(u.fetched(release("3.1.0"), Err("boom".into())).is_err());
    assert_eq!(u.stage(), Stage::Idle);
}

#[test]
fn installing_goes_to_installed_or_back_to_ready() {
    let dir = tempfile::tempdir().unwrap();
    let mut u = updater(&dir, true);
    u.load_active(true);
    let staged = ready(&mut u, &dir);
    assert_eq!(u.begin_install(), Some(staged));
    assert_eq!(u.stage(), Stage::Installing);
    assert!(u.installed(Err("Instalação cancelada".into())).is_err());
    assert_eq!(u.stage(), Stage::Ready);
    u.begin_install();
    u.installed(Ok(())).unwrap();
    assert_eq!(u.stage(), Stage::Installed);
    assert!(!u.relaunches_on_quit());
    u.relaunch_after_quit();
    assert!(u.relaunches_on_quit());
}

mod network {
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    use mx_sys::updater::fetch::{Latest, fetch, latest, sha256_of};
    use mx_sys::updater::install::{INSTALL_ENV, LAUNCHER, RELAUNCH_ENV, install_command, install_error, relaunch_target};

    use super::*;

    fn serve(files: HashMap<String, (u16, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let files = Arc::new(files);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap() == 0 || h == "\r\n" {
                        break;
                    }
                }
                let (code, body) = files.get(&path).cloned().unwrap_or((404, Vec::new()));
                let mut s = stream;
                let head = format!("HTTP/1.1 {code} X\r\nContent-Length: {}\r\nETag: \"e1\"\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        base
    }

    fn sha(bytes: &[u8]) -> String {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f");
        std::fs::write(&f, bytes).unwrap();
        sha256_of(&f).unwrap()
    }

    #[test]
    fn exit_126_means_the_password_dialog_was_cancelled() {
        let deb = std::path::Path::new("/s/x.deb");
        assert_eq!(install_error(Some(126), "", deb), "Instalação cancelada");
        assert_eq!(install_error(Some(100), "a\nE: broken\n\n", deb), "Não deu pra instalar (E: broken). No terminal: sudo apt install /s/x.deb");
    }

    #[test]
    fn the_real_install_is_pkexec_apt_get_and_the_relaunch_is_the_launcher() {
        let deb = std::path::Path::new("/s/maestria-rs_3.1.0-1_amd64.deb");
        if std::env::var_os(INSTALL_ENV).is_none() {
            let (program, args) = install_command(deb);
            assert_eq!(program, "pkexec");
            assert_eq!(args, ["apt-get", "install", "-y", "/s/maestria-rs_3.1.0-1_amd64.deb"]);
        }
        if std::env::var_os(RELAUNCH_ENV).is_none() {
            assert_eq!(relaunch_target(), PathBuf::from(LAUNCHER));
        }
    }

    #[test]
    fn the_latest_release_comes_from_the_maestria_rs_repo() {
        if std::env::var_os(mx_sys::updater::fetch::API_ENV).is_none() {
            assert_eq!(mx_sys::updater::fetch::latest_url(), "https://api.github.com/repos/MatheusPano/MaestrIA-rs/releases/latest");
        }
    }

    #[tokio::test]
    async fn the_latest_release_answers_with_its_etag_and_maestria_rs_asset() {
        let body = json!({"tag_name": "v3.1.0", "assets": [
            {"name": "maestria_3.1.0-1_amd64.deb", "browser_download_url": "https://x/flutter"},
            {"name": "maestria-rs_3.1.0-1_amd64.deb", "browser_download_url": "https://x/rs"}
        ]});
        let base = serve(HashMap::from([("/latest".to_owned(), (200, body.to_string().into_bytes())), ("/same".to_owned(), (304, vec![]))]));
        let Latest::Found { release, etag } = latest(&format!("{base}/latest"), "3.0.0", None).await.unwrap() else { panic!() };
        assert_eq!(release.unwrap().asset_url, "https://x/rs");
        assert_eq!(etag.as_deref(), Some("\"e1\""));
        assert!(matches!(latest(&format!("{base}/same"), "3.0.0", Some("\"e1\"")).await.unwrap(), Latest::Unchanged));
        assert_eq!(latest(&format!("{base}/nope"), "3.0.0", None).await.err().unwrap(), "releases/latest: HTTP 404");
    }

    #[tokio::test]
    async fn the_download_is_checked_against_sha256sums() {
        let deb = b"pacote".to_vec();
        let name = "maestria-rs_3.1.0-1_amd64.deb";
        let sums = format!("{}  {name}\n", sha(&deb));
        let base = serve(HashMap::from([
            ("/sums".to_owned(), (200, sums.into_bytes())),
            ("/bad".to_owned(), (200, format!("{}  {name}\n", "0".repeat(64)).into_bytes())),
            ("/deb".to_owned(), (200, deb)),
        ]));
        let dir = tempfile::tempdir().unwrap();
        let mut r = Release { version: "3.1.0".into(), asset: name.into(), asset_url: format!("{base}/deb"), sums_url: Some(format!("{base}/sums")) };
        let staged = fetch("3.0.0", &r, &dir.path().join("update"), None).await.unwrap();
        assert_eq!(std::fs::read(&staged).unwrap(), b"pacote");
        r.sums_url = Some(format!("{base}/bad"));
        let err = fetch("3.0.0", &r, &dir.path().join("update"), None).await.unwrap_err();
        assert!(err.starts_with(&format!("{name}: sha256 ")), "{err}");
        r.sums_url = None;
        assert_eq!(fetch("3.0.0", &r, &dir.path().join("update"), None).await.unwrap_err(), "3.1.0 sem SHA256SUMS");
    }
}
