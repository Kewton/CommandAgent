//! A closed executable structural check, not general JavaScript Test evidence.
//! Match the whole command without rewriting its registry identity or literals.

pub(super) fn checked_path(command: &str) -> Option<&str> {
    let rest =
        command.strip_prefix(r#"node -e "const fs=require('fs');const s=fs.readFileSync('"#)?;
    let (path, rest) = rest.split_once('\'')?;
    // A literal workspace-relative source file, without shell/JS escapes or
    // traversal. A different path is a different check and cannot replace the
    // original registered command, even if both classify as structural.
    if path.is_empty()
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b))
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        || !matches!(
            std::path::Path::new(path).extension()?.to_str()?,
            "tsx" | "jsx" | "ts" | "js"
        )
    {
        return None;
    }
    // All three predicates execute once against the same read. Additional
    // options, statements, assertions, catches and shell suffixes do not match.
    (rest == concat!(
        r#",'utf8');if(!s.includes('data-anvil-action=\"primary\"'))throw new Error('missing primary');"#,
        r#"if(!s.includes('data-anvil-action=\"input\"'))throw new Error('missing input');"#,
        r#"if(!s.includes('data-anvil-state'))throw new Error('missing state');console.log('ok')""#,
    ))
    .then_some(path)
}
