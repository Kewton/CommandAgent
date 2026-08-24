use std::path::PathBuf;

use anyhow::{Context, bail};

fn main() -> anyhow::Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let pack_dir = arguments
        .next()
        .map(PathBuf::from)
        .context("usage: pack_conformance <pack-dir> [--expect-hash sha256:<hex>]")?;
    let expected_hash = match arguments.next() {
        None => None,
        Some(flag) if flag == "--expect-hash" => Some(
            arguments
                .next()
                .context("--expect-hash requires sha256:<hex>")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("expected hash must be UTF-8"))?,
        ),
        Some(flag) => bail!("unknown argument `{}`", flag.to_string_lossy()),
    };
    if let Some(extra) = arguments.next() {
        bail!("unexpected argument `{}`", extra.to_string_lossy());
    }

    let report = commandagent::planner::pack::conform_directory(&pack_dir)
        .with_context(|| format!("pack conformance failed for `{}`", pack_dir.display()))?;
    if let Some(expected) = expected_hash
        && report.exact_byte_hash != expected
    {
        bail!(
            "pack hash mismatch: expected {expected}, observed {}",
            report.exact_byte_hash
        );
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
