use std::path::Path;

use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::registry::InstallError;

const USER_AGENT: &str = "maestria";

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent(USER_AGENT).build().map_err(|e| e.to_string())
}

pub(crate) async fn get(url: &str) -> Result<String, String> {
    let res = client()?.get(url).send().await.map_err(|e| e.to_string())?;
    let code = res.status().as_u16();
    let body = res.text().await.map_err(|e| e.to_string())?;
    if code != 200 {
        return Err(format!("HTTP {code}"));
    }
    Ok(body)
}

pub(crate) async fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let res = client()?.get(url).send().await.map_err(|e| e.to_string())?;
    let code = res.status().as_u16();
    if code != 200 {
        return Err(format!("HTTP {code}"));
    }
    res.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

pub(crate) async fn download(url: &str, to: &Path) -> Result<(), InstallError> {
    let fail = |e: String| InstallError(e);
    let res = client().map_err(fail)?.get(url).send().await.map_err(|e| fail(e.to_string()))?;
    let code = res.status().as_u16();
    if code != 200 {
        return Err(InstallError(format!("{url}: HTTP {code}")));
    }
    let body = res.bytes().await.map_err(|e| fail(e.to_string()))?;
    std::fs::write(to, &body).map_err(|e| fail(e.to_string()))
}

pub fn sha256_file(path: &Path) -> Result<String, InstallError> {
    let bytes = std::fs::read(path).map_err(|e| InstallError(format!("sha256: {e}")))?;
    Ok(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

pub async fn git_head(git: &str, git_ref: Option<&str>, env: &[(String, String)]) -> Option<String> {
    let out = Command::new("git")
        .args(["ls-remote", git, git_ref.unwrap_or("HEAD")])
        .env_clear()
        .envs(env.iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.split_whitespace().next().map(str::to_owned)
}
