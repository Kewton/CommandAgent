use std::path::Path;

use commandagent::planner::profiles::data::checks;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = args.next().expect("workspace root argument");
    let check = args.next().expect("check id argument");
    let ok = match check.as_str() {
        "data_results_schema" => {
            let evidence = checks::check_results_schema(Path::new(&root))?;
            println!(
                "capability_id={} status={} ok={} error={:?}",
                evidence.capability_id, evidence.status, evidence.ok, evidence.error
            );
            evidence.ok
        }
        "data_reconciliation" => {
            let evidence = checks::check_reconciliation(Path::new(&root))?;
            println!(
                "capability_id={} status={} ok={} equation={:?} failure_kinds={:?}",
                evidence.capability_id,
                evidence.status,
                evidence.ok,
                evidence.equation,
                evidence.failure_kinds
            );
            evidence.ok
        }
        other => anyhow::bail!("unsupported check: {other}"),
    };
    if ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
