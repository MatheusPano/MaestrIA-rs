use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::parse::{UsageAccount, UsageReading, parse, plan_name};

pub const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
pub const NO_TOKEN: &str =
    "Não achei a credencial do Claude nesta máquina — faça login com `claude` uma vez";
pub const EXPIRED: &str =
    "A credencial venceu — ela se renova sozinha da próxima vez que você usar o Claude";
pub const OFFLINE: &str = "Sem rede pra perguntar";
pub const TOO_SLOW: &str = "A api demorou demais pra responder";

const PATIENCE: Duration = Duration::from_secs(10);

pub fn token_in(raw: &str) -> Option<String> {
    let json: Value = serde_json::from_str(raw).ok()?;
    let token = json.get("claudeAiOauth")?.get("accessToken")?.as_str()?;
    (!token.is_empty()).then(|| token.to_owned())
}

pub fn access_token(claude_dir: &Path) -> Option<String> {
    let from_file = std::fs::read_to_string(claude_dir.join(".credentials.json"))
        .ok()
        .and_then(|raw| token_in(&raw));
    from_file.or_else(keychain_token)
}

#[cfg(all(target_os = "macos", not(test)))]
fn keychain_token() -> Option<String> {
    let out = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| token_in(&String::from_utf8_lossy(&out.stdout)))
        .flatten()
}

#[cfg(any(not(target_os = "macos"), test))]
fn keychain_token() -> Option<String> {
    None
}

pub fn account(claude_json: &Path) -> UsageAccount {
    let Some(json) = std::fs::read_to_string(claude_json)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
    else {
        return UsageAccount::default();
    };
    let Some(who) = json.get("oauthAccount").and_then(Value::as_object) else {
        return UsageAccount::default();
    };
    let field = |key: &str| who.get(key).and_then(Value::as_str).map(str::to_owned);
    UsageAccount {
        email: field("emailAddress"),
        organization: field("organizationName"),
        plan: plan_name(who.get("organizationType").and_then(Value::as_str)),
    }
}

pub async fn read_usage(claude_dir: &Path, claude_json: &Path) -> UsageReading {
    fetch_usage(ENDPOINT, access_token(claude_dir), || account(claude_json)).await
}

pub async fn fetch_usage(
    endpoint: &str,
    token: Option<String>,
    account: impl FnOnce() -> UsageAccount,
) -> UsageReading {
    let Some(token) = token else {
        return UsageReading::failed(NO_TOKEN);
    };
    let client = match reqwest::Client::builder().connect_timeout(PATIENCE).build() {
        Ok(client) => client,
        Err(e) => return UsageReading::failed(format!("Não deu pra ler: {e}")),
    };
    let request = client
        .get(endpoint)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send();
    let response = match tokio::time::timeout(PATIENCE, request).await {
        Err(_) => return UsageReading::failed(TOO_SLOW),
        Ok(Err(e)) => return failure(&e),
        Ok(Ok(response)) => response,
    };
    let status = response.status().as_u16();
    let body = match tokio::time::timeout(PATIENCE, response.text()).await {
        Err(_) => return UsageReading::failed(TOO_SLOW),
        Ok(Err(e)) => return failure(&e),
        Ok(Ok(body)) => body,
    };
    match status {
        401 | 403 => UsageReading::failed(EXPIRED),
        200 => parse(&body, account()),
        other => UsageReading::failed(format!("A api respondeu {other}")),
    }
}

fn failure(e: &reqwest::Error) -> UsageReading {
    if e.is_timeout() {
        UsageReading::failed(TOO_SLOW)
    } else if e.is_connect() {
        UsageReading::failed(OFFLINE)
    } else {
        UsageReading::failed(format!("Não deu pra ler: {e}"))
    }
}
