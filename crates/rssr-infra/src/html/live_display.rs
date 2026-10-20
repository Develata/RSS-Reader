use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};

use url::Url;

const IMG_LAZY_SOURCE_ATTRIBUTES: &[&str] =
    &["data-src", "data-original", "data-lazy-src", "data-orig-file"];
const IMG_SRCSET_ATTRIBUTES: &[&str] = &["data-srcset", "srcset"];
// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
const SOURCE_SRCSET_ATTRIBUTES: &[&str] = &["data-srcset", "srcset"];

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalizableImageDocument {
    tags: Vec<HtmlTag>,
    slots: Vec<ImageSlot>,
    image_markup_count: usize,
    unresolved_slot_count: usize,
    unsupported_slot_count: usize,
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageSlot {
    resolved_url: Url,
    rewrite_targets: Vec<ImageRewriteTarget>,
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageRewriteKind {
    Img,
    Source,
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImageRewriteTarget {
    tag_index: usize,
    kind: ImageRewriteKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HtmlTag {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) name: String,
    original_name: String,
    raw: String,
    attributes: Vec<HtmlAttribute>,
    pub(super) is_end_tag: bool,
    self_closing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlAttribute {
    name: String,
    original_name: String,
    value: Option<String>,
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
/// 每张本地化图片允许在正文里出现的 data URL 份数：`<img>` 一份，每个 `<source>` 两份（`src` 与
/// `srcset`），足够七个 `<source>` 加一个 `<img>` 的 `<picture>`。
#[cfg(not(target_arch = "wasm32"))]
const MAX_DATA_URL_COPIES: usize = 16;

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default)]
struct PictureFrame {
    source_candidates: Vec<String>,
    source_tag_indices: Vec<usize>,
    img_candidate: Option<String>,
    img_tag_index: Option<usize>,
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
impl LocalizableImageDocument {
    pub(crate) fn parse(
        html: &str,
        base_url: Option<&Url>,
        max_images: usize,
    ) -> LocalizableImageDocument {
        let tags = parse_html_tags(html);
        let document_base = resolve_document_base_url(&tags, base_url);
        // 解析失败不占图片名额，失败的尝试也要计入解析工作量，否则可以无限次重复。
        let budget = UrlBudget::for_input(html.len());
        let mut slots = Vec::new();
        let mut image_markup_count = 0_usize;
        let mut unresolved_slot_count = 0_usize;
        let mut unsupported_slot_count = 0_usize;
        let mut picture_stack: Vec<PictureFrame> = Vec::new();

        for (tag_index, tag) in tags.iter().enumerate() {
            if tag.is_end_tag {
                if tag.name == "picture"
                    && let Some(frame) = picture_stack.pop()
                {
                    image_markup_count +=
                        frame.source_tag_indices.len() + usize::from(frame.img_tag_index.is_some());
                    let candidate =
                        frame.source_candidates.into_iter().next().or(frame.img_candidate);

                    match candidate {
                        Some(raw_source) if slots.len() < max_images => {
                            match budget.resolve(&raw_source, document_base.as_ref(), base_url) {
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
                                match budget.resolve(&raw_source, document_base.as_ref(), base_url)
                                {
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

    /// 本地化后替换标签的总字节数上限；溢出时返回 `None`（调用方按超限处理）。
    fn localized_output_allowance(
        &self,
        html: &str,
        localized: &BTreeMap<usize, String>,
    ) -> Option<usize> {
        let mut allowance = html.len().checked_mul(2)?.checked_add(1 << 20)?;
        for (slot_index, data_url) in localized {
            if self.slots.get(*slot_index).is_some() {
                let copies = data_url.len().checked_mul(MAX_DATA_URL_COPIES)?;
                allowance = allowance.checked_add(copies)?;
            }
        }
        Some(allowance)
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

    /// 把本地化成功的图片换成 data URL。一个 `<picture>` 可以带任意多个 `<source>`，同一份 data URL
    /// 会被写进每个目标（`<source>` 的 `src` 与 `srcset` 各一份），几百 KB 的正文配一张大图就能放大成
    /// 几十 GB。改写前先按目标算出输出增长，超过上限就整篇保留未本地化的正文。
    pub(crate) fn rewrite_html(&self, html: &str, localized: &BTreeMap<usize, String>) -> String {
        if localized.is_empty() {
            return html.to_string();
        }

        // 输出上限：原正文的 2 倍加 1 MiB，再加每张本地化图片 [`MAX_DATA_URL_COPIES`] 份 data URL。
        // 逐个替换按实际序列化长度（含属性重新转义）累计，超限立即放弃，最多多建一个标签。
        let Some(allowance) = self.localized_output_allowance(html, localized) else {
            return html.to_string();
        };

        // 按标签下标登记替换，再按文档顺序输出：一个 `<picture>` 可以带任意多个 `<source>` 目标，
        // 有序映射会多出对数因子。
        let mut replacements: Vec<Option<String>> = vec![None; self.tags.len()];
        let mut written = 0_usize;
        let mut any = false;
        for (slot_index, data_url) in localized {
            let Some(slot) = self.slots.get(*slot_index) else {
                continue;
            };
            for target in &slot.rewrite_targets {
                let replacement = rewrite_tag_for_localized_asset(
                    &self.tags[target.tag_index],
                    target.kind,
                    data_url,
                );
                match written.checked_add(replacement.len()) {
                    Some(total) if total <= allowance => written = total,
                    _ => return html.to_string(),
                }
                replacements[target.tag_index] = Some(replacement);
                any = true;
            }
        }

        if !any {
            return html.to_string();
        }

        let rewrites = replacements
            .into_iter()
            .enumerate()
            .filter_map(|(tag_index, replacement)| Some((tag_index, replacement?)))
            .collect();
        apply_tag_rewrites(html, &self.tags, rewrites)
    }
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
impl ImageSlot {
    pub(crate) fn resolved_url(&self) -> &Url {
        &self.resolved_url
    }
}

/// 把正文里的图片地址归一化成 WebView 能直接加载的绝对地址（懒加载属性提到 `src`，
/// 相对地址按 `<base href>` 或 feed 地址展开）。
///
/// 展开相对地址会让输出变长：很长的 `<base href>` 乘上大量很短的相对地址，输出按平方放大；
/// 每次相对地址解析还要复制一遍基地址。解析工作量与改写产生的字节数共同受 [`UrlBudget`]
/// 约束；一旦超出，整篇原样返回、不做任何改写，而不是只改写前面一部分。
pub fn normalize_html_for_live_display(html: &str, base_url: Option<&Url>) -> String {
    let tags = parse_html_tags(html);
    if tags.is_empty() {
        return html.to_string();
    }

    let budget = UrlBudget::for_input(html.len());
    let document_base = resolve_document_base_url(&tags, base_url);
    let mut rewrites = Vec::new();

    for (tag_index, tag) in tags.iter().enumerate() {
        if tag.is_end_tag {
            continue;
        }

        let replacement = match tag.name.as_str() {
            "img" => {
                normalize_img_tag_for_live_display(tag, document_base.as_ref(), base_url, &budget)
            }
            "source" => normalize_source_tag_for_live_display(
                tag,
                document_base.as_ref(),
                base_url,
                &budget,
            ),
            _ => None,
        };
        if budget.exhausted() {
            return html.to_string();
        }

        if let Some(replacement) = replacement.filter(|replacement| replacement != &tag.raw) {
            // 改写后的标签要重新转义所有属性值，引号之类的字符会变长，一并计入预算。
            if !budget.charge(replacement.len()) {
                return html.to_string();
            }
            rewrites.push((tag_index, replacement));
        }
    }

    apply_tag_rewrites(html, &tags, rewrites)
}

/// 归一化的工作量与产出上限：输入长度的 2 倍加 1 MiB。计入三项：每次解析地址前，原始地址与
/// 可能用到的基地址的长度（`Url::join` 会复制基地址）；解析出的地址长度；改写后整个标签的长度。
/// 正常正文里每个图片地址最多展开一个 feed 地址那么长，远在上限之内；超出说明是在用短相对地址
/// 放大长的基地址，或用需要转义的字符放大标签。
struct UrlBudget {
    remaining: Cell<usize>,
    exhausted: Cell<bool>,
}

impl UrlBudget {
    const BASE_BYTES: usize = 1 << 20;
    const BYTES_PER_INPUT_BYTE: usize = 2;

    fn for_input(len: usize) -> Self {
        let limit = Self::BASE_BYTES.saturating_add(Self::BYTES_PER_INPUT_BYTE.saturating_mul(len));
        Self { remaining: Cell::new(limit), exhausted: Cell::new(false) }
    }

    fn exhausted(&self) -> bool {
        self.exhausted.get()
    }

    /// 扣除 `bytes`；不够时标记用完并返回 `false`，之后的扣除一律失败。
    fn charge(&self, bytes: usize) -> bool {
        if self.exhausted() {
            return false;
        }
        match self.remaining.get().checked_sub(bytes) {
            Some(remaining) => {
                self.remaining.set(remaining);
                true
            }
            None => {
                self.exhausted.set(true);
                false
            }
        }
    }

    /// 与 [`resolve_asset_url`] 相同，但解析前先按原始地址与基地址的长度扣预算（解析的工作量），
    /// 解析后再按结果长度扣；预算用完后一律返回 `None`。
    fn resolve(
        &self,
        raw: &str,
        document_base_url: Option<&Url>,
        fallback_base_url: Option<&Url>,
    ) -> Option<Url> {
        let base_len = |base: Option<&Url>| base.map_or(0, |base| base.as_str().len());
        let work = raw
            .len()
            .saturating_add(base_len(document_base_url))
            .saturating_add(base_len(fallback_base_url));
        if !self.charge(work) {
            return None;
        }
        let resolved = resolve_asset_url(raw, document_base_url, fallback_base_url)?;
        self.charge(resolved.as_str().len()).then_some(resolved)
    }
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn normalize_image_content_type(raw: &str) -> Option<String> {
    let mime = raw.split(';').next()?.trim().to_ascii_lowercase();
    mime.starts_with("image/").then_some(mime)
}

pub(super) fn parse_html_tags(raw: &str) -> Vec<HtmlTag> {
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

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
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

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
fn source_candidate(tag: &HtmlTag) -> Option<String> {
    srcset_candidate(tag, SOURCE_SRCSET_ATTRIBUTES).or_else(|| {
        attribute_value(tag, "src").filter(|value| !should_skip_asset(value)).map(ToOwned::to_owned)
    })
}

pub(super) fn attribute_value<'a>(tag: &'a HtmlTag, attr: &str) -> Option<&'a str> {
    tag.attributes
        .iter()
        .find(|candidate| candidate.name == attr)
        .and_then(|candidate| candidate.value.as_deref())
}

fn normalize_img_tag_for_live_display(
    tag: &HtmlTag,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
    budget: &UrlBudget,
) -> Option<String> {
    let mut changes = BTreeMap::new();
    let display_src = img_display_src(tag).and_then(|raw_candidate| {
        budget.resolve(&raw_candidate, document_base_url, fallback_base_url)
    });

    if let Some(display_src) = display_src {
        changes.insert("src".to_string(), Some(display_src.to_string()));
        changes.insert("srcset".to_string(), None);
        changes.insert("data-srcset".to_string(), None);
        changes.insert("sizes".to_string(), None);
    } else if let Some(srcset) =
        normalize_srcset_attribute(tag, document_base_url, fallback_base_url, budget)
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
    budget: &UrlBudget,
) -> Option<String> {
    let mut changes = BTreeMap::new();

    if let Some(raw_src) = attribute_value(tag, "src")
        && let Some(resolved) = budget.resolve(raw_src, document_base_url, fallback_base_url)
    {
        changes.insert("src".to_string(), Some(resolved.to_string()));
    }

    if let Some(srcset) =
        normalize_srcset_attribute(tag, document_base_url, fallback_base_url, budget)
    {
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
    budget: &UrlBudget,
) -> Option<String> {
    attribute_value(tag, "data-srcset")
        .or_else(|| attribute_value(tag, "srcset"))
        .and_then(|raw| normalize_srcset(raw, document_base_url, fallback_base_url, budget))
}

fn normalize_srcset(
    raw: &str,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
    budget: &UrlBudget,
) -> Option<String> {
    // 预算用完就停：同一个 srcset 里的成千上万个候选会各自展开一份基地址。
    let normalized = raw
        .split(',')
        .take_while(|_| !budget.exhausted())
        .filter_map(|candidate| {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                return None;
            }

            let Some((url, descriptor)) = split_srcset_candidate(candidate) else {
                return Some(candidate.to_string());
            };

            let Some(resolved) = budget.resolve(url, document_base_url, fallback_base_url) else {
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
        if let Some(value) = attribute_value(tag, attr)
            && let Some(candidate) = select_best_srcset_candidate(value)
            && !should_skip_asset(&candidate)
        {
            return Some(candidate);
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

/// 可解析地址的路径部分（`?`、`#` 之前）最多含多少个点（含 `%2e`）。`url` 2.5 处理每个 `..` 段时
/// 会从序列化结果末尾反向找 `/`，在根路径上要扫过整个 authority；一个带长用户名、几百万个 `../`
/// 的地址就能让一次解析平方级。正常图片地址的路径里只有个位数的点。
const MAX_URL_PATH_DOTS: usize = 64;

pub(crate) fn resolve_asset_url(
    raw: &str,
    document_base_url: Option<&Url>,
    fallback_base_url: Option<&Url>,
) -> Option<Url> {
    if url_path_dots(raw) > MAX_URL_PATH_DOTS {
        return None;
    }
    // 结果只接受 http/https：非 http 的基地址（feed 里的文章链接不限 scheme）拼出来的地址也会被
    // 丢掉，不必先付一遍复制它的代价。
    Url::parse(raw)
        .ok()
        .or_else(|| web_base(document_base_url).and_then(|base| base.join(raw).ok()))
        .or_else(|| web_base(fallback_base_url).and_then(|base| base.join(raw).ok()))
        .filter(is_web_url)
}

fn is_web_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn web_base(base: Option<&Url>) -> Option<&Url> {
    base.filter(|base| is_web_url(base))
}

/// `raw` 在第一个 `?` 或 `#` 之前的点数，`%2e` 按一个点计（`url` 把它当作点段处理）。
/// 与 URL 解析器一样先忽略 TAB、LF、CR：解析器会删掉它们，`%2\ne` 在它眼里就是 `%2e`。
/// 这里数的是所有点（包括主机名与文件名里的），比真正的点段多，只会多拒绝、不会漏。
fn url_path_dots(raw: &str) -> usize {
    let mut dots = 0_usize;
    // 已经读到的 `%2e` 前缀长度：0 无，1 读到 `%`，2 读到 `%2`。
    let mut encoded = 0_u8;
    for byte in raw.bytes().filter(|byte| !matches!(byte, b'\t' | b'\n' | b'\r')) {
        if matches!(byte, b'?' | b'#') {
            break;
        }
        encoded = match (encoded, byte) {
            (1, b'2') => 2,
            (2, b'e' | b'E') => {
                dots += 1;
                0
            }
            (_, b'%') => 1,
            _ => {
                if byte == b'.' {
                    dots += 1;
                }
                0
            }
        };
    }
    dots
}

// 仅正文图片本地化（原生端）使用；Web 端不做本地化，门控掉避免在 wasm 构建里变成死代码。
#[cfg(not(target_arch = "wasm32"))]
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

/// `rewrites` 按标签下标升序排列（调用方按文档顺序收集）。
fn apply_tag_rewrites(html: &str, tags: &[HtmlTag], rewrites: Vec<(usize, String)>) -> String {
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

    // 同名属性只保留第一个，与浏览器的解析结果一致；否则改写后的长地址会写进每一个重复的
    // `src`，几个字节的重复属性就能把输出放大成地址长度的倍数。
    // 预先按属性个数分配：从空表逐个插入时，每次扩容都会重新哈希已有的（可能很长的）属性名。
    let mut seen = HashSet::with_capacity(tag.attributes.len());
    for attribute in &tag.attributes {
        if !seen.insert(attribute.name.as_str()) {
            continue;
        }
        let Some(change) = changes.get(&attribute.name) else {
            push_attribute(&mut output, &attribute.original_name, attribute.value.as_deref());
            continue;
        };

        if let Some(value) = change {
            push_attribute(&mut output, &attribute.original_name, Some(value));
        }
    }

    for (name, value) in changes {
        if seen.contains(name.as_str()) {
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

        // 只在实体名可能的长度之内找分号：不限长的话，一长串 `&` 共用末尾同一个分号，
        // 每个 `&` 都要扫到底，整体平方级。分号是单字节字符，找到的位置一定是字符边界。
        let window_end = raw.len().min(start + 2 + MAX_ENTITY_NAME_BYTES);
        let Some(relative_end) =
            raw.as_bytes()[start + 1..window_end].iter().position(|&byte| byte == b';')
        else {
            output.push('&');
            cursor = start + 1;
            continue;
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

/// 能解码的实体名（`amp`、`#x10FFFF`、带前导零的十进制等）的最大字节数。
const MAX_ENTITY_NAME_BYTES: usize = 32;

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
    } else {
        entity.strip_prefix('#')?.parse::<u32>().ok()?
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

pub(super) fn looks_like_wordpress_emoji_asset(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("s.w.org/images/core/emoji/") || lower.contains("/wp-includes/images/smilies/")
}

// 与 `LocalizableImageDocument` 等类型同样按 target 门控：这些用例里有一半直接使用那些
// 仅原生端编译的类型，不门控的话 `cargo check --target wasm32 --all-targets` 会因未解析导入
// 而失败。单元测试本身只在宿主上跑（wasm 只做 check），因此不损失任何实际覆盖；而
// `normalize_html_for_live_display` 两端是同一份代码，宿主上的用例即可覆盖它。
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        LocalizableImageDocument, decode_html_attribute_entities, normalize_html_for_live_display,
        normalize_image_content_type, resolve_asset_url,
    };
    use url::Url;

    #[test]
    fn attribute_entities_decode_within_a_bounded_window() {
        assert_eq!(decode_html_attribute_entities("a &amp; b &#x41;&#66;"), "a & b AB");
        assert_eq!(decode_html_attribute_entities("&&amp;&lt;"), "&&<");
        assert_eq!(decode_html_attribute_entities("&#0000000065;"), "A");
        // 名字超过实体长度上限的不是实体，原样保留，后面的实体照常解码。
        let long = format!("&{};&amp;", "x".repeat(40));
        assert_eq!(decode_html_attribute_entities(&long), format!("&{};&", "x".repeat(40)));
        // 多字节字符紧跟在 `&` 后面也不会切到字符中间。
        assert_eq!(decode_html_attribute_entities("&数据;&amp;"), "&数据;&");
    }

    #[test]
    fn url_expansion_over_budget_leaves_the_whole_body_unchanged() {
        let base = format!("https://example.com/{}/", "a".repeat(64 * 1024));
        let html = format!(r#"<base href="{base}"><p>x</p>{}"#, "<img src=x>".repeat(64));
        let normalized = normalize_html_for_live_display(&html, None);
        assert_eq!(normalized, html, "no partial rewrite");

        // 同样的基地址，图片少时仍正常展开。
        let html = format!(r#"<base href="{base}"><img src=x>"#);
        let normalized = normalize_html_for_live_display(&html, None);
        assert!(normalized.contains(&format!(r#"<img src="{base}x">"#)));
    }

    #[test]
    fn urls_with_many_dot_segments_are_not_resolved() {
        let base = Url::parse("https://example.com/a/b/").unwrap();
        assert!(resolve_asset_url("../../c.png", Some(&base), None).is_some());
        assert!(resolve_asset_url("/x.png?v=1.2.3", Some(&base), None).is_some());
        let dots = format!("https://{}@e.test/{}", "a".repeat(1 << 16), "../".repeat(1 << 16));
        assert!(resolve_asset_url(&dots, None, None).is_none());
        let encoded = format!("{}c.png", "%2e%2E/".repeat(64));
        assert!(resolve_asset_url(&encoded, Some(&base), None).is_none());
        // 解析器会删掉 TAB/LF/CR，拆开的 `%2\ne` 仍是一个点。
        let split = format!("{}c.png", "%2\ne%2\ne/".repeat(64));
        assert!(resolve_asset_url(&split, Some(&base), None).is_none());
        // 非 http 的基地址不参与拼接。
        let file_base = Url::parse("file:///tmp/").unwrap();
        assert!(resolve_asset_url("x.png", Some(&file_base), None).is_none());
        // 查询串里的点不算。
        let query = format!("/c.png?{}", ".".repeat(1_000));
        assert!(resolve_asset_url(&query, Some(&base), None).is_some());
    }

    #[test]
    fn failed_localizer_resolutions_are_charged() {
        // 评审反例：解析失败不占图片名额；巨大的非 http 基地址曾被每个相对地址复制一遍。
        let file_base = Url::parse(&format!("file:///{}", "a".repeat(1 << 20))).unwrap();
        let html = "<img src=x>".repeat(65_536);
        let document = LocalizableImageDocument::parse(&html, Some(&file_base), 16);
        assert!(document.slots().is_empty());

        // http 基地址同样受解析工作量约束：成功的名额用完前，失败的尝试也会耗尽预算。
        let web_base = Url::parse(&format!("https://e.test/{}", "a".repeat(1 << 20))).unwrap();
        let html = r#"<img src="http://[">"#.repeat(65_536);
        let document = LocalizableImageDocument::parse(&html, Some(&web_base), 16);
        assert!(document.slots().is_empty());
    }

    #[test]
    fn localized_rewrites_cannot_multiply_one_image_without_bound() {
        let html = format!(
            r#"<picture>{}<img src="https://e.test/a"></picture>"#,
            r#"<source srcset="https://e.test/a">"#.repeat(2_000)
        );
        let document = LocalizableImageDocument::parse(&html, None, 1);
        let data_url = format!("data:image/png;base64,{}", "A".repeat(1 << 20));
        let localized = BTreeMap::from([(0, data_url.clone())]);
        assert_eq!(document.rewrite_html(&html, &localized), html);

        // 属性重新转义让标签变长，也算进上限。
        let html = format!(r#"<img src="https://e.test/i" title='{}'>"#, "\"".repeat(1 << 20));
        let document = LocalizableImageDocument::parse(&html, None, 1);
        let small = BTreeMap::from([(0, "data:image/png;base64,AAAA".to_string())]);
        assert_eq!(document.rewrite_html(&html, &small), html);

        // 普通的 picture（两个 source）照常本地化。
        let html = r#"<picture><source srcset="https://e.test/a"><source srcset="https://e.test/a"><img src="https://e.test/a"></picture>"#;
        let document = LocalizableImageDocument::parse(html, None, 1);
        let localized = BTreeMap::from([(0, data_url.clone())]);
        assert_eq!(document.rewrite_html(html, &localized).matches(&data_url).count(), 5);
    }

    #[test]
    fn srcset_candidates_share_the_url_budget() {
        let base = format!("https://example.com/{}/", "a".repeat(64 * 1024));
        let html = format!(
            r#"<base href="{base}"><picture><source srcset="{}"></picture>"#,
            vec!["x"; 64].join(",")
        );
        assert_eq!(normalize_html_for_live_display(&html, None), html);
    }

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
