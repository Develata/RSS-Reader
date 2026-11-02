//! Minimal CSS scanning shared by the stylesheet contract tests. It models comments, strings and
//! parentheses, which is all token rules need, and rejects declaration names it cannot read
//! instead of guessing.

/// The text with each comment replaced by a space (plus its line breaks, so line numbers hold):
/// comments separate tokens, so `--a/**/b` stays two tokens. An unterminated comment runs to the
/// end.
pub fn without_comments(css: &str) -> String {
    let mut text = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        text.push_str(&rest[..start]);
        text.push(' ');
        let (comment, after) =
            rest[start + 2..].split_once("*/").unwrap_or((&rest[start + 2..], ""));
        text.extend(comment.chars().filter(|&c| c == '\n'));
        rest = after;
    }
    text.push_str(rest);
    text
}

/// Calls `visit(index, character, depth)` for each character outside strings, with the
/// parenthesis depth before that character. Comments must already be removed.
fn scan_outside_strings(css: &str, mut visit: impl FnMut(usize, char, usize) -> bool) {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in css.char_indices() {
        if let Some(open) = quote {
            match character {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                _ if character == open => quote = None,
                _ => {}
            }
            continue;
        }
        if !visit(index, character, depth) {
            return;
        }
        match character {
            '"' | '\'' => quote = Some(character),
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
}

/// Byte index of the `)` closing a group whose contents start at `start` (just after its `(`).
pub fn closing_paren(css: &str, start: usize) -> Option<usize> {
    let mut found = None;
    scan_outside_strings(&css[start..], |index, character, depth| {
        if character == ')' && depth == 0 {
            found = Some(start + index);
            return false;
        }
        true
    });
    found
}

/// The declarations of a rule body, split at `;` outside strings and parentheses.
pub fn top_level_declarations(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut part_start = 0;
    scan_outside_strings(body, |index, character, depth| {
        if character == ';' && depth == 0 {
            parts.push(&body[part_start..index]);
            part_start = index + 1;
        }
        true
    });
    parts.push(&body[part_start..]);
    parts.into_iter().map(str::trim).filter(|part| !part.is_empty()).collect()
}

/// Whether `name` is one custom-property identifier (`--` plus name characters).
pub fn is_custom_property(name: &str) -> bool {
    name.strip_prefix("--").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii())
    })
}

/// The custom properties `var()` references in `css`, each with the byte index of its `var(`.
pub fn var_references(css: &str) -> Vec<(usize, &str)> {
    css.match_indices("var(")
        .filter_map(|(start, call)| {
            let argument = css[start + call.len()..].trim_start();
            let end = argument
                .find(|c: char| {
                    !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii())
                })
                .unwrap_or(argument.len());
            let name = &argument[..end];
            is_custom_property(name).then_some((start, name))
        })
        .collect()
}

/// Byte index of the first `}` at or after `start` outside strings and parentheses.
pub fn rule_end(css: &str, start: usize) -> Option<usize> {
    let mut found = None;
    scan_outside_strings(&css[start..], |index, character, depth| {
        if character == '}' && depth == 0 {
            found = Some(start + index);
            return false;
        }
        true
    });
    found
}
