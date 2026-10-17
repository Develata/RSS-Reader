//! Bounds on what feed-rs reads and builds from a feed, checked before it reads the feed.
//!
//! feed-rs builds its whole model of a feed before it is normalised, and the model can be many
//! times the size of the text: an empty `<item/>` becomes an entry of about a kilobyte, every
//! address in a feed is resolved against the `xml:base` in force and kept as its own string, and a
//! few values are copied into many places. The checks here bound the nesting, the bases, the
//! attributes of a tag, the copying and an estimate of the model, so that the work and memory of
//! one parse follow the estimate rather than the shape of the feed (reviews C3-R5 R5-02, C3-R6,
//! C3-R7, C3-R8). The estimate is calibrated by measurement, not a proven bound on the heap.
//!
//! The estimate mirrors feed-rs 2.4.0, which the workspace pins. Its parsers (`src/parser`) and
//! XML layer (`src/xml`) were read for every loop, copy and panic: the copies that grow faster
//! than the text are the JSON Feed author inheritance and author comparison, and the Media RSS
//! rating propagation (into `media:content` and RSS `enclosure`); serde reads a JSON object
//! written as an array; and the Media RSS time parser (`parse_npt`) panics on numbers it cannot
//! hold. All are charged, bounded or refused here. A new feed-rs version needs the same reading.

use std::borrow::Cow;

use anyhow::ensure;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use rssr_application::MAX_URL_BYTES;
use url::Url;

/// Elements a feed may nest at most. feed-rs walks a feed recursively and keeps, for every open
/// element, a copy of the base address in force.
pub(crate) const MAX_XML_DEPTH: usize = 256;
/// `xml:base` attributes in force at once at most. feed-rs resolves each against the one above it
/// and keeps them all until their elements close, so nested relative bases grow the retained
/// addresses with their number squared (review C3-R5 R5-02).
pub(crate) const MAX_XML_BASES: usize = 16;
/// Attributes one start tag may have at most, counting those that cannot be read. quick-xml finds
/// a repeated name, or a name whose hash (fixed keys, so collisions can be precomputed) matches an
/// earlier one, by scanning the tag's earlier names; this bounds each scan (review C3-R6 R6-01).
/// Real feeds and OPML files use a few attributes per tag.
pub(crate) const MAX_XML_ATTRIBUTES: usize = 256;
/// Ratings one media object (an item, entry or `media:group`) may have at most. feed-rs copies each
/// into every content of the object read before it, walking them all (review C3-R7 R7A-02). Real
/// feeds give one.
pub(crate) const MAX_MEDIA_RATINGS: usize = 16;
/// Authors one JSON Feed author list may have at most. feed-rs compares each with the ones before
/// it (review C3-R7 R7A-04). Real feeds list a few.
pub(crate) const MAX_JSON_AUTHORS: usize = 64;
/// Objects and arrays a JSON Feed may nest at most: serde_json's own limit, so nothing feed-rs
/// would read is refused, and the scan keeps a bounded stack (review C3-R8 R8C-01).
pub(crate) const MAX_JSON_DEPTH: usize = 128;
/// Digits a Media RSS time or iTunes duration may run to. feed-rs parses each run into a `u64`
/// and multiplies the hours by 3 600, and panics when it cannot (review C3-R8 R8A-04).
pub(crate) const MAX_TIME_DIGITS: usize = 12;

/// The estimated size of what feed-rs may build from one feed, at most.
///
/// Measured at the 8 MiB response limit (feed-rs 2.4.0, peak heap of the parse): a podcast of
/// about 3 000 episodes with full descriptions estimates at 15 MiB (30 MiB measured, most of it
/// the text); a full-text Atom feed of about 4 700 entries with inline XHTML and an `xml:base`
/// at 22 MiB (27 MiB measured). Feeds of tens of thousands of entries with little text each
/// exceed it. The estimate is not a proven bound on the heap: the largest inputs within it measured
/// so far peaked at 30–100 MiB (empty `media:group` elements the most); before it, 8 MiB of empty
/// items took about 1 GB and links under a long base about 3 GB.
pub(crate) const MAX_FEED_MODEL_BYTES: usize = 64 * 1024 * 1024;
/// Estimated cost of an element feed-rs may build a value from (a link, category, person, media
/// object …): 170–500 bytes measured each, the larger ones while their lists grow.
const ELEMENT_COST: usize = 256;
/// Added for an `item` or `entry` element: an entry and its normalised copy, about 900 bytes
/// measured each.
const ENTRY_COST: usize = 1024;
/// A media rating copied into a content: two strings and their headers, besides their text.
const RATING_COST: usize = 64;
/// A JSON Feed object (an item, author, attachment …): about 2 000 bytes measured per item.
const JSON_OBJECT_COST: usize = 2048;
/// A JSON string that is a value, not a key: a tag becomes a category of about 200 bytes.
const JSON_STRING_COST: usize = 256;
/// A feed author copied into an item, besides its text: a person and its headers.
const JSON_AUTHOR_COST: usize = 128;

const ATOM_NAMESPACE: &[u8] = b"http://www.w3.org/2005/Atom";
/// The Atom text constructs: feed-rs keeps everything inside them as text (or skips it), so the
/// elements there build nothing of their own.
const ATOM_TEXT_CONSTRUCTS: [&[u8]; 5] = [b"title", b"subtitle", b"summary", b"rights", b"content"];

/// Refuses a feed feed-rs would read with more than [`MAX_XML_DEPTH`] nested elements, more
/// than [`MAX_XML_BASES`] `xml:base` attributes in force at once, an `xml:base` longer than
/// [`MAX_URL_BYTES`], a tag of more than [`MAX_XML_ATTRIBUTES`] attributes, more than
/// [`MAX_MEDIA_RATINGS`] ratings in one media object, or a model estimated over
/// [`MAX_FEED_MODEL_BYTES`] (see [`xml_model`]).
pub(crate) fn check_feed_structure(raw: &str) -> anyhow::Result<()> {
    xml_model(raw, MAX_FEED_MODEL_BYTES).map(drop)
}

/// Refuses a JSON Feed with an author list of more than [`MAX_JSON_AUTHORS`] authors or a model
/// estimated over [`MAX_FEED_MODEL_BYTES`] (see [`json_model`]).
pub(crate) fn check_json_feed_structure(raw: &str) -> anyhow::Result<()> {
    json_model(raw, MAX_FEED_MODEL_BYTES).map(drop)
}

/// The part of a name after its last colon, which feed-rs matches (`xml/mod.rs`, `parse_name`).
/// quick-xml's local name is what follows the first colon (review C3-R7 R7A-03).
fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|&byte| byte == b':').next().unwrap_or(name)
}

/// The base feed-rs puts in force for the `xml:base` value `raw` under `parent`
/// (`xml/mod.rs`, `xml_base_push`): an absolute address replaces the parent, a relative one is
/// resolved against it, and one that cannot be resolved puts nothing in force.
fn resolve_base(raw: &[u8], parent: Option<&Url>) -> Option<Url> {
    let decoded = std::str::from_utf8(raw).ok()?;
    let value = quick_xml::escape::unescape(decoded).unwrap_or(Cow::Borrowed(decoded));
    match Url::parse(&value) {
        Ok(url) => Some(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => parent?.join(&value).ok(),
        Err(_) => None,
    }
}

/// A media object being read (an item, entry or `media:group`), or a `media:content`: a rating
/// inside a content is its own, not copied (`mediarss.rs`, `handle_media_content`).
struct MediaScope {
    depth: usize,
    content: bool,
    contents: usize,
    ratings: usize,
}

/// Refuses a Media RSS time or iTunes duration feed-rs would panic on (`parser/util.rs`,
/// `parse_npt`): a decimal digit outside ASCII, or a run of more than [`MAX_TIME_DIGITS`] digits.
/// `run` carries the run across the pieces of one value.
fn check_time(run: &mut usize, text: &str) -> anyhow::Result<()> {
    for character in text.chars() {
        if character.is_ascii_digit() {
            *run += 1;
            ensure!(*run <= MAX_TIME_DIGITS, "feed 的时长或时间数字过长");
        } else {
            ensure!(!character.is_numeric(), "feed 的时长或时间含非 ASCII 数字");
            *run = 0;
        }
    }
    Ok(())
}

/// The text feed-rs makes of a reference (`xml/mod.rs`, `fetch_next`): the character, the
/// predefined entity, or the reference kept as written.
fn reference_text(reference: &quick_xml::events::BytesRef<'_>) -> String {
    let name = String::from_utf8_lossy(reference.as_ref());
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return character.to_string();
    }
    match quick_xml::escape::resolve_predefined_entity(&name) {
        Some(resolved) => resolved.to_owned(),
        None => format!("&{name};"),
    }
}

/// A media rating being read: the contents it is copied into and its bytes so far.
struct Rating {
    depth: usize,
    contents: usize,
    bytes: usize,
}

/// The estimated model of an XML feed, refused past `limit` (and past the structure bounds).
///
/// Read with the quick-xml version and reader settings feed-rs uses, so the elements are the ones
/// it sees, and a document this reader cannot read is refused here, as feed-rs would refuse it
/// (review C3-R5 R5-03). Attributes are read as feed-rs reads them, duplicate checks included,
/// skipping those it cannot read: after a repeated name quick-xml resumes reading inside the
/// value, and both readers then see the same attributes (review C3-R6 R6-02). Names are matched
/// after their last colon, as feed-rs matches them.
///
/// Every element outside an Atom text construct is charged [`ELEMENT_COST`], plus
/// [`ENTRY_COST`] if it is an `item` or `entry`, plus the length of the base in force (resolved
/// and serialised as feed-rs does, so percent-encoding and international domain names are
/// counted) for the element and for each of its attributes. A rating of a media object (not one
/// inside a `media:content`) is charged, for every content or enclosure read before it in the
/// object, [`RATING_COST`] and its bytes, an unknown reference counted with its delimiters as
/// feed-rs keeps it. Times and durations feed-rs parses are checked with [`check_time`]. Text and attribute values
/// are copied a few times (decoding, normalisation, percent-encoding and international domain
/// names in addresses): that grows with the length of the text, not with its structure, and is not
/// charged.
fn xml_model(raw: &str, limit: usize) -> anyhow::Result<usize> {
    let mut reader = quick_xml::NsReader::from_str(raw);
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);
    // For every open element: whether it has an `xml:base`, and whether that put a base in force.
    let mut open: Vec<(bool, bool)> = Vec::new();
    let mut bases = 0_usize;
    let mut in_force: Vec<Url> = Vec::new();
    // The depth of the Atom text construct being read, if any.
    let mut text_construct = None;
    let mut scopes: Vec<MediaScope> = Vec::new();
    let mut rating: Option<Rating> = None;
    // The depth of the duration being read and its current run of digits.
    let mut duration: Option<(usize, usize)> = None;
    let mut model = 0_usize;
    loop {
        match reader.read_resolved_event()? {
            (namespace, Event::Start(tag)) => {
                ensure!(open.len() < MAX_XML_DEPTH, "feed 的元素嵌套超过 {MAX_XML_DEPTH} 层");
                let mut attributes = 0_usize;
                let mut attribute_bytes = 0_usize;
                let mut base = None;
                for attribute in tag.attributes() {
                    attributes += 1;
                    ensure!(
                        attributes <= MAX_XML_ATTRIBUTES,
                        "feed 的标签属性超过 {MAX_XML_ATTRIBUTES} 个"
                    );
                    // feed-rs skips attributes it cannot read; so does this.
                    let Ok(attribute) = attribute else { continue };
                    attribute_bytes += attribute.value.len();
                    if attribute.key.as_ref() == b"xml:base" {
                        ensure!(
                            attribute.value.len() <= MAX_URL_BYTES,
                            "feed 的 xml:base 超过 {MAX_URL_BYTES} 字节"
                        );
                        base = Some(attribute.value);
                    }
                }
                let mut pushed = false;
                if let Some(value) = &base {
                    bases += 1;
                    ensure!(bases <= MAX_XML_BASES, "feed 的 xml:base 嵌套超过 {MAX_XML_BASES} 层");
                    if let Some(url) = resolve_base(value, in_force.last()) {
                        in_force.push(url);
                        pushed = true;
                    }
                }
                if text_construct.is_none() {
                    let name = tag.name();
                    let local = local_name(name.as_ref());
                    let entry = matches!(local, b"item" | b"entry");
                    let base_length = in_force.last().map_or(0, |url| url.as_str().len());
                    let cost = ELEMENT_COST
                        + if entry { ENTRY_COST } else { 0 }
                        + base_length * (1 + attributes);
                    model = model.saturating_add(cost);
                    ensure!(model <= limit, "feed 的条目与地址过多");
                    let atom = matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == ATOM_NAMESPACE);
                    if atom && ATOM_TEXT_CONSTRUCTS.contains(&local) {
                        text_construct = Some(open.len());
                    } else if matches!(local, b"item" | b"entry" | b"group") {
                        let depth = open.len();
                        scopes.push(MediaScope { depth, content: false, contents: 0, ratings: 0 });
                    } else if matches!(local, b"content" | b"enclosure") {
                        // feed-rs adds both to the contents of the media object around them.
                        if let Some(scope) = scopes.iter_mut().rev().find(|scope| !scope.content) {
                            scope.contents += 1;
                        }
                        if local == b"content" {
                            let depth = open.len();
                            scopes.push(MediaScope {
                                depth,
                                content: true,
                                contents: 0,
                                ratings: 0,
                            });
                        }
                    } else if local == b"rating"
                        && rating.is_none()
                        && let Some(scope) = scopes.last_mut()
                        && !scope.content
                    {
                        scope.ratings += 1;
                        ensure!(
                            scope.ratings <= MAX_MEDIA_RATINGS,
                            "feed 的一个媒体对象评级超过 {MAX_MEDIA_RATINGS} 个"
                        );
                        rating = Some(Rating {
                            depth: open.len(),
                            contents: scope.contents,
                            bytes: attribute_bytes,
                        });
                    }
                }
                if text_construct.is_none() {
                    let name = tag.name();
                    let local = local_name(name.as_ref());
                    let times: &[&[u8]] = match local {
                        b"thumbnail" => &[b"time"],
                        b"text" => &[b"start", b"end"],
                        _ => &[],
                    };
                    for attribute in tag.attributes().flatten() {
                        if times.contains(&attribute.key.as_ref()) {
                            let value = String::from_utf8_lossy(&attribute.value);
                            let value = quick_xml::escape::unescape(&value)
                                .map(|text| text.into_owned())
                                .unwrap_or_else(|_| value.into_owned());
                            check_time(&mut 0, &value)?;
                        }
                    }
                    if local == b"duration" && duration.is_none() {
                        duration = Some((open.len(), 0));
                    }
                }
                open.push((base.is_some(), pushed));
            }
            (_, Event::Text(text)) => {
                add_rating_bytes(&mut rating, text.len());
                if let Some((_, run)) = &mut duration {
                    check_time(run, &String::from_utf8_lossy(&text))?;
                }
            }
            (_, Event::CData(text)) => {
                add_rating_bytes(&mut rating, text.len());
                if let Some((_, run)) = &mut duration {
                    check_time(run, &String::from_utf8_lossy(&text))?;
                }
            }
            (_, Event::GeneralRef(reference)) => {
                // feed-rs keeps an unknown reference with its `&` and `;` (review C3-R8 R8A-03).
                add_rating_bytes(&mut rating, reference.len() + 2);
                if let Some((_, run)) = &mut duration {
                    check_time(run, &reference_text(&reference))?;
                }
            }
            (_, Event::End(_)) => {
                if let Some((has_base, pushed)) = open.pop() {
                    bases -= usize::from(has_base);
                    if pushed {
                        in_force.pop();
                    }
                }
                let depth = open.len();
                if text_construct == Some(depth) {
                    text_construct = None;
                }
                if let Some(read) = rating.take_if(|read| read.depth == depth) {
                    let copy = RATING_COST.saturating_add(read.bytes);
                    model = model.saturating_add(read.contents.saturating_mul(copy));
                    ensure!(model <= limit, "feed 的条目与地址过多");
                }
                if scopes.last().is_some_and(|scope| scope.depth == depth) {
                    scopes.pop();
                }
                if duration.is_some_and(|(at, _)| at == depth) {
                    duration = None;
                }
            }
            (_, Event::Eof) => return Ok(model),
            _ => {}
        }
    }
}

fn add_rating_bytes(rating: &mut Option<Rating>, bytes: usize) {
    if let Some(rating) = rating {
        rating.bytes = rating.bytes.saturating_add(bytes);
    }
}

/// What a JSON value is to feed-rs, by where it is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Other,
    Root,
    Items,
    Item,
    FeedAuthors,
    FeedAuthor,
    ItemAuthors,
    ItemAuthor,
}

/// An open JSON object or array: its role, the role of its next value (from the key before it),
/// and the authors it holds if it is an author list.
struct Frame {
    role: Role,
    next: Role,
    authors: usize,
}

/// What a value under `key` of an object in `role` is.
fn role_of(role: Role, key: &str) -> Role {
    match (role, key) {
        (Role::Root, "items") => Role::Items,
        (Role::Root, "author") => Role::FeedAuthor,
        (Role::Root, "authors") => Role::FeedAuthors,
        (Role::Item, "author") => Role::ItemAuthor,
        (Role::Item, "authors") => Role::ItemAuthors,
        _ => Role::Other,
    }
}

/// The estimated model of a JSON Feed, refused past `limit` (and past [`MAX_JSON_AUTHORS`]).
///
/// Every object is charged [`JSON_OBJECT_COST`] and every string that is not a key
/// [`JSON_STRING_COST`]. feed-rs copies the feed's authors into every item that names none, and the
/// normalisation copies an item's first author: every item is charged the feed's authors, each
/// [`JSON_AUTHOR_COST`] and its strings (review C3-R7 R7A-01). Keys are compared after their
/// escapes are decoded, as serde matches them. serde also reads an object from an array of its
/// fields in order, which JSON Feed does not allow: an array where the root, an item or an author
/// is expected is refused, so every one of them is counted (review C3-R8 R8A-01). Nesting deeper
/// than [`MAX_JSON_DEPTH`] is refused. The scan follows strings, objects and arrays only, so a
/// document that is not JSON is estimated as if it were; feed-rs rejects it afterwards.
fn json_model(raw: &str, limit: usize) -> anyhow::Result<usize> {
    let bytes = raw.as_bytes();
    let mut frames: Vec<Frame> = Vec::new();
    let mut model = 0_usize;
    let mut items = 0_usize;
    let mut feed_authors = 0_usize;
    let mut feed_author_bytes = 0_usize;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            open @ (b'{' | b'[') => {
                let role = match frames.last_mut() {
                    None => Role::Root,
                    Some(parent) => match parent.role {
                        Role::Items => Role::Item,
                        Role::FeedAuthors => Role::FeedAuthor,
                        Role::ItemAuthors => Role::ItemAuthor,
                        _ => std::mem::replace(&mut parent.next, Role::Other),
                    },
                };
                ensure!(frames.len() < MAX_JSON_DEPTH, "feed 的 JSON 嵌套超过 {MAX_JSON_DEPTH} 层");
                ensure!(
                    open == b'{'
                        || !matches!(
                            role,
                            Role::Root | Role::Item | Role::FeedAuthor | Role::ItemAuthor
                        ),
                    "feed 的 JSON 对象写成了数组"
                );
                if open == b'{' {
                    model = model.saturating_add(JSON_OBJECT_COST);
                    if let Some(parent) = frames.last_mut()
                        && matches!(parent.role, Role::FeedAuthors | Role::ItemAuthors)
                    {
                        parent.authors += 1;
                        ensure!(
                            parent.authors <= MAX_JSON_AUTHORS,
                            "feed 的作者列表超过 {MAX_JSON_AUTHORS} 个"
                        );
                    }
                    items += usize::from(role == Role::Item);
                    feed_authors += usize::from(role == Role::FeedAuthor);
                }
                // Roots and items are objects, author lists arrays: anything else is not one.
                let role = match (open, role) {
                    (b'{', Role::Root | Role::Item | Role::FeedAuthor | Role::ItemAuthor)
                    | (b'[', Role::Items | Role::FeedAuthors | Role::ItemAuthors) => role,
                    _ => Role::Other,
                };
                frames.push(Frame { role, next: Role::Other, authors: 0 });
            }
            b'}' | b']' => {
                frames.pop();
            }
            b'"' => {
                let start = at;
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
                let end = at.min(bytes.len());
                let key = bytes.get(at + 1..).and_then(|rest| {
                    rest.iter().find(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
                }) == Some(&b':');
                if key {
                    if let Some(frame) = frames.last_mut() {
                        let text = &raw[start + 1..end];
                        let decoded = if text.contains('\\') {
                            serde_json::from_str::<String>(&raw[start..=end.min(raw.len() - 1)])
                                .map(Cow::Owned)
                                .unwrap_or(Cow::Borrowed(text))
                        } else {
                            Cow::Borrowed(text)
                        };
                        frame.next = role_of(frame.role, &decoded);
                    }
                } else {
                    model = model.saturating_add(JSON_STRING_COST);
                    match frames.last_mut() {
                        Some(frame) if frame.role == Role::FeedAuthor => {
                            feed_author_bytes = feed_author_bytes.saturating_add(end - start);
                        }
                        Some(frame) => frame.next = Role::Other,
                        None => {}
                    }
                }
            }
            _ => {}
        }
        ensure!(model <= limit, "feed 的条目过多");
        at += 1;
    }
    let author = JSON_AUTHOR_COST.saturating_mul(feed_authors).saturating_add(feed_author_bytes);
    model = model.saturating_add(items.saturating_mul(author));
    ensure!(model <= limit, "feed 的条目过多");
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml(raw: &str) -> usize {
        xml_model(raw, usize::MAX).unwrap()
    }

    fn json(raw: &str) -> usize {
        json_model(raw, usize::MAX).unwrap()
    }

    const ATOM: &str = "http://www.w3.org/2005/Atom";

    #[test]
    fn elements_and_entries_are_charged() {
        let rss = r#"<rss version="2.0"><channel><title>t</title><item/></channel></rss>"#;
        assert_eq!(xml(rss), 4 * ELEMENT_COST + ENTRY_COST);
    }

    #[test]
    fn names_are_matched_after_their_last_colon() {
        // feed-rs reads `a:b:entry` as an Atom `entry`; quick-xml's local name is `b:entry`.
        let feed = format!(r#"<feed xmlns="{ATOM}" xmlns:a="{ATOM}"><a:b:entry/></feed>"#);
        assert_eq!(xml(&feed), 2 * ELEMENT_COST + ENTRY_COST);
        // And `a:b:content` is an Atom text construct: what is inside it is text.
        let feed = format!(r#"<feed xmlns:a="{ATOM}"><a:b:content><x/><y/></a:b:content></feed>"#);
        assert_eq!(xml(&feed), 2 * ELEMENT_COST);
    }

    #[test]
    fn the_base_in_force_is_charged_as_feed_rs_resolves_it() {
        let base = "https://example.com/a/";
        let feed = format!(r#"<feed xmlns="{ATOM}" xml:base="{base}"><link href="x"/></feed>"#);
        // The feed element has two attributes, the link one; the base applies to both.
        assert_eq!(xml(&feed), 2 * ELEMENT_COST + base.len() * 3 + base.len() * 2);
        // A relative base under it is resolved against it; one without a base above it puts
        // nothing in force.
        let feed = format!(r#"<feed xml:base="{base}"><x xml:base="b/"/></feed>"#);
        let resolved = "https://example.com/a/b/".len();
        assert_eq!(xml(&feed), 2 * ELEMENT_COST + base.len() * 2 + resolved * 2);
        assert_eq!(xml(r#"<feed xml:base="b/"><x/></feed>"#), 2 * ELEMENT_COST);
        // International domain names are counted as serialised (review C3-R7 R7A-05).
        let host = "\u{130}.".repeat(20);
        let base = format!("https://{host}/");
        let serialized = Url::parse(&base).unwrap().as_str().len();
        assert!(serialized > 3 * base.len(), "{serialized}");
        let feed = format!(r#"<feed xml:base="{base}"/>"#);
        assert_eq!(xml(&feed), ELEMENT_COST + serialized * 2);
    }

    #[test]
    fn a_rating_is_charged_for_each_content_it_is_copied_into() {
        let media = |inner: &str| {
            format!(r#"<rss xmlns:media="m"><channel><item>{inner}</item></channel></rss>"#)
        };
        let base = xml(&media(""));
        let contents = r#"<media:content url="a"/><media:content url="b"/>"#;
        let rated = media(&format!(r#"{contents}<media:rating scheme="s">abc</media:rating>"#));
        // Three elements, and two copies of a rating of one attribute byte and three text bytes.
        assert_eq!(xml(&rated), base + 3 * ELEMENT_COST + 2 * (RATING_COST + 4));
        // Contents after the rating, or in another group, receive no copy.
        let later = media(&format!(r#"<media:rating>abc</media:rating>{contents}"#));
        assert_eq!(xml(&later), base + 3 * ELEMENT_COST);
        let grouped = media(&format!(
            r#"<media:group>{contents}</media:group><media:rating>abc</media:rating>"#
        ));
        assert_eq!(xml(&grouped), base + 4 * ELEMENT_COST);
        // A media object takes at most MAX_MEDIA_RATINGS ratings.
        let ratings = |count| media(&"<media:rating>r</media:rating>".repeat(count));
        assert!(xml_model(&ratings(MAX_MEDIA_RATINGS), usize::MAX).is_ok());
        assert!(xml_model(&ratings(MAX_MEDIA_RATINGS + 1), usize::MAX).is_err());
        // RSS enclosures receive copies too (review C3-R8 R8A-02).
        let enclosures = r#"<enclosure url="a"/><enclosure url="b"/>"#;
        let rated = media(&format!(r#"{enclosures}<media:rating>abc</media:rating>"#));
        assert_eq!(xml(&rated), base + 3 * ELEMENT_COST + 2 * (RATING_COST + 3));
        // A rating inside a content is its own: no copies, and it takes no place in the object's
        // ratings (review C3-R8 R8A-05).
        let own = r#"<media:content url="a"><media:rating>r</media:rating></media:content>"#;
        let gallery = media(&own.repeat(MAX_MEDIA_RATINGS + 1));
        assert_eq!(xml(&gallery), base + 2 * (MAX_MEDIA_RATINGS + 1) * ELEMENT_COST);
        // An unknown reference is kept with its delimiters (review C3-R8 R8A-03).
        let referenced = media(&format!(r#"{contents}<media:rating>&x;&x;</media:rating>"#));
        assert_eq!(xml(&referenced), base + 3 * ELEMENT_COST + 2 * (RATING_COST + 6));
    }

    #[test]
    fn times_feed_rs_cannot_hold_are_refused() {
        let item = |inner: &str| {
            format!(
                r#"<rss xmlns:itunes="i" xmlns:media="m"><channel><item>{inner}</item></channel></rss>"#
            )
        };
        let ok = |inner: &str| xml_model(&item(inner), usize::MAX).is_ok();
        let digits = |count: usize| "9".repeat(count);
        let twelve = digits(MAX_TIME_DIGITS);
        let thirteen = digits(MAX_TIME_DIGITS + 1);
        assert!(ok(&format!("<itunes:duration>{twelve}:00:00.{twelve}</itunes:duration>")));
        assert!(!ok(&format!("<itunes:duration>{thirteen}</itunes:duration>")));
        assert!(!ok(&format!("<itunes:duration>0.{thirteen}</itunes:duration>")));
        // A run split by references is still one run in the text feed-rs reads.
        let split = format!("{}&#57;{}", digits(6), digits(6));
        assert!(!ok(&format!("<itunes:duration>{split}</itunes:duration>")));
        // A decimal digit outside ASCII, written or referenced (U+0663 ARABIC-INDIC DIGIT THREE).
        let arabic = char::from_u32(0x663).unwrap();
        assert!(!ok(&format!("<itunes:duration>1{arabic}</itunes:duration>")));
        assert!(!ok("<itunes:duration>&#x663;</itunes:duration>"));
        assert!(!ok(&format!(r#"<media:thumbnail url="a" time="{thirteen}"/>"#)));
        assert!(!ok(&format!(r#"<media:text start="{thirteen}">t</media:text>"#)));
        assert!(!ok(&format!(r#"<media:text end="{thirteen}">t</media:text>"#)));
        // Other attributes, and these names on other elements, are not times.
        assert!(ok(&format!(r#"<media:thumbnail url="{thirteen}" time="1"/>"#)));
        assert!(ok(&format!(r#"<media:content time="{thirteen}"/>"#)));
    }

    #[test]
    fn a_json_feed_is_charged_for_its_objects_strings_and_inherited_authors() {
        let feed = r#"{"version":"v","title":"t","author":{"name":"ab","url":"cd"},
            "items":[{"id":"1"},{"id":"2","authors":[{"name":"x"}]}]}"#;
        // Five objects, seven value strings, and two items each charged the feed's author.
        let author = JSON_AUTHOR_COST + "\"ab".len() + "\"cd".len();
        assert_eq!(json(feed), 5 * JSON_OBJECT_COST + 7 * JSON_STRING_COST + 2 * author);
        // Escaped keys are the same keys to serde: `author` with its first letter as `u0061` after
        // a backslash (built here, so no tool or editor decodes it first).
        let escape =
            |key: &str| format!("{}u{:04x}{}", char::from(92), key.as_bytes()[0], &key[1..]);
        let escaped = format!(
            r#"{{"{}":{{"name":"ab","url":"cd"}},"{}":[{{"id":"1"}}]}}"#,
            escape("author"),
            escape("items")
        );
        assert!(escaped.contains("u0061uthor"), "{escaped}");
        assert_eq!(
            json(&escaped),
            3 * JSON_OBJECT_COST + 3 * JSON_STRING_COST + JSON_AUTHOR_COST + 6
        );
        // serde reads an object from an array of its fields: where an object is counted, an array
        // is refused (review C3-R8 R8A-01).
        let rejected = |raw: &str| json_model(raw, usize::MAX).is_err();
        assert!(rejected(r#"["v","t"]"#));
        assert!(rejected(r#"{"items":[["1"]]}"#));
        assert!(rejected(r#"{"author":["a","https://example.com/"],"items":[]}"#));
        assert!(rejected(r#"{"authors":[["a",null]],"items":[]}"#));
        assert!(rejected(r#"{"items":[{"id":"1","author":["a",null]}]}"#));
        assert!(rejected(r#"{"items":[{"id":"1","authors":[["a",null]]}]}"#));
        assert!(!rejected(r#"{"items":[{"id":"1","tags":["a"],"attachments":[]}]}"#));
        // Nesting is bounded like serde_json's (review C3-R8 R8C-01).
        let nested =
            |depth: usize| format!(r#"{{"x":{}{}}}"#, "[".repeat(depth - 1), "]".repeat(depth - 1));
        assert!(!rejected(&nested(MAX_JSON_DEPTH)));
        assert!(rejected(&nested(MAX_JSON_DEPTH + 1)));
        // An author list takes at most MAX_JSON_AUTHORS authors.
        let authors = |count| {
            let list = vec![r#"{"name":"a"}"#; count].join(",");
            format!(r#"{{"items":[{{"id":"1","authors":[{list}]}}]}}"#)
        };
        assert!(json_model(&authors(MAX_JSON_AUTHORS), usize::MAX).is_ok());
        assert!(json_model(&authors(MAX_JSON_AUTHORS + 1), usize::MAX).is_err());
    }
}
