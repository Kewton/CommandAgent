use super::find;
use crate::tools::bash_write_guard::verifier_mutation::rejection;

fn operation(command: &str) -> Option<String> {
    find(command).map(|mutation| mutation.operation)
}

#[test]
fn opaque_execution_forms_are_refused_whatever_their_body() {
    for command in [
        "sh -c 'cat a.txt'",
        "sh -c 'cp always-pass.sh smoke-check.sh'",
        "bash -c 'echo ok'",
        "dash -c true",
        "zsh -c true",
        "ksh -c true",
        "bash -lc 'npm test'",
        "bash -ec 'npm test'",
        "sh -xc true",
        "bash --norc -c true",
        "bash -o pipefail -c true",
        "bash -eo pipefail -c true",
        "bash -O extglob -c true",
        "bash --rcfile rc -c true",
        "eval 'echo ok'",
        "eval cat a.txt",
        "echo a.txt | xargs cat",
        "busybox cat a.txt",
        "busybox sh -c true",
        "find . -name '*.ts' -exec cat {} ;",
        "find . -execdir cat {} ;",
        "find . -ok cat {} ;",
        "find . -okdir cat {} ;",
        "find . -delete",
        "find . -fprint out.txt",
        "find . -name x -exec true ;",
        "echo 'cp a b' | sh",
        "echo 'cp a b' | bash -s",
        "echo 'cp a b' | sh -",
    ] {
        assert!(find(command).is_some(), "{command}");
        assert!(rejection(command, true).is_some(), "{command}");
    }
}

#[test]
fn inline_interpreters_are_refused_whatever_their_body() {
    for command in [
        "node -e 'console.log(1)'",
        "node -p 1",
        "node -pe 1",
        "node -ep 1",
        "node --eval 'console.log(1)'",
        "node --eval='console.log(1)'",
        "node --print 1",
        "node --print=1",
        "node -e",
        "node -r ts-node/register -e 1",
        "node --require ./hook.js -p 1",
        "node --max-old-space-size 4096 -e 1",
        "node --no-warnings -p 1",
        "node -",
        "nodejs -p 1",
        "python -c 'print(1)'",
        "python3 -c print\\(1\\)",
        "python3.12 -c 1",
        "python3 -Bc 1",
        "python3 -W ignore -c 1",
        "python3 -X dev -c 1",
        "python3 -",
        "perl -e 1",
        "perl -E 'say 1'",
        "perl -ne 'print' a.txt",
        "perl -pe 1 a.txt",
        "perl -MData::Dumper -e 1",
        "perl -0777 -ne 'print' a.txt",
        "perl5.38.2 -e 1",
        "perl -",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
}

#[test]
fn sed_outside_the_numeric_print_grammar_is_refused_unread() {
    for command in [
        "sed 's/a/b/' a.txt",
        "sed -e p a.txt",
        "sed -n p a.txt",
        "sed -n '$p' a.txt",
        "sed -n '/x/p' a.txt",
        "sed -n -e '1,5p' a.txt",
        "sed -n '1,5w out.txt' a.txt",
        "sed -n '1,5p;w out.txt' a.txt",
        "sed -n '1e touch x' a.txt",
        "sed -n '1,5p' -i a.txt",
        "sed -n '1,5p' -f prog.sed",
        "sed -f prog.sed a.txt",
        "sed --file=prog.sed a.txt",
        "sed -n 's/a/b/w out.txt' a.txt",
        "sed -n '1,2,3p' a.txt",
        "sed -n ',5p' a.txt",
        "sed -n '1,p' a.txt",
        "sed",
        "gsed 's/a/b/' a.txt",
        "gsed -n '1,5w out' a.txt",
        "/usr/bin/sed -n 'w out' a.txt",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
}

#[test]
fn the_numeric_print_grammar_and_ordinary_verifiers_are_kept() {
    for command in [
        "sed -n '1,5p' src/a.ts",
        "sed -n 1,10p src/a.ts",
        "sed -n 3p src/a.ts",
        "sed -n '1,5p' a.ts b.ts",
        "gsed -n '1,5p' a.ts",
        "cat a.ts | sed -n '1,5p'",
        "npm test",
        "npm run build",
        "npx tsc",
        "npx tsc --noEmit",
        "node smoke-check.js",
        "node --check smoke-check.js",
        "node --test tests",
        "node -v",
        "node --version",
        "node smoke-check.js --label=-e",
        "node smoke-check.js -e",
        "node smoke-check.js --eval --print",
        "node -r ts-node/register smoke-check.js -p",
        "node --experimental-vm-modules smoke-check.mjs",
        "python3 -m pytest",
        "python -m pytest -c cfg.ini",
        "python3 -m pytest -q -c cfg.ini",
        "python3 -Bm pytest -c cfg.ini",
        "python3 -W ignore -m pytest",
        "python3 smoke-check.py -c",
        "python3 -m compileall -q src",
        "python3 --version",
        "pytest",
        "pytest -c pytest.ini",
        "perl script.pl -e",
        "perl -MData::Dumper script.pl",
        "perl -v",
        "sh smoke-check.sh",
        "sh smoke-check.sh -c",
        "bash smoke-check.sh -c",
        "bash -x smoke-check.sh",
        "bash -n smoke-check.sh",
        "bash -eu smoke-check.sh",
        "bash -o pipefail smoke-check.sh",
        "bash +c smoke-check.sh",
        "bash -- smoke-check.sh",
        "bash --version",
        "find . -name '*.ts'",
        "find . -name '*.ts' -print",
        "find src -type f -name '*.rs' -maxdepth 2",
        "find . -name -exec",
        "find . -path '*-exec*'",
        "grep -n 'node -e' src/a.ts",
        "grep -rn \"sh -c\" src",
        "echo \"sh -c 'cp a b'\"",
        "echo 'eval cat a.txt'",
        "printf '%s\\n' 'xargs rm'",
        "timeout 5 npm test",
        "nice -n5 npm test",
        "timeout -sKILL 5 npm test",
        "command -v node",
        "cd src && npm test",
        "npm test 2>&1",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}

#[test]
fn prefixes_quotes_and_absolute_paths_do_not_hide_the_program() {
    for command in [
        "/bin/sh -c true",
        "/usr/bin/env bash -lc true",
        "'sh' -c true",
        "\"bash\" -c true",
        "\\sh -c true",
        "env FOO=1 sh -c true",
        "env -i sh -c true",
        "command sh -c true",
        "exec sh -c true",
        "nohup sh -c true",
        "timeout 5 sh -c true",
        "timeout 5 /usr/bin/node -p 1",
        "nice -n 5 python3 -c 1",
        "FOO=1 perl -e 1",
        "FOO=1 BAR=2 sh -c true",
        "env FOO=1 timeout 5 command xargs cat",
        "/usr/bin/eval x",
        "env find . -exec true ;",
        "command busybox true",
        "sudo bash -c true",
        "time sh -c true",
        "! sh -c true",
        "if sh -c true; then npm test; fi",
        "while sh -c true; do npm test; done",
        "sh -c true >/dev/null",
        "sh -c true 2>&1",
        "/usr/bin/sed -n 'w out' a.txt",
        "env sed -e p a.txt",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
}

#[test]
fn an_opaque_form_in_any_segment_is_refused_whatever_runs_first() {
    for command in [
        "sh make-sentinel.sh && sh -c true",
        "npm test; node -p 1",
        "npm test || python3 -c 1",
        "npm test | xargs cat",
        "npm test\nsh -c true",
        "cd src && bash -lc true",
        "npm test & sh -c true",
        "echo ok && find . -exec true ;",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
}

#[test]
fn an_unresolved_prefix_does_not_hide_the_target() {
    for command in [
        "timeout -sKILL 5 sh -c true",
        "nice -n5 bash -lc true",
        "timeout -k5 10 node -p 1",
        "nice -n5 python3 -c 1",
        "timeout -sKILL 5 perl -e 1",
        "nice -n5 xargs cat",
        "timeout -sKILL 5 find . -exec true ;",
        "nice -n5 sed -e p a.txt",
        "nice -n5 busybox true",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
}

#[test]
fn operations_name_the_refused_form() {
    for (command, expected) in [
        ("sh -c true", "sh -c"),
        ("bash -lc true", "bash -c"),
        ("eval x", "eval"),
        ("xargs cat", "xargs"),
        ("busybox true", "busybox"),
        ("find . -exec true ;", "find -exec"),
        ("find . -delete", "find -delete"),
        ("node -p 1", "node -e/-p"),
        ("python3 -c 1", "python3 -c"),
        ("perl -e 1", "perl -e"),
        ("sed -f p a.txt", "sed outside `-n N,Mp`"),
        ("echo x | sh", "sh reading a script from stdin"),
    ] {
        assert_eq!(operation(command).as_deref(), Some(expected), "{command}");
    }
    let refused = find("sh -c true").unwrap();
    assert!(refused.opaque);
    assert_eq!(refused.describe(), "`sh -c`");
}

#[test]
fn the_composed_entry_keeps_the_explicit_update_reading_first() {
    let update = rejection("sed -i s/1/2/ a.ts", false).unwrap();
    assert_eq!(update.operation, "sed -i");
    assert!(!update.opaque);
    let write = rejection("sh -c true && rm a.ts", false).unwrap();
    assert_eq!(write.operation, "rm");
    let opaque = rejection("sh -c true", false).unwrap();
    assert!(opaque.opaque);
    assert!(rejection("npm test 2>/dev/null", true).is_none());
    assert!(rejection("npm test 2>/dev/null", false).is_some());
}

#[test]
fn a_command_the_lexer_cannot_read_is_left_to_the_write_table() {
    // The write table names an unreadable command; this module adds nothing.
    assert!(find("sh -c 'unterminated").is_none());
    assert!(rejection("sh -c 'unterminated", false).is_some());
}

#[test]
fn an_unknown_node_option_never_hides_an_inline_flag() {
    for command in [
        "node --unknown -e 1",
        "node --unknown -p 1",
        "node --unknown value -e 1",
        "node --unknown value --eval=1",
        "node --unknown value --print 1",
        "node --unknown value -pe 1",
        "node --unknown --other value -e 1",
        "node --unknown -r hook -e 1",
        "node --unknown -",
        "nice -n5 node --unknown value -e 1",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
    for command in [
        "node --unknown smoke-check.js",
        "node --unknown value smoke-check.js -e",
        "node --unknown=-e smoke-check.js",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}
