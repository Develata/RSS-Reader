use std::collections::BTreeMap;

use anyhow::{Context, Result};
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

        loop {
            match reader.read_event()? {
                Event::Start(event) if event.name().as_ref() == b"outline" => {
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
                Event::Empty(event) if event.name().as_ref() == b"outline" => {
                    let outline = OutlineAttrs::from_event(&event, reader.decoder())?;
                    if let Some(url) = outline.xml_url {
                        feeds.push(ConfigFeed {
                            url,
                            title: outline.title.or(outline.text),
                            folder: current_folder(&folder_stack),
                        });
                    }
                }
                Event::End(event) if event.name().as_ref() == b"outline" => {
                    if outline_depths.pop().unwrap_or(false) {
                        folder_stack.pop();
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }

        Ok(feeds)
    }
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
