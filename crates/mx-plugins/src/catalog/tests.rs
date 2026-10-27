use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::*;
use crate::Plugins;

type Files = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct Fake {
    base: String,
    files: Files,
    entries: Mutex<Vec<Value>>,
    tmp: tempfile::TempDir,
}

impl Fake {
    fn start() -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let files: Files = Arc::default();
        let served = files.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                }
                let body = served.lock().unwrap().get(&path).cloned();
                let (code, body) = body.map_or((404, Vec::new()), |b| (200, b));
                let mut s = stream;
                let head = format!("HTTP/1.1 {code} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        Fake { base, files, entries: Mutex::new(Vec::new()), tmp: tempfile::tempdir().unwrap() }
    }

    fn url(&self) -> String {
        format!("{}/catalog.json", self.base)
    }

    fn sync(&self) {
        let body = json!({"schema": 1, "plugins": *self.entries.lock().unwrap()}).to_string();
        self.files.lock().unwrap().insert("/catalog.json".into(), body.into_bytes());
    }

    fn publish(&self, id: &str, version: &str, permissions: &[&str], said: Option<&[&str]>) {
        let src = self.tmp.path().join(format!("src-{id}-{version}"));
        let folder = src.join(id.trim_start_matches("mx."));
        write_manifest(&folder, id, version, permissions);
        let zip = self.tmp.path().join(format!("{id}-{version}.zip"));
        let ok = Command::new("zip").arg("-qr").arg(&zip).arg(folder.file_name().unwrap()).current_dir(&src).status().unwrap();
        assert!(ok.success());
        let bytes = std::fs::read(&zip).unwrap();
        let name = format!("/{id}-{version}.zip");
        let sha = sha256_file(&zip).unwrap();
        self.files.lock().unwrap().insert(name.clone(), bytes);
        let entry = json!({
            "id": id, "name": id, "version": version,
            "permissions": said.unwrap_or(permissions),
            "url": format!("{}{name}", self.base), "sha256": sha,
        });
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|e| e["id"] != id);
        entries.push(entry);
        drop(entries);
        self.sync();
    }

    fn put_entry(&self, entry: Value) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|e| e["id"] != entry["id"]);
        entries.push(entry);
        drop(entries);
        self.sync();
    }
}

fn write_manifest(dir: &Path, id: &str, version: &str, permissions: &[&str]) {
    std::fs::create_dir_all(dir).unwrap();
    let m = json!({"id": id, "name": id, "version": version, "permissions": permissions});
    std::fs::write(dir.join(crate::MANIFEST_NAME), m.to_string()).unwrap();
}

struct Rig {
    fake: Fake,
    plugins: Plugins,
    updates: PluginUpdates,
    _home: tempfile::TempDir,
}

fn rig() -> Rig {
    let fake = Fake::start();
    fake.sync();
    let home = tempfile::tempdir().unwrap();
    let plugins = Plugins::with_dirs(home.path().join("plugins"), home.path().join("plugin-data"));
    plugins.scan();
    let updates = PluginUpdates::new(plugins.clone(), vec![fake.url()]);
    Rig { fake, plugins, updates, _home: home }
}

impl Rig {
    fn version(&self, id: &str) -> Option<String> {
        self.plugins.get(id).and_then(|p| p.manifest).map(|m| m.version.clone())
    }

    fn staging_empty(&self) -> bool {
        std::fs::read_dir(self.plugins.root().join(".staging")).map_or(true, |mut d| d.next().is_none())
    }

    fn on_disk(&self) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(self.plugins.root())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.'))
            .collect();
        out.sort();
        out
    }

    async fn install(&self, id: &str) -> Result<crate::PluginInfo, InstallError> {
        let catalog = self.updates.load(&self.fake.url()).await.unwrap();
        let entry = catalog.by_id(id).unwrap().clone();
        let staged = self.updates.stage_entry(&entry, &self.fake.url()).await?;
        self.plugins.commit(&staged).await
    }
}

use crate::registry::InstallError;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
}

fn git_repo(parent: &Path, id: &str, version: &str) -> PathBuf {
    let repo = parent.join("lucas.git");
    write_manifest(&repo, id, version, &[]);
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "first"]);
    repo
}

#[test]
fn parsing_drops_bad_entries_and_normalizes_the_rest() {
    let body = json!({"schema": 1, "plugins": [
        {"id": "a", "version": "1.0.0", "url": "https://x/a.zip", "sha256": "AB", "kinds": ["theme", "nova"]},
        {"id": "b", "version": "1.0.0", "url": "https://x/b.zip"},
        {"id": "c", "version": "0.1.0", "git": "https://g/c", "ref": "main", "url": "https://x/c.zip", "sha256": "cc"},
        {"id": "d", "version": "0.1.0", "git": "https://g/d", "maestria": 99},
        "lixo",
        {"id": 3, "version": "1"}
    ]});
    let c = PluginCatalog::parse("u", &body.to_string()).unwrap();
    let ids: Vec<&str> = c.plugins.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["a", "c", "d"]);
    let a = c.by_id("a").unwrap();
    assert_eq!(a.sha256.as_deref(), Some("ab"));
    assert_eq!(a.kinds, BTreeSet::from([PluginKind::Theme]));
    assert_eq!(a.name, "a");
    let cc = c.by_id("c").unwrap();
    assert!(cc.kinds.is_empty());
    assert_eq!((cc.git.as_deref(), cc.url.as_deref(), cc.sha256.as_deref()), (Some("https://g/c"), None, None));
    assert_eq!(cc.git_ref.as_deref(), Some("main"));
    assert!(!c.by_id("d").unwrap().fits());
    assert!(a.fits());
    assert_eq!(PluginCatalog::parse("u", "[]").unwrap_err(), "não é um catálogo");
}

#[test]
fn a_minimum_maestria_version_is_compared_against_the_compat_level() {
    let entry = |min: &str| CatalogEntry::from_json(&json!({"id": "x", "version": "1", "git": "g", "maestria": min})).unwrap();
    assert!(entry("2.7.0").fits());
    assert!(entry("2.6").fits());
    assert!(!entry("2.8.0").fits());
    assert!(!entry("3.0.0").fits());
}

#[test]
fn the_real_catalog_shape_parses() {
    let body = r#"{"schema":1,"generatedAt":"2026-10-24T13:01:45.927Z","plugins":[{"id":"maestria.argocd","name":"ArgoCD","version":"1.0.0","description":"d","author":"Lucas","maestria":1,"permissions":["sessions.create"],"kinds":["feature"],"icon":"https://raw.githubusercontent.com/x/argo.svg","url":"https://github.com/x/argocd-1.0.0.zip","sha256":"a8aec4e20ee7010366433ee2485ac16c0a87972ea73f82cd6f8e7c5541a9aa21"}]}"#;
    let c = PluginCatalog::parse(OFFICIAL_CATALOG, body).unwrap();
    let e = &c.plugins[0];
    assert_eq!((e.name.as_str(), e.author.as_str(), e.api), ("ArgoCD", "Lucas", 1));
    assert_eq!(e.permissions, ["sessions.create"]);
    assert_eq!(e.kinds, BTreeSet::from([PluginKind::Feature]));
    assert!(e.fits());
}

#[tokio::test(flavor = "multi_thread")]
async fn installing_from_the_catalog_records_the_catalog_origin() {
    let r = rig();
    r.fake.publish("mx.git", "1.0.0", &[], None);
    r.fake.publish("mx.ssh", "1.0.0", &[], None);
    r.install("mx.git").await.unwrap();
    assert_eq!(r.version("mx.git").as_deref(), Some("1.0.0"));
    assert_eq!(r.on_disk(), ["mx.git"]);
    assert_eq!(r.plugins.origin_of("mx.git"), Some(PluginOrigin::Catalog(r.fake.url())));
    assert!(r.plugins.log_of("mx.git").last().unwrap().contains("mx.git-1.0.0.zip"));
    assert!(r.staging_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_package_that_does_not_match_its_hash_is_refused() {
    let r = rig();
    r.fake.publish("mx.git", "1.0.0", &[], None);
    let mut e = r.fake.entries.lock().unwrap()[0].clone();
    e["sha256"] = json!("0".repeat(64));
    r.fake.put_entry(e);
    let err = r.install("mx.git").await.unwrap_err();
    assert!(err.0.starts_with("O pacote de mx.git não confere (sha256 "), "{}", err.0);
    assert!(r.version("mx.git").is_none());
    assert!(r.staging_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_catalog_plugin_updates_itself_on_check() {
    let r = rig();
    r.fake.publish("mx.git", "1.0.0", &[], None);
    r.install("mx.git").await.unwrap();
    r.fake.publish("mx.git", "1.1.0", &[], None);
    r.updates.check().await;
    assert_eq!(r.version("mx.git").as_deref(), Some("1.1.0"));
    assert!(r.updates.available().is_empty());
    assert_eq!(r.plugins.origin_of("mx.git"), Some(PluginOrigin::Catalog(r.fake.url())));
    assert!(r.updates.checked_at().is_some() && !r.updates.checking());
}

#[tokio::test(flavor = "multi_thread")]
async fn manual_plugins_and_auto_update_off_wait_for_the_user() {
    let r = rig();
    r.fake.publish("mx.git", "1.0.0", &[], None);
    r.install("mx.git").await.unwrap();
    r.fake.publish("mx.git", "1.1.0", &[], None);
    r.plugins.set_manual_update("mx.git", true);
    r.updates.check().await;
    assert_eq!(r.version("mx.git").as_deref(), Some("1.0.0"));
    assert_eq!(r.updates.available()["mx.git"].to, "1.1.0");
    r.plugins.set_manual_update("mx.git", false);
    r.plugins.set_auto_update(false);
    r.updates.check().await;
    assert_eq!(r.version("mx.git").as_deref(), Some("1.0.0"));
    let u = r.updates.available()["mx.git"].clone();
    let staged = r.updates.prepare(&u).await.unwrap();
    r.updates.finish(&staged).await.unwrap();
    assert_eq!(r.version("mx.git").as_deref(), Some("1.1.0"));
    assert!(r.updates.available().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_version_asking_a_new_permission_waits() {
    let r = rig();
    r.fake.publish("mx.git", "1.0.0", &[], None);
    r.install("mx.git").await.unwrap();
    r.fake.publish("mx.git", "2.0.0", &["hooks"], None);
    r.updates.check().await;
    assert_eq!(r.version("mx.git").as_deref(), Some("1.0.0"));
    assert!(r.updates.available()["mx.git"].asks_more);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lying_catalog_is_caught_by_the_downloaded_manifest() {
    let r = rig();
    r.fake.publish("mx.git", "1.0.0", &[], None);
    r.install("mx.git").await.unwrap();
    r.fake.publish("mx.git", "2.0.0", &["terminal.write"], Some(&[]));
    r.updates.check().await;
    assert_eq!(r.version("mx.git").as_deref(), Some("1.0.0"));
    assert!(r.updates.available()["mx.git"].asks_more);
    assert!(r.staging_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_git_plugin_updates_only_when_the_repo_moves() {
    let r = rig();
    let repo = git_repo(r.fake.tmp.path(), "com.lucas.foo", "0.1.0");
    let src = repo.to_string_lossy().into_owned();
    let staged = r.plugins.stage(&src).await.unwrap();
    r.plugins.commit(&staged).await.unwrap();
    assert_eq!(r.plugins.origin_of("com.lucas.foo"), Some(PluginOrigin::Git { url: src.clone(), git_ref: None }));
    r.updates.check().await;
    assert!(r.updates.available().is_empty());
    write_manifest(&repo, "com.lucas.foo", "0.2.0", &[]);
    git(&repo, &["commit", "-qam", "bump"]);
    r.updates.check().await;
    assert_eq!(r.version("com.lucas.foo").as_deref(), Some("0.2.0"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_git_plugin_in_the_catalog_is_offered_adoption() {
    let r = rig();
    let repo = git_repo(r.fake.tmp.path(), "com.lucas.foo", "0.1.0");
    let staged = r.plugins.stage(&repo.to_string_lossy()).await.unwrap();
    r.plugins.commit(&staged).await.unwrap();
    r.fake.put_entry(json!({"id": "com.lucas.foo", "version": "0.1.0", "git": repo.to_string_lossy()}));
    r.updates.check().await;
    let offer = r.updates.offers()["com.lucas.foo"].clone();
    assert!(!offer.newer());
    let staged = r.updates.prepare(&offer).await.unwrap();
    r.updates.finish(&staged).await.unwrap();
    assert_eq!(r.plugins.origin_of("com.lucas.foo"), Some(PluginOrigin::Catalog(r.fake.url())));
    assert!(r.updates.offers().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dev_link_is_never_touched() {
    let r = rig();
    let dev = r.fake.tmp.path().join("dev-git");
    write_manifest(&dev, "mx.git", "0.0.1", &[]);
    r.plugins.link(&dev.to_string_lossy()).await.unwrap();
    r.fake.publish("mx.git", "9.0.0", &[], None);
    r.updates.check().await;
    assert!(r.updates.available().is_empty());
    assert!(r.updates.offers().is_empty());
    assert_eq!(r.version("mx.git").as_deref(), Some("0.0.1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_update_settings_round_trip_through_the_config() {
    let r = rig();
    r.fake.publish("mx.git", "1.0.0", &[], None);
    r.install("mx.git").await.unwrap();
    r.plugins.set_auto_update(false);
    r.plugins.set_manual_update("mx.git", true);
    let saved = r.plugins.to_json();
    assert_eq!(saved["autoUpdate"], json!(false));
    assert_eq!(saved["manualUpdate"], json!(["mx.git"]));
    assert_eq!(saved["origins"], json!({"mx.git": {"catalog": r.fake.url()}}));
    let fresh = Plugins::with_dirs(r.fake.tmp.path().join("p2"), r.fake.tmp.path().join("d2"));
    fresh.load(&saved);
    assert!(!fresh.auto_update());
    assert_eq!(fresh.manual_update(), BTreeSet::from(["mx.git".to_owned()]));
    assert_eq!(fresh.origin_of("mx.git"), Some(PluginOrigin::Catalog(r.fake.url())));
    fresh.load(&json!({}));
    assert!(fresh.auto_update());
    assert_eq!(fresh.to_json(), json!({}));

    let folder = r.fake.tmp.path().join("folder-git");
    write_manifest(&folder, "mx.git", "1.0.1", &[]);
    let staged = r.plugins.stage(&folder.to_string_lossy()).await.unwrap();
    r.plugins.commit(&staged).await.unwrap();
    assert_eq!(r.plugins.origin_of("mx.git"), None);
    r.plugins.uninstall("mx.git").await;
    assert!(r.plugins.manual_update().is_empty());
}

#[test]
fn origins_that_are_not_objects_or_lack_both_keys_are_ignored() {
    let p = Plugins::with_dirs("/nope/p".into(), "/nope/d".into());
    p.load(&json!({"origins": {"a": "x", "b": {}, "c": {"git": "g", "ref": "dev"}}}));
    assert_eq!(p.origin_of("a"), None);
    assert_eq!(p.origin_of("b"), None);
    assert_eq!(p.origin_of("c"), Some(PluginOrigin::Git { url: "g".into(), git_ref: Some("dev".into()) }));
}

#[test]
fn manifest_kinds_follow_what_the_plugin_declares() {
    let base = crate::PluginManifest::parse(json!({"id": "k", "version": "1"}).as_object().unwrap()).unwrap();
    let themed = crate::PluginManifest { themes: vec!["t".into()], ..base.clone() };
    assert_eq!(themed.kinds(), BTreeSet::from([PluginKind::Theme]));
    let program = crate::PluginManifest { main: Some(vec!["node".into(), "main.js".into()]), ..base.clone() };
    assert_eq!(program.kinds(), BTreeSet::from([PluginKind::Feature]));
    let with_commands = crate::PluginManifest::parse(
        json!({"id": "k", "version": "1", "contributes": {"commands": [{"id": "c", "title": "C", "run": "echo"}]}}).as_object().unwrap(),
    )
    .unwrap();
    let both = crate::PluginManifest { themes: vec!["t".into()], ..with_commands };
    assert_eq!(both.kinds(), BTreeSet::from([PluginKind::Theme, PluginKind::Feature]));
    assert!(base.kinds().is_empty());
}

#[test]
fn origins_describe_where_new_versions_come_from() {
    assert_eq!(PluginOrigin::Catalog(OFFICIAL_CATALOG.into()).describe(), "Vem do catálogo oficial");
    assert_eq!(PluginOrigin::Catalog("u".into()).describe(), "Vem do catálogo u");
    assert_eq!(PluginOrigin::Git { url: "g".into(), git_ref: None }.describe(), "Vem de g");
    assert_eq!(PluginOrigin::Git { url: "g".into(), git_ref: Some("dev".into()) }.describe(), "Vem de g (dev)");
}

#[test]
fn kinds_have_portuguese_labels() {
    assert_eq!(PluginKind::Theme.label(), "Tema");
    assert_eq!(PluginKind::Feature.label(), "Recurso");
    assert_eq!(PluginKind::by_id("feature"), Some(PluginKind::Feature));
    assert_eq!(PluginKind::by_id("nova"), None);
}
