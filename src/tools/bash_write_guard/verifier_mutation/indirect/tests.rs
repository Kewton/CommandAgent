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

#[test]
fn fix2_and_launcher_forms_are_refused_whatever_their_body() {
    for command in [
        "bash -oc pipefail 'cp a b'",
        "bash -Oc extglob 'cp a b'",
        "bash -eoc pipefail 'cp a b'",
        "env FOO=1 bash -oc pipefail 'cp a b'",
        "perl -M File::Copy -e 'print 1'",
        "perl -MFile::Copy -m Foo -e 1",
        "flock lockfile sh -c 'cp a b'",
        "chrt 1 sh -c 'cp a b'",
        "taskset 1 sh -c 'cp a b'",
        "unshare sh -c 'cp a b'",
        "strace -o /dev/null sh -c 'cp a b'",
        "watch -n1 sh -c 'cp a b'",
        "npx sh -c 'cp a b'",
        "npm exec -- sh -c 'cp a b'",
        "pnpm exec sh -c 'cp a b'",
        "npx -c 'cp a b'",
        "npx --call 'cp a b'",
        "npm exec -c 'cp a b'",
        "flock lockfile -c 'cp a b'",
        "script -qc 'cp a b' /dev/null",
        "node --import data:text/javascript,1 smoke-check.js",
        "node --import=data:text/javascript,1 smoke-check.js",
        "node -r data:text/javascript,1 smoke-check.js",
        "node --loader=data:text/javascript,1 smoke-check.js",
        "node --experimental-loader data:text/javascript,1 smoke-check.js",
        "perl -M'5;system(1)' script.pl",
        "perl -m'Foo bar' script.pl",
        "node --unknown -e 1",
        "node --foo=1 -p 1",
        "node --inspect-port 1 -e 1",
    ] {
        assert!(find(command).is_some(), "{command}");
        assert!(rejection(command, true).is_some(), "{command}");
    }
}

#[test]
fn launchers_option_values_and_literal_text_keep_ordinary_verifiers() {
    for command in [
        "bash -o pipefail smoke-check.sh",
        "bash -O extglob smoke-check.sh",
        "perl -M File::Copy script.pl -e",
        "perl -MFile::Copy script.pl",
        "perl -MFoo=a,b script.pl",
        "perl -M-strict script.pl",
        "node --trace-uncaught smoke-check.js -e",
        "node --trace-uncaught smoke-check.js",
        "node --no-warnings --trace-exit smoke-check.js -p",
        "node --require ./setup.js smoke-check.js",
        "node --import ./setup.mjs smoke-check.mjs",
        "npx tsc --noEmit",
        "npx tsc",
        "npx vitest run",
        "npx vitest run -c vitest.config.ts",
        "npm exec -- tsc --noEmit",
        "npm test",
        "pnpm exec tsc --noEmit",
        "flock lockfile npm test",
        "taskset -c 0 npm test",
        "echo sh -c",
        "grep -r 'sh -c' src",
        "grep 'node -e' README.md",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}

#[test]
fn node_boolean_known_valued_unknown_and_entrypoint_arguments_are_told_apart() {
    for command in [
        // known boolean, then the entrypoint, then its own arguments
        "node --trace-uncaught smoke-check.js -e",
        "node --trace-uncaught --no-warnings smoke-check.js -p",
        // known valued option, entrypoint after it
        "node --require ./hook.js smoke-check.js -e",
        // unknown option swallows a plain word, entrypoint after
        "node --unknown value smoke-check.js -e",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
    for command in [
        "node --trace-uncaught -e 1",
        "node --trace-uncaught --unknown -e 1",
        "node --unknown -e 1",
        "node --foo=1 -p 1",
        "node --inspect-port 1 -e 1",
        "node --require ./hook.js -e 1",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
}

#[test]
fn an_option_cluster_goes_on_after_taking_its_value() {
    for command in [
        "bash -oc pipefail true",
        "bash -Oc extglob true",
        "bash -eoc pipefail true",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
    for command in [
        "bash -o pipefail smoke-check.sh",
        "bash -O extglob smoke-check.sh",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}

#[test]
fn a_perl_module_switch_value_is_not_the_script() {
    for command in [
        "perl -M File::Copy -e 1",
        "perl -m Foo -I lib -e 1",
        "perl -I lib -e 1",
        "perl -M'5;system(1)' x.pl",
        "perl -M'Foo=a;b' x.pl",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
    for command in [
        "perl -M File::Copy script.pl -e",
        "perl -I lib script.pl -e",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}

#[test]
fn a_data_url_module_option_is_code() {
    for command in [
        "node --import data:text/javascript,1 x.js",
        "node --import=DATA:text/javascript,1 x.js",
        "node -r data:text/javascript,1 x.js",
        "node -rdata:text/javascript,1 x.js",
        "node --loader data:text/javascript,1 x.js",
        "node --experimental-loader=data:text/javascript,1 x.js",
    ] {
        assert!(find(command).is_some(), "{command}");
    }
    for command in ["node --import ./a.mjs x.js", "node -r ./a.js x.js"] {
        assert_eq!(operation(command), None, "{command}");
    }
}

#[test]
fn a_launcher_started_by_a_launcher_keeps_its_shell_string_option_refused() {
    for command in [
        "chrt 1 npx -c 'cp a b'",
        "taskset 1 npm exec -c 'cp a b'",
        "setsid script -qc 'cp a b' /dev/null",
        "nice -n 5 chrt 1 flock f -c 'cp a b'",
        "timeout 5 strace -o /dev/null npx -c 'cp a b'",
        "chrt 1 taskset 1 setsid sh -c 'cp a b'",
    ] {
        assert!(find(command).is_some(), "{command}");
        assert!(rejection(command, true).is_some(), "{command}");
    }
    for command in [
        "chrt 1 npx tsc --noEmit",
        "taskset -c 0 npm exec -- tsc --noEmit",
        "nice -n 5 chrt 1 npx vitest run",
        "timeout 5 strace -o /dev/null npm test",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}

#[test]
fn unresolved_prefixes_option_values_and_stdin_shells_do_not_hide_a_launcher_string() {
    for command in [
        "nice -n 5 npx -c 'cp a b'",
        "nice -n 5 flock f -c 'cp a b'",
        "nice -n 5 npm exec -c 'cp a b'",
        "nice -n 5 sh -c 'cp a b'",
        "nice -n5 npx -c 'cp a b'",
        "nice -n5 flock f -c 'cp a b'",
        "nice -n5 npm exec -c 'cp a b'",
        "nice -n5 sh -c 'cp a b'",
        "timeout -s KILL 5 npx -c 'cp a b'",
        "timeout -s KILL 5 flock f -c 'cp a b'",
        "timeout -s KILL 5 npm exec -c 'cp a b'",
        "timeout -s KILL 5 sh -c 'cp a b'",
        "timeout -sKILL 5 npx -c 'cp a b'",
        "timeout -sKILL 5 flock f -c 'cp a b'",
        "timeout -sKILL 5 npm exec -c 'cp a b'",
        "timeout -sKILL 5 sh -c 'cp a b'",
        "npm exec -p -c 'cp a b'",
        "npm x -p --call 'cp a b'",
        "npx -p -c 'cp a b'",
        "chrt -o 0 bash -sc 'cp a b'",
        "chrt -o 0 bash -s -c 'cp a b'",
        "taskset 1 sh -cs 'cp a b'",
        "bash -sc 'cp a b'",
        "bash -s -c 'cp a b'",
    ] {
        assert!(find(command).is_some(), "{command}");
        assert!(rejection(command, true).is_some(), "{command}");
    }
    for command in [
        "npx -p typescript tsc --noEmit",
        "npm exec -p -- tsc --noEmit",
        "nice -n5 npx tsc --noEmit",
        "timeout -sKILL 5 npx vitest run",
        "chrt 1 npx tsc --noEmit",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
    assert!(find("echo x | bash -s").is_some());
    assert!(find("bash -s -- a b").is_some());
}

#[test]
fn a_deep_launcher_nesting_is_read_in_one_pass() {
    let deep = "npx ".repeat(64);
    assert_eq!(operation(&format!("{deep}tsc --noEmit")), None);
    assert_eq!(operation(&format!("{deep}vitest run")), None);
    assert!(find(&format!("{deep}-c 'cp a b'")).is_some());
    assert!(find(&format!("{deep}sh -c 'cp a b'")).is_some());
    let mixed = "chrt 1 ".repeat(64);
    assert!(find(&format!("{mixed}npm exec -c 'cp a b'")).is_some());
    assert_eq!(operation(&format!("{mixed}npm exec -- tsc --noEmit")), None);
}

#[test]
fn launcher_option_values_stdin_shells_and_config_flags_are_told_apart() {
    for command in [
        "npx --cache npm-cache -c 'echo hi'",
        "npx --cache=npm-cache -c 'echo hi'",
        "npm exec --cache npm-cache --call 'echo hi'",
        "npm exec vitest -c 'echo hi'",
        "chrt -o 0 bash -s",
        "chrt -o 0 bash -s -- a b",
        "chrt 1 bash",
        "nice -n 5 chrt 1 bash -s",
        "bash -s --help",
        "bash -s --version",
        "chrt 1 bash -s --version",
        "nice -n 5 npx -c 'true>smoke-check.sh'",
        "timeout -s KILL 5 npx -c 'true>smoke-check.sh'",
        "nice -n5 npx -c 'true>smoke-check.sh'",
        "timeout -sKILL 5 npx -c 'true>smoke-check.sh'",
    ] {
        assert!(find(command).is_some(), "{command}");
        assert!(rejection(command, true).is_some(), "{command}");
    }
    for command in [
        "npx -p typescript tsc --noEmit",
        "npx vitest run -c vitest.config.ts",
        "npm exec -- tsc --noEmit",
        "npm exec -- vitest run -c cfg",
        "npx vitest -c vitest.config.ts",
        "npx --yes vitest run -c vitest.config.ts",
        "npx -y jest -c jest.config.js",
        "npm exec -- vitest -c vitest.config.ts",
        "npx --yes vitest -c cfg",
        "npx -q --no-install vitest -c cfg",
        "chrt 1 bash smoke-check.sh",
        "chrt 1 bash -- smoke-check.sh -c",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}

#[test]
fn only_the_closed_option_list_is_read_before_an_npx_or_npm_exec_package() {
    for command in [
        "npx -C . -c 'true>smoke-check.sh'",
        "npx -m label -c 'true>smoke-check.sh'",
        "npx --cal 'true>smoke-check.sh'",
        "npm exec --cal 'true>smoke-check.sh'",
        "npm x -C . --call 'true>smoke-check.sh'",
        "npm exec --prefix . -c 'true>smoke-check.sh'",
        "npx -yc 'true>smoke-check.sh'",
        "npx --cac npm-cache --call 'true>smoke-check.sh'",
        "npx --offline vitest -c cfg",
        "npm exec --message label --call 'true>smoke-check.sh'",
        "nice -n5 chrt 1 bash -s",
    ] {
        assert!(find(command).is_some(), "{command}");
        assert!(rejection(command, true).is_some(), "{command}");
    }
    for command in [
        "npx --package typescript tsc --noEmit",
        "npx --package=typescript tsc",
        "npx -p typescript -y tsc --noEmit",
        "npm exec -w web -- tsc",
        "npm exec --workspace=web -- tsc --noEmit",
        "npx --quiet tsc",
    ] {
        assert_eq!(operation(command), None, "{command}");
    }
}
