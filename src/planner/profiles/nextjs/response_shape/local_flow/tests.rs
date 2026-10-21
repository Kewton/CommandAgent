use super::*;

#[test]
fn issue457_type_valid_ambiguity_corpus_never_adds_acceptance_failures() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../tests/corpus/apps/issue457-nextjs-contracts/controls.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/app/api/projects")).unwrap();
        std::fs::write(
            root.path().join("src/app/page.tsx"),
            case["client"].as_str().unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/app/api/projects/route.ts"),
            case["server"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(failure(root.path()), None, "{}", case["name"]);
    }
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for item in std::fs::read_dir(from).unwrap() {
        let item = item.unwrap();
        if item.file_type().unwrap().is_dir() {
            copy(&item.path(), &to.join(item.file_name()));
        } else {
            std::fs::copy(item.path(), to.join(item.file_name())).unwrap();
        }
    }
}

#[test]
fn issue457_saved_wrapper_contract_reaches_profile_verification() {
    let fixture = Path::new("tests/corpus/apps/issue457-nextjs-contracts");
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.join("cases.json")).unwrap())
            .unwrap();
    for case in cases.as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        copy(
            Path::new("tests/corpus/apps/issue456-create-recovery/original"),
            root.path(),
        );
        for overlay in case["overlays"].as_array().unwrap() {
            copy(
                &fixture.join("overlays").join(overlay.as_str().unwrap()),
                root.path(),
            );
        }
        let expected = if case["name"] == "original"
            || case["name"] == "role-only"
            || case["name"] == "wrong-field"
        {
            Some("request field mismatch")
        } else {
            None
        };
        let failure = failure(root.path());
        if let Some(expected) = expected {
            assert!(
                failure.as_deref().is_some_and(|s| s.contains(expected)),
                "{}: {failure:?}",
                case["name"]
            );
            let report = crate::planner::profile::verify_profile_final(
                root.path(),
                "nextjs",
                "Build a project task manager on port 60302",
            );
            assert!(
                report.profile_failures.iter().any(|s| s.contains(expected)),
                "{report:?}"
            );
        } else {
            assert_eq!(failure, None);
        }
        let hint = advisory(root.path());
        if matches!(
            case["name"].as_str().unwrap(),
            "original" | "role-only" | "missing-list"
        ) {
            assert!(
                hint.as_deref()
                    .is_some_and(|s| s.contains("not a proven failure")),
                "{hint:?}"
            );
        } else {
            assert_eq!(hint, None);
        }
    }
}

#[test]
fn issue457_optional_default_and_mixed_request_access_do_not_fail() {
    let root = tempfile::tempdir().unwrap();
    copy(
        Path::new("tests/corpus/apps/issue456-create-recovery/original"),
        root.path(),
    );
    let page = root.path().join("src/app/page.tsx");
    let text = std::fs::read_to_string(&page).unwrap().replace(
        "useState<Member[]>([])",
        "useState<Member[]>(DEFAULT_MEMBERS)",
    );
    std::fs::write(page, text).unwrap();
    let route = root.path().join("src/app/api/tasks/[id]/route.ts");
    std::fs::write(route, "export async function PATCH(request: Request) { const body = await request.json(); const { status } = body; if (body.assignee) return Response.json({item: body.assignee}); return Response.json({item: status}); }").unwrap();
    assert_eq!(failure(root.path()), None);
    assert!(advisory(root.path()).unwrap().contains("local default"));
}

#[test]
fn issue457_parameter_shadowing_and_property_guards_are_unknown() {
    let original = std::fs::read_to_string(
        "tests/corpus/apps/issue456-create-recovery/original/src/app/page.tsx",
    )
    .unwrap();
    for suffix in [
        "async function other(fetchApi: (url: string) => Promise<unknown>) { return await fetchApi('/api/projects'); }",
        "async function other({ fetchApi }: { fetchApi: (url: string) => Promise<unknown> }) { return await fetchApi('/api/projects'); }",
        "const other = async (fetchApi: (url: string) => Promise<unknown>) => await fetchApi('/api/projects');",
    ] {
        assert!(json_helpers(&Source::parse(&(original.clone() + suffix)).unwrap()).is_empty());
    }
    for usage in [
        "!body.assignee",
        "typeof body.assignee === 'string'",
        "!body['assignee']",
        "typeof body['assignee'] === 'string'",
    ] {
        let source = format!(
            "export async function PATCH(request: Request) {{ const body = await request.json(); const {{status}} = body; if ({usage}) return Response.json({{item: status}}); return Response.json({{item: status}}); }}"
        );
        assert_eq!(
            request_keys(&Source::parse(&source).unwrap(), "PATCH"),
            None,
            "{usage}"
        );
    }
}

#[test]
fn issue457_unknown_wrappers_and_optional_logging_are_not_proven_contracts() {
    let original = std::fs::read_to_string(
        "tests/corpus/apps/issue456-create-recovery/original/src/app/page.tsx",
    )
    .unwrap();
    let parsed = Source::parse(&original).unwrap();
    assert_eq!(json_helpers(&parsed), ["fetchApi"]);
    for (old, new) in [
        ("return res.json();", "return transform(res.json());"),
        ("return res.json();", "return { items: [] };"),
        ("const res = await fetch", "let res = await fetch"),
        ("fetch(url, init)", "fetch(other, init)"),
        ("if (!res.ok)", "if (res.status === 400)"),
    ] {
        assert!(json_helpers(&Source::parse(&original.replace(old, new)).unwrap()).is_empty());
    }
    let shape = [Shape::Object(BTreeSet::from(["items".into()]))];
    for source in [
        "if (Array.isArray(data.people)) console.log(data.people);",
        "const [people, setPeople] = useState([]); if (data.people) setPeople(data.people);",
    ] {
        assert_eq!(
            consumed_collection(&Source::parse(source).unwrap(), 0, "data", &shape),
            None
        );
    }
}

#[test]
fn issue457_collection_names_are_not_domain_rules() {
    let source = "const [actors, storeActors] = useState([]); if (Array.isArray(json.actors)) storeActors(json.actors);";
    let source = Source::parse(source).unwrap();
    assert_eq!(
        consumed_collection(
            &source,
            0,
            "json",
            &[Shape::Object(BTreeSet::from(["items".into()]))]
        ),
        Some("actors".into())
    );
    assert_eq!(
        consumed_collection(
            &source,
            0,
            "json",
            &[Shape::Object(BTreeSet::from(["actors".into()]))]
        ),
        None
    );
}
