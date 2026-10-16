//! Bounds on what feed-rs reads and builds from a feed, checked before it reads the feed.
//!
//! feed-rs builds its whole model of a feed before it is normalised, and the model can be many
//! times the size of the text: an empty `<item/>` becomes an entry of about a kilobyte, and every
//! address in a feed is resolved against the `xml:base` attributes in force and kept as its own
//! string. The checks here bound the nesting, the bases, the attributes of a tag and an estimate
//! of the model, so the work and memory of one parse stay within a fixed budget whatever the
//! feed (reviews C3-R5 R5-02, C3-R6 R6-01–R6-03).

use anyhow::ensure;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use rssr_application::MAX_URL_BYTES;

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

/// The estimated size of what feed-rs may build from one feed, at most.
///
/// Measured at the 8 MiB response limit (feed-rs 2.4.0, peak heap of the parse): a podcast of
/// about 3 000 episodes with full descriptions estimates at 15 MiB (30 MiB measured, most of it
/// the text); a full-text Atom feed of about 4 700 entries with inline XHTML and an `xml:base`
/// at 22 MiB (27 MiB measured). Feeds of tens of thousands of entries with little text each
/// exceed it. The largest floods within it measured 30–68 MiB, and 100 MiB for empty
/// `media:group` elements; before it, 8 MiB of empty items took about 1 GB and links under a
/// long base about 3 GB.
pub(crate) const MAX_FEED_MODEL_BYTES: usize = 64 * 1024 * 1024;
/// Estimated cost of an element feed-rs may build a value from (a link, category, person, media
/// object …): 170–500 bytes measured each, the larger ones while their lists grow.
const ELEMENT_COST: usize = 256;
/// Added for an `item` or `entry` element: an entry and its normalised copy, about 900 bytes
/// measured each.
const ENTRY_COST: usize = 1024;
/// A JSON Feed object (an item, author, attachment …): about 2 000 bytes measured per item.
const JSON_OBJECT_COST: usize = 2048;
/// A JSON string that is a value, not a key: a tag becomes a category of about 200 bytes.
const JSON_STRING_COST: usize = 256;

const ATOM_NAMESPACE: &[u8] = b"http://www.w3.org/2005/Atom";
/// The Atom text constructs: feed-rs keeps everything inside them as text (or skips it), so the
/// elements there build nothing of their own.
const ATOM_TEXT_CONSTRUCTS: [&[u8]; 5] = [b"title", b"subtitle", b"summary", b"rights", b"content"];

/// Refuses a feed feed-rs would read with more than [`MAX_XML_DEPTH`] nested elements, more
/// than [`MAX_XML_BASES`] `xml:base` attributes in force at once, an `xml:base` longer than
/// [`MAX_URL_BYTES`], a tag of more than [`MAX_XML_ATTRIBUTES`] attributes, or a model estimated
/// over [`MAX_FEED_MODEL_BYTES`].
///
/// Read with the quick-xml version and reader settings feed-rs uses, so the elements are the ones
/// it sees, and a document this reader cannot read is refused here, as feed-rs would refuse it
/// (review C3-R5 R5-03). Attributes are read as feed-rs reads them, duplicate checks included,
/// skipping those it cannot read: after a repeated name quick-xml resumes reading inside the
/// value, and both readers then see the same attributes (review C3-R6 R6-02).
///
/// Every element outside an Atom text construct is charged [`ELEMENT_COST`], plus
/// [`ENTRY_COST`] if it is an `item` or `entry`, plus three times the bytes of the bases in force
/// (an address resolved against them, percent-encoded) for the element and for each of its
/// attributes. Text and attribute values are copied about once and are not charged.
pub(crate) fn check_feed_structure(raw: &str) -> anyhow::Result<()> {
    let mut reader = quick_xml::NsReader::from_str(raw);
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);
    // For every open element: the length of the `xml:base` it sets, if any.
    let mut open: Vec<Option<usize>> = Vec::new();
    let mut bases = 0_usize;
    let mut base_bytes = 0_usize;
    // The depth of the Atom text construct being read, if any.
    let mut text_construct = None;
    let mut model = 0_usize;
    loop {
        match reader.read_resolved_event()? {
            (namespace, Event::Start(tag)) => {
                ensure!(open.len() < MAX_XML_DEPTH, "feed 的元素嵌套超过 {MAX_XML_DEPTH} 层");
                let mut attributes = 0_usize;
                let mut base = None;
                for attribute in tag.attributes() {
                    attributes += 1;
                    ensure!(
                        attributes <= MAX_XML_ATTRIBUTES,
                        "feed 的标签属性超过 {MAX_XML_ATTRIBUTES} 个"
                    );
                    // feed-rs skips attributes it cannot read; so does this.
                    let Ok(attribute) = attribute else { continue };
                    if attribute.key.as_ref() == b"xml:base" {
                        ensure!(
                            attribute.value.len() <= MAX_URL_BYTES,
                            "feed 的 xml:base 超过 {MAX_URL_BYTES} 字节"
                        );
                        base = Some(attribute.value.len());
                    }
                }
                if let Some(length) = base {
                    bases += 1;
                    base_bytes += length;
                    ensure!(bases <= MAX_XML_BASES, "feed 的 xml:base 嵌套超过 {MAX_XML_BASES} 层");
                }
                if text_construct.is_none() {
                    let local_name = tag.local_name();
                    let entry = matches!(local_name.as_ref(), b"item" | b"entry");
                    let cost = ELEMENT_COST
                        + if entry { ENTRY_COST } else { 0 }
                        + 3 * base_bytes * (1 + attributes);
                    model = model.saturating_add(cost);
                    ensure!(model <= MAX_FEED_MODEL_BYTES, "feed 的条目与地址过多");
                    let atom = matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == ATOM_NAMESPACE);
                    if atom && ATOM_TEXT_CONSTRUCTS.contains(&local_name.as_ref()) {
                        text_construct = Some(open.len());
                    }
                }
                open.push(base);
            }
            (_, Event::End(_)) => {
                if let Some(Some(length)) = open.pop() {
                    bases -= 1;
                    base_bytes -= length;
                }
                if text_construct == Some(open.len()) {
                    text_construct = None;
                }
            }
            (_, Event::Eof) => return Ok(()),
            _ => {}
        }
    }
}

/// Refuses a JSON Feed whose model is estimated over [`MAX_FEED_MODEL_BYTES`]: every object is
/// charged [`JSON_OBJECT_COST`] and every string that is not a key [`JSON_STRING_COST`]. Only
/// strings are tracked, so a document that is not JSON is estimated as if it were; feed-rs
/// rejects it afterwards.
pub(crate) fn check_json_feed_structure(raw: &str) -> anyhow::Result<()> {
    let bytes = raw.as_bytes();
    let mut model = 0_usize;
    let mut at = 0;
    while at < bytes.len() {
        let cost = match bytes[at] {
            b'{' => JSON_OBJECT_COST,
            b'"' => {
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
                let key = bytes.get(at + 1..).and_then(|rest| {
                    rest.iter().find(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
                }) == Some(&b':');
                if key { 0 } else { JSON_STRING_COST }
            }
            _ => 0,
        };
        model = model.saturating_add(cost);
        ensure!(model <= MAX_FEED_MODEL_BYTES, "feed 的条目过多");
        at += 1;
    }
    Ok(())
}
