use std::path::Path;

pub fn stem(p: &str) -> &str {
    Path::new(p)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
}
