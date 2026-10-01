use std::collections::BTreeMap;

use anyhow::{Context, Result, bail, ensure};
use quick_xml::encoding::Decoder;
use quick_xml::{
    Reader, Writer,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};
use rssr_domain::ConfigFeed;

#[derive(Debug, Clone, Default)]
pub struct OpmlCodec;

impl OpmlCodec {
    pub fn new() -> Self {
        Self
    }

    pub fn encode(&self, feeds: &[ConfigFeed]) -> Result<String> {
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        let mut opml = BytesStart::new("opml");
        opml.push_attribute(("version", "2.0"));
        writer.write_event(Event::Start(opml))?;

        writer.write_event(Event::Start(BytesStart::new("body")))?;

        let mut grouped: BTreeMap<Option<String>, Vec<&ConfigFeed>> = BTreeMap::new();
        for feed in feeds {
            grouped.entry(feed.folder.clone()).or_default().push(feed);
        }

        for (folder, group_feeds) in grouped {
            if let Some(folder) = folder.as_deref() {
                let mut outline = BytesStart::new("outline");
                outline.push_attribute(("text", folder));
                outline.push_attribute(("title", folder));
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

                    if name == b"outline" && body_depth.is_some() {
                        let outline = OutlineAttrs::from_event(&event, reader.decoder())?;
                        if let Some(url) = outline.xml_url {
                            feeds.push(ConfigFeed {
                                url,
                                title: outline.title.or(outline.text),
                                folder: current_folder(&folder_stack),
                            });
                            outline_depths.push(false);
                        } else {
                            folder_stack.push(outline.title.or(outline.text));
                            outline_depths.push(true);
                        }
                    }
                    element_stack.push(name);
                }
                Event::Empty(event) => {
                    let name = event.name().as_ref().to_vec();
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
                        if let Some(url) = outline.xml_url {
                            feeds.push(ConfigFeed {
                                url,
                                title: outline.title.or(outline.text),
                                folder: current_folder(&folder_stack),
                            });
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

fn write_feed_outline(writer: &mut Writer<Vec<u8>>, feed: &ConfigFeed) -> Result<()> {
    let title = feed.title.as_deref().unwrap_or(&feed.url);
    let mut outline = BytesStart::new("outline");
    outline.push_attribute(("text", title));
    outline.push_attribute(("title", title));
    outline.push_attribute(("type", "rss"));
    outline.push_attribute(("xmlUrl", feed.url.as_str()));
    writer.write_event(Event::Empty(outline))?;
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
}
