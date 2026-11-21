use super::find;

fn operation(command: &str) -> Option<String> {
    find(command, false).map(|mutation| mutation.operation)
}

#[test]
fn sed_in_place_spellings_are_updates() {
    for command in [
        "sed -i 's/1/2/' src/a.ts",
        "sed -i '' 's/1/2/' src/a.ts",
        "sed -i.bak 's/1/2/' src/a.ts",
        "sed --in-place 's/1/2/' src/a.ts",
        "sed --in-place=.bak 's/1/2/' src/a.ts",
        "sed --in 's/1/2/' src/a.ts",
        "sed -ni 's/1/2/p' src/a.ts",
        "sed -Ei 's/1/2/' src/a.ts",
        "sed -I .bak 's/1/2/' src/a.ts",
        "sed -n -e p -i src/a.ts",
        "sed -e 's/1/2/' src/a.ts -i",
        "gsed -i 's/1/2/' src/a.ts",
        "/usr/bin/sed -i 's/1/2/' src/a.ts",
        "'sed' -i 's/1/2/' src/a.ts",
        "\"sed\" -i 's/1/2/' src/a.ts",
        "\\sed -i 's/1/2/' src/a.ts",
        "sed -l -i '' 's/1/2/' src/a.ts",
        "sed -li '' 's/1/2/' src/a.ts",
        "sed -n -l -i.bak 's/1/2/p' src/a.ts",
        "sed -l 80 -i 's/1/2/' src/a.ts",
    ] {
        assert_eq!(operation(command).as_deref(), Some("sed -i"), "{command}");
    }
}

#[test]
fn perl_in_place_spellings_are_updates() {
    for command in [
        "perl -i -pe 's/1/2/' src/a.ts",
        "perl -pi -e 's/1/2/' src/a.ts",
        "perl -pi.bak -e 's/1/2/' src/a.ts",
        "perl -i.bak -pe 's/1/2/' src/a.ts",
        "perl -p -i -e 's/1/2/' src/a.ts",
        "perl -0777pi -e 's/1/2/' src/a.ts",
        "perl -lpi -e 's/1/2/' src/a.ts",
        "perl -e 's/1/2/' -i src/a.ts",
        "/usr/bin/perl -pi -e 's/1/2/' src/a.ts",
        "perl5.38.2 -pi -e 's/1/2/' src/a.ts",
    ] {
        assert_eq!(operation(command).as_deref(), Some("perl -i"), "{command}");
    }
}

#[test]
fn unresolvable_prefix_before_sed_or_perl_fails_closed() {
    for command in [
        "timeout -sKILL 5 sed -i 's/1/2/' a.ts",
        "timeout -k5 10 perl -pi -e 's/1/2/' a.ts",
        "nice -n5 sed -i 's/1/2/' a.ts",
        "nice -n5 /usr/bin/perl5.38 -pi -e x a.ts",
        "nice -n5 'gsed' --in-place x a.ts",
    ] {
        let mutation = find(command, false).unwrap_or_else(|| panic!("{command}"));
        assert_eq!(
            mutation.operation,
            "unresolved command prefix before sed/perl"
        );
    }
}

#[test]
fn the_existing_write_table_is_an_update() {
    for (command, expected) in [
        ("rm src/a.ts", "rm"),
        ("mv src/a.ts src/b.ts", "mv"),
        ("cp fixtures/a.ts src/a.ts", "cp"),
        ("tee src/a.ts", "tee"),
        ("tee -a report.txt", "tee"),
        ("touch marker", "touch"),
        ("truncate -s 0 src/a.ts", "truncate"),
        ("mkdir -p out", "mkdir"),
        ("echo x > src/a.ts", "output redirection"),
        ("printf x >> src/a.ts", "output redirection"),
    ] {
        assert_eq!(operation(command).as_deref(), Some(expected), "{command}");
    }
}

#[test]
fn prefixes_quotes_and_segments_do_not_hide_an_update() {
    for command in [
        "env FOO=1 sed -i 's/1/2/' a.ts",
        "command sed -i 's/1/2/' a.ts",
        "timeout 5 sed -i 's/1/2/' a.ts",
        "FOO=1 perl -pi -e 's/1/2/' a.ts",
        "npm run build && sed -i 's/1/2/' a.ts",
        "npm run build; rm a.ts",
        "npm run build\nperl -pi -e 's/1/2/' a.ts",
        "cd src && cp a.ts b.ts",
        "pytest | tee out.txt",
        "env -i timeout 5 command sed --in-place s/1/2/ a.ts",
    ] {
        assert!(find(command, false).is_some(), "{command}");
    }
}

#[test]
fn read_only_and_build_commands_are_not_updates() {
    for command in [
        "npm run build",
        "npx tsc",
        "npx tsc --noEmit",
        "node smoke-check.js",
        "pytest",
        "python3 -m pytest -q",
        "python3 -m compileall -q src",
        "sed -n '1,5p' src/a.ts",
        "sed -n -e 's/1/2/p' src/a.ts",
        "sed -e 's/1/2/' src/a.ts",
        "sed -E 's/(a|b)/c/' src/a.ts",
        "sed -s -n p a.ts b.ts",
        "sed --expression=p src/a.ts",
        "sed -e -i src/a.ts",
        "sed -- -i",
        "perl -pe 's/1/2/' src/a.ts",
        "perl -ne 'print if /x/' src/a.ts",
        "perl -e 'print 1'",
        "perl script.pl -i",
        "perl -MData::Dumper -e 1",
        "cd src && npm test",
        "timeout 600 cargo test",
        "grep -rn 'sed -i' src",
        "echo 'rm src/a.ts'",
        "echo \"perl -pi -e x\" # sed -i a.ts",
        "cargo test 2>&1",
        "timeout 5 npm test",
        "nice -n5 npm test",
        "timeout -sKILL 5 npm test",
        "sed -l 80 -n p a.ts",
        "sed -n -l p a.ts",
    ] {
        assert!(find(command, false).is_none(), "{command}");
    }
}

#[test]
fn null_sink_exemption_is_opt_in() {
    assert!(find("npm test 2>/dev/null", false).is_some());
    assert!(find("npm test 2>/dev/null", true).is_none());
    assert!(find("npm test >/dev/null && rm a.ts", true).is_some());
}

#[test]
fn description_names_the_update() {
    let sed = find("sed -i s/1/2/ a.ts", false).unwrap();
    assert_eq!(sed.describe(), "`sed -i`");
    let rm = find("rm a.ts", false).unwrap();
    assert_eq!(rm.describe(), "`rm` of `a.ts`");
}
