use mx_core::paths::SEPARATORS;

pub fn dirname(path: &str) -> String {
    let trimmed = path.trim_end_matches(SEPARATORS);
    if cfg!(windows) && trimmed.len() == 2 && trimmed.ends_with(':') {
        return format!("{trimmed}\\");
    }
    if trimmed.is_empty() {
        return if path.is_empty() { ".".into() } else { "/".into() };
    }
    match trimmed.rfind(SEPARATORS) {
        None => ".".into(),
        Some(i) => {
            let parent = trimmed[..i].trim_end_matches(SEPARATORS);
            if parent.is_empty() {
                "/".into()
            } else if cfg!(windows) && parent.len() == 2 && parent.ends_with(':') {
                format!("{parent}\\")
            } else {
                parent.to_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirname_matches_dart_file_parent() {
        assert_eq!(dirname("/a/b"), "/a");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(dirname("foo"), ".");
        assert_eq!(dirname("/a/b/"), "/a");
    }
}
