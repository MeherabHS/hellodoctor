//! Server-side sanitization for free-text fields. Client-side validation (Flutter,
//! admin-web) is a UX convenience, not a security boundary — every free-text field
//! that ends up in the database goes through this first.

use crate::error::AppError;

/// Trims the input, rejects NUL bytes and non-whitespace control characters, and
/// enforces a hard max length — returning a clear 422 rather than silently truncating
/// or letting Postgres reject it with a raw constraint-violation error.
pub fn sanitize_text(input: &str, max_len: usize) -> Result<String, AppError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "This field cannot be empty.".into(),
        ));
    }

    if trimmed.chars().count() > max_len {
        return Err(AppError::UnprocessableEntity(format!(
            "This field must be {max_len} characters or fewer."
        )));
    }

    if trimmed
        .chars()
        .any(|c| c == '\0' || (c.is_control() && c != '\n' && c != '\r' && c != '\t'))
    {
        return Err(AppError::UnprocessableEntity(
            "This field contains characters that are not allowed.".into(),
        ));
    }

    Ok(trimmed.to_string())
}

/// Kinds of Admin-authored text (product owner, 2026-11-14: Admin uploads are
/// sanitized at the source so every reader, present or future, gets safe text).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    /// One line: titles, names, tags, locations.
    Line,
    /// Several lines of plain text: descriptions, push messages, quiz text, notes.
    Paragraphs,
    /// Article bodies (Markdown). Links may only point to https, http, mailto or tel.
    Markdown,
}

/// Returns why `text` is refused, or `None` when it is acceptable.
///
/// Refused everywhere: NUL and other control characters (tabs and line breaks
/// only where lines are allowed), Unicode direction overrides and isolates
/// (used to disguise text), zero-width space, byte-order mark and the Unicode
/// line/paragraph separators, HTML tags (`<b>`, `<script`, `<!--`; a plain
/// "<" as in "CD4 < 200" or "<3" is fine), and `javascript:` / `vbscript:`
/// URLs. Markdown link and image targets must use https, http, mailto or tel.
pub fn content_issue(text: &str, kind: TextKind) -> Option<&'static str> {
    for c in text.chars() {
        let line_break = matches!(c, '\n' | '\r' | '\t');
        if (c.is_control() && !(line_break && kind != TextKind::Line))
            || (line_break && kind == TextKind::Line)
        {
            return Some("contains control characters or line breaks that are not allowed");
        }
        if matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}' | '\u{FEFF}' | '\u{2028}' | '\u{2029}')
        {
            return Some("contains invisible formatting characters that are not allowed");
        }
    }
    let bytes = text.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'<' {
            let next = bytes.get(i + 1).copied().unwrap_or(b' ');
            let after = if next == b'/' || next == b'!' || next == b'?' {
                bytes.get(i + 2).copied().unwrap_or(b' ')
            } else {
                next
            };
            if after.is_ascii_alphabetic() || (next == b'!' && after == b'-') {
                return Some("contains HTML tags; use plain text (and Markdown in article bodies)");
            }
        }
    }
    let lower = text.to_ascii_lowercase();
    let squeezed: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
    if squeezed.contains("javascript:") || squeezed.contains("vbscript:") {
        return Some("contains a script link, which is not allowed");
    }
    if kind == TextKind::Markdown {
        for target in markdown_targets(text) {
            let t = target.trim().trim_start_matches('<').to_ascii_lowercase();
            let allowed = ["https://", "http://", "mailto:", "tel:"]
                .iter()
                .any(|p| t.starts_with(p));
            if !allowed {
                return Some("has a link that is not https, http, mailto or tel");
            }
        }
    }
    None
}

/// Link/image targets: inline `](target)` and reference `[id]: target`.
fn markdown_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find("](") {
        let after = &rest[pos + 2..];
        let end = after.find([')', ' ', '\n']).unwrap_or(after.len());
        out.push(after[..end].to_string());
        rest = &after[end..];
    }
    for line in text.lines() {
        let l = line.trim_start();
        if let Some(close) = l.strip_prefix('[').and_then(|s| s.find("]:")) {
            let target = l[close + 3..].split_whitespace().next().unwrap_or("");
            out.push(target.to_string());
        }
    }
    out
}

/// `content_issue` as a 422 naming the field.
pub fn check_content(field: &str, text: &str, kind: TextKind) -> Result<(), AppError> {
    match content_issue(text, kind) {
        Some(why) => Err(AppError::Validation(format!("{field} {why}."))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod content_tests {
    use super::{content_issue, TextKind::*};

    #[test]
    fn ordinary_health_text_is_accepted() {
        for ok in [
            "Know your status: get tested",
            "CD4 < 200 needs follow-up; <3 to all",
            "Dozi ya PrEP — kila siku 💊",
            "Mother & child 👩\u{200D}👧 care",
            "Key data: 5 people",
        ] {
            assert_eq!(content_issue(ok, Line), None, "{ok}");
        }
        let md = "## Steps\n\n- Visit [the clinic](https://example.org/a_b)\n- Call [us](tel:+254700000000)\n\n[ref]: https://example.org";
        assert_eq!(content_issue(md, Markdown), None);
        assert_eq!(content_issue("Line one\nLine two\ttab", Paragraphs), None);
    }

    #[test]
    fn unsafe_text_is_refused() {
        for (bad, kind) in [
            ("<script>alert(1)</script>", Paragraphs),
            ("<img src=x onerror=alert(1)>", Markdown),
            ("Hi </b>", Line),
            ("<!-- hidden -->", Markdown),
            ("Click [here](javascript:alert(1))", Markdown),
            ("[x]( JaVaScRiPt :alert(1))", Markdown),
            ("[x](data:text/html;base64,PHNjcmlwdD4=)", Markdown),
            ("[x](//evil.example)", Markdown),
            ("[ref]: file:///etc/passwd", Markdown),
            ("Title\u{202E}gnp.exe", Line),
            ("zero\u{200B}width", Line),
            ("two\nlines", Line),
            ("bell\u{7}", Paragraphs),
            ("vbscript:msgbox", Paragraphs),
        ] {
            assert!(content_issue(bad, kind).is_some(), "{bad:?}");
        }
    }
}
