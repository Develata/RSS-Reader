use std::collections::BTreeMap;

use anyhow::{Context, Result, bail, ensure};
use quick_xml::encoding::Decoder;
use quick_xml::{
    Reader, Writer,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};
use rssr_domain::{ConfigFeed, OpmlFeed};

/// OPML 编解码，原生与 Web 共用（Web 端原先有一份更宽松的副本，审计 D5）。
///
/// 订阅名称按 OPML 2.0：`text` 是显示的名称，`title` 来自订阅源。导出时 `text` 写显示名称
/// （没有名称时写空串），`title` 只写已知的源标题；导入时名称取 `text`，空白时退回 `title`
/// （审计 B6）。不拿名称与订阅地址比较来猜名称的来源。
#[derive(Debug, Clone, Default)]
pub struct OpmlCodec;

impl OpmlCodec {
    pub fn new() -> Self {
        Self
    }

    pub fn encode(&self, feeds: &[OpmlFeed]) -> Result<String> {
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        let mut opml = BytesStart::new("opml");
        opml.push_attribute(("version", "2.0"));
        writer.write_event(Event::Start(opml))?;
        // OPML 2.0 的骨架是一个 <head> 加一个 <body>；头部信息都是可选的。
        writer.write_event(Event::Empty(BytesStart::new("head")))?;

        writer.write_event(Event::Start(BytesStart::new("body")))?;

        let mut grouped: BTreeMap<Option<String>, Vec<&OpmlFeed>> = BTreeMap::new();
        for feed in feeds {
            grouped.entry(feed.folder.clone()).or_default().push(feed);
        }

        for (folder, group_feeds) in grouped {
            if let Some(folder) = folder.as_deref() {
                let mut outline = BytesStart::new("outline");
                push_attribute(&mut outline, "text", folder);
                push_attribute(&mut outline, "title", folder);
                writer.write_event(Event::Start(outline))?;

                for feed in group_feeds {
                    write_feed_outline(&mut writer, feed)?;
                }

                writer.write_event(Event::End(BytesEnd::new("outline")))?;
            } else {
                for feed in group_feeds {
                    write_feed_outline(&mut writer, feed)?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new("body")))?;
        writer.write_event(Event::End(BytesEnd::new("opml")))?;

        String::from_utf8(writer.into_inner()).context("OPML 输出不是有效 UTF-8")
    }

    pub fn decode(&self, raw: &str) -> Result<Vec<ConfigFeed>> {
        let mut reader = Reader::from_str(raw);
        reader.config_mut().trim_text(true);

        let mut feeds = Vec::new();
        let mut folder_stack: Vec<Option<String>> = Vec::new();
        let mut outline_depths: Vec<bool> = Vec::new();
        let mut element_stack: Vec<Vec<u8>> = Vec::new();
        let mut saw_opml = false;
        let mut saw_body = false;
        let mut body_depth: Option<usize> = None;
        let mut root_closed = false;

        loop {
            let event = reader.read_event()?;

            if !saw_opml && element_stack.is_empty() {
                match &event {
                    Event::Start(event) | Event::Empty(event)
                        if event.name().as_ref() == b"opml" => {}
                    Event::Text(text)
                        if text
                            .as_ref()
                            .iter()
                            .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                    Event::Decl(_) | Event::Comment(_) | Event::PI(_) | Event::DocType(_) => {}
                    Event::Eof => {}
                    _ => bail!("OPML 根元素之前存在额外内容"),
                }
            }

            if root_closed {
                match &event {
                    Event::Eof => break,
                    Event::Text(text)
                        if text
                            .as_ref()
                            .iter()
                            .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) =>
                    {
                        continue;
                    }
                    Event::Comment(_) | Event::PI(_) => continue,
                    _ => bail!("OPML 根元素之后存在额外内容"),
                }
            }

            match event {
                Event::Start(event) => {
                    let name = event.name().as_ref().to_vec();
                    check_attributes(&event)?;
                    if element_stack.is_empty() {
                        ensure!(!saw_opml && name == b"opml", "OPML 必须只有一个 <opml> 根元素");
                        validate_opml_root(&event, reader.decoder())?;
                        saw_opml = true;
                    } else if element_stack.len() == 1
                        && element_stack[0].as_slice() == b"opml"
                        && name == b"body"
                    {
                        ensure!(!saw_body, "OPML 只能包含一个 <body>");
                        saw_body = true;
                        body_depth = Some(element_stack.len() + 1);
                    }

                    ensure_outline_in_body(&name, &element_stack, body_depth)?;
                    if name == b"outline" && body_depth.is_some() {
                        let outline = OutlineAttrs::from_event(&event, reader.decoder())?;
                        if let Some(feed) = outline.feed(&folder_stack) {
                            feeds.push(feed);
                            outline_depths.push(false);
                        } else {
                            folder_stack.push(outline.name());
                            outline_depths.push(true);
                        }
                    }
                    element_stack.push(name);
                }
                Event::Empty(event) => {
                    let name = event.name().as_ref().to_vec();
                    check_attributes(&event)?;
                    ensure_outline_in_body(&name, &element_stack, body_depth)?;
                    if element_stack.is_empty() {
                        ensure!(!saw_opml && name == b"opml", "OPML 必须只有一个 <opml> 根元素");
                        validate_opml_root(&event, reader.decoder())?;
                        saw_opml = true;
                        root_closed = true;
                    } else if element_stack.len() == 1
                        && element_stack[0].as_slice() == b"opml"
                        && name == b"body"
                    {
                        ensure!(!saw_body, "OPML 只能包含一个 <body>");
                        saw_body = true;
                    } else if name == b"outline" && body_depth.is_some() {
                        let outline = OutlineAttrs::from_event(&event, reader.decoder())?;
                        if let Some(feed) = outline.feed(&folder_stack) {
                            feeds.push(feed);
                        }
                    }
                }
                Event::End(event) => {
                    let name = event.name().as_ref().to_vec();
                    let open = element_stack.pop().context("OPML 出现了没有起始标签的结束标签")?;
                    ensure!(open == name, "OPML 标签没有正确闭合");

                    if name.as_slice() == b"outline" && body_depth.is_some() {
                        if outline_depths.pop().unwrap_or(false) {
                            folder_stack.pop();
                        }
                    } else if name.as_slice() == b"body" {
                        body_depth = None;
                    } else if name.as_slice() == b"opml" {
                        root_closed = true;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }

        ensure!(saw_opml, "不是 OPML：缺少 <opml> 根元素");
        ensure!(saw_body, "不是 OPML：缺少 <body>");
        ensure!(root_closed && element_stack.is_empty(), "OPML 文档被截断或存在未闭合标签");
        ensure!(
            outline_depths.is_empty() && folder_stack.is_empty(),
            "OPML outline 结构未完整闭合"
        );

        Ok(feeds)
    }
}

fn validate_opml_root(event: &BytesStart<'_>, decoder: Decoder) -> Result<()> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if attribute.key.as_ref() == b"version" {
            let version = attribute.decode_and_unescape_value(decoder)?;
            if !matches!(version.as_ref(), "1.0" | "1.1" | "2.0") {
                bail!("不支持的 OPML 版本：{version}");
            }
        }
    }
    Ok(())
}

fn write_feed_outline(writer: &mut Writer<Vec<u8>>, feed: &OpmlFeed) -> Result<()> {
    // OPML 2.0 要求每个 outline 都有 `text`。没有名称时写空串，而不是订阅地址：地址写进去，
    // 导入方（包括本应用）只能把它当作名称。源标题未知时不写 `title`，不拿显示名称冒充。
    let mut outline = BytesStart::new("outline");
    push_attribute(&mut outline, "text", feed.name.as_deref().unwrap_or_default());
    if let Some(source_title) = feed.source_title.as_deref() {
        push_attribute(&mut outline, "title", source_title);
    }
    outline.push_attribute(("type", "rss"));
    push_attribute(&mut outline, "xmlUrl", &feed.url);
    writer.write_event(Event::Empty(outline))?;
    Ok(())
}

/// 写一个属性值。除常规转义外，制表、换行、回车写成字符引用（XML 解析器按属性值规范化会把
/// 字面的这三种字符变成空格），XML 1.0 不允许出现的字符直接去掉：导出的文件必须是合法 XML，
/// 一个远端标题里的控制字符不应让整份导出失败。
fn push_attribute(element: &mut BytesStart<'_>, key: &str, raw: &str) {
    let mut value = String::with_capacity(raw.len());
    for character in raw.chars() {
        match character {
            '&' => value.push_str("&amp;"),
            '<' => value.push_str("&lt;"),
            '>' => value.push_str("&gt;"),
            '"' => value.push_str("&quot;"),
            '\'' => value.push_str("&apos;"),
            '\t' => value.push_str("&#9;"),
            '\n' => value.push_str("&#10;"),
            '\r' => value.push_str("&#13;"),
            '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}' => {
                value.push(character)
            }
            _ => {}
        }
    }
    // `(&[u8], &[u8])` 原样写入；`(&str, &str)` 会再转义一次。
    element.push_attribute((key.as_bytes(), value.as_bytes()));
}

/// quick-xml 产生起始标签事件时不检查属性语法，只有读取属性才会报错；每个元素都读一遍，
/// 缺等号、缺引号或重复属性的文档整体拒绝。这不是完整的 XML 良构性校验（属性值里的裸 `<`
/// 等仍会通过）。
///
/// A tag longer than [`crate::xml_declaration::MAX_XML_TAG_BYTES`] is refused before its
/// attributes are read: the duplicate check costs their number squared (review C3-R4 sweep).
fn check_attributes(event: &BytesStart<'_>) -> Result<()> {
    ensure!(
        event.len() <= crate::xml_declaration::MAX_XML_TAG_BYTES,
        "OPML 元素过长（超过 {} KiB）",
        crate::xml_declaration::MAX_XML_TAG_BYTES / 1024
    );
    for attribute in event.attributes() {
        attribute?;
    }
    Ok(())
}

/// 订阅只能在 <body> 里：外面的 outline 以前被静默忽略，文件里的订阅少导入了却报告成功。
fn ensure_outline_in_body(
    name: &[u8],
    element_stack: &[Vec<u8>],
    body_depth: Option<usize>,
) -> Result<()> {
    ensure!(
        name != b"outline" || element_stack.is_empty() || body_depth.is_some(),
        "OPML 的 outline 必须位于 <body> 内"
    );
    Ok(())
}

fn current_folder(folder_stack: &[Option<String>]) -> Option<String> {
    folder_stack.iter().rev().flatten().next().cloned()
}

struct OutlineAttrs {
    text: Option<String>,
    title: Option<String>,
    xml_url: Option<String>,
}

impl OutlineAttrs {
    fn from_event(event: &BytesStart<'_>, decoder: Decoder) -> Result<Self> {
        let mut text = None;
        let mut title = None;
        let mut xml_url = None;

        for attribute in event.attributes() {
            let attribute = attribute?;
            let value = attribute.decode_and_unescape_value(decoder)?.into_owned();
            match attribute.key.as_ref() {
                b"text" => text = Some(value),
                b"title" => title = Some(value),
                b"xmlUrl" => xml_url = Some(value),
                _ => {}
            }
        }

        Ok(Self { text, title, xml_url })
    }

    /// 显示的名称：`text` 优先，空白时退回 `title`（OPML 2.0：`text` 是显示文本，`title`
    /// 来自订阅源；其他阅读器里改过的名称在 `text`）。
    fn name(&self) -> Option<String> {
        [&self.text, &self.title]
            .into_iter()
            .flatten()
            .find(|value| !value.trim().is_empty())
            .cloned()
    }

    /// 带 `xmlUrl` 的 outline 是一条订阅。
    fn feed(&self, folder_stack: &[Option<String>]) -> Option<ConfigFeed> {
        Some(ConfigFeed {
            url: self.xml_url.clone()?,
            title: self.name(),
            folder: current_folder(folder_stack),
        })
    }
}
