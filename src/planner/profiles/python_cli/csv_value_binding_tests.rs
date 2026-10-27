use super::csv_value_binding::{Binding, bind, expected_rows};
use super::{PythonCliProfile, write_csv_probe_fixtures};
use crate::planner::profile::DomainProfile;

const GOAL: &str = "Build a CLI that reads a CSV file and prints sum, mean, max, and min";

// Fixture contents of the recorded campaign runs (the previous probe fixtures).
const OLD_A: &str = "name,score,count\nalpha,10,2\nbeta,20,4\ngamma,30,6\n";
const OLD_B: &str = "name,score,count\nalpha,3,1\nbeta,9,5\ngamma,12,7\n";

// Shared head of the generated CLIs used by the end-to-end probes below.
const CLI_HEAD: &str = r#"import csv
import sys

rows = list(csv.DictReader(open(sys.argv[1])))
stats = {}
for col in rows[0]:
    try:
        vals = [float(r[col]) for r in rows]
    except ValueError:
        continue
    stats[col] = (sum(vals), sum(vals) / len(vals), max(vals), min(vals))
"#;

const CORRECT_TAIL: &str = r#"print("column | sum | mean | max | min")
for c, (s, m, mx, mn) in stats.items():
    print(f"{c} | {s:g} | {m:g} | {mx:g} | {mn:g}")
"#;

// The measured IC-37 bug: the header says max, min but the rows print min, max.
const SWAPPED_TAIL: &str = r#"print("column | sum | mean | max | min")
for c, (s, m, mx, mn) in stats.items():
    print(f"{c} | {s:g} | {m:g} | {mn:g} | {mx:g}")
"#;

const SECOND_ONLY_SWAPPED_TAIL: &str = r#"print("column | sum | mean | max | min")
for c, (s, m, mx, mn) in stats.items():
    if sys.argv[1].endswith("input-b.csv"):
        mx, mn = mn, mx
    print(f"{c} | {s:g} | {m:g} | {mx:g} | {mn:g}")
"#;

const PROSE_TAIL: &str = r#"for c, (s, m, mx, mn) in stats.items():
    print(f"{c}: sum is {s:g}, mean is {m:g}, max is {mx:g}, min is {mn:g}")
"#;

const EXIT_ONE_TAIL: &str = r#"print("column | sum | mean | max | min")
for c, (s, m, mx, mn) in stats.items():
    print(f"{c} | {s:g} | {m:g} | {mx:g} | {mn:g}")
sys.exit(1)
"#;

fn probe(tail: &str) -> (&'static str, Vec<String>) {
    probe_source(&format!("{CLI_HEAD}{tail}"))
}

fn probe_source(source: &str) -> (&'static str, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("src/app/main.py");
    std::fs::create_dir_all(main.parent().unwrap()).unwrap();
    std::fs::write(&main, source).unwrap();
    let report = PythonCliProfile
        .behavior_probe(dir.path(), GOAL, &[], false)
        .unwrap();
    (report.status, report.reasons)
}

#[test]
fn swapped_max_min_columns_fail_the_probe() {
    let (status, reasons) = probe(SWAPPED_TAIL);
    assert_eq!(status, "failed", "{reasons:?}");
    assert!(
        reasons
            .iter()
            .any(|reason| reason
                .contains("first_stdout_value_mismatch:score:max:expected_65:got_15")),
        "{reasons:?}"
    );
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("second_stdout_value_mismatch:score:min")),
        "{reasons:?}"
    );
    // The old token check alone could not see this: every expected number is
    // still present in stdout.
    assert!(
        !reasons
            .iter()
            .any(|reason| reason.contains("missing_aggregate")),
        "{reasons:?}"
    );
}

#[test]
fn a_swap_on_only_the_second_fixture_still_fails() {
    let (status, reasons) = probe(SECOND_ONLY_SWAPPED_TAIL);
    assert_eq!(status, "failed", "{reasons:?}");
    assert!(
        reasons
            .iter()
            .all(|reason| reason.contains("second_stdout_value_mismatch")),
        "{reasons:?}"
    );
}

#[test]
fn correct_table_passes_the_probe() {
    let (status, reasons) = probe(CORRECT_TAIL);
    assert_eq!(status, "pass", "{reasons:?}");
    assert!(reasons.is_empty(), "{reasons:?}");
}

#[test]
fn recorded_ic37_stdout_is_a_mismatch() {
    // campaign-G1-3a11 cli-q, first fixture of that run (line breaks restored).
    let stdout = "column | sum | mean | max | min\n-------+-----+------+-----+----\nscore | 60 | 20 | 10 | 30\ncount | 12 | 4 | 2 | 6\n";
    let Binding::Mismatch(mismatches) = bind(stdout, OLD_A) else {
        panic!("swapped columns must bind and mismatch");
    };
    assert_eq!(
        mismatches,
        vec![
            "score:max:expected_30:got_10",
            "score:min:expected_10:got_30",
            "count:max:expected_6:got_2",
            "count:min:expected_2:got_6",
        ]
    );
}

#[test]
fn well_formed_tables_pass_in_every_accepted_format() {
    let csv = "name,score\nalpha,15\nbeta,40\ngamma,65\n";
    let tables = [
        "column | sum | mean | max | min\n--+--+--+--+--\nscore | 120 | 40 | 65 | 15\n",
        "| column | sum | mean | max | min |\n+---+---+---+---+---+\n| score | 120 | 40 | 65 | 15 |\n",
        "column,sum,mean,max,min\nscore,120,40,65,15\n",
        "column\tsum\tmean\tmax\tmin\nscore\t120\t40\t65\t15\n",
        "column  sum   mean  max  min\nscore   120   40    65   15\n",
        "Column | Sum | Mean | Max | Min\n-----\nscore | 120.00 | 40.00 | 65.00 | 15.00\n",
        "column | sum | mean | max | min\nscore | 120.0 | 40.0 | 65.0 | 15.0\n",
        "┌───┬───┐\n│ column │ sum │ mean │ max │ min │\n├───┼───┤\n│ score │ 120 │ 40 │ 65 │ 15 │\n└───┴───┘\n",
        "title line\ncolumn | count | sum | mean | max | min\nscore | 3 | 120 | 40 | 65 | 15\n",
        "sum mean max min\nscore 120 40 65 15\n",
    ];
    for table in tables {
        assert_eq!(bind(table, csv), Binding::Bound, "{table:?}");
    }
}

#[test]
fn a_count_column_and_a_count_label_do_not_swap_rows_and_columns() {
    // campaign-G1-3a10 cli-q shape: `count` is both an aggregate header and a
    // fixture column label, and min precedes max.
    let stdout = "Statistics for input-a.csv\n+--------+-------+-----+------+-----+-----+\n| column | count | sum | mean | min | max |\n+--------+-------+-----+------+-----+-----+\n| score | 3 | 60 | 20 | 10 | 30 |\n| count | 3 | 12 | 4 | 2 | 6 |\n+--------+-------+-----+------+-----+-----+\n";
    assert_eq!(bind(stdout, OLD_A), Binding::Bound);
    // The same table with the count row's max and min swapped must not hide
    // behind the aggregate named `count`.
    let wrong = stdout.replace(
        "| count | 3 | 12 | 4 | 2 | 6 |",
        "| count | 3 | 12 | 4 | 6 | 2 |",
    );
    assert!(matches!(bind(&wrong, OLD_A), Binding::Mismatch(_)));
}

#[test]
fn fixtures_use_distinct_multi_digit_values_without_accidental_matches() {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = write_csv_probe_fixtures(dir.path()).unwrap();
    let mut numbers: Vec<String> = Vec::new();
    for fixture in &fixtures {
        let rows = expected_rows(fixture.content);
        assert_eq!(rows.len(), 2, "score and count are the numeric columns");
        let mut computed = vec![rows[0].label.clone(), rows[1].label.clone()];
        for row in &rows {
            for value in [row.sum, row.mean, row.max, row.min] {
                assert_eq!(value.fract(), 0.0, "{} {value}", row.label);
                assert!(value >= 10.0, "{} {value} must be multi-digit", row.label);
                computed.push(format!("{value}"));
            }
        }
        // The literal expected tokens are exactly the computed aggregates.
        let mut tokens = fixture
            .expected_tokens
            .iter()
            .map(|token| token.to_string())
            .collect::<Vec<_>>();
        tokens.sort();
        computed.sort();
        assert_eq!(tokens, computed);
        numbers.extend(computed.into_iter().filter(|t| t.parse::<f64>().is_ok()));
    }
    assert_eq!(numbers.len(), 16);
    for (i, left) in numbers.iter().enumerate() {
        for (j, right) in numbers.iter().enumerate() {
            if i != j {
                assert!(
                    !right.contains(left.as_str()),
                    "{left} is a substring of {right}"
                );
            }
        }
    }
}

#[test]
fn unreadable_output_is_partial_with_the_unbound_reason() {
    let (status, reasons) = probe(PROSE_TAIL);
    assert_eq!(status, "partial", "{reasons:?}");
    assert_eq!(reasons, vec!["python_cli_behavior_values_unbound"]);
}

#[test]
fn outputs_without_a_readable_table_are_unbound() {
    let csv = "name,score\nalpha,15\nbeta,40\ngamma,65\n";
    for stdout in [
        "score: sum is 120, mean is 40, max is 65, min is 15\n",
        "score 120 40 65 15\n",
        "column | sum | mean | max\nscore | 120 | 40 | 65\n",
        "column | sum | sum | mean | max | min\nscore | 120 | 120 | 40 | 65 | 15\n",
        "column | sum | mean | max | min\nrow | 120 | 40 | 65 | 15\n",
        "column | sum | mean | max | min\nscore | 120 | 40 | 65\n",
        "column | sum | mean | max | min\nscore | 120 | 40 | n/a | 15\n",
        "column | sum | mean | max | min\nscore | 120 | 40 | 65 | 15\nscore | 1 | 1 | 1 | 1\n",
        "",
    ] {
        assert_eq!(bind(stdout, csv), Binding::Unbound, "{stdout:?}");
    }
    assert_eq!(bind("anything", "name\nalpha\n"), Binding::Unbound);
}

#[test]
fn accepted_aggregate_names_are_a_closed_list() {
    let csv = "name,score\nalpha,15\nbeta,40\ngamma,65\n";
    let accepted = [
        ("sum", "mean", "max", "min"),
        ("total", "average", "maximum", "minimum"),
        ("Total", "Avg", "Max", "Min"),
        ("合計", "平均", "最大", "最小"),
    ];
    for (sum, mean, max, min) in accepted {
        let table =
            format!("column | {sum} | {mean} | {max} | {min}\nscore | 120 | 40 | 65 | 15\n");
        assert_eq!(bind(&table, csv), Binding::Bound, "{table:?}");
    }
    for outside in [
        "median",
        "highest",
        "lowest",
        "mx",
        "total_sum",
        "mean_value",
    ] {
        let as_mean = format!("column | sum | {outside} | max | min\nscore | 120 | 40 | 65 | 15\n");
        let as_max = format!("column | sum | mean | {outside} | min\nscore | 120 | 40 | 65 | 15\n");
        assert_eq!(bind(&as_mean, csv), Binding::Unbound, "{as_mean:?}");
        assert_eq!(bind(&as_max, csv), Binding::Unbound, "{as_max:?}");
    }
}

#[test]
fn a_bound_table_cannot_turn_a_failed_probe_into_a_pass() {
    // Correct, bound values but a non-zero exit code: the existing gates stay.
    let (status, reasons) = probe(EXIT_ONE_TAIL);
    assert_eq!(status, "failed", "{reasons:?}");
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("first_exit_code")),
        "{reasons:?}"
    );
    // The two-point guard of the old token check still fires on its own.
    let (status, reasons) = probe("print('score sum 1')\n");
    assert_eq!(status, "failed", "{reasons:?}");
}

// Recorded stdout of real cli runs (campaign-G1-3a6, 3a7, 3a8 and 3a10 under
// `stage3a/cli-q` and `cli-g`; 3a8 cli-g has no record). The evidence stores
// stdout with newlines flattened to spaces, so the line breaks below are
// restored by hand at the row boundaries; cell text is unchanged. The recorded
// fixtures are the previous probe fixtures (OLD_A and OLD_B).
const RECORDED: &[(&str, &str, &str)] = &[
    (
        "3a6 cli-q",
        "| column | sum | mean | max | min | count |\n+--------+-----+------+-----+-----+-------+\n| score | 60 | 20 | 30 | 10 | 3 |\n| count | 12 | 4 | 6 | 2 | 3 |\n",
        "| column | sum | mean | max | min | count |\n+--------+-----+--------+-----+-----+-------+\n| score | 24 | 8 | 12 | 3 | 3 |\n| count | 13 | 4.3333 | 7 | 1 | 3 |\n",
    ),
    (
        "3a6 cli-g",
        "Column | Sum | Mean | Max | Min\n-------------------------------------------------------------------\nscore | 60.00 | 20.00 | 30.00 | 10.00\ncount | 12.00 | 4.00 | 6.00 | 2.00\n",
        "Column | Sum | Mean | Max | Min\n-------------------------------------------------------------------\nscore | 24.00 | 8.00 | 12.00 | 3.00\ncount | 13.00 | 4.33 | 7.00 | 1.00\n",
    ),
    (
        "3a7 cli-q",
        "┌────────┬───────┬─────┬─────────┬─────┬─────┐\n│ column │ count │ sum │ average │ max │ min │\n├────────┼───────┼─────┼─────────┼─────┼─────┤\n│ score │ 3 │ 60 │ 20 │ 30 │ 10 │\n│ count │ 3 │ 12 │ 4 │ 6 │ 2 │\n└────────┴───────┴─────┴─────────┴─────┴─────┘\n",
        "┌────────┬───────┬─────┬─────────┬─────┬─────┐\n│ column │ count │ sum │ average │ max │ min │\n├────────┼───────┼─────┼─────────┼─────┼─────┤\n│ score │ 3 │ 24 │ 8 │ 12 │ 3 │\n│ count │ 3 │ 13 │ 4.3333 │ 7 │ 1 │\n└────────┴───────┴─────┴─────────┴─────┴─────┘\n",
    ),
    (
        "3a7 cli-g",
        "Column | Sum | Mean | Max | Min\n--------------------------------------\nscore | 60.00 | 20.00 | 30.00 | 10.00\ncount | 12.00 | 4.00 | 6.00 | 2.00\n",
        "Column | Sum | Mean | Max | Min\n------------------------------------\nscore | 24.00 | 8.00 | 12.00 | 3.00\ncount | 13.00 | 4.33 | 7.00 | 1.00\n",
    ),
    (
        "3a8 cli-q",
        "csv-analyzer: summary for .commandagent/evidence/python-cli-fixtures/input-a.csv\nColumn | Count | Sum | Mean | Max | Min\n-------+-------+-----+------+-----+----\nscore | 3 | 60 | 20 | 30 | 10\ncount | 3 | 12 | 4 | 6 | 2\n",
        "csv-analyzer: summary for .commandagent/evidence/python-cli-fixtures/input-b.csv\nColumn | Count | Sum | Mean | Max | Min\n-------+-------+-----+--------+-----+----\nscore | 3 | 24 | 8 | 12 | 3\ncount | 3 | 13 | 4.3333 | 7 | 1\n",
    ),
    (
        "3a10 cli-q",
        "Statistics for .commandagent/evidence/python-cli-fixtures/input-a.csv\n+--------+-------+-----+------+-----+-----+\n| column | count | sum | mean | min | max |\n+--------+-------+-----+------+-----+-----+\n| score | 3 | 60 | 20 | 10 | 30 |\n| count | 3 | 12 | 4 | 2 | 6 |\n+--------+-------+-----+------+-----+-----+\n",
        "Statistics for .commandagent/evidence/python-cli-fixtures/input-b.csv\n+--------+-------+-----+---------+-----+-----+\n| column | count | sum | mean | min | max |\n+--------+-------+-----+---------+-----+-----+\n| score | 3 | 24 | 8 | 3 | 12 |\n| count | 3 | 13 | 4.33333 | 1 | 7 |\n+--------+-------+-----+---------+-----+-----+\n",
    ),
    (
        "3a10 cli-g",
        "Column | Sum | Avg | Max | Min\n--------------------------------------------------------------------------------\nscore | 60.00 | 20.00 | 30.00 | 10.00\ncount | 12.00 | 4.00 | 6.00 | 2.00\n",
        "Column | Sum | Avg | Max | Min\n--------------------------------------------------------------------------------\nscore | 24.00 | 8.00 | 12.00 | 3.00\ncount | 13.00 | 4.33 | 7.00 | 1.00\n",
    ),
];

#[test]
fn recorded_real_cli_outputs_stay_bound_and_correct() {
    for (run, first, second) in RECORDED {
        assert_eq!(bind(first, OLD_A), Binding::Bound, "{run} first");
        assert_eq!(bind(second, OLD_B), Binding::Bound, "{run} second");
    }
}

#[test]
fn corpus_swapped_columns_app_reproduces_the_recorded_expectation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/python-cli-csv-swapped-columns/src/csv_stats/main.py"
    ));
    let expected: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/python-cli-csv-swapped-columns/fixtures/expected-probe.json"
    )))
    .unwrap();
    let (status, reasons) = probe_source(source);
    assert_eq!(Some(status), expected["status"].as_str(), "{reasons:?}");
    for reason in expected["reasons"].as_array().unwrap() {
        let reason = reason.as_str().unwrap();
        assert!(reasons.iter().any(|actual| actual == reason), "{reasons:?}");
    }
}
