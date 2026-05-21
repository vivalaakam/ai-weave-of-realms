pub fn sanitize_save_filename(name: &str) -> String {
    let mut cleaned = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            cleaned.push(ch.to_ascii_uppercase());
        }
    }
    if cleaned.is_empty() {
        cleaned.push_str("SAVE");
    }
    cleaned.truncate(7);
    format!("{cleaned}.RPGS")
}
