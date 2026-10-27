use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{ASSET_SUFFIX, PACKAGE_PREFIX, Release, expected_sum};

pub const RELEASES: &str = env!("MAESTRIA_RS_RELEASES");

pub fn latest_url() -> String {
    format!("https://api.github.com/repos/{RELEASES}/releases/latest")
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

pub async fn fetch(current: &str, release: &Release, dir: &Path) -> Result<PathBuf, String> {
    let Some(sums_url) = &release.sums_url else {
        return Err(format!("{} sem SHA256SUMS", release.version));
    };
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
    Ok(file)
}
