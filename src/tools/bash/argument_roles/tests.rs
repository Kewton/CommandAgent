//! Issue #930 unit tests: argument roles, option boundaries, program side
//! effects, unknown syntax, and the path candidates that must remain.

use super::read_only_program::{self, Verdict};
use super::{Roles, data_only_words, judge};
use crate::tools::bash::path_tokens::{CrMode, read_segments};

fn roles(command: &str) -> (Vec<Vec<String>>, Roles) {
    let segments = read_segments(command, CrMode::Separator)
        .unwrap_or_else(|| panic!("readable shell text: {command:?}"));
    let words = segments
        .iter()
        .map(|segment| segment.words.iter().map(|word| word.text.clone()).collect())
        .collect();
    (words, judge(&segments))
}

/// The words of `command` whose role is proven data, in order.
fn data(command: &str) -> Vec<String> {
    let (words, roles) = roles(command);
    let mut found = Vec::new();
    for (segment, segment_words) in words.iter().enumerate() {
        for (index, word) in segment_words.iter().enumerate() {
            if roles.is_data(segment, index) {
                found.push(word.clone());
            }
        }
    }
    found
}

/// The words of `command` that keep the path proof, in order.
fn kept(command: &str) -> Vec<String> {
    let (words, roles) = roles(command);
    let mut found = Vec::new();
    for (segment, segment_words) in words.iter().enumerate() {
        for (index, word) in segment_words.iter().enumerate() {
            if !roles.is_data(segment, index) {
                found.push(word.clone());
            }
        }
    }
    found
}

#[test]
fn argument_roles_sed_program_is_data_and_file_operand_is_kept() {
    for (command, program) in [
        ("sed -n '/begin/,/end/p' in.txt", "/begin/,/end/p"),
        ("sed -n -e '/begin/,/end/p' in.txt", "/begin/,/end/p"),
        (
            "sed -n --expression '/begin/,/end/p' in.txt",
            "/begin/,/end/p",
        ),
        ("sed '1d;$d' in.txt", "1d;$d"),
        ("sed -e 's#/old#/new#g' in.txt", "s#/old#/new#g"),
        ("sed -E 's/a|b/c/gI' in.txt", "s/a|b/c/gI"),
        ("sed -- 's/x/y/' in.txt", "s/x/y/"),
    ] {
        assert_eq!(data(command), [program], "{command}");
        assert!(kept(command).contains(&"in.txt".to_string()), "{command}");
    }
    // A joined expression is the whole word.
    assert_eq!(data("sed -ne/x/p in.txt"), ["-ne/x/p"]);
    assert_eq!(
        data("sed --expression=/a/,/b/p in.txt"),
        ["--expression=/a/,/b/p"]
    );
    // Every file operand after the program stays a candidate.
    assert_eq!(
        kept("sed -n p in.txt sub/link/secret /outside/x ../up"),
        [
            "sed",
            "-n",
            "in.txt",
            "sub/link/secret",
            "/outside/x",
            "../up"
        ]
    );
    // With `-e`, the first operand is a file, not a program.
    assert_eq!(data("sed -e p sub/link/secret"), ["p"]);
    assert!(kept("sed -e p sub/link/secret").contains(&"sub/link/secret".to_string()));
}

#[test]
fn argument_roles_awk_program_and_separator_are_data() {
    for (command, expected) in [
        (
            "awk '/begin/{f=1;next} /end/{f=0} f{print}' in.txt",
            vec!["/begin/{f=1;next} /end/{f=0} f{print}"],
        ),
        (
            "awk '/<script>/{f=1;next} /<\\/script>/{f=0} f' index.html",
            vec!["/<script>/{f=1;next} /<\\/script>/{f=0} f"],
        ),
        ("awk -F, '{print $2}' in.txt", vec!["-F,", "{print $2}"]),
        ("awk -F / '/x/' in.txt", vec!["/", "/x/"]),
        ("awk -- '{print}' in.txt", vec!["{print}"]),
    ] {
        assert_eq!(data(command), expected, "{command}");
        assert!(
            kept(command)
                .iter()
                .any(|word| word.ends_with(".txt") || word.ends_with(".html"))
        );
    }
    assert_eq!(
        kept("awk '{print}' in.txt sub/link/secret"),
        ["awk", "in.txt", "sub/link/secret"]
    );
}

#[test]
fn argument_roles_grep_pattern_is_data_and_pattern_file_is_a_read() {
    assert_eq!(data("grep -n '</tag>' in.txt"), ["</tag>"]);
    assert_eq!(data("grep -e '/item' -- in.txt"), ["/item"]);
    assert_eq!(data("grep --regexp=/item in.txt"), ["--regexp=/item"]);
    assert_eq!(data("grep -e /a -e /b in.txt"), ["/a", "/b"]);
    assert_eq!(data("grep -E -n '/x|/y' in.txt"), ["/x|/y"]);
    // A pattern that starts with `/` does not change the file operand.
    assert_eq!(
        kept("grep '/x' sub/link/secret"),
        ["grep", "sub/link/secret"]
    );
    // `-e` given: every operand is a file.
    assert_eq!(
        kept("grep -e x sub/link/secret in.txt"),
        ["grep", "-e", "sub/link/secret", "in.txt"]
    );
    // A pattern file is never a pattern.
    for command in [
        "grep -f sub/link/patterns in.txt",
        "grep --file sub/link/patterns in.txt",
        "grep --file=sub/link/patterns in.txt",
        "grep -fsub/link/patterns in.txt",
        "grep -nfsub/link/patterns in.txt",
    ] {
        assert!(data(command).is_empty(), "{command}");
    }
    let (_, joined) = roles("grep -fsub/link/patterns in.txt");
    assert_eq!(joined.extra_reads, ["sub/link/patterns"]);
    let (_, clustered) = roles("grep -nfsub/link/patterns in.txt");
    assert_eq!(clustered.extra_reads, ["sub/link/patterns"]);
    let (_, sed_file) = roles("sed -fsub/link/script in.txt");
    assert_eq!(sed_file.extra_reads, ["sub/link/script"]);
    let (_, awk_file) = roles("awk -fsub/link/script in.txt");
    assert_eq!(awk_file.extra_reads, ["sub/link/script"]);
}

#[test]
fn argument_roles_echo_and_printf_display_data() {
    assert_eq!(
        data(r#"echo '{"pointer":"/item"}'"#),
        [r#"{"pointer":"/item"}"#]
    );
    assert_eq!(
        data(r#"printf '%s\n' '{"pointer":"/item/2"}'"#),
        [r"%s\n", r#"{"pointer":"/item/2"}"#]
    );
    assert_eq!(data(r"printf 'literal-data\n'"), [r"literal-data\n"]);
    assert_eq!(data("printf -- '%s' /a"), ["--", "%s", "/a"]);
    let long = "x".repeat(1500);
    assert_eq!(
        data(&format!("printf '%s' '{long}'")),
        ["%s", long.as_str()]
    );
    // A format with a side effect or an unknown conversion is not data.
    for command in [
        "printf -v X '%s' /a",
        "printf '%b' /a",
        "printf '%n' /a",
        "printf '%d' /a",
        r"printf '\101' /a",
        r"printf '\c' /a",
        "printf",
    ] {
        assert!(data(command).is_empty(), "{command}");
    }
}

/// A program whose parse finds file I/O or command execution is never data,
/// so its word keeps the candidate reading it had before the fix, and the files
/// it names are returned for the existing proof (reads and writes apart).
#[test]
fn argument_roles_program_io_keeps_the_word_and_returns_its_files() {
    for (command, reads, writes) in [
        ("sed '1w /tmp/out' in.txt", vec![], vec!["/tmp/out"]),
        (
            "sed '1r sub/link/secret' in.txt",
            vec!["sub/link/secret"],
            vec![],
        ),
        ("sed 's/a/b/w ../out' in.txt", vec![], vec!["../out"]),
        ("sed -n '1,5w out.txt' in.txt", vec![], vec!["out.txt"]),
        ("sed -n '/x/W out' in.txt", vec![], vec!["out"]),
        (
            "sed 'R sub/link/secret' in.txt",
            vec!["sub/link/secret"],
            vec![],
        ),
        ("sed -e p -e '1w out' in.txt", vec![], vec!["out"]),
        ("sed -i 's/a/b/w out' in.txt", vec![], vec!["out"]),
        ("sed 's/a/b/e' in.txt", vec![], vec![]),
        ("sed '1e cat sub/link/secret' in.txt", vec![], vec![]),
        ("sed e in.txt", vec![], vec![]),
        (
            r#"awk '{print > "/tmp/out"}' in.txt"#,
            vec![],
            vec!["/tmp/out"],
        ),
        (
            r#"awk '{print >> "../out"}' in.txt"#,
            vec![],
            vec!["../out"],
        ),
        (
            r#"awk '{printf("%s", $0) > "out"}' in.txt"#,
            vec![],
            vec!["out"],
        ),
        (
            r#"awk '{getline line < "sub/link/secret"; print line}' in.txt"#,
            vec!["sub/link/secret"],
            vec![],
        ),
        (r#"awk '{print | "sh"}' in.txt"#, vec![], vec![]),
        (
            r#"awk '{system("cat sub/link/secret")}' in.txt"#,
            vec![],
            vec![],
        ),
        (r#"awk '{print > $2}' in.txt"#, vec![], vec![]),
        (r#"awk '@include "x"' in.txt"#, vec![], vec![]),
    ] {
        let (_, judged) = roles(command);
        assert_eq!(judged.extra_reads, reads, "{command}");
        assert_eq!(judged.extra_writes, writes, "{command}");
        assert!(data(command).is_empty(), "{command}");
    }
}

#[test]
fn argument_roles_unsupported_forms_keep_every_word() {
    for command in [
        // Unknown or unsupported options and missing values.
        "sed -i 's/a/b/' in.txt",
        "sed -s p in.txt",
        "sed -x p in.txt",
        "sed -e",
        "grep -r '/x' .",
        "grep -A 2 '/x' in.txt",
        "grep -e",
        "grep",
        "awk -v x=1 '{print}' in.txt",
        "awk -f prog.awk in.txt",
        "awk -F",
        // An option after an operand.
        "sed p in.txt -i",
        "grep x in.txt -e y",
        "awk '{print}' in.txt -F,",
        // Unsupported sed/awk syntax.
        "sed '1,5{p}' in.txt",
        "sed 'y/a/b/' in.txt",
        "sed '1a text' in.txt",
        "sed '/x/!d' in.txt",
        "sed 's/[/]/x/' in.txt",
        "awk 'BEGIN{print}' in.txt",
        "awk '{print $1+$2}' in.txt",
        "awk '$1 > 5' in.txt",
        "awk 'NR==1' in.txt",
        // A dynamic or glob word anywhere in the command.
        r#"sed "$X" in.txt"#,
        r#"sed "1d;$d" in.txt"#,
        "grep -n x $FILE",
        "grep -n x `pwd`/in.txt",
        "grep -n x $(pwd)/in.txt",
        "grep -n /x *.txt",
        "echo $HOME /a",
        // A quoted or escaped command name, a prefix, a wrapper.
        "'sed' p in.txt",
        r"\grep /x in.txt",
        "s'ed' p in.txt",
        "X=1 grep /x in.txt",
        "env grep /x in.txt",
        "command grep /x in.txt",
        "timeout 5 grep /x in.txt",
        "then grep /x in.txt",
    ] {
        assert!(data(command).is_empty(), "{command}");
    }
}

#[test]
fn argument_roles_redirects_keep_targets_and_refuse_input_forms() {
    // An output redirect target is a kept word; the data before it is data.
    assert_eq!(
        data(r"printf 'literal-data\n' > local.txt"),
        [r"literal-data\n"]
    );
    assert!(kept(r"printf 'literal-data\n' > local.txt").contains(&"local.txt".to_string()));
    assert_eq!(data("grep /x in.txt 2> err.log"), ["/x"]);
    assert!(
        kept("grep /x in.txt 2> err.log").contains(&"2".to_string())
            || kept("grep /x in.txt 2> err.log").contains(&"err.log".to_string())
    );
    assert!(kept("grep /x in.txt 2>sub/link/err").contains(&"sub/link/err".to_string()));
    assert_eq!(data("grep /x in.txt 2>&1"), ["/x"]);
    assert!(kept("sed p in.txt >> ../out").contains(&"../out".to_string()));
    // An input redirect, a here-string, a heredoc, or a descriptor stdin.
    for command in [
        "grep /x < in.txt",
        "grep /x <<< '/item'",
        "grep /x 0<in.txt",
        "grep /x <<EOF",
        "sed p <(cat in.txt)",
        "sed p in.txt < in.txt",
    ] {
        assert!(data(command).is_empty(), "{command}");
    }
    // A word after a redirect target is an argument the target hides.
    assert!(data("grep x > out -f sub/link/p").is_empty());
}

#[test]
fn argument_roles_pipeline_partners() {
    for command in [
        r#"echo '{"p":"/a/1"}' | python3 patchdoc.py"#,
        r#"printf '%s\n' '{"p":"/b"}' | python3 app.py"#,
        "sed -n '/a/,/b/p' in.txt | sed '1d;$d' > out.js",
        "grep -n /x in.txt | cat",
        "cat in.txt | grep /x",
    ] {
        assert!(!data(command).is_empty(), "{command}");
    }
    for command in [
        "printf '/a' | sh",
        "printf '/a' | bash",
        "printf '/a' | xargs cat",
        "printf '/a' | python3 -",
        "printf '/a' | python3 -c 'import sys'",
        "printf '/a' | python3",
        "printf '/a' | node app.js",
        "printf '/a' | python3 /outside/app.py",
        "printf '/a' | python3 ../app.py",
        "printf '/a' |& cat",
        "printf '/a' 2>&1 | sh",
        "printf '/a' | sed e",
        "printf '/a' | tee out",
        "grep /x in.txt | sed '1r sub/link/secret'",
    ] {
        assert!(data(command).is_empty(), "{command}");
    }
}

/// Each simple command after `;`, `&&`, `||`, or a newline is judged on its
/// own: the first safe command never admits the rest.
#[test]
fn argument_roles_following_commands_keep_their_words() {
    for (command, tail) in [
        ("sed -n p in.txt; tee sub/link/f", "sub/link/f"),
        ("grep /x in.txt && cat /outside/x", "/outside/x"),
        ("printf '/a'\ncat .env", ".env"),
        ("echo /a || cat sub/link/secret", "sub/link/secret"),
        ("echo /a & cat sub/link/secret", "sub/link/secret"),
    ] {
        assert!(kept(command).contains(&tail.to_string()), "{command}");
    }
}

#[test]
fn argument_roles_closing_words_and_groups_close_every_role() {
    for command in [
        "alias grep=cat; grep sub/link/secret",
        "grep() { cat \"$@\"; }; grep sub/link/secret",
        "function grep { cat; }; grep /x in.txt",
        "hash -p /bin/cat grep; grep sub/link/secret",
        "PATH=bin; grep sub/link/secret in.txt",
        "export PATH=bin; grep sub/link/secret in.txt",
        "BASH_CMDS[grep]=x; grep sub/link/secret in.txt",
        "enable -n echo; echo /a",
        ". ./defs.sh; grep /x in.txt",
        "source defs.sh; grep /x in.txt",
        "eval x; grep /x in.txt",
        "(grep /x in.txt)",
        "{ grep /x in.txt; }",
        "printf '/a' > list; xargs cat < list",
        "printf 'cat /a' > x.sh; sh x.sh",
    ] {
        assert!(data(command).is_empty(), "{command}");
    }
}

#[test]
fn argument_roles_data_only_words_exclude_any_other_occurrence() {
    let only = data_only_words("echo link && cat link", CrMode::Separator);
    assert!(!only.contains("link"));
    let only = data_only_words("echo link && cat in.txt", CrMode::Separator);
    assert!(only.contains("link"));
    assert!(!only.contains("in.txt"));
    let only = data_only_words("echo out > out", CrMode::Separator);
    assert!(!only.contains("out"));
    assert!(data_only_words("echo 'unterminated", CrMode::Separator).is_empty());
}

#[test]
fn argument_roles_sed_grammar_table() {
    for script in [
        "p",
        "d",
        "1d;$d",
        "/begin/,/end/p",
        "/<script>/,/<\\/script>/p",
        "s#/old#/new#g",
        "s/a/b/",
        "s/a\\/b/c/p",
        "s/[ab]/c/I",
        "s/[]a]/c/",
        "1,3p",
        "$p",
        " 1 d ; 2 p ",
        "1p\n2d",
    ] {
        assert_eq!(
            read_only_program::sed(script).0,
            Verdict::ReadOnly,
            "{script:?}"
        );
    }
    for script in [
        "1w /tmp/out",
        "w out",
        "1r sub/link/secret",
        "R x",
        "W x",
        "e",
        "1e date",
        "s/a/b/w out",
        "s/a/b/gw out",
        "s/a/b/e",
        "p;w out",
        "1,5{w out\n}",
        "s#a#b#w out",
    ] {
        assert_eq!(
            read_only_program::sed(script).0,
            Verdict::Effect,
            "{script:?}"
        );
    }
    for script in [
        "",
        ";",
        "1",
        "q",
        "1,5{p}",
        "y/a/b/",
        "1a w /tmp/out",
        "/x/!d",
        "/x/Ip",
        "\\%x%p",
        "0~2p",
        "1,+2p",
        "s/[/]/x/",
        "s/[\\/]/x/",
        "s/[[:alpha:]]/x/",
        "s/a/b/2",
        "s/a/b",
        "s/a/b/x",
        "s a b ",
        "/unterminated",
        "pp",
        "# comment\np",
    ] {
        assert_eq!(
            read_only_program::sed(script).0,
            Verdict::Unsupported,
            "{script:?}"
        );
    }
}

#[test]
fn argument_roles_awk_grammar_table() {
    for program in [
        "/begin/{f=1;next} /end/{f=0} f{print}",
        "/<script>/{f=1;next} /<\\/script>/{f=0} f",
        "f{print} /<script>$/{f=1} /<\\/script>/{f=0}",
        "{print}",
        "{print $1, $NF, \"x\", 2, f}",
        "/x/",
        "f",
        "/a/\n/b/{print}",
        "{ f = 1 }",
    ] {
        assert_eq!(
            read_only_program::awk(program).0,
            Verdict::ReadOnly,
            "{program:?}"
        );
    }
    for program in [
        "{print > \"/tmp/out\"}",
        "{print >> \"out\"}",
        "{print $1 > $2}",
        "{printf \"%s\", $0 > \"out\"}",
        "{getline < \"sub/link/secret\"}",
        "{\"cat x\" | getline}",
        "{print | \"sh\"}",
        "{print |& \"sh\"}",
        "{system(\"cat x\")}",
        "@include \"x\"",
        "BEGIN{system(\"x\")}",
    ] {
        assert_eq!(
            read_only_program::awk(program).0,
            Verdict::Effect,
            "{program:?}"
        );
    }
    for program in [
        "",
        "BEGIN{print}",
        "END{print NR}",
        "{print $1+$2}",
        "{print (\"x\")}",
        "{f=g}",
        "{f=1.5}",
        "{print \"a\\n\"}",
        "$1 > 5",
        "NR==1",
        "f /x/",
        "/[/]/",
        "{print",
        "\"unterminated",
        "/unterminated",
        "{print a b}",
        "{x()}",
        "function f(){}",
        "{print}`",
    ] {
        assert_eq!(
            read_only_program::awk(program).0,
            Verdict::Unsupported,
            "{program:?}"
        );
    }
}

/// A word that was quoted, escaped, or carries an expansion is told apart by
/// the scanner, and the opener of each segment is recorded.
#[test]
fn argument_roles_scanner_records_openers_and_quoting() {
    use crate::tools::bash::path_tokens::Opener;
    let segments = read_segments(
        "a 'b' c\\d | e && f; g & h 2>&1 >> i < j (k) |& l",
        CrMode::Separator,
    )
    .unwrap();
    let openers: Vec<Opener> = segments.iter().map(|segment| segment.opener).collect();
    assert_eq!(
        openers,
        [
            Opener::Start,
            Opener::Pipe,
            Opener::List,
            Opener::List,
            Opener::Background,
            Opener::Output,
            Opener::Duplicate,
            Opener::Output,
            Opener::Output,
            Opener::Input,
            Opener::Group,
            Opener::Group,
            Opener::Pipe,
            Opener::Pipe,
        ]
    );
    let first = &segments[0].words;
    assert!(!first[0].quoted && first[1].quoted && first[2].quoted);
    assert!(segments[4].glued_close, "`2>` glues the descriptor");
    assert!(!segments[0].glued_close);
}
