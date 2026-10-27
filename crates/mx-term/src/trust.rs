use std::sync::LazyLock;

use regex::Regex;

static TRUST_PROMPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)trust this folder|Do you trust the files").expect("valid regex"));

pub fn asks_trust(screen: &str) -> bool {
    TRUST_PROMPT.is_match(screen)
}

#[cfg(test)]
mod tests {
    use super::asks_trust;

    #[test]
    fn the_folder_trust_question_is_recognised_in_either_wording() {
        assert!(asks_trust("❯ No, exit\n  Yes, I trust this folder\n"));
        assert!(asks_trust("DO YOU TRUST THE FILES in this folder?"));
        assert!(!asks_trust("> "));
    }
}
