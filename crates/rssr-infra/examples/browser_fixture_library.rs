//! Prepares a data directory for the browser regression (phase C, C4c): the library of a seed
//! under `tests/fixtures/browser_state/`, with every subscription served by the local feed fixture
//! server, and an outbound policy that lets the server reach that loopback server only.
//!
//! ```text
//! browser_fixture_library --data-dir <dir> [--seeds <dir> --seed <name> --feed-base <url>
//!     [--custom-css <file>]]
//! ```
//!
//! The data directory must be new or empty (nothing is written otherwise). Without `--seed` only
//! the policy is written: the server creates an empty library on its first start, as on a first
//! run. Prints the active subscriptions as JSON
//! (`{"feeds":[{"id":1,"url":"…","fetched_at_ms":4070908800000}]}`, the seeded fetch time in Unix
//! milliseconds or null) for the harness, which waits until the server's start-up refresh has
//! fetched each of them and stored a new fetch time.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use rssr_infra::store::fixture::{BrowserStateSeed, write_library};

/// Only the loopback host the feed fixture server runs on (test data only): `public_only` with
/// 127.0.0.1 allowed, and every other address denied (`deny` wins over the public addresses
/// `public_only` would allow).
fn policy() -> String {
    let deny: Vec<String> = all_but_loopback().into_iter().chain(["::/0".to_owned()]).collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "mode": "public_only",
        "allow": ["127.0.0.1/32"],
        "deny": deny,
    }))
    .expect("a JSON value serializes")
        + "\n"
}

/// The IPv4 networks that together hold every address except 127.0.0.1: for each prefix length
/// `p` in 1..=32, the addresses that agree with 127.0.0.1 on the first `p - 1` bits and differ in
/// bit `p`. They are disjoint and their union is everything else.
fn all_but_loopback() -> Vec<String> {
    let loopback = u32::from(std::net::Ipv4Addr::LOCALHOST);
    (1..=32u32)
        .map(|prefix| {
            let shift = 32 - prefix;
            let network = (loopback ^ (1 << shift)) & (u32::MAX << shift);
            format!("{}/{prefix}", std::net::Ipv4Addr::from(network))
        })
        .collect()
}

struct Options {
    data_dir: PathBuf,
    library: Option<Library>,
}

/// The library to write: a seed, its subscriptions served from `feed_base`.
struct Library {
    seeds: PathBuf,
    seed: String,
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
    let data_dir = data_dir.context("--data-dir is required")?;
    let Some(seed) = seed else {
        if seeds.is_some() || feed_base.is_some() || custom_css.is_some() {
            bail!("--seeds, --feed-base and --custom-css need --seed");
        }
        return Ok(Options { data_dir, library: None });
    };
    let library = Library {
        seeds: seeds.context("--seeds is required with --seed")?,
        seed,
        feed_base: feed_base.context("--feed-base is required with --seed")?,
        custom_css,
    };
    Ok(Options { data_dir, library: Some(library) })
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
    std::io::Write::write_all(&mut file, policy().as_bytes())?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let options = options()?;
    let seed = options.library.as_ref().map(read_seed).transpose()?;
    // Only a new test data directory: never a policy or a library written into a directory that
    // already holds something (a user's data directory given by mistake stays unchanged).
    std::fs::create_dir_all(&options.data_dir)?;
    if std::fs::read_dir(&options.data_dir)?.next().is_some() {
        bail!("{} is not empty; give a new data directory", options.data_dir.display());
    }
    write_policy(&options.data_dir)?;
    let mut feeds = Vec::new();
    if let Some(seed) = &seed {
        write_library(&options.data_dir, seed).await.context("write the library")?;
        feeds = seed
            .active_feeds()
            .into_iter()
            .map(|feed| {
                let fetched_at_ms =
                    feed.last_fetched_at.map(|at| (at.unix_timestamp_nanos() / 1_000_000) as i64);
                serde_json::json!({ "id": feed.id, "url": feed.url, "fetched_at_ms": fetched_at_ms })
            })
            .collect();
    }
    println!("{}", serde_json::json!({ "feeds": feeds }));
    Ok(())
}

fn read_seed(library: &Library) -> Result<BrowserStateSeed> {
    let mut seed = BrowserStateSeed::read(&library.seeds, &file_prefix(&library.seed))
        .with_context(|| format!("read seed {}", library.seed))?;
    seed.serve_feeds_from(&library.feed_base, &library.seed);
    if let Some(css) = &library.custom_css {
        seed.set_custom_css(
            std::fs::read_to_string(css).with_context(|| format!("read {}", css.display()))?,
        );
    }
    Ok(seed)
}
