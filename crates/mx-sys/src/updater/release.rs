use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub asset: String,
    pub asset_url: String,
    pub sums_url: Option<String>,
}

impl Release {
    pub fn from_json(json: &Value, prefix: &str, suffix: &str) -> Option<Release> {
        let tag = json.get("tag_name")?.as_str()?;
        let version = tag.strip_prefix('v').unwrap_or(tag).to_owned();
        let mut asset = None;
        let mut sums_url = None;
        for a in json.get("assets").and_then(Value::as_array).into_iter().flatten() {
            let Some(a) = a.as_object() else { continue };
            let name = a.get("name").and_then(Value::as_str).unwrap_or("");
            let url = a.get("browser_download_url").and_then(Value::as_str);
            if name == "SHA256SUMS" {
                sums_url = url.map(str::to_owned);
            } else if name.starts_with(prefix) && name.ends_with(suffix) {
                asset = Some((name.to_owned(), url.map(str::to_owned)));
            }
        }
        let (asset, url) = asset?;
        Some(Release { version, asset, asset_url: url?, sums_url })
    }
}
