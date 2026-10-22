//! 超出树构建预算的正文降级为纯文本段落。
//!
//! 只用 html5ever 的分词器（状态机，线性），不进树构建器：文本取自字符 token（实体已解码），
//! script/style 的内容丢弃，块级标签处断段。产物只有 `<p>` 和转义后的文本，不需要再过
//! ammonia。分词器不知道外来内容（svg/math 里的 `<style>` 不是 raw text），这类正文降级时
//! 可能丢掉一段文字，但不会多出任何标记。

use std::cell::RefCell;

use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, EndTag, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
};

/// 把整段正文转成 `<p>` 段落；没有任何可见文字时返回 `None`。
pub(super) fn plain_text_paragraphs(html: &str) -> Option<String> {
    let queue = BufferQueue::default();
    queue.push_back(html.into());
    let tokenizer = Tokenizer::new(TextSink::default(), Default::default());
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
    let text = tokenizer.sink.state.into_inner().text;

    let mut output = String::new();
    for line in text.lines() {
        let mut words = line.split_whitespace().peekable();
        if words.peek().is_none() {
            continue;
        }
        output.push_str("<p>");
        for (index, word) in words.enumerate() {
            if index > 0 {
                output.push(' ');
            }
            push_escaped(&mut output, word);
        }
        output.push_str("</p>");
    }
    (!output.is_empty()).then_some(output)
}

fn push_escaped(output: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            other => output.push(other),
        }
    }
}

#[derive(Default)]
struct TextState {
    text: String,
    /// 正在丢弃其内容的 raw text 元素（script/style）。
    skipping: Option<&'static str>,
}

#[derive(Default)]
struct TextSink {
    state: RefCell<TextState>,
}

impl TokenSink for TextSink {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        let mut state = self.state.borrow_mut();
        match token {
            Token::CharacterTokens(text) => {
                if state.skipping.is_none() {
                    state.text.push_str(&text);
                }
            }
            Token::TagToken(tag) => {
                let name = tag.name.as_ref();
                if let Some(skipped) = state.skipping {
                    if tag.kind == EndTag && name == skipped {
                        state.skipping = None;
                    }
                    return TokenSinkResult::Continue;
                }
                if tag.kind == StartTag {
                    match name {
                        "script" => {
                            state.skipping = Some("script");
                            return TokenSinkResult::RawData(RawKind::ScriptData);
                        }
                        "style" => {
                            state.skipping = Some("style");
                            return TokenSinkResult::RawData(RawKind::Rawtext);
                        }
                        _ => {}
                    }
                }
                if breaks_paragraph(name) {
                    state.text.push('\n');
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

fn breaks_paragraph(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "br"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}

#[cfg(test)]
mod tests {
    use super::plain_text_paragraphs;

    #[test]
    fn keeps_text_breaks_blocks_and_escapes() {
        let output = plain_text_paragraphs(
            "<div><p>a &amp; b &lt;c&gt;</p><p>second <b>line</b></p></div><br>third",
        );
        assert_eq!(
            output.as_deref(),
            Some("<p>a &amp; b &lt;c&gt;</p><p>second line</p><p>third</p>")
        );
    }

    #[test]
    fn drops_script_and_style_contents() {
        let output = plain_text_paragraphs(
            "<p>x</p><script>alert('<p>no</p>')</script><style>p{}</style><p>y</p>",
        );
        assert_eq!(output.as_deref(), Some("<p>x</p><p>y</p>"));
    }

    #[test]
    fn markup_only_body_has_no_text() {
        assert_eq!(plain_text_paragraphs("<div><img src=x></div>"), None);
    }
}
