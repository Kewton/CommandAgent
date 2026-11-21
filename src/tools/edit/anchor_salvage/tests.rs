use super::indentation_preserving_replacement;
use crate::tools::edit::run;

const REPLAY: &str = "class Replay:\n    def __init__(self):\n        self.items = []\n\n    def push(self, rec):\n        self.items.append(rec)\n\n    def records(self):\n        return list(self.items)\n\n    def report(self):\n        return len(self.items)\n";

/// `push` and `records` without the class indentation, as the model sent them.
const DEDENTED_OLD: &str = "def push(self, rec):\n    self.items.append(rec)\n\ndef records(self):\n    return list(self.items)";
const DEDENTED_NEW: &str = "def push(self, rec):\n    if rec is not None:\n        self.items.append(rec)\n\ndef records(self):\n    return [r for r in self.items]";

fn edit_file(
    name: &str,
    content: &str,
    old: &str,
    new: &str,
) -> (tempfile::TempDir, Result<String, String>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    let result = run(dir.path(), &path, old, new, false).map_err(|err| err.to_string());
    (dir, result)
}

fn read(dir: &tempfile::TempDir, name: &str) -> String {
    std::fs::read_to_string(dir.path().join(name)).unwrap()
}

/// Names of the `def` lines at exactly `indent` spaces that follow `class`
/// before the next line at the class's own level: the class body methods.
fn class_methods(text: &str, class: &str, indent: usize) -> Vec<String> {
    let header = format!("class {class}");
    let mut lines = text.lines().skip_while(|line| !line.starts_with(&header));
    assert!(lines.next().is_some(), "class {class} missing:\n{text}");
    let mut methods = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let width = line.len() - line.trim_start().len();
        if width == 0 {
            break;
        }
        if width == indent
            && let Some(rest) = line.trim_start().strip_prefix("def ")
        {
            methods.push(rest.split('(').next().unwrap().to_string());
        }
    }
    methods
}

fn assert_refused_unchanged(
    dir: &tempfile::TempDir,
    name: &str,
    before: &str,
    result: &Result<String, String>,
) {
    let err = result.as_ref().unwrap_err();
    assert!(err.contains("edit_anchor_not_found"), "{err}");
    assert!(err.contains("Deterministic best-match region"), "{err}");
    assert!(err.contains("Re-anchor mandate"), "{err}");
    assert!(!err.contains("edit_anchor_salvaged"), "{err}");
    assert_eq!(read(dir, name), before);
}

#[test]
fn salvage_shifts_dedented_class_methods_back_into_the_class() {
    let (dir, result) = edit_file("log_replay.py", REPLAY, DEDENTED_OLD, DEDENTED_NEW);
    let output = result.unwrap();
    assert!(
        output.contains("edit_anchor_salvaged at line 5"),
        "{output}"
    );
    let edited = read(&dir, "log_replay.py");
    assert_eq!(
        edited,
        "class Replay:\n    def __init__(self):\n        self.items = []\n\n    def push(self, rec):\n        if rec is not None:\n            self.items.append(rec)\n\n    def records(self):\n        return [r for r in self.items]\n\n    def report(self):\n        return len(self.items)\n"
    );
    assert_eq!(
        class_methods(&edited, "Replay", 4),
        ["__init__", "push", "records", "report"]
    );
}

#[test]
fn salvage_refuses_new_that_mixes_the_dedented_and_real_levels() {
    // `old` is uniformly 4 spaces shallow, but `new` starts at the real level
    // and keeps the bodies shallow: neither (a) nor (b) holds.
    let new = "    def push(self, rec):\n    self.items.append(rec)\n\n    def records(self):\n    return list(self.items)";
    let (dir, result) = edit_file("log_replay.py", REPLAY, DEDENTED_OLD, new);
    assert_refused_unchanged(&dir, "log_replay.py", REPLAY, &result);
    let err = result.unwrap_err();
    assert!(err.contains("log_replay.py:5"), "{err}");
    assert!(err.contains("   5 |     def push(self, rec):"), "{err}");
    assert!(
        err.contains("   6 |         self.items.append(rec)"),
        "{err}"
    );
}

#[test]
fn salvage_refuses_new_line_above_what_a_negative_shift_can_remove() {
    // `old` is uniformly 4 spaces deeper than the top-level region, but one
    // `new` line is already at the top level.
    let content = "def run():\n    return  1\n";
    let old = "    def run():\n        return 1";
    let new = "    def run():\nreturn 2";
    let (dir, result) = edit_file("a.py", content, old, new);
    assert_refused_unchanged(&dir, "a.py", content, &result);
}

#[test]
fn salvage_refuses_shallow_new_even_when_old_indentation_matches() {
    let content = "class A:\n    x  = 1\n    y = 2\n";
    let (dir, result) = edit_file("a.py", content, "    x = 1\n    y = 2", "x = 3\ny = 2");
    assert_refused_unchanged(&dir, "a.py", content, &result);
}

#[test]
fn salvage_refuses_mixed_tab_and_space_indentation() {
    let content = "class A:\n\tdef f(self):\n\t\treturn  1\n";
    let old = "    def f(self):\n        return 1";
    let new = "    def f(self):\n        return 2";
    let (dir, result) = edit_file("a.py", content, old, new);
    assert_refused_unchanged(&dir, "a.py", content, &result);
}

#[test]
fn salvage_shifts_with_tabs_when_every_indent_is_a_tab() {
    let content = "class A:\n\tdef f(self):\n\t\treturn  1\n";
    let (dir, result) = edit_file(
        "a.py",
        content,
        "def f(self):\n\treturn 1",
        "def f(self):\n\treturn 2",
    );
    assert!(result.unwrap().contains("edit_anchor_salvaged"));
    assert_eq!(
        read(&dir, "a.py"),
        "class A:\n\tdef f(self):\n\t\treturn 2\n"
    );
}

#[test]
fn salvage_keeps_blank_lines_inside_the_region() {
    // Region blank lines carry trailing spaces; `old` and `new` blank lines
    // are empty and stay empty after the shift.
    let content = "class A:\n    def f(self):\n        a = 1\n    \n        return  a\n";
    let old = "def f(self):\n    a = 1\n\n    return a";
    let new = "def f(self):\n    a = 2\n\n    return a";
    let (dir, result) = edit_file("a.py", content, old, new);
    assert!(result.unwrap().contains("edit_anchor_salvaged"));
    assert_eq!(
        read(&dir, "a.py"),
        "class A:\n    def f(self):\n        a = 2\n\n        return a\n"
    );
}

#[test]
fn salvage_at_top_level_accepts_flattened_old_and_deeper_old() {
    let content = "def run():\n    return  1\n\nprint(run())\n";
    // (a): the body was flattened in `old`, `new` carries the real levels.
    let (dir, result) = edit_file(
        "a.py",
        content,
        "def run():\nreturn 1",
        "def run():\n    return 2",
    );
    assert!(result.unwrap().contains("edit_anchor_salvaged at line 1"));
    assert_eq!(
        read(&dir, "a.py"),
        "def run():\n    return 2\n\nprint(run())\n"
    );
    // (b) with a negative shift: `old` and `new` are 4 spaces too deep.
    let (dir, result) = edit_file(
        "a.py",
        content,
        "    def run():\n        return 1",
        "    def run():\n        return 3",
    );
    assert!(result.unwrap().contains("edit_anchor_salvaged at line 1"));
    assert_eq!(
        read(&dir, "a.py"),
        "def run():\n    return 3\n\nprint(run())\n"
    );
}

#[test]
fn salvage_shift_may_add_a_method_after_the_region() {
    let old = "def report(self):\n    return len(self.items)";
    let new =
        "def report(self):\n    return len(self.items)\n\ndef clear(self):\n    self.items.clear()";
    let content = REPLAY.replace("return len(self.items)", "return  len(self.items)");
    let (dir, result) = edit_file("log_replay.py", &content, old, new);
    assert!(result.unwrap().contains("edit_anchor_salvaged"));
    assert_eq!(
        class_methods(&read(&dir, "log_replay.py"), "Replay", 4),
        ["__init__", "push", "records", "report", "clear"]
    );
}

#[test]
fn salvage_rules_are_the_same_for_every_file_name() {
    let js = "class Box {\n  get() {\n    return  1;\n  }\n}\n";
    let js_old = "get() {\n  return 1;\n}";
    let js_new = "get() {\n  return 2;\n}";
    let js_expected = "class Box {\n  get() {\n    return 2;\n  }\n}\n";
    let yaml = "service:\n  ports:\n    - 80\n    -  443\n";
    let yaml_old = "ports:\n  - 80\n  - 443";
    let yaml_new = "ports:\n  - 80\n  - 8443";
    let yaml_expected = "service:\n  ports:\n    - 80\n    - 8443\n";
    for name in ["box.js", "box.txt", "box"] {
        let (dir, result) = edit_file(name, js, js_old, js_new);
        assert!(result.unwrap().contains("edit_anchor_salvaged"), "{name}");
        assert_eq!(read(&dir, name), js_expected, "{name}");
        let mixed = "get() {\n    return 2;\n  }";
        let (dir, result) = edit_file(name, js, js_old, mixed);
        assert_refused_unchanged(&dir, name, js, &result);
    }
    for name in ["svc.yaml", "svc.yml", "svc.txt"] {
        let (dir, result) = edit_file(name, yaml, yaml_old, yaml_new);
        assert!(result.unwrap().contains("edit_anchor_salvaged"), "{name}");
        assert_eq!(read(&dir, name), yaml_expected, "{name}");
        let (dir, result) = edit_file(name, yaml, yaml_old, "ports:\n- 80\n- 8443");
        assert_refused_unchanged(&dir, name, yaml, &result);
    }
}

#[test]
fn replacement_keeps_crlf_line_endings_while_shifting() {
    let region = "    a  = 1\r\n    b = 2";
    let shifted = indentation_preserving_replacement(region, "a = 1\r\nb = 2", "a = 3\r\nb = 2");
    assert_eq!(shifted.as_deref(), Some("    a = 3\r\n    b = 2"));
}

#[test]
fn replacement_of_a_deleted_region_is_empty() {
    assert_eq!(
        indentation_preserving_replacement("    a  = 1\n    b = 2", "a = 1\nb = 2", "").as_deref(),
        Some("")
    );
}

#[test]
fn salvage_refuses_space_indented_edit_of_a_tab_indented_region() {
    // Region levels are one and two tabs; `old` and `new` are one level
    // shallow with a one-space body. Counting characters alone would shift
    // by one tab and write a tab-plus-space body.
    let content = "class A:\n\tdef f(self):\n\t\treturn  1\n";
    let old = "def f(self):\n return 1";
    let new = "def f(self):\n return 2";
    let (dir, result) = edit_file("a.py", content, old, new);
    assert_refused_unchanged(&dir, "a.py", content, &result);
    let err = result.unwrap_err();
    assert!(err.contains("a.py:2"), "{err}");
    assert!(err.contains("   2 | \tdef f(self):"), "{err}");
    assert_eq!(
        indentation_preserving_replacement("\tdef f(self):\n\t\treturn  1", old, new),
        None
    );
}

#[test]
fn replacement_refuses_old_whose_shift_differs_between_lines() {
    // `old` is 4 then 0 spaces shallower than the region, and `new` does not
    // sit at the region's levels either.
    assert_eq!(
        indentation_preserving_replacement(
            "    x  = 1\n    y = 2",
            "x = 1\n    y = 2",
            "x = 3\ny = 2"
        ),
        None
    );
}

#[test]
fn salvage_refuses_new_line_above_the_region_shallowest_level() {
    // One method body line as the region; `new` starts at that body level but
    // appends a sibling method without the class indentation.
    let content = "class A:\n    def f(self):\n        return  1\n";
    let new = "        return 2\ndef g(self):\n    return 3";
    let (dir, result) = edit_file("a.py", content, "return 1", new);
    assert_refused_unchanged(&dir, "a.py", content, &result);
    let err = result.unwrap_err();
    assert!(err.contains("   3 |         return  1"), "{err}");
    assert_eq!(
        indentation_preserving_replacement("        return  1", "return 1", new),
        None
    );
}

#[test]
fn replacement_refuses_new_whose_first_line_is_deeper_than_the_region() {
    // Every other check holds for `new` as written: the last line matches the
    // region's last line and no line is above the region's level.
    assert_eq!(
        indentation_preserving_replacement(
            "    x  = 1\n    y = 2",
            "x = 1\ny = 2",
            "        x = 3\n    y = 2"
        ),
        None
    );
}
