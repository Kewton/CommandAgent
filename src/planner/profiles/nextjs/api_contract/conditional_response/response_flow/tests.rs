use super::super::ResponseCheck;
use super::check;

const NEEDLE: &str = r#"fetch("/api/bookmarks", { method: "PATCH""#;
const FETCH: &str =
    r#"fetch("/api/bookmarks", { method: "PATCH", body: JSON.stringify({ title }) })"#;
const GUARD_THEN: &str =
    ".then((payload) => {\n  if (!payload) return;\n  setItems(payload.items);\n})";

/// Verdict for the first PATCH fetch of `source`, with the 4,096-byte window
/// used by the caller.
fn verdict(source: &str) -> ResponseCheck {
    verdict_at(source, source.find(NEEDLE).expect("PATCH fetch"))
}

fn verdict_at(source: &str, start: usize) -> ResponseCheck {
    let end = start + NEEDLE.len();
    let window = crate::util::floor_char_boundary(source, end + 4_096);
    check(source, start, end, window)
}

fn source(chain: &str) -> String {
    format!("async function save(title) {{\n  {FETCH}{chain};\n}}\n")
}

#[test]
fn confirms_recorded_two_step_callback() {
    // Lines 111-124 of the recorded game page, with the route renamed.
    let recorded = r#"    fetch("/api/bookmarks", {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name: "PILOT", score: state.score, wave: state.wave }),
    })
      .then((res) =>
        res.ok ? (res.json() as Promise<HighScoreSubmitResponse>) : null,
      )
      .then((data) => {
        if (!data || data.ok !== true) return;
        setRank(data.ranks);
        setApiHighScore((prev) => Math.max(prev ?? 0, data.entry.score));
      })
      .catch(() => undefined);
"#;
    // The chain is the last statement of the effect, as in the record; a
    // statement after it runs before the response arrives.
    for (after, expected) in [
        ("", ResponseCheck::Confirmed),
        ("    setNotice(\"saved\");\n", ResponseCheck::Unconfirmed),
    ] {
        let page = format!(
            "useEffect(() => {{\n    submittedRef.current = true;\n{recorded}{after}  }}, []);\n"
        );
        let start = page.find("fetch(").unwrap();
        let end = page.find("\"PATCH\"").unwrap() + "\"PATCH\"".len();
        assert_eq!(check(&page, start, end, page.len()), expected, "{after:?}");
    }
}

#[test]
fn confirms_callbacks_that_gate_success_on_the_response() {
    for (name, chain) in [
        (
            "ternary then sentinel guard",
            format!(".then((reply) => reply.ok ? reply.json() : null){GUARD_THEN}"),
        ),
        (
            "parenthesized ternary with cast and undefined",
            format!(
                "\n  .then((reply) => (reply.ok ? (reply.json() as Promise<Reply>) : undefined))\n  {GUARD_THEN}\n  .catch(() => {{}})"
            ),
        ),
        (
            "throw first",
            ".then((reply) => { if (!reply.ok) throw new Error(\"save failed\"); return reply.json(); }).then((payload) => setItems(payload.items))".to_string(),
        ),
        (
            "throw inside a block with an or condition",
            ".then(async (reply: Response) => { if (!reply.ok || reply.status === 204) { setNotice(\"x\"); throw new Error(\"x\"); } setItems(await reply.json()); }).catch((err) => { throw err; })".to_string(),
        ),
        (
            "return first, then sentinel guard",
            format!(".then((reply) => {{ if (!reply.ok) return; return reply.json(); }}){GUARD_THEN}"),
        ),
        (
            "return null first without semicolons",
            ".then(reply => {\n  if (!reply.ok) return null\n  setNotice(\"saved\")\n})".to_string(),
        ),
        (
            "success closed in if (ok)",
            ".then((reply) => { if (reply.ok) { setNotice(\"saved\"); } })".to_string(),
        ),
        (
            "throw then inert catch",
            ".then((reply) => { if (!reply.ok) throw new Error(\"x\"); setNotice(\"saved\"); }).catch(() => undefined)".to_string(),
        ),
    ] {
        assert_eq!(verdict(&source(&chain)), ResponseCheck::Confirmed, "{name}");
    }
    let rejected_chain =
        ".then((reply) => { if (!reply.ok) throw new Error(\"x\"); return reply.json(); })";
    for (name, page) in [
        (
            "awaited throw",
            format!("const payload = await {FETCH}{rejected_chain};\nsetItems(payload);\n"),
        ),
        (
            "returned throw",
            format!("return {FETCH}{rejected_chain};\n"),
        ),
        (
            "void sentinel",
            format!("void {FETCH}.then((r) => r.ok ? r.json() : null){GUARD_THEN};\n"),
        ),
        (
            "concise arrow closing a JSX expression",
            format!(
                "const view = <button onClick={{() => {FETCH}.then((r) => {{ if (!r.ok) throw new Error(); }})}}>retry</button>;\n"
            ),
        ),
    ] {
        assert_eq!(verdict(&page), ResponseCheck::Confirmed, "{name}");
    }
}

#[test]
fn rejects_chains_that_do_not_prove_the_check() {
    for (name, chain) in [
        (
            "json only",
            ".then((reply) => reply.json()).then((payload) => setItems(payload))",
        ),
        (
            "payload ok only",
            ".then((reply) => reply.json()).then((payload) => { if (!payload.ok) return; setItems(payload); })",
        ),
        (
            "ok read and ignored",
            ".then((reply) => { reply.ok; return reply.json(); })",
        ),
        (
            "ok in a comma expression",
            ".then((reply) => (reply.ok, reply.json()))",
        ),
        (
            "both branches succeed",
            ".then((reply) => reply.ok ? reply.json() : reply.json())",
        ),
        (
            "both branches are sentinels",
            ".then((reply) => reply.ok ? null : null)",
        ),
        (
            "negated ternary",
            ".then((reply) => !reply.ok ? null : reply.json())",
        ),
        (
            "sentinel then default",
            ".then((reply) => reply.ok ? reply.json() : null ?? {})",
        ),
        (
            "nested ternary",
            ".then((reply) => reply.ok ? a ? b : c : null)",
        ),
        (
            "sentinel without a next then",
            ".then((reply) => reply.ok ? reply.json() : null)",
        ),
        (
            "unconditional success after sentinel",
            ".then((reply) => reply.ok ? reply.json() : null).then(() => setNotice(\"saved\"))",
        ),
        (
            "success before sentinel guard",
            ".then((reply) => reply.ok ? reply.json() : null).then((payload) => { setNotice(\"saved\"); if (!payload) return; })",
        ),
        (
            "second then after return first",
            ".then((reply) => { if (!reply.ok) return; return reply.json(); }).then((payload) => setItems(payload))",
        ),
        (
            "catch returns a success object",
            ".then((reply) => { if (!reply.ok) throw new Error(); return reply.json(); }).catch(() => ({ saved: true })).then((payload) => setItems(payload))",
        ),
        (
            "catch runs success handling",
            ".then((reply) => { if (!reply.ok) throw new Error(); setNotice(\"saved\"); }).catch(() => setNotice(\"saved\"))",
        ),
        (
            "catch then unguarded then",
            ".then((reply) => { if (!reply.ok) throw new Error(); return reply.json(); }).catch(() => undefined).then((payload) => setItems(payload))",
        ),
        (
            "success before the guard",
            ".then((reply) => { setNotice(\"saved\"); if (!reply.ok) throw new Error(); })",
        ),
        (
            "success after if (ok)",
            ".then((reply) => { if (reply.ok) { setItems([]); } setNotice(\"saved\"); })",
        ),
        (
            "if (ok) with else",
            ".then((reply) => { if (reply.ok) { setItems([]); } else { setNotice(\"saved\"); } })",
        ),
        (
            "guard logs only",
            ".then((reply) => { if (!reply.ok) { console.error(reply.status); } setNotice(\"saved\"); })",
        ),
        (
            "guard returns a value",
            ".then((reply) => { if (!reply.ok) return { saved: true }; return reply.json(); })",
        ),
        (
            "and condition",
            ".then((reply) => { if (!reply.ok && strict) throw new Error(); setNotice(\"saved\"); })",
        ),
        (
            "assignment in condition",
            ".then((reply) => { if (!reply.ok || (reply = backup)) throw new Error(); setNotice(\"saved\"); })",
        ),
        (
            "optional chain",
            ".then((reply) => { if (!reply?.ok) throw new Error(); setNotice(\"saved\"); })",
        ),
        (
            "ok-like name",
            ".then((reply) => reply.okay ? reply.json() : null).then((p) => { if (!p) return; })",
        ),
        (
            "other fetch's ok in the ternary",
            ".then((reply) => probe.ok ? reply.json() : null).then((payload) => { if (!payload) return; setItems(payload); })",
        ),
        (
            "other fetch's ok in the guard",
            ".then((reply) => { if (!probe.ok) throw new Error(); setNotice(\"saved\"); })",
        ),
        (
            "reassigned before the guard",
            ".then((reply) => { reply = backup; if (!reply.ok) throw new Error(); setNotice(\"saved\"); })",
        ),
        (
            "destructured parameter",
            ".then(({ ok, json }) => ok ? json() : null).then((p) => { if (!p) return; })",
        ),
        (
            "two parameters",
            ".then((reply, extra) => reply.ok ? reply.json() : null).then((p) => { if (!p) return; })",
        ),
        (
            "sentinel guard on another name",
            ".then((reply) => reply.ok ? reply.json() : null).then((payload) => { if (!items) return; setItems(payload); })",
        ),
        (
            "inner function return only",
            ".then((reply) => { if (!reply.ok) { const stop = () => { return; }; stop(); } setNotice(\"saved\"); })",
        ),
        (
            "conditional exit",
            ".then((reply) => { if (!reply.ok) { if (strict) return; } setNotice(\"saved\"); })",
        ),
        (
            "conditional exit across a line break",
            ".then((reply) => { if (!reply.ok) { if (strict)\n return; } setNotice(\"saved\"); })",
        ),
        (
            "loop exit across a line break",
            ".then((reply) => { if (!reply.ok) { for (const x of xs)\n throw x; } setNotice(\"saved\"); })",
        ),
        (
            "check in an uncalled function",
            ".then((reply) => { const verify = () => { if (!reply.ok) throw new Error(); }; setNotice(\"saved\"); })",
        ),
        (
            "comment ternary",
            ".then((reply) => /* reply.ok ? */ reply.json()).then((p) => { if (!p) return; })",
        ),
        (
            "string ternary",
            ".then((reply) => \"reply.ok ? x : null\" && reply.json()).then((p) => { if (!p) return; })",
        ),
        (
            "template ternary",
            ".then((reply) => `${reply.ok ? 1 : 0}` && reply.json()).then((p) => { if (!p) return; })",
        ),
        (
            "commented sentinel guard",
            ".then((reply) => reply.ok ? reply.json() : null).then((payload) => {\n  // if (!payload) return;\n  setItems(payload);\n})",
        ),
        (
            "rejection handler",
            ".then((reply) => reply.ok ? reply.json() : null, () => ({ saved: true })).then((p) => { if (!p) return; })",
        ),
        ("handler reference", ".then(handleReply)"),
        (
            "finally",
            ".then((reply) => { if (!reply.ok) throw new Error(); }).finally(() => setNotice(\"saved\"))",
        ),
        (
            "optional then",
            "?.then((reply) => { if (!reply.ok) throw new Error(); })",
        ),
        (
            "chain continues",
            ".then((reply) => { if (!reply.ok) throw new Error(); }).value",
        ),
        (
            "success on the failure branch",
            ".then((r) => r.ok ? null : r.json())",
        ),
        ("logical and", ".then((r) => r.ok && r.json())"),
    ] {
        assert_eq!(
            verdict(&source(chain)),
            ResponseCheck::Unconfirmed,
            "{name}"
        );
    }
    let sentinel_chain = format!(".then((r) => r.ok ? r.json() : null){GUARD_THEN}");
    for (name, page) in [
        (
            "awaited sentinel",
            format!("await {FETCH}{sentinel_chain};\nsetNotice(\"saved\");\n"),
        ),
        (
            "returned sentinel",
            format!("return {FETCH}.then((r) => r.ok ? r.json() : null);\n"),
        ),
        (
            "assigned sentinel",
            format!("const done = {FETCH}{sentinel_chain};\n"),
        ),
        (
            "argument sentinel",
            format!("track({FETCH}{sentinel_chain});\n"),
        ),
        (
            "awaited json only",
            format!("const reply = await {FETCH}.then((r) => r.json());\nif (!reply.ok) return;\n"),
        ),
        (
            "other fetch declared first",
            format!(
                "const probe = await fetch(\"/api/health\");\n{FETCH}.then((reply) => probe.ok ? reply.json() : null){GUARD_THEN};\n"
            ),
        ),
        // An un-awaited chain does not hold back the statements after it.
        (
            "throw chain then success",
            format!(
                "{FETCH}\n  .then((reply) => {{ if (!reply.ok) throw new Error(\"x\"); }});\nsetNotice(\"saved\");\n"
            ),
        ),
        (
            "guarded sentinel chain then success",
            format!(
                "{FETCH}\n  .then((reply) => (reply.ok ? reply.json() : null))\n  .then((payload) => {{ if (!payload) return; setItems(payload.items); }});\nsetNotice(\"saved\");\n"
            ),
        ),
        (
            "throw chain then success inside a block",
            format!(
                "function save() {{\n  {FETCH}.then((reply) => {{ if (!reply.ok) throw new Error(); }})\n  setNotice(\"saved\")\n}}\n"
            ),
        ),
        (
            "assigned throw chain then success",
            format!(
                "const pending = {FETCH}\n  .then((reply) => {{ if (!reply.ok) throw new Error(\"x\"); }});\nsetNotice(\"saved\");\n"
            ),
        ),
        (
            "argument throw chain then success",
            format!(
                "track({FETCH}\n  .then((reply) => {{ if (!reply.ok) throw new Error(\"x\"); }}));\nsetNotice(\"saved\");\n"
            ),
        ),
        (
            "object property throw chain then success",
            format!(
                "const tasks = {{ save: {FETCH}\n  .then((reply) => {{ if (!reply.ok) throw new Error(\"x\"); }}) }};\nsetNotice(\"saved\");\n"
            ),
        ),
        (
            "object argument throw chain then success",
            format!(
                "setState({{ saving: {FETCH}\n  .then((reply) => {{ if (!reply.ok) throw new Error(\"x\"); }}) }});\nsetNotice(\"saved\");\n"
            ),
        ),
        (
            "throw chain in a comma expression",
            format!(
                "{FETCH}.then((reply) => {{ if (!reply.ok) throw new Error(); }}), setNotice(\"saved\");\n"
            ),
        ),
    ] {
        assert_eq!(verdict(&page), ResponseCheck::Unconfirmed, "{name}");
    }
}

#[test]
fn leaves_fetches_without_a_direct_then_not_applicable() {
    for page in [
        format!("return {FETCH};\n"),
        format!("const reply = await {FETCH};\nreply.ok;\n"),
        format!("const ok = (await {FETCH}).ok;\n"),
        format!("await {FETCH};\nload().then((r) => r.json());\n"),
        format!("{FETCH}.catch(() => null).then((r) => r.json());\n"),
        // A commented-out call has no `fetch` token to bind.
        format!("// {FETCH}.then((r) => {{ if (!r.ok) throw new Error(); }});\n"),
    ] {
        assert_eq!(verdict(&page), ResponseCheck::NotApplicable, "{page}");
    }
}

#[test]
fn judges_each_fetch_on_its_own() {
    let page = format!(
        "function a() {{\n  {FETCH}.then((r) => {{ if (!r.ok) throw new Error(); }});\n}}\nfunction b() {{\n  {FETCH}.then((r) => r.json());\n}}\nfunction c() {{\n  {FETCH}.then((r) => r.ok ? r.json() : null){GUARD_THEN};\n}}\n"
    );
    let verdicts = page
        .match_indices(NEEDLE)
        .map(|(start, _)| verdict_at(&page, start))
        .collect::<Vec<_>>();
    assert_eq!(
        verdicts,
        [
            ResponseCheck::Confirmed,
            ResponseCheck::Unconfirmed,
            ResponseCheck::Confirmed
        ]
    );
}

#[test]
fn rejects_truncated_or_unclosed_syntax() {
    // The complete chain at the end of the file is the control.
    let complete = format!("{FETCH}.then((r) => {{ if (!r.ok) throw new Error(); }})");
    assert_eq!(verdict(&complete), ResponseCheck::Confirmed);
    for page in [
        format!("{FETCH}.then((r) => r.ok ? r.json() : null).then((p) => {{ if (!p) return;"),
        format!("{FETCH}.then((r) => {{ if (!r.ok) throw new Error(); "),
        format!("{complete};\nconst s = `unclosed"),
        format!("{complete};\n/* unclosed"),
        // A lone JSX backtick before the call stops the lexer, not the raw scan.
        format!("const hint = <p>Press ` to open</p>;\n{complete};\n"),
    ] {
        assert_eq!(verdict(&page), ResponseCheck::Unconfirmed, "{page}");
    }
    // When the call itself never closes, the old result applies unchanged.
    let unclosed_call = "fetch(\"/api/bookmarks\", { method: \"PATCH\" .then((r) => { if (!r.ok) throw new Error(); });";
    assert_eq!(verdict(unclosed_call), ResponseCheck::NotApplicable);
}

#[test]
fn unlexable_file_keeps_fetches_without_then_not_applicable() {
    // A lone backtick in JSX text stops the lexer for the whole file.
    let tail = "\nexport function View() {\n  return <p>Press ` to open</p>;\n}\n";
    for page in [
        format!("const res = await {FETCH};\nif (!res.ok) return;\nload().then(setItems);{tail}"),
        format!("return {FETCH};\nload().then(setItems);{tail}"),
        format!("{FETCH} /* (.then) */;\nwindow.then = 1;{tail}"),
    ] {
        assert_eq!(verdict(&page), ResponseCheck::NotApplicable, "{page}");
    }
    let chained = format!("{FETCH}\n  // note\n  .then((r) => r.json());{tail}");
    assert_eq!(verdict(&chained), ResponseCheck::Unconfirmed);
}

#[test]
fn requires_every_proof_token_inside_the_window() {
    for symbol in ["日", "─", "😀", "a"] {
        for target in 4_094..=4_098 {
            let pad = |extra: usize| {
                format!(
                    "/*{}{}*/",
                    symbol.repeat(extra / symbol.len()),
                    " ".repeat(extra % symbol.len())
                )
            };
            let page_with = |extra: usize| {
                format!(
                    "{FETCH}.then((r) => r.ok ? r.json() : null).then((p) => {{ {} if (!p) return; }});\n",
                    pad(extra)
                )
            };
            let distance = |page: &str| {
                page.find("return").unwrap() + "return".len()
                    - page.find(NEEDLE).unwrap()
                    - NEEDLE.len()
            };
            let page = page_with(target - distance(&page_with(0)));
            assert_eq!(distance(&page), target, "symbol={symbol:?}");
            let expected = if target <= 4_096 {
                ResponseCheck::Confirmed
            } else {
                ResponseCheck::Unconfirmed
            };
            assert_eq!(
                verdict(&page),
                expected,
                "symbol={symbol:?}, target={target}"
            );
        }
    }
}

#[test]
fn proof_token_must_end_before_a_character_cut_by_the_window() {
    for symbol in ["日", "─", "😀"] {
        // The raw window end falls inside `symbol`; the floored end keeps a
        // guard before the symbol and drops one after it.
        for (guard, expected) in [
            (
                format!("if (!r.ok) throw /*{symbol}*/ new Error();"),
                ResponseCheck::Confirmed,
            ),
            (
                format!("if (!r.ok) /*{symbol}*/throw new Error();"),
                ResponseCheck::Unconfirmed,
            ),
        ] {
            for inside in 1..symbol.len() {
                let page_with = |pad: usize| {
                    format!(
                        "{FETCH}/*{}*/.then((r) => {{ {guard} }});\n",
                        " ".repeat(pad)
                    )
                };
                let match_end = |page: &str| page.find(NEEDLE).unwrap() + NEEDLE.len();
                let base = page_with(0);
                let pad = match_end(&base) + 4_096 - inside - base.find(symbol).unwrap();
                let page = page_with(pad);
                let raw_end = match_end(&page) + 4_096;
                assert!(!page.is_char_boundary(raw_end), "symbol={symbol:?}");
                assert_eq!(page.find(symbol).unwrap() + inside, raw_end);
                assert_eq!(verdict(&page), expected, "{guard}, inside={inside}");
            }
        }
    }
}
