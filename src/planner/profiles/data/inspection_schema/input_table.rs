use std::collections::BTreeSet;
use std::io::BufRead;
use std::path::{Path, PathBuf};

const MAX_HEADER_BYTES: usize = 1024 * 1024;

pub(super) struct InputTable {
    pub relative_path: String,
    pub headers: Vec<String>,
    pub row_count: u64,
}

pub(super) fn load(root: &Path, path: PathBuf) -> Result<InputTable, String> {
    let metadata = path
        .metadata()
        .map_err(|error| format!("inspection_schema_violation:input_metadata:{error}"))?;
    if !metadata.is_file() {
        return Err("inspection_schema_violation:input_not_file".to_string());
    }
    let file = std::fs::File::open(&path)
        .map_err(|error| format!("inspection_schema_violation:input_unreadable:{error}"))?;
    let (header, record_count) = read_header_and_record_count(std::io::BufReader::new(file))?;
    let delimiter = delimiter(&path);
    let mut headers = parse_record(&header, delimiter)?;
    if let Some(first) = headers.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }
    if headers.iter().any(|header| header.trim().is_empty())
        || headers.iter().collect::<BTreeSet<_>>().len() != headers.len()
    {
        return Err("inspection_schema_violation:input_header_invalid".to_string());
    }
    Ok(InputTable {
        relative_path: crate::tools::path_guard::relative_display(root, &path),
        headers,
        row_count: record_count - 1,
    })
}

fn read_header_and_record_count(reader: impl BufRead) -> Result<(String, u64), String> {
    let mut header = Vec::new();
    let mut records = 0u64;
    let mut quoted = false;
    let mut record_pending = false;
    let mut bytes = reader.bytes().peekable();
    while let Some(byte) = bytes.next() {
        let byte =
            byte.map_err(|error| format!("inspection_schema_violation:input_unreadable:{error}"))?;
        record_pending = true;
        if records == 0 {
            header.push(byte);
            if header.len() > MAX_HEADER_BYTES {
                return Err("inspection_schema_violation:input_header_invalid".to_string());
            }
        }
        if byte == b'"' {
            if quoted
                && bytes
                    .peek()
                    .is_some_and(|next| next.as_ref().is_ok_and(|escaped| *escaped == b'"'))
            {
                let escaped = bytes.next().unwrap().map_err(|error| {
                    format!("inspection_schema_violation:input_unreadable:{error}")
                })?;
                if records == 0 {
                    header.push(escaped);
                    if header.len() > MAX_HEADER_BYTES {
                        return Err("inspection_schema_violation:input_header_invalid".to_string());
                    }
                }
                continue;
            }
            quoted = !quoted;
        }
        if byte == b'\n' && !quoted {
            records = records.checked_add(1).ok_or_else(|| {
                "inspection_schema_violation:input_row_count_overflow".to_string()
            })?;
            record_pending = false;
        }
    }
    if quoted {
        return Err("inspection_schema_violation:input_record_unclosed_quote".to_string());
    }
    if record_pending {
        records = records
            .checked_add(1)
            .ok_or_else(|| "inspection_schema_violation:input_row_count_overflow".to_string())?;
    }
    if records == 0 {
        return Err("inspection_schema_violation:input_header_invalid".to_string());
    }
    let header = std::str::from_utf8(trim_record_ending(&header))
        .map_err(|error| format!("inspection_schema_violation:input_header:{error}"))?
        .to_string();
    if header.is_empty() {
        return Err("inspection_schema_violation:input_header_invalid".to_string());
    }
    Ok((header, records))
}

fn trim_record_ending(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_suffix(b"\r\n")
        .or_else(|| bytes.strip_suffix(b"\n"))
        .unwrap_or(bytes)
}

fn delimiter(path: &Path) -> char {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("tsv"))
    {
        '\t'
    } else {
        ','
    }
}

fn parse_record(record: &str, delimiter: char) -> Result<Vec<String>, String> {
    let mut fields = vec![String::new()];
    let mut chars = record.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                fields.last_mut().unwrap().push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            value if value == delimiter && !quoted => fields.push(String::new()),
            value => fields.last_mut().unwrap().push(value),
        }
    }
    if quoted {
        Err("inspection_schema_violation:input_header_unclosed_quote".to_string())
    } else {
        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UAT5_RUN1_INPUT: &str = include_str!(
        "../../../../../tests/corpus/apps/test0715_data11_final_scope/fixtures/data5_qwen35_none_001/data/sales.csv"
    );
    const UAT5_RUN1_INSPECTION: &str = include_str!(
        "../../../../../tests/corpus/apps/test0715_data_inspection_schema/fixtures/uat5-run1-row-count-mismatch.json"
    );

    #[test]
    fn quoted_newlines_are_one_data_record_and_tsv_headers_are_supported() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("quoted.csv");
        std::fs::write(&csv, "name,note\r\nA,\"first\nsecond\"\r\nB,plain").unwrap();
        let table = load(dir.path(), csv).unwrap();
        assert_eq!(table.headers, ["name", "note"]);
        assert_eq!(table.row_count, 2);

        let tsv = dir.path().join("input.tsv");
        std::fs::write(&tsv, "name\tamount\nA\t1\n").unwrap();
        let table = load(dir.path(), tsv).unwrap();
        assert_eq!(table.headers, ["name", "amount"]);
        assert_eq!(table.row_count, 1);
    }

    #[test]
    fn measured_uat_row_count_mismatch_reports_expected_and_reported() {
        assert_eq!(UAT5_RUN1_INSPECTION.len(), 411);
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(dir.path().join("data/sales.csv"), UAT5_RUN1_INPUT).unwrap();
        std::fs::write(
            dir.path().join(super::super::INSPECTION_PATH),
            UAT5_RUN1_INSPECTION,
        )
        .unwrap();

        let evidence = super::super::check(dir.path()).unwrap();
        assert_eq!(evidence.input_path.as_deref(), Some("data/sales.csv"));
        assert_eq!(
            evidence.failure_kinds,
            ["inspection_schema_violation:input_row_count_mismatch:expected=60:reported=24"]
        );
    }
}
