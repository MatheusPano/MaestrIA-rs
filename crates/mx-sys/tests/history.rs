use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use mx_sys::history::{ChatEntry, ChatStanding, ReadOptions, clean, dir_for, home_root, read};

const TITLED: &str = "aaaa1111-1111-4111-8111-000000000001";
const SPOKE: &str = "bbbb2222-2222-4222-8222-000000000002";
const MUTE: &str = "cccc3333-3333-4333-8333-000000000003";
const ORPHAN: &str = "dddd4444-4444-4444-8444-000000000004";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/history")
}

fn cwds(list: &[&str]) -> ReadOptions {
    ReadOptions { cwds: Some(list.iter().map(|s| (*s).to_owned()).collect()), ..Default::default() }
}

fn ids(chats: &[ChatEntry]) -> HashSet<&str> {
    chats.iter().map(|c| c.session_id.as_str()).collect()
}

fn by_id<'a>(chats: &'a [ChatEntry], id: &str) -> &'a ChatEntry {
    chats.iter().find(|c| c.session_id == id).unwrap()
}

#[tokio::test]
async fn lists_a_folders_chats_and_not_its_subagents() {
    let chats = read(&fixtures(), &cwds(&["/tmp"])).await;
    assert_eq!(ids(&chats), HashSet::from([TITLED, SPOKE, MUTE]));
    assert!(!chats.iter().any(|c| c.session_id.starts_with("agent-")));
}

#[tokio::test]
async fn names_the_chat_by_the_title_claude_gave_it() {
    let chats = read(&fixtures(), &cwds(&["/tmp"])).await;
    assert_eq!(by_id(&chats, TITLED).title, "Cabeçalho da lateral com a busca");
}

#[tokio::test]
async fn without_ai_title_the_first_thing_the_user_said_names_it() {
    let chats = read(&fixtures(), &cwds(&["/tmp"])).await;
    assert_eq!(by_id(&chats, SPOKE).title, "Tarefa o + da bandeja de grupos ficou torto");
}

#[tokio::test]
async fn without_title_or_speech_the_session_name_remains() {
    let chats = read(&fixtures(), &cwds(&["/tmp"])).await;
    assert_eq!(by_id(&chats, MUTE).title, "maestria_v2");
}

#[tokio::test]
async fn each_entry_knows_id_cwd_size_and_when() {
    let chats = read(&fixtures(), &cwds(&["/tmp"])).await;
    let one = by_id(&chats, TITLED);
    let meta = std::fs::metadata(fixtures().join(format!("-tmp/{TITLED}.jsonl"))).unwrap();
    assert_eq!(one.session_id, TITLED);
    assert_eq!(one.cwd, "/tmp");
    assert_eq!(one.where_(), "tmp");
    assert_eq!(one.bytes, meta.len());
    assert_eq!(one.at, meta.modified().unwrap());
    assert!(!one.missing);
}

#[tokio::test]
async fn the_whole_history_finds_the_chat_of_a_folder_that_no_longer_exists() {
    let chats = read(&fixtures(), &ReadOptions::default()).await;
    assert_eq!(ids(&chats), HashSet::from([TITLED, SPOKE, MUTE, ORPHAN]));
    let gone = by_id(&chats, ORPHAN);
    assert_eq!(gone.cwd, "/repo/sumiu");
    assert_eq!(gone.title, "Migrar o schema do relatório");
    assert!(gone.missing);
}

#[tokio::test]
async fn the_newest_comes_first_and_the_limit_cuts_from_the_top() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(dir_for("/tmp"));
    std::fs::create_dir(&dir).unwrap();
    let now = SystemTime::now();
    for (name, age) in [("velha", 60), ("nova", 0)] {
        let path = dir.join(format!("{name}.jsonl"));
        std::fs::write(&path, format!(r#"{{"type":"ai-title","aiTitle":"{name}"}}"#)).unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(now - Duration::from_secs(age)).unwrap();
    }
    let chats = read(home.path(), &cwds(&["/tmp"])).await;
    let titles: Vec<&str> = chats.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["nova", "velha"]);
    let one = read(home.path(), &ReadOptions { limit: 1, ..cwds(&["/tmp"]) }).await;
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].title, "nova");
}

#[tokio::test]
async fn on_keeps_only_files_modified_that_local_day() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("-x");
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("s-1.jsonl");
    std::fs::write(&path, "").unwrap();
    let old = SystemTime::now() - Duration::from_secs(3 * 86_400);
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();
    let today = chrono::Local::now().date_naive();
    let then = chrono::DateTime::<chrono::Local>::from(old).date_naive();
    let on = |day| ReadOptions { on: Some(day), ..Default::default() };
    assert!(read(home.path(), &on(today)).await.is_empty());
    let found = read(home.path(), &on(then)).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "s");
    assert_eq!(found[0].cwd, "");
    assert!(found[0].missing);
}

#[tokio::test]
async fn a_folder_without_chats_is_not_an_error() {
    assert!(read(&fixtures(), &cwds(&["/nao/existe"])).await.is_empty());
    assert!(read(&fixtures().join("nao-existe"), &ReadOptions::default()).await.is_empty());
}

#[tokio::test]
async fn the_same_folder_asked_twice_does_not_duplicate_the_chat() {
    assert_eq!(read(&fixtures(), &cwds(&["/tmp", "/tmp", "/-tmp"])).await.len(), 3);
}

#[test]
fn dir_for_replaces_every_utf16_unit_outside_ascii_alphanumerics() {
    assert_eq!(dir_for("/home/user/maestria_v2"), "-home-user-maestria-v2");
    assert_eq!(dir_for("/tmp/ação"), "-tmp-a--o");
    assert_eq!(dir_for("/x/😀"), "-x---");
}

#[test]
fn home_root_is_under_dot_claude_projects() {
    assert_eq!(home_root("/home/eu"), PathBuf::from("/home/eu/.claude/projects"));
    assert_eq!(home_root(""), PathBuf::from("/.claude/projects"));
}

#[test]
fn clean_joins_headings_until_the_first_plain_line_and_skips_tags() {
    assert_eq!(clean(Some("<ide>x</ide>\n\n## Tarefa\nfaz isso\nresto")).as_deref(), Some("Tarefa faz isso"));
    assert_eq!(clean(Some("#\n<a>\n   ")), None);
    assert_eq!(clean(None), None);
    let long = "a".repeat(80);
    assert_eq!(clean(Some(&long)).unwrap(), format!("{}…", "a".repeat(72)));
}

fn entry(title: &str, bytes: u64, at: SystemTime) -> ChatEntry {
    ChatEntry { session_id: "s".into(), cwd: String::new(), title: title.into(), at, bytes, missing: true }
}

#[test]
fn label_clips_at_32_utf16_units_and_where_is_question_mark_without_cwd() {
    let now = SystemTime::now();
    assert_eq!(entry(&"b".repeat(32), 0, now).label(), "b".repeat(32));
    assert_eq!(entry(&"b".repeat(33), 0, now).label(), format!("{}…", "b".repeat(32)));
    assert_eq!(entry("x", 0, now).where_(), "?");
}

#[test]
fn ago_reads_in_portuguese_by_bucket() {
    let now = SystemTime::now();
    let ago = |secs: u64| entry("x", 0, now - Duration::from_secs(secs)).ago(now);
    assert_eq!(ago(30), "agora");
    assert_eq!(ago(5 * 60), "há 5 min");
    assert_eq!(ago(3 * 3600), "há 3 h");
    assert_eq!(ago(30 * 3600), "ontem");
    assert_eq!(ago(3 * 86_400), "há 3 dias");
    let at = now - Duration::from_secs(10 * 86_400);
    let local = chrono::DateTime::<chrono::Local>::from(at);
    use chrono::Datelike;
    assert_eq!(entry("x", 0, at).ago(now), format!("{}/{}", local.day(), local.month()));
}

#[test]
fn size_uses_bytes_kilobytes_and_one_decimal_megabytes() {
    let now = SystemTime::now();
    assert_eq!(entry("x", 1023, now).size(), "1023 B");
    assert_eq!(entry("x", 1536, now).size(), "2 kB");
    assert_eq!(entry("x", 1_310_720, now).size(), "1.3 MB");
    assert_eq!(entry("x", 10 * 1024 * 1024, now).size(), "10.0 MB");
}

#[test]
fn only_a_fresh_chat_opens_and_each_standing_has_its_note() {
    assert!(ChatStanding::Fresh.opens());
    assert_eq!(ChatStanding::Fresh.note(), None);
    assert_eq!(ChatStanding::OnScreen.note(), Some("já está num painel"));
    assert_eq!(ChatStanding::Live.note(), Some("aberta fora daqui"));
    assert_eq!(ChatStanding::Gone.note(), Some("a pasta não existe mais"));
    assert!(!ChatStanding::Gone.opens());
    assert!(ChatStanding::Background.opens());
    assert!(ChatStanding::Background.note().unwrap().contains("segundo plano"));
    assert!(!ChatStanding::Live.opens());
}
