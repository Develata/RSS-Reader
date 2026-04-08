pub(super) fn validate_custom_css(raw: &str) -> Result<(), &'static str> {
    let mut stack = Vec::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_comment = false;
    let mut escaped = false;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                let _ = chars.next();
                in_comment = false;
            }
            continue;
        }

        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_single_quote || in_double_quote => escaped = true,
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '/' if !in_single_quote && !in_double_quote && chars.peek() == Some(&'*') => {
                let _ = chars.next();
                in_comment = true;
            }
            '{' | '(' | '[' if !in_single_quote && !in_double_quote => stack.push(ch),
            '}' | ')' | ']' if !in_single_quote && !in_double_quote => {
                let Some(open) = stack.pop() else {
                    return Err("存在未匹配的右括号或右花括号");
                };
                if !matches!((open, ch), ('{', '}') | ('(', ')') | ('[', ']')) {
                    return Err("括号或花括号没有正确配对");
                }
            }
            _ => {}
        }
    }

    if in_comment {
        return Err("注释没有正确闭合");
    }
    if in_single_quote || in_double_quote {
        return Err("字符串引号没有正确闭合");
    }
    if !stack.is_empty() {
        return Err("存在未闭合的括号或花括号");
    }

    Ok(())
}
