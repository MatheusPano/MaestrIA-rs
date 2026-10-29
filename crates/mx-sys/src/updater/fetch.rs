use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{ASSET_SUFFIX, BUNDLE_NAME, PACKAGE_PREFIX, Release, expected_sum};

pub const REPO: &str = "MatheusPano/MaestrIA-rs";
pub const API_ENV: &str = "MAESTRIA_UPDATE_API";

pub fn latest_url() -> String {
    match std::env::var(API_ENV) {
        Ok(url) if !url.trim().is_empty() => url,
        _ => format!("https://api.github.com/repos/{REPO}/releases/latest"),
    }
}

pub fn user_agent(current: &str) -> String {
    format!("maestria-rs/{current}")
}

fn client(current: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent(user_agent(current)).build().map_err(|e| e.to_string())
}

pub enum Latest {
    Unchanged,
    Found { release: Option<Release>, etag: Option<String> },
}

pub async fn latest(url: &str, current: &str, etag: Option<&str>) -> Result<Latest, String> {
    let mut req = client(current)?.get(url).header("Accept", "application/vnd.github+json");
    if let Some(tag) = etag {
        req = req.header("If-None-Match", tag);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    match res.status().as_u16() {
        304 => Ok(Latest::Unchanged),
        200 => {
            let etag = res.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_owned);
            let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Ok(Latest::Found { release: Release::from_json(&json, PACKAGE_PREFIX, ASSET_SUFFIX), etag })
        }
        code => Err(format!("releases/latest: HTTP {code}")),
    }
}

async fn download(client: &reqwest::Client, url: &str, to: &Path) -> Result<(), String> {
    let res = client.get(url).send().await.map_err(|e| e.to_string())?;
    let code = res.status().as_u16();
    if code != 200 {
        return Err(format!("{url}: HTTP {code}"));
    }
    let body = res.bytes().await.map_err(|e| e.to_string())?;
    std::fs::write(to, &body).map_err(|e| e.to_string())
}

pub fn sha256_of(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

async fn run<I, S>(program: &str, args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = tokio::process::Command::new(program).args(args).output().await.map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let last = stderr.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("").trim().to_owned();
    Err(format!("{program}: {last}"))
}

async fn stage_app(dmg: &Path, dir: &Path) -> Result<PathBuf, String> {
    let mnt = dir.join("mnt");
    let app = dir.join(BUNDLE_NAME);
    run("hdiutil", [OsStr::new("attach"), "-nobrowse".as_ref(), "-readonly".as_ref(), "-mountpoint".as_ref(), mnt.as_os_str(), dmg.as_os_str()]).await?;
    let copied = run("ditto", [mnt.join(BUNDLE_NAME).as_os_str(), app.as_os_str()]).await;
    let _ = run("hdiutil", [OsStr::new("detach"), mnt.as_os_str(), "-force".as_ref()]).await;
    let _ = std::fs::remove_file(dmg);
    copied?;
    run("codesign", [OsStr::new("--verify"), "--deep".as_ref(), app.as_os_str()]).await?;
    let _ = run("xattr", [OsStr::new("-dr"), "com.apple.quarantine".as_ref(), app.as_os_str()]).await;
    Ok(app)
}

pub async fn fetch(current: &str, release: &Release, dir: &Path, bundle: Option<&Path>) -> Result<PathBuf, String> {
    let Some(sums_url) = &release.sums_url else {
        return Err(format!("{} sem SHA256SUMS", release.version));
    };
    let is_dmg = release.asset.ends_with(".dmg");
    if is_dmg {
        let parent = bundle.and_then(Path::parent).ok_or("sem o .app instalado pra trocar")?;
        run("test", [OsStr::new("-w"), parent.as_os_str()]).await.map_err(|_| format!("{} não é gravável", parent.display()))?;
    }
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let client = client(current)?;
    let sums_file = dir.join("SHA256SUMS");
    download(&client, sums_url, &sums_file).await?;
    let sums = std::fs::read_to_string(&sums_file).map_err(|e| e.to_string())?;
    let want = expected_sum(&sums, &release.asset).ok_or_else(|| format!("{} não está no SHA256SUMS", release.asset))?;
    let file = dir.join(&release.asset);
    download(&client, &release.asset_url, &file).await?;
    let got = sha256_of(&file)?;
    if got != want {
        return Err(format!("{}: sha256 {got}, esperado {want}", release.asset));
    }
    if is_dmg {
        return stage_app(&file, dir).await;
    }
    Ok(file)
}
