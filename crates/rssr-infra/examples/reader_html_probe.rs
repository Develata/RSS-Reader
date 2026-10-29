//! One bounded fixture per process, for `scripts/measure_reader_html.ps1`.
//! Build with `cargo build -p rssr-infra --release --example reader_html_probe --locked`.

use std::{env, fmt::Write, time::Instant};

use rssr_infra::html::sanitize_reader_html;
use sha2::{Digest, Sha256};

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let shape = args.next().ok_or_else(|| anyhow::anyhow!("expected shape and scale"))?;
    let scale: usize = args.next().ok_or_else(|| anyhow::anyhow!("expected scale"))?.parse()?;
    anyhow::ensure!(args.next().is_none() && (1..=100_000).contains(&scale), "invalid scale");
    let inputs = if shape == "corpus" {
        let snapshot: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/pages-demo/snapshot.json"))?;
        snapshot["feeds"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|feed| {
                feed["feed"]["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| entry["content_html"].as_str().unwrap().to_string())
            })
            .collect::<Vec<_>>()
    } else {
        vec![fixture(&shape, scale)?]
    };
    anyhow::ensure!(inputs.iter().all(|s| s.len() <= 8 * 1024 * 1024), "fixture exceeds 8 MiB");
    let input_bytes: usize = inputs.iter().map(String::len).sum();
    let input_hash = digest(&inputs);
    let started = Instant::now();
    let outputs: Vec<_> =
        inputs.iter().map(|s| sanitize_reader_html(s, None).unwrap_or_default()).collect();
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{}",
        serde_json::json!({
            "shape":shape,"scale":scale,"documents":inputs.len(),"input_bytes":input_bytes,
            "input_sha256":input_hash,"output_sha256":digest(&outputs),
            "output_bytes":outputs.iter().map(String::len).sum::<usize>(),"elapsed_ms":elapsed_ms,
            "peak_working_set_bytes":peak_working_set(),
            "math_before":inputs.iter().map(|s| s.matches("<math").count()).sum::<usize>(),
            "math_after":outputs.iter().map(|s| s.matches("<math").count()).sum::<usize>(),
        })
    );
    Ok(())
}

/// OS peak for this process, including fixture construction. The measurement is
/// read before exit; other platforms report null instead of a sampled estimate.
#[cfg(windows)]
fn peak_working_set() -> Option<usize> {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        sizes: [usize; 8],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
    }
    let mut counters =
        Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
    // SAFETY: PROCESS_MEMORY_COUNTERS has two DWORDs and eight SIZE_T fields.
    // The current-process pseudo handle needs no close; the output is writable
    // for exactly cb bytes and remains alive for the synchronous call.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    (ok != 0).then_some(counters.sizes[0])
}

#[cfg(not(windows))]
fn peak_working_set() -> Option<usize> {
    None
}

fn digest(strings: &[String]) -> String {
    let mut hash = Sha256::new();
    for text in strings {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn fixture(shape: &str, n: usize) -> anyhow::Result<String> {
    Ok(match shape {
        "markers-template" => {
            format!("{}{}", "<template><object></template>".repeat(n), "<b>x</b>".repeat(4 * n))
        }
        "markers-object" => {
            format!("{}{}", "<div><object></div>".repeat(n), "<b>x</b>".repeat(4 * n))
        }
        "attributes" => {
            let mut body = String::from("<p");
            for index in 0..n {
                write!(&mut body, " a{index}=x")?;
            }
            body.push_str(">visible &amp; text</p>");
            body
        }
        "attribute-block" => format!(
            "{}<b title=\"{}\">visible</b>",
            (0..64).map(|i| format!("<b id={i}>")).collect::<String>(),
            "x".repeat(n * 32)
        ),
        "flat" => "<p>Readable <em>article</em> &amp; text.</p>".repeat(n),
        "math" => "<p>Formula <math><mi>x</mi><mo>+</mo><mn>1</mn></math>.</p>".repeat(n),
        _ => anyhow::bail!("unknown shape: {shape}"),
    })
}
