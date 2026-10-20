//! Prepares a data directory for the browser regression (phase C, C4c): the library of a seed
//! under `tests/fixtures/browser_state/`, with every subscription served by the local feed fixture
//! server, and an outbound policy that lets the server reach that loopback server only.
//!
//! ```text
//! browser_fixture_library --seeds <dir> --seed <name> --data-dir <dir> --feed-base <url>
//!     [--custom-css <file>]
//! ```
//!
//! The data directory must not hold a library yet. Prints the subscriptions as JSON
//! (`{"feeds":[{"id":1,"url":"…"}]}`) for the harness, which waits for the server's start-up
//! refresh to fetch each of them.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use rssr_infra::store::fixture::{BrowserStateSeed, write_library};

/// Public only, except the loopback host the feed fixture server runs on (test data only).
const POLICY: &str =
    "{\n  \"mode\": \"public_only\",\n  \"allow\": [\"127.0.0.1/32\"],\n  \"deny\": []\n}\n";

struct Options {
    seeds: PathBuf,
    seed: String,
    data_dir: PathBuf,
    feed_base: String,
    custom_css: Option<PathBuf>,
}

fn options() -> Result<Options> {
    let mut args = std::env::args().skip(1);
    let (mut seeds, mut seed, mut data_dir, mut feed_base, mut custom_css) =
        (None, None, None, None, None);
    while let Some(flag) = args.next() {
        let value = args.next().with_context(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seeds" => seeds = Some(PathBuf::from(value)),
            "--seed" => seed = Some(value),
            "--data-dir" => data_dir = Some(PathBuf::from(value)),
            "--feed-base" => feed_base = Some(value),
            "--custom-css" => custom_css = Some(PathBuf::from(value)),
            other => bail!("unknown option {other}"),
        }
    }
    Ok(Options {
        seeds: seeds.context("--seeds is required")?,
        seed: seed.context("--seed is required")?,
        data_dir: data_dir.context("--data-dir is required")?,
        feed_base: feed_base.context("--feed-base is required")?,
        custom_css,
    })
}

/// Seeds are named with dashes (`home-reader`), their files with underscores (`home_reader_*`).
fn file_prefix(seed: &str) -> String {
    seed.replace('-', "_")
}

fn write_policy(data_dir: &std::path::Path) -> Result<()> {
    let path = data_dir.join(rssr_infra::outbound::POLICY_FILE);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path).with_context(|| format!("create {}", path.display()))?;
    std::io::Write::write_all(&mut file, POLICY.as_bytes())?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let options = options()?;
    let mut seed = BrowserStateSeed::read(&options.seeds, &file_prefix(&options.seed))
        .with_context(|| format!("read seed {}", options.seed))?;
    seed.serve_feeds_from(&options.feed_base, &options.seed);
    if let Some(css) = &options.custom_css {
        seed.set_custom_css(
            std::fs::read_to_string(css).with_context(|| format!("read {}", css.display()))?,
        );
    }
    std::fs::create_dir_all(&options.data_dir)?;
    write_policy(&options.data_dir)?;
    write_library(&options.data_dir, &seed).await.context("write the library")?;
    let feeds: Vec<_> = seed
        .feed_urls()
        .into_iter()
        .map(|(id, url)| serde_json::json!({ "id": id, "url": url }))
        .collect();
    println!("{}", serde_json::json!({ "feeds": feeds }));
    Ok(())
}
