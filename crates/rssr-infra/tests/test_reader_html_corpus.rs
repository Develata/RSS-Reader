//! 用真实订阅源校准阅读页消毒的工具，默认忽略。
//!
//! ```text
//! RSSR_READER_CORPUS=/path/to/feed.xml [RSSR_READER_CORPUS_OUT=/path/to/dir] \
//!   cargo test -p rssr-infra --release --test test_reader_html_corpus -- --ignored --nocapture
//! ```
//!
//! 按真实入库路径解析 feed（`FeedParser`），再按阅读页的正文选择顺序取 `content_html`，逐条
//! 消毒并报告公式保留情况：消毒前后的 `<math>` 个数、降级成 `<code>` 的个数和耗时。设置了
//! `RSSR_READER_CORPUS_OUT` 时，另把每条消毒结果按阅读页的外层结构（`reader-body` 容器）
//! 连同 `reader.css` 写成独立 HTML 页面，供浏览器检查。

use std::{env, fs, path::PathBuf, time::Instant};

use rssr_infra::{html::sanitize_reader_html, parser::FeedParser};

const READER_CSS: &str = include_str!("../../../assets/styles/reader.css");

#[test]
#[ignore = "calibration tool; needs RSSR_READER_CORPUS pointing at a feed file"]
fn reader_html_corpus_report() {
    let Some(corpus) = env::var_os("RSSR_READER_CORPUS").map(PathBuf::from) else {
        panic!("set RSSR_READER_CORPUS to a feed file");
    };
    let output_dir = env::var_os("RSSR_READER_CORPUS_OUT").map(PathBuf::from);
    if let Some(dir) = &output_dir {
        fs::create_dir_all(dir).expect("create output dir");
    }

    let raw = fs::read_to_string(&corpus).expect("read corpus");
    let feed = FeedParser::new().parse(&raw).expect("parse corpus feed");
    let (mut before_total, mut after_total, mut downgraded_total) = (0, 0, 0);
    for (index, entry) in feed.entries.iter().enumerate() {
        let Some(body) = entry.content_html.as_deref().or(entry.summary.as_deref()) else {
            println!("{index:02} {}: no body", entry.title);
            continue;
        };
        let started = Instant::now();
        let sanitized = sanitize_reader_html(body, entry.url.as_ref()).unwrap_or_default();
        let elapsed = started.elapsed();

        let before = body.matches("<math").count();
        let after = sanitized.matches("<math").count();
        let downgraded = sanitized.matches("<code>").count() - body.matches("<code>").count();
        before_total += before;
        after_total += after;
        downgraded_total += downgraded;
        println!(
            "{index:02} {}: math {before} -> {after}, downgraded {downgraded}, {} -> {} B, {elapsed:?}",
            entry.title,
            body.len(),
            sanitized.len()
        );

        if let Some(dir) = &output_dir {
            let page = format!(
                "<!doctype html><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><style>{READER_CSS}</style><div data-layout=\"reader-body\"><div class=\"reader-html\" data-slot=\"reader-body-html\">{sanitized}</div></div>"
            );
            fs::write(dir.join(format!("entry-{index:02}.html")), page).expect("write page");
        }
    }
    println!("total: math {before_total} -> {after_total}, downgraded {downgraded_total}");
}
