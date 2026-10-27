use std::sync::LazyLock;

use regex::Regex;

static LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9a-fA-F]{64})\s+\*?(.+)$").unwrap());

pub fn expected_sum(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let c = LINE.captures(line.trim())?;
        (c[2].trim() == file).then(|| c[1].to_lowercase())
    })
}
