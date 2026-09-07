use super::*;

#[test]
fn frozen_response_shape_corpus() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/corpus/apps/nextjs-domain-response-shapes/cases.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let result = check(
            case["client"].as_str().unwrap(),
            case["route"].as_str().unwrap(),
        );
        assert_eq!(
            result.is_some(),
            case["mismatch"].as_bool().unwrap(),
            "{}: {result:?}",
            case["case"]
        );
    }
}

fn review_control(id: &str) {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/corpus/apps/nextjs-domain-response-shape-review/cases.json"
    ))
    .unwrap();
    let case = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case"] == id)
        .unwrap();
    let result = check(
        case["client"].as_str().unwrap(),
        case["route"].as_str().unwrap(),
    );
    assert_eq!(
        result.is_some(),
        case["mismatch"].as_bool().unwrap(),
        "{id}: {result:?}"
    );
}

#[test]
fn review_optional_cursor_control() {
    review_control("F2-nullish-optional-cursor");
}

#[test]
fn review_expression_arrow_control() {
    review_control("F3-arrow-expression-shadowing");
}

#[test]
fn review_matching_entries_control() {
    review_control("F1-matching-entries-key");
}

#[test]
fn review_parenthesized_expression_arrow_control() {
    assert_eq!(
        check(
            "const res = await fetch('/api/items'); const json = await res.json(); rows.map((json) => json.title); setItems(json.items);",
            "export async function GET() { return Response.json({items: []}); }"
        ),
        None
    );
}

#[test]
fn optional_defaults_preserve_collection_and_unguarded_mismatches() {
    let prefix = "const res = await fetch('/api/items'); const json = await res.json();";
    let route = "export async function GET() { return Response.json({items: []}); }";
    for read in [
        "const nextCursor = json.nextCursor ?? null;",
        "setCursor(((json.nextCursor)) ?? (null));",
        "setEnabled(json.feature ?? false);",
        "setCount(json.total || 0);",
        "setCursor(json.nextCursor ?? fallback());",
    ] {
        assert_eq!(
            check(&format!("{prefix}{read} setItems(json.items);"), route),
            None,
            "{read}"
        );
        let reason = check(&format!("{prefix}{read} setItems(json.missing);"), route).unwrap();
        assert!(reason.contains("json.missing"), "{read}: {reason}");
    }
    for fallback in ["|| []", "?? []", "?? ([])"] {
        let client = format!("{prefix} setItems(json.expenses {fallback});");
        assert!(
            check(
                &client,
                "export async function GET() { return Response.json([]); }"
            )
            .unwrap()
            .contains("json.expenses")
        );
        assert_eq!(
            check(
                &client,
                "export async function GET() { return Response.json({expenses: []}); }"
            ),
            None
        );
        assert!(
            check(&format!("{prefix} setItems(json.data {fallback});"), route)
                .unwrap()
                .contains("json.data")
        );
    }
}

#[test]
fn expression_arrow_scopes_do_not_hide_later_consumption() {
    let prefix = "const res = await fetch('/api/items'); const json = await res.json();";
    let route = "export async function GET() { return Response.json({items: []}); }";
    for read in [
        "rows.map(json => json.title);",
        "rows.map((json) => json.title);",
        "rows.map((json, index) => [json.title, index]);",
        "rows.map((json: Row): string => json.title);",
        "rows.map(json => ({title: json.title}));",
        "rows.map(json => json.title ? json.title : json.fallback);",
        "rows.map(json => json.title ?? json.fallback);",
        "rows.map(json => json.title || json.fallback);",
        "rows.map(json => other.map(row => json.title));",
        "rows.map(json => (json.title, json.fallback));",
        "const unused = () => json.unrelated;",
        "const unused = () => { return json.unrelated; };",
    ] {
        assert_eq!(
            check(&format!("{prefix}{read} setItems(json.items);"), route),
            None,
            "{read}"
        );
        let reason = check(&format!("{prefix}{read} setItems(json.missing);"), route).unwrap();
        assert!(reason.contains("json.missing"), "{read}: {reason}");
    }
    for read in [
        "consume(json => json.title, json.missing);",
        "consume((json) => json.title ? json.title : '', json.missing);",
        "const reads = [json => json.title, json.missing];",
        "const reads = {label: json => json.title, items: json.missing};",
        "const render = json => json.title, items = json.missing;",
        "const read = flag ? json => json.title : fallback; consume(json.missing);",
    ] {
        let reason = check(&format!("{prefix}{read}"), route).unwrap();
        assert!(reason.contains("json.missing"), "{read}: {reason}");
    }
    // Callable-side mutation can change the outer object's shape. It remains
    // unknown, even though ordinary callable reads are isolated from it.
    assert_eq!(
        check(
            &format!(
                "{prefix} const add = () => {{ json.extra = []; }}; add(); setItems(json.extra);"
            ),
            route
        ),
        None
    );
}

#[test]
fn array_method_names_are_object_keys_until_called() {
    let prefix = "const res = await fetch('/api/items'); const json = await res.json();";
    for member in ["entries", "values", "keys", "at", "map", "filter", "slice"] {
        let read = format!("{prefix} setItems(json.{member});");
        assert_eq!(
            check(
                &read,
                &format!(
                    "export async function GET() {{ return Response.json({{{member}: []}}); }}"
                )
            ),
            None,
            "{member}"
        );
        let reason = check(
            &read,
            "export async function GET() { return Response.json({other: []}); }",
        )
        .unwrap();
        assert!(reason.contains(&format!("json.{member}")), "{reason}");
        let call = format!("{prefix} setItems(json.{member}(arg));");
        assert!(
            check(
                &call,
                "export async function GET() { return Response.json({other: []}); }"
            )
            .unwrap()
            .contains("an array from json")
        );
        let array = "export async function GET() { return Response.json([]); }";
        assert_eq!(check(&call, array), None, "{member}");
        assert_eq!(check(&read, array), None, "inherited array member {member}");
    }
}

fn check(client: &str, route: &str) -> Option<String> {
    let root = tempfile::tempdir().unwrap();
    for (path, text) in [
        ("src/app/page.tsx", client),
        ("src/app/api/items/route.ts", route),
    ] {
        let path = root.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    super::super::api_contract::failure(root.path())
}

#[test]
fn s3_candidate_reports_body_mismatch_without_weakening_http_status_check() {
    let root = Path::new("tests/corpus/apps/nextjs-domain-s3-candidate");
    let reason = super::super::api_contract::failure(root).expect("S3 mismatch");
    assert!(reason.contains("response shape mismatch"), "{reason}");
    assert!(reason.contains(".data"), "{reason}");
}

#[test]
fn historical_shapes_and_matching_controls() {
    let prefix = "const res = await fetch('/api/items'); const json = await res.json();";
    for (read, returned, mismatch) in [
        (
            "if (!json.ok) throw Error(); setItems(json.data);",
            "{items: []}",
            true,
        ),
        (
            "setItems(Array.isArray(json) ? json : []);",
            "{reservations: []}",
            true,
        ),
        ("setItems(json.expenses || []);", "[]", true),
        ("setItems(json.items);", "{items: []}", false),
        ("setItems(Array.isArray(json) ? json : []);", "[]", false),
        (
            "if (!json.ok) throw Error(); setItems(json.data);",
            "{ok: true, data: []}",
            false,
        ),
        ("setItems(json.map(render));", "{items: []}", true),
        ("setItems(json.length);", "[]", false),
        ("setItems(json.slice(0, 5));", "[]", false),
        ("setItems(json.includes('x'));", "[]", false),
        ("setItems(json.items);", "{...payload}", false),
        ("setItems(json.items);", "payload", false),
        (
            "setItems(Array.isArray(json) ? json : json.items);",
            "{items: []}",
            false,
        ),
    ] {
        let route =
            format!("export async function GET() {{ return NextResponse.json({returned}); }}");
        let reason = check(&format!("{prefix}{read}"), &route);
        assert_eq!(
            reason.is_some(),
            mismatch,
            "{read} / {returned}: {reason:?}"
        );
    }
}

#[test]
fn isolates_method_errors_bindings_comments_and_nested_keys() {
    let route = r#"
export async function GET() {
  if (bad) return NextResponse.json({error: 'failed'}, {status: 500});
  return NextResponse.json({items: [{nested: true}]});
}
export const POST = async () => { return Response.json({item: {}}, {status: 201}); };
"#;
    for read in [
        "setItems(json.items);",
        "if (!res.ok) throw Error(json.error); setItems(json.items);",
        "// json.missing\nsetItems(json.items);",
        "const example = 'json.missing'; setItems(json.items);",
        "{ const json = other; setItems(json.unrelated); } setItems(json.items);",
    ] {
        assert_eq!(
            check(
                &format!(
                    "const res = await fetch('/api/items?month=09'); const json = await res.json(); {read}"
                ),
                route
            ),
            None
        );
    }
    let wrong = check("const res = await fetch('/api/items'); const json = await res.json(); setItems(json.nested);", route).unwrap();
    assert!(wrong.contains("json.nested"), "{wrong}");
    let wrong = check("const res = await fetch('/api/items', {method: 'POST'}); if (!res.ok) throw Error(); const json = await res.json(); setItems(json.items);", route).unwrap();
    assert!(
        wrong.contains("POST /api/items response shape mismatch"),
        "{wrong}"
    );
    assert_eq!(
        check(
            "async function a() { const res = await fetch('/api/items'); } async function b() { const json = await res.json(); setItems(json.other); }",
            route
        ),
        None
    );
    assert_eq!(
        check(
            "const res = await fetch('/api/items'); const json = await other.json(); setItems(json.other);",
            route
        ),
        None
    );
}

#[test]
fn unknown_status_or_success_variant_does_not_prove_a_mismatch() {
    let client = "const res = await fetch('/api/items'); const json = await res.json(); setItems(json.items);";
    for route in [
        "export async function GET() { return Response.json({error: msg}, {status}); }",
        "export async function GET() { if (a) return Response.json({other: []}); return Response.json(dynamic); }",
        "export async function GET() { if (a) return Response.json({other: []}); return Response.json({items: []}); }",
    ] {
        assert_eq!(check(client, route), None);
    }
    let reason = check(
        client,
        "export async function GET() { const items = []; return Response.json(items); }",
    )
    .unwrap();
    assert!(reason.contains("response shape mismatch"), "{reason}");
}

#[test]
fn error_branch_keys_and_optional_features_do_not_describe_success() {
    let route = "export async function GET() { if (bad) return Response.json({message: 'bad', code: 'E'}, {status: 400}); return Response.json({items: []}); }";
    let prefix = "const res = await fetch('/api/items'); const json = await res.json();";
    for read in [
        "if (!res.ok) { throw Error(json.message + json.code); } setItems(json.items);",
        "if (!res.ok) throw Error(json.message); setItems(json.items);",
        "if (res.ok) { setItems(json.items); } else { showError(json.code); }",
        "if (json.feature) { enable(json.feature); } setItems(json.items);",
        "if ('feature' in json) { enable(json.feature); } setItems(json.items);",
        "if (json.feature !== undefined) enable(json.feature); setItems(json.items);",
        "json.feature && enable(json.feature); setItems(json.items);",
        "const feature = json.feature ? json.feature : false; setItems(json.items);",
        "enable(json?.feature); setItems(json.items);",
        "const unused = () => { return json.unrelated; }; setItems(json.items);",
        "if (res.status >= 400) { show(json.code); } setItems(json.items);",
        "const value = res.ok ? json.items : json.message; setItems(value);",
        "const example = /json.missing/; setItems(json.items);",
        "const present = json.hasOwnProperty('feature'); setItems(json.items);",
    ] {
        assert_eq!(check(&format!("{prefix}{read}"), route), None, "{read}");
    }
    for read in [
        "show(json.message);",
        "show(json.code);",
        "setItems(json.missing);",
        "if (!json.ok) throw Error('failed');",
    ] {
        assert!(
            check(&format!("{prefix}{read}"), route)
                .unwrap()
                .contains("response shape mismatch")
        );
    }
}

#[test]
fn nested_returns_neither_create_nor_mask_a_handler_mismatch() {
    let client = "const res = await fetch('/api/items'); const json = await res.json(); setItems(json.items);";
    assert_eq!(
        check(
            client,
            "import { payload } from './other'; export async function GET() { function unused() { const payload = []; } return Response.json(payload); }"
        ),
        None
    );
    for nested in [
        "function unused() { return Response.json({other: []}); }",
        "const unused = () => { return Response.json({other: []}); };",
        "const unused = { run() { return Response.json({other: []}); } };",
    ] {
        assert_eq!(
            check(
                client,
                &format!(
                    "export async function GET() {{ {nested} return Response.json({{items: []}}); }}"
                )
            ),
            None
        );
        let nested = nested.replace("other", "items");
        assert!(check(client, &format!("export async function GET() {{ {nested} return Response.json({{other: []}}); }}")).unwrap().contains("response shape mismatch"));
    }
}
