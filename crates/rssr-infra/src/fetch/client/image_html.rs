use std::collections::BTreeMap;

use url::Url;

const IMG_LAZY_SOURCE_ATTRIBUTES: &[&str] =
    &["data-src", "data-original", "data-lazy-src", "data-orig-file"];
const IMG_SRCSET_ATTRIBUTES: &[&str] = &["data-srcset", "srcset"];
const SOURCE_SRCSET_ATTRIBUTES: &[&str] = &["data-srcset", "srcset"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalizableImageDocument {
    tags: Vec<HtmlTag>,
    slots: Vec<ImageSlot>,
    image_markup_count: usize,
    unresolved_slot_count: usize,
    unsupported_slot_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageSlot {
    resolved_url: Url,
    rewrite_targets: Vec<ImageRewriteTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageRewriteKind {
    Img,
    Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImageRewriteTarget {
    tag_index: usize,
    kind: ImageRewriteKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlTag {
    start: usize,
    end: usize,
    name: String,
    original_name: String,
    raw: String,
    attributes: Vec<HtmlAttribute>,
    is_end_tag: bool,
    self_closing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlAttribute {
    name: String,
    original_name: String,
    value: Option<String>,
}

#[derive(Debug, Default)]
struct PictureFrame {
    source_candidates: Vec<String>,
    source_tag_indices: Vec<usize>,
    img_candidate: Option<String>,
    img_tag_index: Option<usize>,
}

impl LocalizableImageDocument {
    pub(crate) fn parse(
        html: &str,
        base_url: Option<&Url>,
        max_images: usize,
    ) -> LocalizableImageDocument {
        let tags = parse_html_tags(html);
        let document_base = resolve_document_base_url(&tags, base_url);
        let mut slots = Vec::new();
        let mut image_markup_count = 0_usize;
        let mut unresolved_slot_count = 0_usize;
        let mut unsupported_slot_count = 0_usize;
        let mut picture_stack: Vec<PictureFrame> = Vec::new();

        for (tag_index, tag) in tags.iter().enumerate() {
            if tag.is_end_tag {
                if tag.name == "picture" {
                    if let Some(frame) = picture_stack.pop() {
                        image_markup_count += frame.source_tag_indices.len()
                            + usize::from(frame.img_tag_index.is_some());
                        let candidate =
                            frame.source_candidates.into_iter().next().or(frame.img_candidate);

                        match candidate {
                            Some(raw_source) if slots.len() < max_images => {
                                match resolve_asset_url(
                                    &raw_source,
                                    document_base.as_ref(),
                                    base_url,
                                ) {
                                    Some(resolved_url) => {
                                        let mut rewrite_targets = frame
                                            .source_tag_indices
                                            .into_iter()
                                            .map(|tag_index| ImageRewriteTarget {
                                                tag_index,
                                                kind: ImageRewriteKind::Source,
                                            })
                                            .collect::<Vec<_>>();
                                        if let Some(img_tag_index) = frame.img_tag_index {
                                            rewrite_targets.push(ImageRewriteTarget {
                                                tag_index: img_tag_index,
                                                kind: ImageRewriteKind::Img,
                                            });
                                        }
                                        slots.push(ImageSlot { resolved_url, rewrite_targets });
                                    }
                                    None => unresolved_slot_count += 1,
                                }
                            }
                            Some(_) => {}
                            None => unsupported_slot_count += 1,
                        }
                    }
                }
                continue;
            }

            match tag.name.as_str() {
                "picture" => picture_stack.push(PictureFrame::default()),
                "source" => {
                    if let Some(frame) = picture_stack.last_mut() {
                        frame.source_tag_indices.push(tag_index);
                        if let Some(candidate) = source_candidate(tag) {
                            frame.source_candidates.push(candidate);
                        }
                    }
                }
                "img" => {
                    if let Some(frame) = picture_stack.last_mut() {
                        frame.img_tag_index = Some(tag_index);
                        if frame.img_candidate.is_none() {
                            frame.img_candidate = img_candidate(tag);
                        }
                    } else {
                        image_markup_count += 1;
                        match img_candidate(tag) {
                            Some(raw_source) if slots.len() < max_images => {
                                match resolve_asset_url(
                                    &raw_source,
                                    document_base.as_ref(),
                                    base_url,
                                ) {
                                    Some(resolved_url) => slots.push(ImageSlot {
                                        resolved_url,
                                        rewrite_targets: vec![ImageRewriteTarget {
                                            tag_index,
                                            kind: ImageRewriteKind::Img,
                                        }],
                                    }),
                                    None => unresolved_slot_count += 1,
                                }
                            }
                            Some(_) => {}
                            None => unsupported_slot_count += 1,
                        }
                    }
                }
                _ => {}
            }
        }

        LocalizableImageDocument {
            tags,
            slots,
            image_markup_count,
            unresolved_slot_count,
            unsupported_slot_count,
        }
    }

    pub(crate) fn has_image_markup(&self) -> bool {
        self.image_markup_count > 0
    }

    pub(crate) fn unresolved_slot_count(&self) -> usize {
        self.unresolved_slot_count
    }

    pub(crate) fn unsupported_slot_count(&self) -> usize {
        self.unsupported_slot_count
    }

    pub(crate) fn slots(&self) -> &[ImageSlot] {
        &self.slots
    }

    pub(crate) fn rewrite_html(&self, html: &str, localized: &BTreeMap<usize, String>) -> String {
        if localized.is_empty() {
            return html.to_string();
        }

        let mut rewrites = BTreeMap::new();
        for (slot_index, data_url) in localized {
            let Some(slot) = self.slots.get(*slot_index) else {
                continue;
            };
            for target in &slot.rewrite_targets {
                rewrites.insert(
                    target.tag_index,
                    rewrite_tag_for_localized_asset(
                        &self.tags[target.tag_index],
                        target.kind,
                        data_url,
                    ),
                );
            }
        }

        if rewrites.is_empty() {
            return html.to_string();
        }

        let mut output = String::with_capacity(html.len());
        let mut cursor = 0_usize;
        for (tag_index, replacement) in rewrites {
            let tag = &self.tags[tag_index];
            output.push_str(&html[cursor..tag.start]);
            output.push_str(&replacement);
            cursor = tag.end;
        }
        output.push_str(&html[cursor..]);
        output
    }
}

impl ImageSlot {
    pub(crate) fn resolved_url(&self) -> &Url {
        &self.resolved_url
    }
}

pub fn normalize_html_for_live_display(html: &str, base_url: Option<&Url>) -> String {
    let tags = parse_html_tags(html);
    if tags.is_empty() {
        return html.to_string();
    }

    let document_base = resolve_document_base_url(&tags, base_url);
    let mut rewrites = BTreeMap::new();

    for (tag_index, tag) in tags.iter().enumerate() {
        if tag.is_end_tag {
            continue;
        }

        let replacement = match tag.name.as_str() {
            "img" => normalize_img_tag_for_live_display(tag, document_base.as_ref(), base_url),
            "source" => {
                normalize_source_tag_for_live_display(tag, document_base.as_ref(), base_url)
            }
            _ => None,
        };

        if let Some(replacement) = replacement.filter(|replacement| replacement != &tag.raw) {
            rewrites.insert(tag_index, replacement);
        }
    }

    apply_tag_rewrites(html, &tags, rewrites)
}

pub(crate) fn normalize_image_content_type(raw: &str) -> Option<String> {
    let mime = raw.split(';').next()?.trim().to_ascii_lowercase();
    mime.starts_with("image/").then_some(mime)
}

fn parse_html_tags(raw: &str) -> Vec<HtmlTag> {
    let bytes = raw.as_bytes();
    let mut tags = Vec::new();
    let mut index = 0_usize;

    while index < bytes.len() {
        let Some(relative_start) = raw[index..].find('<') else {
            break;
        };
        let start = index + relative_start;
        let Some(end) = find_tag_end(raw, start) else {
            break;
        };

        if let Some(tag) = parse_tag(&raw[start..end], start, end) {
            tags.push(tag);
        }
        index = end;
    }

    tags
}

fn find_tag_end(raw: &str, start: usize) -> Option<usize> {
    let bytes = raw.as_bytes();
    let mut cursor = start + 1;
    let mut quote = None;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        match quote {
            Some(active) if byte == active => quote = None,
            None if byte == b'\'' || byte == b'"' => quote = Some(byte),
            None if byte == b'>' => return Some(cursor + 1),
            _ => {}
        }
        cursor += 1;
    }

    None
}

fn parse_tag(raw_tag: &str, start: usize, end: usize) -> Option<HtmlTag> {
    if raw_tag.len() < 3 || !raw_tag.starts_with('<') || !raw_tag.ends_with('>') {
        return None;
    }

    let inner = raw_tag[1..raw_tag.len() - 1].trim();
    if inner.is_empty() || inner.starts_with('!') || inner.starts_with('?') {
        return None;
    }

    let is_end_tag = inner.starts_with('/');
    let inner = if is_end_tag { inner[1..].trim_start() } else { inner };
    if inner.is_empty() {
        return None;
    }

    let self_closing = !is_end_tag && inner.ends_with('/');
    let inner = if self_closing { inner.trim_end_matches('/').trim_end() } else { inner };

    let name_end = inner.find(|ch: char| ch.is_ascii_whitespace()).unwrap_or(inner.len());
    let original_name = inner[..name_end].to_string();
    if original_name.is_empty() {
        return None;
    }

    let attributes = if is_end_tag { Vec::new() } else { parse_attributes(&inner[name_end..]) };

    Some(HtmlTag {
        start,
        end,
        name: original_name.to_ascii_lowercase(),
        original_name,
        raw: raw_tag.to_string(),
        attributes,
        is_end_tag,
        self_closing,
    })
}

fn parse_attributes(raw: &str) -> Vec<HtmlAttribute> {
    let bytes = raw.as_bytes();
    let mut attributes = Vec::new();
    let mut index = 0_usize;

    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }

        if bytes[index] == b'/' {
            break;
        }

        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && bytes[index] != b'='
            && bytes[index] != b'/'
        {
            index += 1;
        }

        if name_start == index {
            index += 1;
            continue;
        }

        let original_name = raw[name_start..index].to_string();
        let name = original_name.to_ascii_lowercase();

        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }

        let value = if index < bytes.len() && bytes[index] == b'=' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            if index >= bytes.len() {
                Some(String::new())
            } else if bytes[index] == b'"' || bytes[index] == b'\'' {
                let quote = bytes[index];
                index += 1;
                let value_start = index;
                while index < bytes.len() && bytes[index] != quote {
                    index += 1;
                }
                let raw_value = raw[value_start..index].to_string();
                if index < bytes.len() {
                    index += 1;
                }
                let _ = quote;
                Some(decode_html_attribute_entities(&raw_value))
            } else {
                let value_start = index;
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && bytes[index] != b'/'
                {
                    index += 1;
                }
                let raw_value = raw[value_start..index].to_string();
                Some(decode_html_attribute_entities(&raw_value))
            }
        } else {
            None
        };

        attributes.push(HtmlAttribute { name, original_name, value });
    }

    attributes
}

fn resolve_document_base_url(tags: &[HtmlTag], base_url: Option<&Url>) -> Option<Url> {
    tags.iter()
        .find(|tag| !tag.is_end_tag && tag.name == "base")
        .and_then(|tag| attribute_value(tag, "href"))
        .and_then(|href| resolve_asset_url(href, None, base_url))
        .or_else(|| base_url.cloned())
}

fn img_candidate(tag: &HtmlTag) -> Option<String> {
    for attr in IMG_LAZY_SOURCE_ATTRIBUTES {
        if let Some(value) = attribute_value(tag, attr).filter(|value| !should_skip_asset(value)) {
            return Some(value.to_string());
        }
    }

    if let Some(value) = attribute_value(tag, "src").filter(|value| !should_skip_asset(value)) {
        return Some(value.to_string());
    }

    srcset_candidate(tag, IMG_SRCSET_ATTRIBUTES)
}

fn source_candidate(tag: &HtmlTag) -> Option<String> {
    srcset_candidate(tag, SOURCE_SRCSET_ATTRIBUTES).or_else(|| {
        attribute_value(tag, "src").filter(|value| !should_skip_asset(value)).map(ToOwned::to_owned)
    })
}

fn attribute_value<'a>(tag: &'a HtmlTag, attr: &str) -> Option<&'a str> {
    tag.attributes
        .iter()
        .find(|candidate| candidate.name == attr)
        .and_then(|candidate| candidate.value.as_deref())
}

fn normalize_img_tag_for_live_display(
    tag: &HtmlTag,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
) -> Option<String> {
    let mut changes = BTreeMap::new();
    let display_src = img_display_src(tag).and_then(|raw_candidate| {
        resolve_asset_url(&raw_candidate, document_base_url, fallback_base_url)
    });

    if let Some(display_src) = display_src {
        changes.insert("src".to_string(), Some(display_src.to_string()));
        changes.insert("srcset".to_string(), None);
        changes.insert("data-srcset".to_string(), None);
        changes.insert("sizes".to_string(), None);
    } else if let Some(srcset) =
        normalize_srcset_attribute(tag, document_base_url, fallback_base_url)
    {
        changes.insert("srcset".to_string(), Some(srcset));
        if attribute_value(tag, "data-srcset").is_some() {
            changes.insert("data-srcset".to_string(), None);
        }
    }

    changes.insert("loading".to_string(), None);
    changes.insert("fetchpriority".to_string(), None);

    (!changes.is_empty()).then(|| rewrite_tag_attributes(tag, &changes))
}

fn normalize_source_tag_for_live_display(
    tag: &HtmlTag,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
) -> Option<String> {
    let mut changes = BTreeMap::new();

    if let Some(raw_src) = attribute_value(tag, "src") {
        if let Some(resolved) = resolve_asset_url(raw_src, document_base_url, fallback_base_url) {
            changes.insert("src".to_string(), Some(resolved.to_string()));
        }
    }

    if let Some(srcset) = normalize_srcset_attribute(tag, document_base_url, fallback_base_url) {
        changes.insert("srcset".to_string(), Some(srcset));
        if attribute_value(tag, "data-srcset").is_some() {
            changes.insert("data-srcset".to_string(), None);
        }
    }

    (!changes.is_empty()).then(|| rewrite_tag_attributes(tag, &changes))
}

fn img_display_src(tag: &HtmlTag) -> Option<String> {
    for attr in IMG_LAZY_SOURCE_ATTRIBUTES {
        if let Some(value) = attribute_value(tag, attr).filter(|value| !should_skip_asset(value)) {
            return Some(value.to_string());
        }
    }

    if let Some(value) = attribute_value(tag, "src").filter(|value| !should_skip_asset(value)) {
        return Some(value.to_string());
    }

    srcset_candidate(tag, IMG_SRCSET_ATTRIBUTES)
}

fn normalize_srcset_attribute(
    tag: &HtmlTag,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
) -> Option<String> {
    attribute_value(tag, "data-srcset")
        .or_else(|| attribute_value(tag, "srcset"))
        .and_then(|raw| normalize_srcset(raw, document_base_url, fallback_base_url))
}

fn normalize_srcset(
    raw: &str,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
) -> Option<String> {
    let normalized = raw
        .split(',')
        .filter_map(|candidate| {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                return None;
            }

            let Some((url, descriptor)) = split_srcset_candidate(candidate) else {
                return Some(candidate.to_string());
            };

            let Some(resolved) = resolve_asset_url(url, document_base_url, fallback_base_url)
            else {
                return Some(candidate.to_string());
            };

            Some(if descriptor.is_empty() {
                resolved.to_string()
            } else {
                format!("{} {}", resolved, descriptor)
            })
        })
        .collect::<Vec<_>>();

    (!normalized.is_empty()).then(|| normalized.join(", "))
}

fn split_srcset_candidate(raw: &str) -> Option<(&str, &str)> {
    let url_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
    let url = raw[..url_end].trim();
    (!url.is_empty()).then_some((url, raw[url_end..].trim()))
}

fn srcset_candidate(tag: &HtmlTag, srcset_attrs: &[&str]) -> Option<String> {
    for attr in srcset_attrs {
        if let Some(value) = attribute_value(tag, attr) {
            if let Some(candidate) = select_best_srcset_candidate(value) {
                if !should_skip_asset(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

fn select_best_srcset_candidate(raw: &str) -> Option<String> {
    raw.split(',')
        .filter_map(|candidate| parse_srcset_candidate(candidate.trim()))
        .max_by_key(|(_, score)| *score)
        .map(|(url, _)| url)
}

fn parse_srcset_candidate(raw: &str) -> Option<(String, u32)> {
    if raw.is_empty() {
        return None;
    }

    let mut parts = raw.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    let url = parts.remove(0).to_string();
    let descriptor = parts.first().copied().unwrap_or_default();
    let score = if let Some(value) = descriptor.strip_suffix('w') {
        value.parse::<u32>().unwrap_or(0)
    } else if let Some(value) = descriptor.strip_suffix('x') {
        value.parse::<f32>().map(|value| (value * 1000.0) as u32).unwrap_or(0)
    } else {
        0
    };

    Some((url, score))
}

pub(crate) fn resolve_asset_url(
    raw: &str,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
) -> Option<Url> {
    Url::parse(raw)
        .ok()
        .or_else(|| document_base_url.and_then(|base| base.join(raw).ok()))
        .or_else(|| fallback_base_url.and_then(|base| base.join(raw).ok()))
        .filter(|resolved| matches!(resolved.scheme(), "http" | "https"))
}

fn rewrite_tag_for_localized_asset(
    tag: &HtmlTag,
    kind: ImageRewriteKind,
    localized_data_url: &str,
) -> String {
    let mut changes = BTreeMap::new();
    match kind {
        ImageRewriteKind::Img => {
            changes.insert("src".to_string(), Some(localized_data_url.to_string()));
            changes.insert("srcset".to_string(), None);
            changes.insert("data-srcset".to_string(), None);
        }
        ImageRewriteKind::Source => {
            changes.insert("src".to_string(), Some(localized_data_url.to_string()));
            changes.insert("srcset".to_string(), Some(localized_data_url.to_string()));
            changes.insert("data-srcset".to_string(), None);
        }
    }

    rewrite_tag_attributes(tag, &changes)
}

fn apply_tag_rewrites(html: &str, tags: &[HtmlTag], rewrites: BTreeMap<usize, String>) -> String {
    if rewrites.is_empty() {
        return html.to_string();
    }

    let mut output = String::with_capacity(html.len());
    let mut cursor = 0_usize;
    for (tag_index, replacement) in rewrites {
        let tag = &tags[tag_index];
        output.push_str(&html[cursor..tag.start]);
        output.push_str(&replacement);
        cursor = tag.end;
    }
    output.push_str(&html[cursor..]);
    output
}

fn rewrite_tag_attributes(tag: &HtmlTag, changes: &BTreeMap<String, Option<String>>) -> String {
    if tag.is_end_tag {
        return tag.raw.clone();
    }

    let mut output = String::from("<");
    output.push_str(&tag.original_name);

    let mut applied = BTreeMap::new();
    for attribute in &tag.attributes {
        let Some(change) = changes.get(&attribute.name) else {
            push_attribute(&mut output, &attribute.original_name, attribute.value.as_deref());
            continue;
        };

        applied.insert(attribute.name.clone(), true);
        if let Some(value) = change {
            push_attribute(&mut output, &attribute.original_name, Some(value));
        }
    }

    for (name, value) in changes {
        if applied.contains_key(name) {
            continue;
        }
        if let Some(value) = value {
            push_attribute(&mut output, name, Some(value));
        }
    }

    if tag.self_closing {
        output.push_str("/>");
    } else {
        output.push('>');
    }
    output
}

fn push_attribute(output: &mut String, name: &str, value: Option<&str>) {
    output.push(' ');
    output.push_str(name);
    if let Some(value) = value {
        output.push_str("=\"");
        output.push_str(&escape_html_attribute(value));
        output.push('"');
    }
}

fn escape_html_attribute(raw: &str) -> String {
    raw.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

fn decode_html_attribute_entities(raw: &str) -> String {
    let mut decoded = raw.to_string();
    for _ in 0..8 {
        let next = decode_html_attribute_entities_once(&decoded);
        if next == decoded {
            return next;
        }
        decoded = next;
    }

    decoded
}

fn decode_html_attribute_entities_once(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }

    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0_usize;

    while let Some(relative_start) = raw[cursor..].find('&') {
        let start = cursor + relative_start;
        output.push_str(&raw[cursor..start]);

        let Some(relative_end) = raw[start + 1..].find(';') else {
            output.push_str(&raw[start..]);
            return output;
        };

        let end = start + 1 + relative_end;
        let entity = &raw[start + 1..end];
        if let Some(decoded) = decode_html_entity(entity) {
            output.push(decoded);
            cursor = end + 1;
        } else {
            output.push('&');
            cursor = start + 1;
        }
    }

    output.push_str(&raw[cursor..]);
    output
}

fn decode_html_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "quot" => Some('"'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => decode_numeric_html_entity(entity),
    }
}

fn decode_numeric_html_entity(entity: &str) -> Option<char> {
    let value = if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
        u32::from_str_radix(hex, 16).ok()?
    } else if let Some(decimal) = entity.strip_prefix('#') {
        decimal.parse::<u32>().ok()?
    } else {
        return None;
    };

    char::from_u32(value)
}

fn should_skip_asset(raw: &str) -> bool {
    raw.is_empty()
        || raw.starts_with("data:")
        || raw.starts_with("blob:")
        || looks_like_wordpress_emoji_asset(raw)
        || looks_like_placeholder_asset(raw)
}

fn looks_like_placeholder_asset(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("placeholder")
        || lower.ends_with("/blank.gif")
        || lower.ends_with("/transparent.gif")
        || lower.ends_with("/spacer.gif")
        || lower.ends_with("/1x1.gif")
        || lower.ends_with("/pixel.gif")
}

fn looks_like_wordpress_emoji_asset(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("s.w.org/images/core/emoji/") || lower.contains("/wp-includes/images/smilies/")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        LocalizableImageDocument, normalize_html_for_live_display, normalize_image_content_type,
        resolve_asset_url,
    };
    use url::Url;

    #[test]
    fn resolve_asset_url_uses_document_base_for_relative_paths() {
        let document_base = Url::parse("https://example.com/posts/index.html").expect("base");
        let fallback = Url::parse("https://fallback.example.com/article").expect("fallback");
        let resolved = resolve_asset_url("/images/pic.png", Some(&document_base), Some(&fallback))
            .expect("resolved image");

        assert_eq!(resolved.as_str(), "https://example.com/images/pic.png");
    }

    #[test]
    fn resolve_asset_url_rejects_non_http_schemes() {
        assert!(resolve_asset_url("mailto:test@example.com", None, None).is_none());
        assert!(resolve_asset_url("javascript:alert(1)", None, None).is_none());
    }

    #[test]
    fn normalize_image_content_type_only_accepts_images() {
        assert_eq!(
            normalize_image_content_type("image/png; charset=binary").as_deref(),
            Some("image/png")
        );
        assert_eq!(normalize_image_content_type("text/html"), None);
    }

    #[test]
    fn document_collects_img_and_prefers_lazy_sources() {
        let base = Url::parse("https://example.com/posts/entry").expect("base");
        let document = LocalizableImageDocument::parse(
            r#"<img src="/blank.gif" data-src="/real.webp" data-srcset="/lazy-small.webp 1x, /lazy-large.webp 2x">"#,
            Some(&base),
            8,
        );

        assert_eq!(document.slots().len(), 1);
        assert_eq!(document.slots()[0].resolved_url().as_str(), "https://example.com/real.webp");
    }

    #[test]
    fn document_prefers_img_src_over_srcset_for_display_candidate() {
        let base = Url::parse("https://example.com/posts/entry").expect("base");
        let document = LocalizableImageDocument::parse(
            r#"<img srcset="/small.jpg 480w, /large.jpg 960w" src="/fallback.jpg">"#,
            Some(&base),
            8,
        );

        assert_eq!(document.slots().len(), 1);
        assert_eq!(document.slots()[0].resolved_url().as_str(), "https://example.com/fallback.jpg");
    }

    #[test]
    fn normalize_html_for_live_display_promotes_lazy_sources_and_resolves_relative_urls() {
        let article = Url::parse("https://example.com/posts/entry").expect("article");
        let normalized = normalize_html_for_live_display(
            r#"
            <base href="https://cdn.example.com/assets/">
            <p><img src="/blank.gif" data-src="hero.jpg" data-srcset="hero.jpg 1x, hero@2x.jpg 2x"></p>
            <picture>
              <source data-srcset="gallery.avif 1x, gallery@2x.avif 2x" type="image/avif">
              <img src="gallery.jpg">
            </picture>
            "#,
            Some(&article),
        );

        assert!(normalized.contains(r#"src="https://cdn.example.com/assets/hero.jpg""#));
        assert!(normalized.contains(r#"src="https://cdn.example.com/assets/hero.jpg""#));
        assert!(normalized.contains("<source"));
        assert!(normalized.contains(
            r#"srcset="https://cdn.example.com/assets/gallery.avif 1x, https://cdn.example.com/assets/gallery@2x.avif 2x""#
        ));
        assert!(normalized.contains(r#"type="image/avif""#));
        assert!(normalized.contains(r#"src="https://cdn.example.com/assets/gallery.jpg""#));
        assert!(!normalized.contains(r#"src="/blank.gif""#));
        assert!(!normalized.contains(r#"srcset="https://cdn.example.com/assets/hero.jpg 1x"#));
        assert!(!normalized.contains("data-srcset"));
        assert!(!normalized.contains(r#"loading="lazy""#));
    }

    #[test]
    fn normalize_html_for_live_display_collapses_wordpress_img_to_single_display_source() {
        let normalized = normalize_html_for_live_display(
            r#"<img loading="lazy" decoding="async" class="size-large wp-image-92306" src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png" alt="" width="1200" height="675" srcset="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png 1680w, https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora.png 1920w" sizes="auto, (max-width: 1200px) 100vw, 1200px">"#,
            None,
        );

        assert!(normalized.contains(
            r#"src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png""#
        ));
        assert!(!normalized.contains("Filmora.png 1920w"));
        assert!(!normalized.contains("srcset="));
        assert!(!normalized.contains("sizes="));
        assert!(!normalized.contains(r#"loading="lazy""#));
    }

    #[test]
    fn normalize_html_for_live_display_canonicalizes_polluted_alt_entities() {
        let article = Url::parse("https://example.com/posts/entry").expect("article");
        let normalized = normalize_html_for_live_display(
            r#"<img src="/hero.jpg" alt="Image describing the &amp;amp;quot;inference iceberg.&amp;amp;quot; &amp;amp; platform tradeoffs" loading="lazy">"#,
            Some(&article),
        );

        assert!(normalized.contains(r#"src="https://example.com/hero.jpg""#));
        assert!(normalized.contains(
            r#"alt="Image describing the &quot;inference iceberg.&quot; &amp; platform tradeoffs""#,
        ));
        assert!(!normalized.contains("&amp;amp;quot;"));
        assert!(!normalized.contains("&amp;amp;"));
    }

    #[test]
    fn document_uses_wordpress_display_src_instead_of_full_size_srcset_candidate() {
        let document = LocalizableImageDocument::parse(
            r#"<img loading="lazy" decoding="async" class="size-large wp-image-92306" src="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png" alt="" width="1200" height="675" srcset="https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png 1680w, https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1536x864.png 1536w, https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-2048x1152.png 2048w, https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora.png 1920w" sizes="auto, (max-width: 1200px) 100vw, 1200px">"#,
            None,
            8,
        );

        assert_eq!(document.slots().len(), 1);
        assert_eq!(
            document.slots()[0].resolved_url().as_str(),
            "https://blogs.nvidia.com/wp-content/uploads/2026/04/Filmora-1680x945.png"
        );
    }

    #[test]
    fn document_supports_picture_and_base_href() {
        let article = Url::parse("https://example.com/articles/post").expect("article");
        let document = LocalizableImageDocument::parse(
            r#"
            <base href="https://cdn.example.com/assets/">
            <picture>
              <source srcset="hero.avif 1x, hero@2x.avif 2x" type="image/avif">
              <img src="hero.jpg" alt="hero">
            </picture>
            "#,
            Some(&article),
            8,
        );

        assert_eq!(document.slots().len(), 1);
        assert_eq!(
            document.slots()[0].resolved_url().as_str(),
            "https://cdn.example.com/assets/hero@2x.avif"
        );
    }

    #[test]
    fn document_supports_unquoted_attributes_and_source_data_srcset() {
        let base = Url::parse("https://example.com/posts/entry").expect("base");
        let document = LocalizableImageDocument::parse(
            r#"<picture><source data-srcset="/hero.webp 1x, /hero@2x.webp 2x"><img data-src=/fallback.jpg src=/blank.gif></picture>"#,
            Some(&base),
            8,
        );

        assert_eq!(document.slots().len(), 1);
        assert_eq!(document.slots()[0].resolved_url().as_str(), "https://example.com/hero@2x.webp");
    }

    #[test]
    fn document_skips_placeholder_assets_and_counts_unsupported_markup() {
        let base = Url::parse("https://example.com/posts/entry").expect("base");
        let document = LocalizableImageDocument::parse(
            r#"<img src="/blank.gif"><img src="data:image/png;base64,abcd">"#,
            Some(&base),
            8,
        );

        assert!(document.has_image_markup());
        assert!(document.slots().is_empty());
        assert_eq!(document.unsupported_slot_count(), 2);
    }

    #[test]
    fn document_skips_wordpress_emoji_assets_without_consuming_slots() {
        let document = LocalizableImageDocument::parse(
            r#"<img src="https://s.w.org/images/core/emoji/17.0.2/72x72/1f4f9.png" alt="📹"><img src="https://example.com/hero.jpg">"#,
            None,
            1,
        );

        assert_eq!(document.slots().len(), 1);
        assert_eq!(document.slots()[0].resolved_url().as_str(), "https://example.com/hero.jpg");
        assert_eq!(document.unsupported_slot_count(), 1);
    }

    #[test]
    fn document_tracks_unresolved_relative_sources_without_base() {
        let document = LocalizableImageDocument::parse(r#"<img src="/hero.jpg">"#, None, 8);

        assert!(document.has_image_markup());
        assert!(document.slots().is_empty());
        assert_eq!(document.unresolved_slot_count(), 1);
    }

    #[test]
    fn rewrite_html_updates_picture_and_img_targets_without_touching_other_tags() {
        let base = Url::parse("https://example.com/posts/entry").expect("base");
        let document = LocalizableImageDocument::parse(
            r#"<p>lead</p><picture><source srcset="/hero.avif 1x, /hero@2x.avif 2x"><img src="/hero.jpg" data-srcset="/hero.webp 1x, /hero@2x.webp 2x"></picture><img src="/keep.png">"#,
            Some(&base),
            8,
        );
        let mut localized = BTreeMap::new();
        localized.insert(0, "data:image/avif;base64,abcd".to_string());

        let rewritten = document.rewrite_html(
            r#"<p>lead</p><picture><source srcset="/hero.avif 1x, /hero@2x.avif 2x"><img src="/hero.jpg" data-srcset="/hero.webp 1x, /hero@2x.webp 2x"></picture><img src="/keep.png">"#,
            &localized,
        );

        assert!(rewritten.contains(
            r#"<source srcset="data:image/avif;base64,abcd" src="data:image/avif;base64,abcd">"#
        ));
        assert!(rewritten.contains(r#"<img src="data:image/avif;base64,abcd">"#));
        assert!(rewritten.contains(r#"<img src="/keep.png">"#));
    }
}
