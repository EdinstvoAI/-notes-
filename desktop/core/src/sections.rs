//! Только свои разделы: повтор заменяет результат, сохраняя текст человека и frontmatter.
fn remove(name: &str, text: &str, blank_ends: bool) -> String {
    let mut lines: Vec<_> = text.split('\n').collect();
    if let Some(start) = lines.iter().position(|l| *l == format!("## {name}")) {
        let mut end = start + 1;
        let mut content = false;
        while end < lines.len() && !lines[end].starts_with("## ") && !lines[end].starts_with("# ") {
            let blank = lines[end].trim().is_empty();
            end += 1;
            if blank_ends && blank && content { break; }
            content |= !blank;
        }
        lines.drain(start..end);
    }
    lines.join("\n")
}
pub fn summary(text: &str, result: &str) -> String {
    let body = remove("Кратко", text, true);
    let mut lines: Vec<_> = body.split('\n').map(str::to_string).collect();
    let mut at = if lines.first().is_some_and(|s| s == "---") {
        lines.iter().skip(1).position(|s| s == "---").map_or(0, |n| n + 2)
    } else { 0 };
    while at < lines.len() && lines[at].trim().is_empty() { at += 1; }
    if at < lines.len() && lines[at].starts_with("# ") { at += 1; }
    lines.splice(at..at, [String::new(), "## Кратко".into(), String::new(), result.trim().into(), String::new()]);
    lines.join("\n").replace("\n\n\n", "\n\n")
}
pub fn tasks(text: &str, tasks: &[(String, Option<String>)]) -> String {
    if tasks.is_empty() { return text.into(); }
    let lines: Vec<_> = tasks.iter().map(|(title, date)| format!("- [ ] {}{}", title.replace(['\n', '\r'], " "), date.as_ref().map_or(String::new(), |d| format!(" 📅 {d}")))).collect();
    format!("{}\n\n## Задачи\n\n{}\n", remove("Задачи", text, false).trim(), lines.join("\n"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_summary_keeps_frontmatter_and_body() {
        let original = "---\na: b\n---\n# Заметка\n\nМой текст\n\n## Другое\nосталось\n";
        let once = summary(original, "- первое");
        let twice = summary(&once, "- второе");
        assert!(twice.starts_with("---\na: b\n---\n# Заметка\n"));
        assert_eq!(twice.matches("## Кратко").count(), 1);
        assert!(!twice.contains("первое"));
        assert!(twice.contains("Мой текст\n\n## Другое\nосталось"));
    }
    #[test]
    fn repeated_tasks_replace_only_their_section() {
        let once = tasks("# Текст\n", &[("сделать".into(), Some("2026-11-09".into()))]);
        let twice = tasks(&(once + "\n## Другое\nмой текст"), &[("проверить\nстроку".into(), None)]);
        assert!(!twice.contains("сделать"));
        assert!(twice.contains("## Другое\nмой текст"));
        assert!(twice.contains("- [ ] проверить строку"));
        assert_eq!(tasks(&twice, &[]), twice);
    }
}
