# Issue #23 Verification

- Status: `passed`

## Checks

- `ruby -e 'checks={"LICENSE"=>["MIT License", "Copyright (c) 2026 Kewton", "Permission is hereby granted, free of charge", "The above copyright notice and this permission notice", "THE SOFTWARE IS PROVIDED \"AS IS\""], "CONTRIBUTING.md"=>["Rust 1.88", "cargo test --all-targets", "ANVIL_PTY_TESTS=1 cargo test --test tui_pty", "Python 3.10", "docs/dev-guardrails.md", "baseline plus 2%", "new subsystem in a new module", "docs/mechanism-ledger.md", "event names", "JSON keys", "schemas", "cargo test --test corpus_regression", "cargo test --test conformance", "cargo test --test generality_guardrails", "README.md", "README.ja.md", "docs/guide/en/", "docs/guide/ja/", "doc-drift", "RUSTFLAGS=\"-D warnings\"", "## [Unreleased]"], "CHANGELOG.md"=>["Keep a Changelog", "## [Unreleased]", "version 0.1.0\n(2026-07)", "Git history", "docs/mechanism-ledger.md"], "README.md"=>["[MIT License](LICENSE)"], "README.ja.md"=>["[MIT License](LICENSE)"]}; missing=checks.flat_map{|file, needles| text=File.read(file); needles.reject{|needle| text.include?(needle)}.map{|needle| "#{file}: #{needle}"}}; abort(missing.join("\n")) unless missing.empty?'`: `passed`
- `ruby -e 'files=%w[README.md README.ja.md CONTRIBUTING.md CHANGELOG.md dev-reports/issue-23/design.md]; bad=[]; files.each{|f| File.read(f).scan(/\]\(([^)]+)\)/).flatten.each{|t| next if t.match?(/\A(?:https?:|mailto:|#)/); p=t.split(%q{#},2).first; path=File.expand_path(p,File.dirname(f)); bad << "#{f}: #{t}" unless File.exist?(path)}}; abort(bad.join("\n")) unless bad.empty?'`: `passed`
- ``ruby -e 'en=File.read("README.md"); ja=File.read("README.ja.md"); abort("bash blocks differ") unless en.scan(/```bash\n(.*?)```/m)==ja.scan(/```bash\n(.*?)```/m); abort("section counts differ") unless en.lines.grep(/^## /).length==ja.lines.grep(/^## /).length && en.lines.grep(/^### /).length==ja.lines.grep(/^### /).length; abort("license links differ") unless [en,ja].all?{|text| text.include?("[MIT License](LICENSE)")}'``: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets`: `passed`
- `git diff --cached --check`: `passed`

## Environment Note

The initial sandboxed all-target run was stopped after localhost and process
control fixtures received `Operation not permitted`. The same required command
was rerun outside the sandbox and passed with warnings denied. No test was
weakened, skipped beyond its normal built-in opt-in behavior, or rewritten.

## Scope Note

This is a documentation-only change. Rust formatting and clippy checks were not
required because no Rust source or generated Rust artifact changed.
