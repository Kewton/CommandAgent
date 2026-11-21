//! Binds the expected CSV aggregates to their label and column in a CLI's stdout.
//!
//! The token check in the CSV behavior probe only asks whether an expected
//! number appears somewhere in stdout, so a table that prints max under the
//! `min` header still passes. This leaf reads stdout as a table and compares the
//! value under each `sum`, `mean`, `max` and `min` header for each numeric
//! column. It can only add conditions to a pass: output it cannot read as a
//! table is `Unbound` (the caller reports partial), and a bound value that
//! differs is a `Mismatch` (the caller reports failed).

/// Tolerance for display rounding such as `20.00` or `4.33` against `4.3333`.
const VALUE_TOLERANCE: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stat {
    Sum,
    Mean,
    Max,
    Min,
}

const STATS: [Stat; 4] = [Stat::Sum, Stat::Mean, Stat::Max, Stat::Min];

impl Stat {
    fn name(self) -> &'static str {
        match self {
            Stat::Sum => "sum",
            Stat::Mean => "mean",
            Stat::Max => "max",
            Stat::Min => "min",
        }
    }

    /// The closed list of header names accepted for each aggregate. A header
    /// name outside this list is not guessed at; the table stays unbound.
    fn from_header(cell: &str) -> Option<Stat> {
        match cell.to_lowercase().as_str() {
            "sum" | "total" | "合計" => Some(Stat::Sum),
            "mean" | "average" | "avg" | "平均" => Some(Stat::Mean),
            "max" | "maximum" | "最大" => Some(Stat::Max),
            "min" | "minimum" | "最小" => Some(Stat::Min),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ExpectedRow {
    pub(super) label: String,
    pub(super) sum: f64,
    pub(super) mean: f64,
    pub(super) max: f64,
    pub(super) min: f64,
}

impl ExpectedRow {
    fn value(&self, stat: Stat) -> f64 {
        match stat {
            Stat::Sum => self.sum,
            Stat::Mean => self.mean,
            Stat::Max => self.max,
            Stat::Min => self.min,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Binding {
    /// Every expected value sits under the matching header on its label's row.
    Bound,
    /// The table was read and at least one value differs from the expectation.
    Mismatch(Vec<String>),
    /// stdout could not be read as a table that binds each value to a label
    /// and a column.
    Unbound,
}

/// Computes the expected aggregates of every fully numeric column of a CSV
/// fixture, in header order.
pub(super) fn expected_rows(csv: &str) -> Vec<ExpectedRow> {
    let mut lines = csv.lines().filter(|line| !line.trim().is_empty());
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let names = header.split(',').map(str::trim).collect::<Vec<_>>();
    let records = lines
        .map(|line| line.split(',').map(str::trim).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let values = records
            .iter()
            .map(|record| record.get(index).and_then(|cell| cell.parse::<f64>().ok()))
            .collect::<Option<Vec<_>>>();
        let Some(values) = values.filter(|values| !values.is_empty()) else {
            continue;
        };
        let sum = values.iter().sum::<f64>();
        rows.push(ExpectedRow {
            label: (*name).to_string(),
            sum,
            mean: sum / values.len() as f64,
            max: values.iter().copied().fold(f64::MIN, f64::max),
            min: values.iter().copied().fold(f64::MAX, f64::min),
        });
    }
    rows
}

/// Reads `stdout` as a table and compares the expected aggregates of `csv`.
pub(super) fn bind(stdout: &str, csv: &str) -> Binding {
    bind_rows(stdout, &expected_rows(csv))
}

fn bind_rows(stdout: &str, expected: &[ExpectedRow]) -> Binding {
    if expected.is_empty() {
        return Binding::Unbound;
    }
    let rows = table_rows(stdout);
    let Some((header_at, columns)) = find_header(&rows) else {
        return Binding::Unbound;
    };
    let header_width = rows[header_at].len();
    let mut mismatches = Vec::new();
    // An unreadable label does not hide a mismatch found for another label.
    let mut unbound = false;
    for want in expected {
        let mut found = rows[header_at + 1..].iter().filter(|row| {
            row.first()
                .is_some_and(|cell| same_label(cell, &want.label))
        });
        let Some(row) = found.next() else {
            return Binding::Unbound;
        };
        if found.next().is_some() {
            unbound = true;
            continue;
        }
        // A header that omits the label column's title is one cell shorter.
        let offset = if row.len() == header_width {
            0
        } else if row.len() == header_width + 1 {
            1
        } else {
            unbound = true;
            continue;
        };
        for (stat, column) in columns {
            let Some(got) = row.get(column + offset).and_then(|cell| parse_number(cell)) else {
                unbound = true;
                continue;
            };
            let expected_value = want.value(stat);
            if (got - expected_value).abs() > VALUE_TOLERANCE {
                mismatches.push(format!(
                    "{}:{}:expected_{}:got_{}",
                    want.label,
                    stat.name(),
                    expected_value,
                    got
                ));
            }
        }
    }
    if !mismatches.is_empty() {
        Binding::Mismatch(mismatches)
    } else if unbound {
        Binding::Unbound
    } else {
        Binding::Bound
    }
}

fn same_label(cell: &str, label: &str) -> bool {
    cell.trim_matches('"').eq_ignore_ascii_case(label)
}

fn parse_number(cell: &str) -> Option<f64> {
    cell.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The first row naming each of the four aggregates exactly once, with the
/// column index of each.
fn find_header(rows: &[Vec<String>]) -> Option<(usize, [(Stat, usize); 4])> {
    'rows: for (at, row) in rows.iter().enumerate() {
        let mut columns = [(Stat::Sum, 0usize); 4];
        for (slot, stat) in STATS.into_iter().enumerate() {
            let mut hits = row
                .iter()
                .enumerate()
                .filter(|(_, cell)| Stat::from_header(cell) == Some(stat));
            let (Some((index, _)), None) = (hits.next(), hits.next()) else {
                continue 'rows;
            };
            columns[slot] = (stat, index);
        }
        return Some((at, columns));
    }
    None
}

/// Splits stdout into rows of cells, dropping rule lines such as `+---+---+`.
fn table_rows(stdout: &str) -> Vec<Vec<String>> {
    stdout
        .lines()
        .filter(|line| line.chars().any(char::is_alphanumeric))
        .map(split_cells)
        .collect()
}

fn split_cells(line: &str) -> Vec<String> {
    let mut cells = if line.contains(['|', '│']) {
        line.split(['|', '│']).map(str::trim).collect::<Vec<_>>()
    } else if line.contains('\t') {
        line.split('\t').map(str::trim).collect::<Vec<_>>()
    } else if line.contains(',') {
        line.split(',').map(str::trim).collect::<Vec<_>>()
    } else {
        line.split_whitespace().collect::<Vec<_>>()
    };
    // Outer pipes leave empty cells at both ends; interior empties stay so a
    // ragged row is not silently realigned.
    while cells.first().is_some_and(|cell| cell.is_empty()) {
        cells.remove(0);
    }
    while cells.last().is_some_and(|cell| cell.is_empty()) {
        cells.pop();
    }
    cells.into_iter().map(str::to_string).collect()
}
