//! Contracts of the local html5ever patch: order/first value, errors and state
//! across input chunks and successive tags. No wall-clock assertions in CI.
use std::cell::RefCell;

use html5ever::tokenizer::{BufferQueue, Tag, Token, TokenSink, TokenSinkResult, Tokenizer};

#[derive(Default)]
struct Tags {
    tags: RefCell<Vec<Tag>>,
    duplicates: RefCell<usize>,
}

impl TokenSink for Tags {
    type Handle = ();

    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        match token {
            Token::TagToken(tag) => self.tags.borrow_mut().push(tag),
            Token::ParseError(error) if error == "Duplicate attribute" => {
                *self.duplicates.borrow_mut() += 1;
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

fn tokenize(html: &str, chunk_bytes: usize) -> Tags {
    let tokenizer = Tokenizer::new(Tags::default(), Default::default());
    let queue = BufferQueue::default();
    for chunk in html.as_bytes().chunks(chunk_bytes) {
        queue.push_back(std::str::from_utf8(chunk).unwrap().into());
        let _ = tokenizer.feed(&queue);
    }
    tokenizer.end();
    tokenizer.sink
}

#[test]
fn duplicate_attributes_keep_first_value_and_order_across_index_boundary() {
    for count in [0, 15, 16, 17, 256, 10_000] {
        let attrs: String = (0..count).map(|i| format!(" a{i}=v{i}")).collect();
        let html = format!("<p first=original{attrs} FIRST=duplicate last=end first=again>");
        for chunk_bytes in [1, 31, html.len()] {
            let sink = tokenize(&html, chunk_bytes);
            let tags = sink.tags.borrow();
            let tag = &tags[0];
            assert!(tag.had_duplicate_attributes);
            assert_eq!(*sink.duplicates.borrow(), 2);
            assert_eq!(tag.attrs.len(), count + 2);
            assert_eq!(&*tag.attrs[0].name.local, "first");
            assert_eq!(&*tag.attrs[0].value, "original");
            for (i, attr) in tag.attrs[1..=count].iter().enumerate() {
                assert_eq!(&*attr.name.local, format!("a{i}"));
                assert_eq!(&*attr.value, format!("v{i}"));
            }
            assert_eq!(&*tag.attrs.last().unwrap().name.local, "last");
        }
    }
}

#[test]
fn attribute_index_does_not_leak_into_later_tags_or_end_tags() {
    let attrs: String = (0..32).map(|i| format!(" a{i}=value")).collect();
    let html = format!("<p{attrs}></p{attrs}><div a0=fresh><p{attrs} a0=duplicate>");
    let sink = tokenize(&html, 7);
    let tags = sink.tags.borrow();
    assert_eq!(tags.len(), 4);
    assert_eq!(tags[0].attrs.len(), 32);
    assert_eq!(tags[1].attrs.len(), 32);
    assert_eq!(&*tags[2].attrs[0].value, "fresh");
    assert!(!tags[2].had_duplicate_attributes);
    assert!(tags[3].had_duplicate_attributes);
    assert_eq!(*sink.duplicates.borrow(), 1);
}

#[test]
fn unfinished_wide_tag_is_not_emitted_at_eof() {
    let attrs: String = (0..128).map(|i| format!(" a{i}=value")).collect();
    let sink = tokenize(&format!("<p{attrs}"), 11);
    assert!(sink.tags.borrow().is_empty());
}
